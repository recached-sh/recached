//! Latency of the sync paths a reconnecting client takes — outbox replay,
//! live-query hydration, restart recovery — and the server's memory after
//! them, against the real binary.
//!
//! Ignored by default: it measures, it does not check. Run in release, and
//! point `RECACHED_TEST_SERVER_BIN` at another build to compare:
//!
//! ```sh
//! cargo test --release -p recached --test sync_bench -- --ignored --nocapture --test-threads=1
//! ```
//!
//! - **replay**: one client pipelines `REPLAY` DEDUP writes, reconnects and
//!   replays all of them (each answered `+DUP`), as after a lost connection.
//! - **hydration**: `QSUB` over `HYDRATE` existing keys, timed to its `qstate`.
//! - **recovery**: a server killed after `RECOVER` DEDUP writes (AOF on) is
//!   restarted; timed until its WebSocket port accepts, then a replay of the
//!   last write must be answered `+DUP`.
//! - **memory**: the server's RSS when idle and after the above.
//!
//! Medians of `ROUNDS`. Wall-clock numbers on a shared machine are noisy:
//! compare builds back to back on the same host, not against figures from
//! elsewhere.

use futures_util::{SinkExt, StreamExt};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

const REPLAY: u64 = 10_000;
const BATCH: u64 = 200;
const HYDRATE: &[u64] = &[1_000, 10_000];
const RECOVER: u64 = 50_000;
const ROUNDS: usize = 5;

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Server {
    child: Child,
    ws: u16,
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

fn start(dir: &Path, aof: bool) -> (Server, Duration) {
    let bin = std::env::var("RECACHED_TEST_SERVER_BIN")
        .unwrap_or_else(|_| env!("CARGO_BIN_EXE_recached-server").to_string());
    let (tcp, ws) = (free_port(), free_port());
    let mut cmd = Command::new(bin);
    cmd.current_dir(dir)
        .env_clear()
        .env("RECACHED_BIND", "127.0.0.1")
        .env("RECACHED_PORT", tcp.to_string())
        .env("RECACHED_WS_PORT", ws.to_string())
        .env("RECACHED_METRICS_PORT", "0")
        .env("RECACHED_SAVE_INTERVAL", "0")
        .env("RECACHED_SAVE_PATH", dir.join("dump.rdb"))
        .env("RECACHED_MAX_QSUB_INITIAL_KEYS", "100000")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if aof {
        cmd.env("RECACHED_AOF_PATH", dir.join("append.aof"))
            .env("RECACHED_AOF_SYNC", "everysec");
    }
    let started = Instant::now();
    let child = cmd.spawn().expect("start recached-server");
    let deadline = started + Duration::from_secs(60);
    while std::net::TcpStream::connect(("127.0.0.1", ws)).is_err() {
        assert!(Instant::now() < deadline, "server did not start");
        std::thread::sleep(Duration::from_millis(2));
    }
    (Server { child, ws }, started.elapsed())
}

/// Resident set size in KiB, from `ps` (macOS and Linux).
fn rss_kib(pid: u32) -> u64 {
    let out = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .expect("ps");
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

fn resp(parts: &[&str]) -> String {
    let mut out = format!("*{}\r\n", parts.len());
    for p in parts {
        out.push_str(&format!("${}\r\n{p}\r\n", p.len()));
    }
    out
}

fn dedup_set(client: &str, id: u64, key: &str) -> String {
    resp(&[
        "DEDUP",
        client,
        &id.to_string(),
        "SET",
        key,
        "value-of-moderate-length",
    ])
}

async fn connect(port: u16) -> Ws {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .unwrap();
    // The opt-in a 0.5 client sends; an older server answers an error.
    ws.send(Message::Text(
        resp(&["CLIENT", "DEDUPBARRIER", "ON"]).into(),
    ))
    .await
    .unwrap();
    ws.next().await.unwrap().unwrap();
    ws
}

async fn next_text(ws: &mut Ws) -> String {
    loop {
        match ws.next().await {
            Some(Ok(Message::Text(t))) => return t.to_string(),
            Some(Ok(Message::Binary(b))) => return String::from_utf8_lossy(&b).into_owned(),
            Some(Ok(Message::Close(f))) => panic!("server closed the socket: {f:?}"),
            Some(Ok(_)) => {}
            other => panic!("socket ended: {other:?}"),
        }
    }
}

/// Pipelines `frames` in batches and waits for one reply each. Returns the
/// replies that started with `prefix`.
async fn pipeline(ws: &mut Ws, frames: &[String], prefix: &str) -> usize {
    let mut matching = 0;
    for chunk in frames.chunks(BATCH as usize) {
        for f in chunk {
            ws.feed(Message::Text(f.clone().into())).await.unwrap();
        }
        ws.flush().await.unwrap();
        for _ in chunk {
            if next_text(ws).await.starts_with(prefix) {
                matching += 1;
            }
        }
    }
    matching
}

fn median(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[0], v[v.len() - 1])
}

fn temp(tag: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("recached-sync-bench-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
#[ignore = "benchmark; run with --ignored --nocapture in release"]
async fn sync_paths() {
    // ── replay and hydration, AOF off ───────────────────────────────────
    let dir = temp("live");
    let (server, _) = start(&dir, false);
    let pid = server.child.id();
    println!("rss idle                     {:>8} KiB", rss_kib(pid));

    let mut replay_ms = Vec::new();
    for r in 0..ROUNDS {
        let client = format!("replay-{r}");
        let frames: Vec<String> = (1..=REPLAY)
            .map(|i| dedup_set(&client, i, &format!("replay:{r}:{}", i % 1_000)))
            .collect();
        let mut ws = connect(server.ws).await;
        pipeline(&mut ws, &frames, "+").await;
        drop(ws);
        // The reconnect: the whole outbox again, every write already applied.
        let mut ws = connect(server.ws).await;
        let started = Instant::now();
        let dups = pipeline(&mut ws, &frames, "+DUP").await;
        replay_ms.push(started.elapsed().as_secs_f64() * 1e3);
        assert_eq!(dups as u64, REPLAY, "every replayed write is answered +DUP");
    }
    let (m, lo, hi) = median(replay_ms);
    println!("replay {REPLAY} applied writes   median {m:>8.1} ms  (min {lo:.1}, max {hi:.1})");
    println!("rss after replay             {:>8} KiB", rss_kib(pid));

    for &n in HYDRATE {
        let mut ws = connect(server.ws).await;
        let frames: Vec<String> = (1..=n)
            .map(|i| dedup_set(&format!("seed-{n}"), i, &format!("hyd{n}:{i}")))
            .collect();
        pipeline(&mut ws, &frames, "+").await;
        let mut times = Vec::new();
        for _ in 0..ROUNDS {
            let mut ws = connect(server.ws).await;
            let started = Instant::now();
            ws.send(Message::Text(resp(&["QSUB", &format!("hyd{n}:*")]).into()))
                .await
                .unwrap();
            let reply = next_text(&mut ws).await;
            times.push(started.elapsed().as_secs_f64() * 1e3);
            assert!(
                reply.contains("qstate"),
                "QSUB answered {}",
                &reply[..reply.len().min(80)]
            );
        }
        let (m, lo, hi) = median(times);
        println!("hydrate {n:>6} keys            median {m:>8.1} ms  (min {lo:.1}, max {hi:.1})");
    }
    println!("rss after hydration          {:>8} KiB", rss_kib(pid));
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);

    // ── recovery, AOF on ────────────────────────────────────────────────
    let dir = temp("recover");
    let (mut server, _) = start(&dir, true);
    let frames: Vec<String> = (1..=RECOVER)
        .map(|i| dedup_set("recover", i, &format!("recover:{}", i % 5_000)))
        .collect();
    let mut ws = connect(server.ws).await;
    pipeline(&mut ws, &frames, "+").await;
    drop(ws);
    // Past the everysec fsync, then a crash.
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    let _ = server.child.kill();
    let _ = server.child.wait();
    let aof_bytes = std::fs::metadata(dir.join("append.aof"))
        .map(|m| m.len())
        .unwrap_or(0);
    let mut times = Vec::new();
    for _ in 0..ROUNDS {
        let (server, took) = start(&dir, true);
        times.push(took.as_secs_f64() * 1e3);
        let mut ws = connect(server.ws).await;
        let last = frames.last().unwrap().clone();
        assert_eq!(
            pipeline(&mut ws, &[last], "+DUP").await,
            1,
            "a replay after restart is answered +DUP"
        );
        // Kill rather than stop, so every round replays the same AOF.
        let mut server = server;
        let _ = server.child.kill();
        let _ = server.child.wait();
    }
    let (m, lo, hi) = median(times);
    println!(
        "restart on {RECOVER} DEDUP writes ({} KiB AOF)  median {m:>8.1} ms  (min {lo:.1}, max {hi:.1})",
        aof_bytes / 1024
    );
    let (server, _) = start(&dir, true);
    println!(
        "rss after recovery           {:>8} KiB",
        rss_kib(server.child.id())
    );
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}
