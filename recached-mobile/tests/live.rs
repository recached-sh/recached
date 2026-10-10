//! End-to-end tests against a real server, with a tokio-tungstenite socket
//! standing in for OkHttp / URLSession.
//!
//! Skipped unless `RECACHED_MOBILE_TEST_URL` is set, so `cargo test` stays
//! green without a server running:
//!
//! ```sh
//! cargo run -p recached --bin recached-server &
//! RECACHED_MOBILE_TEST_URL=ws://127.0.0.1:6380 cargo test -p recached-mobile --test live
//! ```

use futures_util::{SinkExt, StreamExt};
use recached_mobile::{ClientConfig, FrameSink, RecachedClient};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

macro_rules! server_url {
    () => {
        match std::env::var("RECACHED_MOBILE_TEST_URL") {
            Ok(u) => u,
            Err(_) => {
                eprintln!("skipped: set RECACHED_MOBILE_TEST_URL to run");
                return;
            }
        }
    };
}

/// Unique per run, so repeated runs never see a previous run's keys.
fn ns(tag: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("mobiletest:{tag}:{nanos:x}:{n}")
}

/// Long enough for a frame to cross the loopback socket and be applied.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(250)).await;
}

/// An on-disk database removed, with its WAL files, when dropped.
struct TempDb(PathBuf);

impl TempDb {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{}.db", ns(tag).replace(':', "-")));
        Self(path)
    }

    fn open(&self) -> Arc<RecachedClient> {
        RecachedClient::open(
            self.0.to_string_lossy().into_owned(),
            ClientConfig::default(),
        )
        .expect("open")
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let mut p = self.0.clone().into_os_string();
            p.push(suffix);
            let _ = std::fs::remove_file(p);
        }
    }
}

struct ChannelSink(mpsc::UnboundedSender<Vec<u8>>);

impl FrameSink for ChannelSink {
    fn send(&self, frame: Vec<u8>) {
        let _ = self.0.send(frame);
    }
}

/// What the platform layer does: own the socket, hand frames to the client,
/// and report the keys each frame changed.
struct Connection {
    client: Arc<RecachedClient>,
    changed: Arc<Mutex<Vec<String>>>,
    tasks: Vec<JoinHandle<()>>,
}

impl Connection {
    /// `deliver_frames: false` reads nothing from the server, as if the app
    /// were killed before any reply arrived.
    async fn open(url: &str, client: Arc<RecachedClient>, deliver_frames: bool) -> Self {
        let (ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("connect");
        let (mut write, mut read) = ws.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        client.connection_opened(Arc::new(ChannelSink(tx)));

        let writer = tokio::spawn(async move {
            while let Some(frame) = rx.recv().await {
                let msg = match String::from_utf8(frame) {
                    Ok(text) => Message::text(text),
                    Err(e) => Message::binary(e.into_bytes()),
                };
                if write.send(msg).await.is_err() {
                    break;
                }
            }
        });

        let changed = Arc::new(Mutex::new(Vec::new()));
        let reader = {
            let client = Arc::clone(&client);
            let changed = Arc::clone(&changed);
            tokio::spawn(async move {
                while let Some(Ok(msg)) = read.next().await {
                    if !deliver_frames {
                        continue;
                    }
                    let bytes = match msg {
                        Message::Text(t) => t.as_bytes().to_vec(),
                        Message::Binary(b) => b.to_vec(),
                        Message::Close(_) => break,
                        _ => continue,
                    };
                    let outcome = client.frame_received(bytes).expect("persist frame");
                    changed.lock().unwrap().extend(outcome.changed_keys);
                    assert!(!outcome.reconnect, "the server sent an unparseable frame");
                }
            })
        };

        Self {
            client,
            changed,
            tasks: vec![writer, reader],
        }
    }

    fn take_changed(&self) -> Vec<String> {
        std::mem::take(&mut *self.changed.lock().unwrap())
    }

    /// Drop the socket and tell the client, as the platform would.
    fn close(self) {
        for task in &self.tasks {
            task.abort();
        }
        self.client.connection_closed();
    }
}

#[tokio::test]
async fn a_write_another_client_makes_arrives_and_names_its_key() {
    let url = server_url!();
    let key = ns("propagate");
    let (db_a, db_b) = (TempDb::new("a"), TempDb::new("b"));

    let a = Connection::open(&url, db_a.open(), true).await;
    a.client.watch(format!("{key}*"));
    let b = Connection::open(&url, db_b.open(), true).await;
    settle().await;
    a.take_changed();

    b.client.set(key.clone(), b"hello".to_vec()).unwrap();
    settle().await;

    assert_eq!(a.client.get(key.clone()), Some(b"hello".to_vec()));
    assert!(a.take_changed().contains(&key));
    assert_eq!(b.client.pending_writes(), 0, "the server acknowledged it");
}

#[tokio::test]
async fn a_write_replayed_after_a_kill_applies_exactly_once() {
    let url = server_url!();
    let key = ns("once");
    let db = TempDb::new("once");

    // The server applies the increment, but the app dies before its reply is
    // processed, so the write is still queued on disk.
    {
        let conn = Connection::open(&url, db.open(), false).await;
        conn.client.incr_by(key.clone(), 5).unwrap();
        settle().await;
        assert_eq!(conn.client.pending_writes(), 1);
        conn.close();
    }

    // Restart: the queued increment replays, the server recognises it, and the
    // durable row retires.
    let client = db.open();
    assert_eq!(client.pending_writes(), 1);
    let conn = Connection::open(&url, client, true).await;
    settle().await;
    assert_eq!(conn.client.pending_writes(), 0);

    let observer_db = TempDb::new("observer");
    let observer = Connection::open(&url, observer_db.open(), true).await;
    observer.client.watch(key.clone());
    settle().await;
    assert_eq!(
        observer.client.get_string(key).unwrap().as_deref(),
        Some("5"),
        "applied once, not twice"
    );
}

#[tokio::test]
async fn a_key_deleted_while_offline_is_gone_after_reconnect_and_restart() {
    let url = server_url!();
    let prefix = ns("offline");
    let (keep, gone) = (format!("{prefix}:keep"), format!("{prefix}:gone"));
    let (db_a, db_b) = (TempDb::new("a"), TempDb::new("b"));

    let writer = Connection::open(&url, db_b.open(), true).await;
    writer.client.set(keep.clone(), b"1".to_vec()).unwrap();
    writer.client.set(gone.clone(), b"2".to_vec()).unwrap();

    let a = db_a.open();
    a.watch(format!("{prefix}:*"));
    let conn = Connection::open(&url, Arc::clone(&a), true).await;
    settle().await;
    assert!(a.exists(gone.clone()));
    conn.close();

    writer.client.del(gone.clone()).unwrap();
    settle().await;
    assert!(a.exists(gone.clone()), "offline: nothing has told it yet");

    let conn = Connection::open(&url, Arc::clone(&a), true).await;
    settle().await;
    assert!(!a.exists(gone.clone()));
    assert!(
        conn.take_changed().contains(&gone),
        "observers hear about it"
    );
    conn.close();
    drop(a);

    let restarted = db_a.open();
    assert!(!restarted.exists(gone), "the deletion reached the disk");
    assert!(restarted.exists(keep));
}

#[tokio::test]
async fn a_cold_start_with_no_network_reads_the_last_synced_server_state() {
    let url = server_url!();
    let key = ns("cold");
    let (db_a, db_b) = (TempDb::new("a"), TempDb::new("b"));

    let writer = Connection::open(&url, db_b.open(), true).await;
    writer
        .client
        .jset(
            key.clone(),
            "$".into(),
            r#"{"title":"from the server"}"#.into(),
        )
        .unwrap();

    let a = db_a.open();
    a.watch(key.clone());
    let conn = Connection::open(&url, Arc::clone(&a), true).await;
    settle().await;
    conn.close();
    drop(a);

    // Never connected: everything comes from the device.
    let offline = db_a.open();
    assert_eq!(
        offline.get_json(key, None).as_deref(),
        Some(r#"{"title":"from the server"}"#)
    );
}
