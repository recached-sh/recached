//! Throughput of duplicate-suppressed writes over the WebSocket port — the
//! path every browser and mobile write takes — against the real binary.
//!
//! Ignored by default: it measures, it does not check. Run in release, and
//! point `RECACHED_TEST_SERVER_BIN` at another build to compare:
//!
//! ```sh
//! cargo test --release -p recached --test dedup_bench -- --ignored --nocapture
//! ```
//!
//! Each round pipelines `WRITES` DEDUP-wrapped writes on one socket, in
//! batches of `BATCH`, and times until the last reply. The median of
//! `ROUNDS` rounds is reported, per configuration.

use futures_util::{SinkExt, StreamExt};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

const WRITES: u64 = 50_000;
/// Below the ~600 writes at which a 0.4 server disconnected a pipelining
/// client (its own fan-out lagged), so older builds can be compared too.
const BATCH: u64 = 200;
const ROUNDS: usize = 7;

struct Server(Child, u16);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn start(dir: &std::path::Path, aof: bool) -> Server {
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
        .stdout(Stdio::null())
        .stderr(if std::env::var_os("BENCH_SERVER_LOG").is_some() {
            Stdio::inherit()
        } else {
            Stdio::null()
        });
    if aof {
        cmd.env("RECACHED_AOF_PATH", dir.join("append.aof"))
            .env("RECACHED_AOF_SYNC", "everysec");
    }
    let child = cmd.spawn().expect("start recached-server");
    let deadline = Instant::now() + Duration::from_secs(20);
    while std::net::TcpStream::connect(("127.0.0.1", ws)).is_err() {
        assert!(Instant::now() < deadline, "server did not start");
        std::thread::sleep(Duration::from_millis(25));
    }
    Server(child, ws)
}

fn frame(client: &str, id: u64, op: &str) -> String {
    let id_s = id.to_string();
    let key = format!("bench:{}", id % 1_000);
    let mut parts: Vec<&str> = vec!["DEDUP", client, &id_s];
    match op {
        "SET" => parts.extend(["SET", &key, "value-of-moderate-length"]),
        _ => parts.extend(["INCR", &key]),
    }
    let mut out = format!("*{}\r\n", parts.len());
    for p in parts {
        out.push_str(&format!("${}\r\n{p}\r\n", p.len()));
    }
    out
}

async fn round(port: u16, client: &str, op: &str) -> f64 {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .unwrap();
    // The opt-in a 0.5 client sends; an older server answers an error.
    ws.send(Message::Text(
        "*3\r\n$6\r\nCLIENT\r\n$12\r\nDEDUPBARRIER\r\n$2\r\nON\r\n".into(),
    ))
    .await
    .unwrap();
    ws.next().await.unwrap().unwrap();
    let started = Instant::now();
    let mut id = 1;
    while id <= WRITES {
        let end = (id + BATCH).min(WRITES + 1);
        for i in id..end {
            ws.feed(Message::Text(frame(client, i, op).into()))
                .await
                .unwrap();
        }
        ws.flush().await.unwrap();
        for _ in id..end {
            match ws.next().await {
                Some(Ok(Message::Close(frame))) => panic!("server closed the socket: {frame:?}"),
                Some(Ok(_)) => {}
                other => panic!("socket ended: {other:?}"),
            }
        }
        id = end;
    }
    WRITES as f64 / started.elapsed().as_secs_f64()
}

#[tokio::test]
#[ignore = "benchmark; run with --ignored --nocapture in release"]
async fn dedup_write_throughput_over_websocket() {
    for aof in [false, true] {
        for op in ["SET", "INCR"] {
            let dir = std::env::temp_dir().join(format!(
                "recached-bench-{}-{}-{op}",
                std::process::id(),
                aof
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let server = start(&dir, aof);
            let mut rates = Vec::new();
            for r in 0..ROUNDS {
                // A fresh identity per round, so every write is new.
                rates.push(round(server.1, &format!("bench-{r}"), op).await);
            }
            rates.sort_by(|a, b| a.partial_cmp(b).unwrap());
            println!(
                "aof={aof:<5} {op:<4} median {:>9.0} writes/s  (min {:.0}, max {:.0})",
                rates[ROUNDS / 2],
                rates[0],
                rates[ROUNDS - 1]
            );
            drop(server);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
