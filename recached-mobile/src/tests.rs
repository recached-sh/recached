//! Unit tests: the client against hand-written server frames, no socket.
//! `tests/live.rs` covers the same paths against a real server.

use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// A database path removed, with its WAL files, when dropped.
struct TempDb(PathBuf);

impl TempDb {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("recached-mobile-{}-{n}.db", std::process::id()));
        let db = Self(path);
        db.cleanup();
        db
    }

    fn open(&self) -> Arc<RecachedClient> {
        RecachedClient::open(
            self.0.to_string_lossy().into_owned(),
            ClientConfig::default(),
        )
        .expect("open")
    }

    fn cleanup(&self) {
        for suffix in ["", "-wal", "-shm"] {
            let mut p = self.0.clone().into_os_string();
            p.push(suffix);
            let _ = std::fs::remove_file(p);
        }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// A sink that records what the client sends.
#[derive(Default)]
struct Recorder(Mutex<Vec<Vec<u8>>>);

impl FrameSink for Recorder {
    fn send(&self, frame: Vec<u8>) {
        self.0.lock().unwrap().push(frame);
    }
}

impl Recorder {
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap())
            .into_iter()
            .map(|f| String::from_utf8(f).unwrap())
            .collect()
    }
}

fn connect(client: &RecachedClient) -> Arc<Recorder> {
    let sink = Arc::new(Recorder::default());
    client.connection_opened(sink.clone());
    sink
}

fn s(text: &str) -> String {
    text.to_string()
}

const OK: &[u8] = b"+OK\r\n";

fn keychange(key: &str, value: &str) -> Vec<u8> {
    to_resp(&["keychange", key, value])
}

/// The `DEDUP <client> <wire-id>` envelope of a sent write.
fn dedup_header(frame: &str) -> (String, u64) {
    let parts: Vec<&str> = frame.split("\r\n").collect();
    assert_eq!(parts[2], "DEDUP", "not a dedup frame: {frame:?}");
    (parts[4].to_string(), parts[6].parse().unwrap())
}

// ── durability ────────────────────────────────────────────────────────────────

#[test]
fn offline_writes_survive_a_restart_with_their_values() {
    let db = TempDb::new();
    {
        let c = db.open();
        c.set(s("k"), b"v".to_vec()).unwrap();
        assert_eq!(c.incr_by(s("n"), 2).unwrap(), 2);
        c.jset(s("doc"), s("$"), s(r#"{"a":1}"#)).unwrap();
        assert_eq!(c.pending_writes(), 3);
    }
    let c = db.open();
    assert_eq!(c.get(s("k")), Some(b"v".to_vec()));
    assert_eq!(c.get_string(s("n")).unwrap().as_deref(), Some("2"));
    assert_eq!(c.get_json(s("doc"), None).as_deref(), Some(r#"{"a":1}"#));
    assert_eq!(c.pending_writes(), 3, "the queued writes came back too");
}

#[test]
fn a_delete_is_durable() {
    let db = TempDb::new();
    {
        let c = db.open();
        c.set(s("k"), b"v".to_vec()).unwrap();
        assert!(c.del(s("k")).unwrap());
    }
    assert!(!db.open().exists(s("k")));
}

#[test]
fn a_refused_command_changes_and_queues_nothing() {
    let db = TempDb::new();
    let c = db.open();
    c.set(s("text"), b"abc".to_vec()).unwrap();
    let err = c.incr_by(s("text"), 1).unwrap_err();
    assert!(matches!(err, RecachedError::Command { .. }), "{err:?}");
    assert_eq!(c.pending_writes(), 1);
}

#[test]
fn binary_values_have_no_string_form() {
    let db = TempDb::new();
    let c = db.open();
    c.set(s("bin"), vec![0xff, 0x00]).unwrap();
    assert_eq!(c.get(s("bin")), Some(vec![0xff, 0x00]));
    assert!(matches!(
        c.get_string(s("bin")),
        Err(RecachedError::NotUtf8 { .. })
    ));
}

// ── sync ──────────────────────────────────────────────────────────────────────

#[test]
fn queued_writes_follow_the_session_on_connect() {
    let db = TempDb::new();
    let c = db.open();
    c.watch(s("cart:*"));
    c.set(s("a"), b"1".to_vec()).unwrap();

    let sent = connect(&c).take();
    assert!(sent[0].contains("DELTA"), "{sent:?}");
    assert!(sent[1].contains("QSUB"), "{sent:?}");
    assert!(sent[2].contains("SET"), "{sent:?}");
    assert_eq!(sent.len(), 3);
}

#[test]
fn writes_go_out_at_once_while_connected_and_not_after_close() {
    let db = TempDb::new();
    let c = db.open();
    let sink = connect(&c);
    sink.take();

    c.set(s("a"), b"1".to_vec()).unwrap();
    assert_eq!(sink.take().len(), 1);

    let delay = c.connection_closed();
    assert!(delay > 0);
    c.set(s("b"), b"2".to_vec()).unwrap();
    assert!(sink.take().is_empty(), "nothing is sent on a closed socket");
}

#[test]
fn an_acknowledged_write_leaves_the_durable_outbox() {
    let db = TempDb::new();
    {
        let c = db.open();
        let _sink = connect(&c);
        c.set(s("a"), b"1".to_vec()).unwrap();
        c.frame_received(OK.to_vec()).unwrap(); // CLIENT DELTA ON
        assert_eq!(c.pending_writes(), 1);
        c.frame_received(OK.to_vec()).unwrap(); // SET
        assert_eq!(c.pending_writes(), 0);
    }
    assert_eq!(db.open().pending_writes(), 0);
}

#[test]
fn a_restart_replays_with_the_same_identity_and_original_wire_ids() {
    // The exactly-once argument: a write the server applied before the app
    // died is replayed under its original wire id, which the server has
    // already seen and skips. New writes use a higher epoch.
    let db = TempDb::new();
    let first = {
        let c = db.open();
        c.set(s("a"), b"1".to_vec()).unwrap();
        let sent = connect(&c).take();
        dedup_header(sent.last().unwrap())
    };

    let c = db.open();
    let sink = connect(&c);
    let replayed = dedup_header(sink.take().last().unwrap());
    assert_eq!(replayed, first, "the replay is byte-identical");

    c.set(s("b"), b"2".to_vec()).unwrap();
    let (client, wire) = dedup_header(&sink.take()[0]);
    assert_eq!(client, first.0, "the client id persists");
    assert!(
        wire >> 32 > first.1 >> 32,
        "a new session has a higher epoch"
    );
}

#[test]
fn server_changes_are_persisted_and_reported() {
    let db = TempDb::new();
    {
        let c = db.open();
        let outcome = c.frame_received(keychange("x", "1")).unwrap();
        assert_eq!(outcome.changed_keys, vec![s("x")]);
        assert!(!outcome.reconnect);
    }
    let c = db.open();
    assert_eq!(c.get(s("x")), Some(b"1".to_vec()));

    // A nil value is a deletion. `to_resp` cannot encode nil, so by hand.
    let frame = b"*3\r\n$9\r\nkeychange\r\n$1\r\nx\r\n$-1\r\n".to_vec();
    assert_eq!(c.frame_received(frame).unwrap().changed_keys, vec![s("x")]);
    drop(c);
    assert!(!db.open().exists(s("x")), "the deletion was saved too");
}

#[test]
fn a_key_deleted_while_offline_is_removed_on_reconnect_and_on_disk() {
    let db = TempDb::new();
    let c = db.open();
    c.watch(s("cart:*"));
    connect(&c);
    c.frame_received(OK.to_vec()).unwrap();
    c.frame_received(to_resp(&[
        "qstate", "cart:*", "cart:1", "apple", "cart:2", "pear",
    ]))
    .unwrap();
    c.connection_closed();

    connect(&c);
    c.frame_received(OK.to_vec()).unwrap();
    let mut outcome = c
        .frame_received(to_resp(&["qstate", "cart:*", "cart:1", "apple"]))
        .unwrap();
    outcome.changed_keys.sort();
    assert_eq!(outcome.changed_keys, vec![s("cart:1"), s("cart:2")]);
    drop(c);

    let c = db.open();
    assert!(c.exists(s("cart:1")));
    assert!(!c.exists(s("cart:2")));
}

#[test]
fn an_unparseable_frame_asks_for_a_reconnect() {
    let db = TempDb::new();
    let c = db.open();
    connect(&c);
    let outcome = c.frame_received(b"*not resp".to_vec()).unwrap();
    assert!(outcome.reconnect);
}

#[test]
fn get_matching_is_sorted_and_marks_collections() {
    let db = TempDb::new();
    let c = db.open();
    c.set(s("p:2"), b"b".to_vec()).unwrap();
    c.set(s("p:1"), b"a".to_vec()).unwrap();
    // A set arrives type-tagged; it has no single value.
    c.frame_received(
        b"*3\r\n$9\r\nkeychange\r\n$3\r\np:3\r\n*2\r\n$3\r\nset\r\n$1\r\nm\r\n".to_vec(),
    )
    .unwrap();
    let found = c.get_matching(s("p:*"));
    assert_eq!(
        found,
        vec![
            Entry {
                key: s("p:1"),
                value: Some(b"a".to_vec())
            },
            Entry {
                key: s("p:2"),
                value: Some(b"b".to_vec())
            },
            Entry {
                key: s("p:3"),
                value: None
            },
        ]
    );
}
