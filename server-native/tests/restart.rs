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

/// A port nothing in this test process has been handed before.
///
/// Binding port 0 and releasing it returns a free number, but the tests here
/// run in parallel and the OS may hand the same number out again before the
/// server binds it. Two servers then got the same port: one failed to start,
/// and its readiness probe connected to the *other* server's RESP port, so the
/// WebSocket handshake failed. That was this file's intermittent CI failure.
fn free_port() -> u16 {
    static GIVEN: std::sync::Mutex<Vec<u16>> = std::sync::Mutex::new(Vec::new());
    let mut given = GIVEN.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        if !given.contains(&port) {
            given.push(port);
            return port;
        }
    }
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
    // A port released by `free_port` can be taken by an unrelated socket
    // before the server binds it. That start fails with "address in use" and
    // proves nothing about recovery, so it is retried on fresh ports. Any other
    // startup failure — a refused AOF, say — is the test's subject and panics
    // with the server's own message.
    for attempt in 1..=3 {
        match try_start(dir, aof_sync) {
            Ok(server) => return server,
            Err(log) if attempt < 3 && is_port_collision(&log) => continue,
            Err(log) => panic!("recached-server did not start:\n{log}"),
        }
    }
    unreachable!("the last attempt returns or panics")
}

fn is_port_collision(log: &str) -> bool {
    log.contains("AddrInUse") || log.contains("Address already in use")
}

fn try_start(dir: &Path, aof_sync: &str) -> Result<Server, String> {
    let (tcp_port, ws_port) = (free_port(), free_port());
    // `RECACHED_TEST_SERVER_BIN` runs these against another build — an older
    // release, to show what they catch.
    let bin = std::env::var("RECACHED_TEST_SERVER_BIN")
        .unwrap_or_else(|_| env!("CARGO_BIN_EXE_recached-server").to_string());
    let stderr = dir.join("server.stderr");
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
        // Kept, so a failed start says why instead of just "exit status: 1".
        .stderr(std::fs::File::create(&stderr).expect("stderr log"))
        .spawn()
        .expect("start recached-server");
    let mut server = Server { child, ws_port };
    // Ready means *this* child is alive and completes a WebSocket handshake —
    // not merely that something accepts TCP on the port.
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(Some(status)) = server.child.try_wait() {
            let log = std::fs::read_to_string(&stderr).unwrap_or_default();
            return Err(format!("exit {status}\n{log}"));
        }
        if std::net::TcpStream::connect(("127.0.0.1", ws_port)).is_ok()
            && tokio_tungstenite::tungstenite::connect(format!("ws://127.0.0.1:{ws_port}")).is_ok()
        {
            return Ok(server);
        }
        if Instant::now() >= deadline {
            return Err("no WebSocket handshake within 20 s".to_string());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
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
