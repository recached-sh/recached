//! Crash recovery against the real binary: start `recached-server` with an
//! AOF, write over its WebSocket port, kill it with SIGKILL — no shutdown
//! checkpoint, the reply possibly still unsent — start it again on the same
//! files, and replay.
//!
//! This is the restart the in-process tests can only imitate: a fresh
//! process, recovering from snapshot, dedup sidecars and AOF exactly as an
//! operator's server would.

use futures_util::{SinkExt, StreamExt};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

struct Server {
    child: Child,
    ws_port: u16,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("recached-restart-{name}-{nanos:x}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Start the server on `dir`'s files. The working directory is `dir` too, so
/// nothing lands next to the test binary.
fn start(dir: &Path, aof_sync: &str) -> Server {
    let (tcp_port, ws_port) = (free_port(), free_port());
    // `RECACHED_TEST_SERVER_BIN` runs these against another build — an older
    // release, to show what they catch.
    let bin = std::env::var("RECACHED_TEST_SERVER_BIN")
        .unwrap_or_else(|_| env!("CARGO_BIN_EXE_recached-server").to_string());
    let child = Command::new(bin)
        .current_dir(dir)
        .env_clear()
        .env("RECACHED_BIND", "127.0.0.1")
        .env("RECACHED_PORT", tcp_port.to_string())
        .env("RECACHED_WS_PORT", ws_port.to_string())
        .env("RECACHED_METRICS_PORT", "0")
        .env("RECACHED_SAVE_PATH", dir.join("dump.rdb"))
        .env("RECACHED_AOF_PATH", dir.join("append.aof"))
        .env("RECACHED_AOF_SYNC", aof_sync)
        // No autosave: every checkpoint in these tests is an explicit SAVE.
        .env("RECACHED_SAVE_INTERVAL", "0")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start recached-server");
    let deadline = Instant::now() + Duration::from_secs(20);
    while std::net::TcpStream::connect(("127.0.0.1", ws_port)).is_err() {
        assert!(Instant::now() < deadline, "server did not start");
        std::thread::sleep(Duration::from_millis(25));
    }
    Server { child, ws_port }
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(port: u16) -> Ws {
    tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .expect("websocket")
        .0
}

fn resp(args: &[&str]) -> String {
    let mut out = format!("*{}\r\n", args.len());
    for a in args {
        out.push_str(&format!("${}\r\n{}\r\n", a.len(), a));
    }
    out
}

async fn send(ws: &mut Ws, args: &[&str]) {
    ws.send(Message::Text(resp(args).into())).await.unwrap();
}

/// The next reply, skipping pushes.
async fn reply(ws: &mut Ws) -> String {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .expect("a reply in time")
            .expect("socket open")
            .unwrap();
        if let Message::Text(t) = msg {
            if t.starts_with('>') {
                continue;
            }
            return t.to_string();
        }
    }
}

async fn cmd(ws: &mut Ws, args: &[&str]) -> String {
    send(ws, args).await;
    reply(ws).await
}

fn incr(id: &str, by: &str) -> Vec<String> {
    ["DEDUP", "phone-1", id, "INCRBY", "visits", by]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn args(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

#[tokio::test]
async fn test_03_04_an_incr_survives_sigkill_and_its_retry_is_suppressed() {
    let dir = scratch("incr");
    {
        let server = start(&dir, "always");
        let mut ws = connect(server.ws_port).await;
        assert_eq!(cmd(&mut ws, &args(&incr("1", "5"))).await, ":5\r\n");
        // Sent, and the process killed before the reply can be read: the
        // client cannot know whether it was applied.
        send(&mut ws, &args(&incr("2", "1"))).await;
        drop(server); // SIGKILL
    }
    let server = start(&dir, "always");
    let mut ws = connect(server.ws_port).await;
    // The reconnecting client replays both, in order.
    assert_eq!(cmd(&mut ws, &args(&incr("1", "5"))).await, "+DUP\r\n");
    let second = cmd(&mut ws, &args(&incr("2", "1"))).await;
    assert!(
        second == "+DUP\r\n" || second == ":6\r\n",
        "applied at most once: {second:?}"
    );
    assert_eq!(cmd(&mut ws, &["GET", "visits"]).await, "$1\r\n6\r\n");
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn marks_from_a_checkpoint_and_from_the_aof_after_it_both_survive_sigkill() {
    let dir = scratch("checkpoint");
    {
        let server = start(&dir, "always");
        let mut ws = connect(server.ws_port).await;
        assert_eq!(cmd(&mut ws, &args(&incr("10", "1"))).await, ":1\r\n");
        assert_eq!(cmd(&mut ws, &["SAVE"]).await, "+OK\r\n");
        assert_eq!(cmd(&mut ws, &args(&incr("11", "10"))).await, ":11\r\n");
        drop(server);
    }
    assert!(
        dir.join("dump.dedup").exists(),
        "checkpoint wrote the marks"
    );
    let server = start(&dir, "always");
    let mut ws = connect(server.ws_port).await;
    for id in ["10", "11"] {
        assert_eq!(
            cmd(&mut ws, &args(&incr(id, "100"))).await,
            "+DUP\r\n",
            "id {id}"
        );
    }
    assert_eq!(cmd(&mut ws, &["GET", "visits"]).await, "$2\r\n11\r\n");
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_refused_write_is_still_refused_on_replay_after_sigkill() {
    let dir = scratch("refusal");
    let refused = ["DEDUP", "phone-1", "20", "INCRBY", "label", "1"];
    {
        let server = start(&dir, "always");
        let mut ws = connect(server.ws_port).await;
        assert_eq!(cmd(&mut ws, &["SET", "label", "text"]).await, "+OK\r\n");
        assert!(cmd(&mut ws, &refused).await.starts_with("-ERR"));
        // A later write is accepted, so the refused id sits below the mark,
        // and a checkpoint makes that mark durable.
        assert_eq!(cmd(&mut ws, &args(&incr("21", "1"))).await, ":1\r\n");
        assert_eq!(cmd(&mut ws, &["SAVE"]).await, "+OK\r\n");
        drop(server);
    }
    let server = start(&dir, "always");
    let mut ws = connect(server.ws_port).await;
    let replay = cmd(&mut ws, &refused).await;
    assert!(
        replay.starts_with("-ERR"),
        "refused, not reported applied: {replay:?}"
    );
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}
