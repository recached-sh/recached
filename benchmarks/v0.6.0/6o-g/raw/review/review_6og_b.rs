//! Review B probes for 6O-G two-phase EXEC (d492a94). Not for commit.

use super::mset_atomicity::{Pipe, WITHIN, bulk};
use super::request_path::Shared;
use crate::*;
use core_engine::resp::Value;
use core_engine::store::{EvictionPolicy, KeyValueStore};
use std::time::{Duration, Instant};

fn ok() -> Value {
    Value::SimpleString("OK".into())
}

async fn queue(pipe: &mut Pipe, commands: &[Vec<String>]) {
    assert_eq!(pipe.cmd(&["MULTI"]).await, ok());
    for command in commands {
        let args: Vec<&str> = command.iter().map(String::as_str).collect();
        assert_eq!(
            pipe.cmd(&args).await,
            Value::SimpleString("QUEUED".into()),
            "{command:?}"
        );
    }
}

fn cmds(list: &[&[&str]]) -> Vec<Vec<String>> {
    list.iter()
        .map(|c| c.iter().map(|s| s.to_string()).collect())
        .collect()
}

async fn exec(pipe: &mut Pipe) -> Value {
    pipe.send(&["EXEC"]).await;
    tokio::time::timeout(WITHIN * 30, pipe.reply())
        .await
        .expect("EXEC reply")
}

// ── B1: the notification plan is quadratic in one command's keys ────────────

async fn timed_del(shared: &Shared, n: usize, in_tx: bool, tag: &str) -> Duration {
    let mut c = Pipe::tcp(shared);
    let mut del: Vec<String> = vec!["DEL".into()];
    del.extend((0..n).map(|i| format!("rb1:{tag}:{n}:{i}")));
    let args: Vec<&str> = del.iter().map(String::as_str).collect();
    let started;
    if in_tx {
        queue(&mut c, std::slice::from_ref(&del)).await;
        started = Instant::now();
        let reply = exec(&mut c).await;
        assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
    } else {
        started = Instant::now();
        c.send(&args).await;
        tokio::time::timeout(WITHIN * 30, c.reply())
            .await
            .expect("DEL");
    }
    started.elapsed()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b1_a_wide_del_in_exec_with_any_watcher_is_quadratic() {
    let shared = Shared::new(false);
    let mut results = Vec::new();
    // Without any watcher registered (control).
    for n in [5_000usize, 20_000] {
        let t = timed_del(&shared, n, true, "nowatch").await;
        results.push(format!("exec no-watcher n={n}: {t:?}"));
    }
    // Any WATCH anywhere on the server, of an unrelated key.
    let mut watcher = Pipe::tcp(&shared);
    assert_eq!(watcher.cmd(&["WATCH", "rb1:unrelated"]).await, ok());
    let mut tx_times = Vec::new();
    for n in [5_000usize, 10_000, 20_000] {
        let plain = timed_del(&shared, n, false, "plain").await;
        let t = timed_del(&shared, n, true, "watch").await;
        results.push(format!("plain DEL with watcher n={n}: {plain:?}; EXEC DEL with watcher n={n}: {t:?}"));
        tx_times.push(t);
    }
    for r in &results {
        eprintln!("B1 {r}");
    }
    let ratio = tx_times[2].as_secs_f64() / tx_times[0].as_secs_f64();
    eprintln!("B1 ratio t(20k)/t(5k) with watcher = {ratio:.1} (linear ~4, quadratic ~16)");
    assert!(
        ratio < 8.0,
        "EXEC of one wide DEL scales quadratically when anything is watched: ratio {ratio:.1}"
    );
}

// ── B2: AOF and replication streams equal per-command execution ────────────

async fn run_streams(
    in_tx: bool,
    fanout: AofFanout,
    sync: AofSync,
    watcher: bool,
    commands: &[Vec<String>],
) -> (Vec<u8>, Vec<u8>, u64) {
    let path = std::env::temp_dir().join(format!(
        "recached_review_b2_{}_{}.aof",
        std::process::id(),
        next_conn_id()
    ));
    let _ = std::fs::remove_file(&path);
    let (aof, stats) = AofWriter::open(path.clone(), sync)
        .await
        .unwrap()
        .with_private_stats();
    aof.set_fanout(fanout);
    let mut shared = Shared::with_aof(false, Some(Arc::new(aof)));
    shared.store = Arc::new(KeyValueStore::with_config(
        Some(1),
        None,
        EvictionPolicy::AllKeysLru,
    ));
    shared.state.replicas.configure_backlog(1 << 20);
    let mut replica = shared
        .state
        .replicas
        .attach_test_replica(1024, 1 << 24)
        .await;
    let mut w = Pipe::tcp(&shared);
    if watcher {
        assert_eq!(w.cmd(&["WATCH", "rb2:unrelated"]).await, ok());
    }
    let mut c = Pipe::tcp(&shared);
    let before = stats.batches.load(Ordering::SeqCst);
    if in_tx {
        queue(&mut c, commands).await;
        let reply = exec(&mut c).await;
        assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
    } else {
        for command in commands {
            let args: Vec<&str> = command.iter().map(String::as_str).collect();
            c.cmd(&args).await;
        }
    }
    let batches = stats.batches.load(Ordering::SeqCst) - before;
    shared.state.aof.as_ref().unwrap().flush().await.unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let mut repl = Vec::new();
    while let Ok(frame) = replica.try_recv() {
        repl.extend_from_slice(frame.bytes());
    }
    (bytes, repl, batches)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b2_aof_and_replication_streams_match_per_command_execution() {
    let commands = cmds(&[
        &["SET", "rb2:a", "1"],
        &["SET", "rb2:b", "2"], // evicts a
        &["INCR", "rb2:b"],
        &["SET", "rb2:b", "9", "NX"], // not effective
        &["APPEND", "rb2:c", "x"], // evicts b
        &["DEL", "rb2:c"],
        &["SET", "rb2:d", "4"],
        &["SET", "rb2:d", "5", "XX"],
        &["MSET", "rb2:d", "6", "rb2:d", "7"],
    ]);
    for fanout in [AofFanout::Inline, AofFanout::Coalesced] {
        for sync in [AofSync::Always, AofSync::No] {
            for watcher in [false, true] {
                let (aof_seq, repl_seq, _) =
                    run_streams(false, fanout, sync, watcher, &commands).await;
                let (aof_tx, repl_tx, batches) =
                    run_streams(true, fanout, sync, watcher, &commands).await;
                let label = format!("{fanout:?} {sync:?} watcher={watcher}");
                assert_eq!(
                    String::from_utf8_lossy(&aof_tx),
                    String::from_utf8_lossy(&aof_seq),
                    "{label}: AOF bytes differ"
                );
                assert_eq!(
                    String::from_utf8_lossy(&repl_tx),
                    String::from_utf8_lossy(&aof_tx),
                    "{label}: replication stream differs from the AOF"
                );
                assert_eq!(repl_seq, aof_seq, "{label}: baseline");
                assert_eq!(batches, 1, "{label}: transaction split across batches");
                assert!(
                    String::from_utf8_lossy(&aof_tx).contains("DEL"),
                    "{label}: no eviction DEL"
                );
            }
        }
    }
}

// ── B3: sync pushes per command, in order ──────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b3_sync_pushes_stay_per_command_in_order() {
    let commands = cmds(&[
        &["SET", "rb3:a", "1"],
        &["APPEND", "rb3:a", "2"],
        &["SET", "rb3:a", "x", "NX"],
        &["MSET", "rb3:b", "1", "rb3:b", "2"],
        &["DEL", "rb3:a"],
        &["RPUSH", "rb3:l", "p", "q"],
    ]);
    let mut pushes: Vec<Vec<Vec<u8>>> = Vec::new();
    for in_tx in [false, true] {
        let shared = Shared::new(false);
        let mut sub = shared
            .state
            .replicas
            .subscribe_pushes(&shared.tx, Routes::Everything)
            .await;
        let mut c = Pipe::tcp(&shared);
        if in_tx {
            queue(&mut c, &commands).await;
            exec(&mut c).await;
        } else {
            for command in &commands {
                let args: Vec<&str> = command.iter().map(String::as_str).collect();
                c.cmd(&args).await;
            }
        }
        let mut got = Vec::new();
        while let Ok(Ok(push)) =
            tokio::time::timeout(Duration::from_millis(200), sub.recv()).await
        {
            got.push(push.resp.clone());
        }
        pushes.push(got);
    }
    assert_eq!(pushes[0].len(), 5);
    assert_eq!(pushes[0], pushes[1]);
}

// ── B4: delta/QSUB clients converge in edge cases ──────────────────────────

struct WsPipe {
    ws: tokio_tungstenite::WebSocketStream<tokio::io::DuplexStream>,
}

impl WsPipe {
    async fn connect(shared: &Shared) -> WsPipe {
        let (client, server) = tokio::io::duplex(1 << 20);
        tokio::spawn(shared.serve_ws(server));
        let (ws, _) = tokio_tungstenite::client_async("ws://localhost/", client)
            .await
            .expect("handshake");
        WsPipe { ws }
    }

    async fn send(&mut self, args: &[&str]) {
        use futures_util::SinkExt;
        let mut req = format!("*{}\r\n", args.len());
        for a in args {
            req.push_str(&format!("${}\r\n{a}\r\n", a.len()));
        }
        self.ws
            .send(tokio_tungstenite::tungstenite::Message::Text(req.into()))
            .await
            .expect("send");
    }

    async fn frame(&mut self, ms: u64) -> Option<Value> {
        use futures_util::StreamExt;
        let fut = async {
            loop {
                let raw = match self.ws.next().await? {
                    Ok(tokio_tungstenite::tungstenite::Message::Text(t)) => t.as_bytes().to_vec(),
                    Ok(tokio_tungstenite::tungstenite::Message::Binary(b)) => b.to_vec(),
                    Ok(_) => continue,
                    Err(_) => return None,
                };
                if let Ok((v, _)) = Value::parse(&raw) {
                    return Some(v);
                }
            }
        };
        tokio::time::timeout(Duration::from_millis(ms), fut)
            .await
            .ok()
            .flatten()
    }
}

#[derive(Default)]
struct Copy {
    values: HashMap<String, Value>,
    pattern: String,
}

fn text(v: &Value) -> String {
    match v {
        Value::BulkString(Some(b)) => String::from_utf8_lossy(b).into_owned(),
        Value::SimpleString(s) => s.clone(),
        other => panic!("{other:?}"),
    }
}

impl Copy {
    fn apply(&mut self, v: Value) -> bool {
        let items = match v {
            Value::Array(Some(items)) | Value::Push(items) => items,
            _ => return false,
        };
        if items.is_empty() {
            return false;
        }
        let Value::BulkString(Some(_)) = &items[0] else {
            return false;
        };
        let kind = text(&items[0]);
        if kind != "keychange" && kind != "keydelta" {
            return false;
        }
        let key = text(&items[1]);
        if kind == "keychange" {
            if key == self.pattern {
                // FLUSHDB sentinel.
                assert!(matches!(items[2], Value::BulkString(None)));
                self.values.clear();
                return true;
            }
            match &items[2] {
                Value::BulkString(None) => {
                    self.values.remove(&key);
                }
                value => {
                    self.values.insert(key, value.clone());
                }
            }
            return true;
        }
        let op = text(&items[2]);
        let args: Vec<Vec<u8>> = items[3..]
            .iter()
            .map(|v| match v {
                Value::BulkString(Some(b)) => b.clone(),
                other => panic!("{other:?}"),
            })
            .collect();
        let entry = self.values.entry(key).or_insert(Value::BulkString(None));
        match op.as_str() {
            "append" => {
                let mut now = match entry {
                    Value::BulkString(Some(b)) => b.clone(),
                    _ => Vec::new(),
                };
                now.extend_from_slice(&args[0]);
                *entry = Value::BulkString(Some(now));
            }
            "rpush" | "lpush" => {
                let mut now = match entry {
                    Value::Array(Some(items)) => items.clone(),
                    _ => Vec::new(),
                };
                for arg in args {
                    if op == "rpush" {
                        now.push(Value::BulkString(Some(arg)));
                    } else {
                        now.insert(0, Value::BulkString(Some(arg)));
                    }
                }
                *entry = Value::Array(Some(now));
            }
            other => panic!("model lacks {other}"),
        }
        true
    }

    async fn drain(&mut self, ws: &mut WsPipe) {
        while let Some(v) = ws.frame(300).await {
            self.apply(v);
        }
    }
}

async fn converge_case(shared: &Shared, prep: &[Vec<String>], tx: &[Vec<String>], keys: &[&str]) {
    let mut viewer = WsPipe::connect(shared).await;
    viewer.send(&["CLIENT", "DELTA", "ON"]).await;
    viewer.send(&["QSUB", "rb4:*"]).await;
    let mut copy = Copy {
        pattern: "rb4:*".into(),
        ..Copy::default()
    };
    let mut writer = Pipe::tcp(shared);
    writer.cmd(&["FLUSHDB"]).await;
    copy.drain(&mut viewer).await;
    copy.values.clear();
    for c in prep {
        let args: Vec<&str> = c.iter().map(String::as_str).collect();
        writer.cmd(&args).await;
    }
    copy.drain(&mut viewer).await;
    queue(&mut writer, tx).await;
    let reply = exec(&mut writer).await;
    assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
    copy.drain(&mut viewer).await;
    for key in keys {
        let server = match shared.store.get_current_with_ttl(key).0 {
            Value::BulkString(None) => None,
            v => Some(v),
        };
        assert_eq!(
            copy.values.get(*key).cloned(),
            server,
            "{key} diverged after {tx:?} (reply {reply:?})"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn b4_delta_clients_converge_in_edge_cases() {
    let shared = Shared::new(false);
    let cases: Vec<(Vec<Vec<String>>, Vec<Vec<String>>, Vec<&str>)> = vec![
        (
            cmds(&[&["SET", "rb4:k", "z"]]),
            cmds(&[&["SET", "rb4:k", "a"], &["FLUSHDB"], &["APPEND", "rb4:k", "b"]]),
            vec!["rb4:k"],
        ),
        (
            cmds(&[]),
            cmds(&[&["MSET", "rb4:m", "1", "rb4:m", "2"], &["APPEND", "rb4:n", "q"]]),
            vec!["rb4:m", "rb4:n"],
        ),
        (
            cmds(&[&["RPUSH", "rb4:s", "a"]]),
            cmds(&[&["RENAME", "rb4:s", "rb4:t"], &["RENAME", "rb4:t", "rb4:s"], &["RPUSH", "rb4:s", "b"]]),
            vec!["rb4:s", "rb4:t"],
        ),
        (
            cmds(&[&["SET", "rb4:x", "old"]]),
            cmds(&[&["SET", "rb4:x", "1"], &["DEL", "rb4:x"], &["RPUSH", "rb4:x", "a"]]),
            vec!["rb4:x"],
        ),
        (
            cmds(&[&["APPEND", "rb4:o", "a"]]),
            cmds(&[&["APPEND", "rb4:o", "b"], &["SET", "rb4:o", "c", "NX"], &["GET", "rb4:o"]]),
            vec!["rb4:o"],
        ),
        (
            cmds(&[&["SET", "rb4:c", "v"]]),
            cmds(&[&["COPY", "rb4:c", "rb4:d"], &["APPEND", "rb4:c", "w"], &["APPEND", "rb4:d", "y"]]),
            vec!["rb4:c", "rb4:d"],
        ),
        (
            cmds(&[&["SET", "rb4:f", "v"]]),
            cmds(&[&["FLUSHDB"], &["FLUSHDB"], &["APPEND", "rb4:f", "y"]]),
            vec!["rb4:f"],
        ),
    ];
    for (prep, tx, keys) in &cases {
        converge_case(&shared, prep, tx, keys).await;
    }

    // Capped: a key evicted, then re-created by a delta write, in one EXEC.
    let mut capped = Shared::new(false);
    capped.store = Arc::new(KeyValueStore::with_config(
        Some(1),
        None,
        EvictionPolicy::AllKeysLru,
    ));
    converge_case(
        &capped,
        &cmds(&[&["SET", "rb4:e1", "a"]]),
        &cmds(&[&["SET", "rb4:e2", "b"], &["APPEND", "rb4:e1", "z"]]),
        &["rb4:e1", "rb4:e2"],
    )
    .await;
    converge_case(
        &capped,
        &cmds(&[&["SET", "rb4:g1", "a"]]),
        &cmds(&[&["APPEND", "rb4:g1", "b"], &["SET", "rb4:g2", "c"]]),
        &["rb4:g1", "rb4:g2"],
    )
    .await;
}

// ── B5: PUBLISH inside EXEC ────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b5_publish_inside_exec_keeps_order_and_counts() {
    let shared = Shared::new(false);
    let mut sub = Pipe::tcp(&shared);
    sub.send(&["SUBSCRIBE", "rb5:ch"]).await;
    sub.reply().await;
    let mut c = Pipe::tcp(&shared);
    queue(
        &mut c,
        &cmds(&[
            &["PUBLISH", "rb5:ch", "m1"],
            &["SET", "rb5:k", "v"],
            &["PUBLISH", "rb5:ch", "m2"],
            &["PUBLISH", "rb5:none", "m3"],
        ]),
    )
    .await;
    assert_eq!(
        exec(&mut c).await,
        Value::Array(Some(vec![
            Value::Integer(1),
            ok(),
            Value::Integer(1),
            Value::Integer(0)
        ]))
    );
    let m1 = tokio::time::timeout(WITHIN, sub.reply()).await.unwrap();
    let m2 = tokio::time::timeout(WITHIN, sub.reply()).await.unwrap();
    let msg = |v: &Value| match v {
        Value::Array(Some(items)) | Value::Push(items) => items.last().map(text),
        _ => None,
    };
    assert_eq!(msg(&m1).as_deref(), Some("m1"), "{m1:?}");
    assert_eq!(msg(&m2).as_deref(), Some("m2"), "{m2:?}");
    let _ = bulk("x");
}

// ── B6: no EXEC reply before its frames are written ─────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b6_no_exec_reply_precedes_its_aof_write() {
    for fanout in [AofFanout::Inline, AofFanout::Coalesced] {
        for mode in ["tcp-deferred", "tcp-watcher", "ws"] {
            let path = std::env::temp_dir().join(format!(
                "recached_review_b6_{}_{}.aof",
                std::process::id(),
                next_conn_id()
            ));
            let _ = std::fs::remove_file(&path);
            let aof = AofWriter::open(path.clone(), AofSync::EverySec).await.unwrap();
            aof.set_fanout(fanout);
            let shared = Shared::with_aof(false, Some(Arc::new(aof)));
            let mut w = Pipe::tcp(&shared);
            if mode == "tcp-watcher" {
                w.cmd(&["WATCH", "rb6:unrelated"]).await;
            }
            let big = "B".repeat(200_000);
            let n = 300usize;
            let mut tx: Vec<Vec<String>> = vec![vec!["SET".into(), "rb6:big".into(), big.clone()]];
            tx.extend((0..n).map(|i| vec!["SET".into(), format!("rb6:{i}"), "v".into()]));
            let reply = if mode == "ws" {
                let mut ws = WsPipe::connect(&shared).await;
                ws.send(&["MULTI"]).await;
                ws.frame(5_000).await.unwrap();
                for c in &tx {
                    let args: Vec<&str> = c.iter().map(String::as_str).collect();
                    ws.send(&args).await;
                    ws.frame(5_000).await.unwrap();
                }
                ws.send(&["EXEC"]).await;
                ws.frame(20_000).await.unwrap()
            } else {
                let mut c = Pipe::tcp(&shared);
                queue(&mut c, &tx).await;
                exec(&mut c).await
            };
            let on_disk = std::fs::read(&path).unwrap_or_default();
            let logged = on_disk.windows(7).filter(|w| w == b"$3\r\nSET").count();
            assert!(matches!(reply, Value::Array(Some(ref v)) if v.len() == n + 1), "{reply:?}");
            assert_eq!(
                logged,
                n + 1,
                "{fanout:?} {mode}: EXEC answered with {logged} of {} SETs in the AOF",
                n + 1
            );
            let _ = std::fs::remove_file(&path);
        }
    }
}
