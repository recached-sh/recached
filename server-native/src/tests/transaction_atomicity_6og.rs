//! 6O-G: two-phase `EXEC` — a transaction's writes are visible to readers
//! outside it all at once (6O-E finding F1).
//!
//! The transaction is stopped inside its write span, synchronously, with its
//! store stripes held and marked (`write_pause::after_transaction_write_of`).
//! A reader on another connection then either answers with the state before
//! the transaction — only if it never reached a marked stripe — or waits,
//! which the hook counts; after the stop is released it must see the whole
//! transaction. Before 6O-G the probe in this file answered `[new, old]` in
//! 10 runs out of 10 (`benchmarks/v0.6.0/6o-g/raw/g0/`).
//!
//! The rest covers what the two phases must keep: read-your-writes with
//! every observer on, notifications coalesced per key (WATCH, live queries,
//! delta clients), ephemeral state changed in command order, the AOF batch,
//! and failing closed when a queued write panics.

use super::mset_atomicity::{Pipe, WITHIN, bulk};
use super::request_path::Shared;
use super::{WsClient, spawn_ws_server};
use crate::*;
use core_engine::resp::Value;
use core_engine::store::{EvictionPolicy, transaction_fault, write_pause};
use std::time::{Duration, Instant};

fn ok() -> Value {
    Value::SimpleString("OK".into())
}

fn array(items: Vec<Value>) -> Value {
    Value::Array(Some(items))
}

fn nil() -> Value {
    Value::BulkString(None)
}

async fn queue(pipe: &mut Pipe, commands: &[&[&str]]) {
    assert_eq!(pipe.cmd(&["MULTI"]).await, ok());
    for command in commands {
        assert_eq!(
            pipe.cmd(command).await,
            Value::SimpleString("QUEUED".into()),
            "{command:?}"
        );
    }
}

/// Send `EXEC` on `pipe` and read its reply.
async fn exec(pipe: &mut Pipe) -> Value {
    pipe.send(&["EXEC"]).await;
    reply_within(pipe, "EXEC").await
}

async fn reply_within(pipe: &mut Pipe, what: &str) -> Value {
    tokio::time::timeout(WITHIN, pipe.reply())
        .await
        .unwrap_or_else(|_| panic!("no {what} reply"))
}

/// The reply to a read already sent on `pipe`, if it arrives before the
/// reader is seen waiting on the stopped transaction (`None` once it is).
async fn reply_or_blocked(pipe: &mut Pipe, pause: &write_pause::Pause) -> Option<Value> {
    let deadline = Instant::now() + WITHIN;
    while Instant::now() < deadline {
        if pause.readers_blocked() > 0 {
            return None;
        }
        if let Ok(reply) = tokio::time::timeout(Duration::from_millis(2), pipe.reply()).await {
            return Some(reply);
        }
    }
    panic!("the reader neither answered nor waited for the transaction");
}

fn reached(pause: &write_pause::Pause) {
    assert!(
        tokio::task::block_in_place(|| pause.wait_reached(WITHIN)),
        "the transaction never reached its stop"
    );
}

/// A key on none of `keys`' store stripes.
fn apart_from(keys: &[&str], prefix: &str) -> String {
    (0..)
        .map(|i| format!("{prefix}{i}"))
        .find(|other| {
            keys.iter()
                .all(|key| write_pause::stripe_of(key) != write_pause::stripe_of(other))
        })
        .unwrap()
}

// ── Outside readers (G1) ───────────────────────────────────────────────────

/// The F1 reproduction, now a regression test: `MULTI SET a new SET b new
/// EXEC` stopped after its first write; another connection's `MGET a b`.
/// Redis answers `[old, old]` or `[new, new]`; before 6O-G, `[new, old]`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_outside_reader_never_sees_a_transaction_part_way() {
    let shared = Shared::new(false);
    let (a, b) = ("6og:f1:a", "6og:f1:b");
    let (mut txc, mut reader) = (Pipe::tcp_on_own_thread(&shared), Pipe::tcp(&shared));
    assert_eq!(txc.cmd(&["MSET", a, "old", b, "old"]).await, ok());
    queue(&mut txc, &[&["SET", a, "new"], &["SET", b, "new"]]).await;
    let pause = write_pause::after_transaction_write_of(&shared.store, a, &[a, b]);
    txc.send(&["EXEC"]).await;
    reached(&pause);
    reader.send(&["MGET", a, b]).await;
    let during = reply_or_blocked(&mut reader, &pause).await;
    drop(pause);
    assert_eq!(
        reply_within(&mut txc, "EXEC").await,
        array(vec![ok(), ok()])
    );
    let seen = match during.clone() {
        Some(seen) => seen,
        None => reply_within(&mut reader, "MGET").await,
    };
    assert!(
        during.is_none() && seen == array(vec![bulk("new"), bulk("new")]),
        "an outside MGET saw the transaction part-way: {seen:?} (during: {during:?})"
    );
}

/// Every kind of outside read of the transaction's keys, from the first key
/// it writes and from a later one, waits for the whole transaction.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn outside_reads_of_every_kind_wait_for_the_whole_transaction() {
    let shared = Shared::new(false);
    type Read<'a> = (&'a str, &'a [&'a str], fn(&str, &str) -> Value);
    let reads: &[Read<'_>] = &[
        ("get-first", &["GET", "{a}"], |_, _| bulk("new")),
        ("get-later", &["GET", "{b}"], |_, _| bulk("new")),
        ("mget", &["MGET", "{b}", "{a}"], |_, _| {
            array(vec![bulk("new"), bulk("new")])
        }),
        ("exists", &["EXISTS", "{a}", "{b}"], |_, _| {
            Value::Integer(2)
        }),
        ("type", &["TYPE", "{b}"], |_, _| {
            Value::SimpleString("string".into())
        }),
        ("strlen", &["STRLEN", "{b}"], |_, _| Value::Integer(3)),
    ];
    for (label, read, expected) in reads {
        let a = format!("6og:kinds:{label}:a");
        let b = format!("6og:kinds:{label}:b");
        let read: Vec<String> = read
            .iter()
            .map(|arg| arg.replace("{a}", &a).replace("{b}", &b))
            .collect();
        let read: Vec<&str> = read.iter().map(String::as_str).collect();
        let (mut txc, mut reader) = (Pipe::tcp_on_own_thread(&shared), Pipe::tcp(&shared));
        queue(&mut txc, &[&["SET", &a, "new"], &["SET", &b, "new"]]).await;
        let pause = write_pause::after_transaction_write_of(&shared.store, &a, &[&a, &b]);
        txc.send(&["EXEC"]).await;
        reached(&pause);
        reader.send(&read).await;
        let during = reply_or_blocked(&mut reader, &pause).await;
        drop(pause);
        reply_within(&mut txc, "EXEC").await;
        assert_eq!(during, None, "{label}: answered inside the span");
        assert_eq!(
            reply_within(&mut reader, label).await,
            expected(&a, &b),
            "{label}"
        );
    }
}

/// Several readers wait together and all see the whole transaction; a
/// writer of an unrelated key on another store stripe is not held back.
// Each waiting reader blocks a worker thread, as the design states: leave
// the test task workers to run on.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn several_readers_wait_and_an_unrelated_writer_does_not() {
    let shared = Shared::new(false);
    let (a, b) = ("6og:many:a", "6og:many:b");
    let unrelated = apart_from(&[a, b], "6og:many:u");
    let mut txc = Pipe::tcp_on_own_thread(&shared);
    let mut readers: Vec<Pipe> = (0..3).map(|_| Pipe::tcp(&shared)).collect();
    let mut writer = Pipe::tcp(&shared);
    queue(&mut txc, &[&["SET", a, "new"], &["SET", b, "new"]]).await;
    let pause = write_pause::after_transaction_write_of(&shared.store, a, &[a, b]);
    txc.send(&["EXEC"]).await;
    reached(&pause);
    for reader in &mut readers {
        reader.send(&["MGET", a, b]).await;
    }
    // Every reader is stuck: none answers while stopped.
    let deadline = Instant::now() + WITHIN;
    while pause.readers_blocked() < readers.len() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert!(pause.readers_blocked() >= readers.len());
    assert_eq!(
        tokio::time::timeout(WITHIN, writer.cmd(&["SET", &unrelated, "x"])).await,
        Ok(ok()),
        "an unrelated writer waited for the transaction"
    );
    drop(pause);
    reply_within(&mut txc, "EXEC").await;
    for reader in &mut readers {
        assert_eq!(
            reply_within(reader, "MGET").await,
            array(vec![bulk("new"), bulk("new")])
        );
    }
}

/// A WebSocket connection over an in-memory stream, served by `shared`
/// (optionally on a thread of its own, as [`Pipe::tcp_on_own_thread`]).
struct WsPipe {
    ws: tokio_tungstenite::WebSocketStream<tokio::io::DuplexStream>,
}

impl WsPipe {
    async fn connect(shared: &Shared, own_thread: bool) -> WsPipe {
        let (client, server) = tokio::io::duplex(64 * 1024);
        let serve = shared.serve_ws(server);
        if own_thread {
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("runtime")
                    .block_on(serve);
            });
        } else {
            tokio::spawn(serve);
        }
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

    /// The next command reply, skipping pushes and notifications.
    async fn reply(&mut self) -> Value {
        use futures_util::StreamExt;
        loop {
            let raw = match self.ws.next().await.expect("open").expect("frame") {
                tokio_tungstenite::tungstenite::Message::Text(t) => t.as_bytes().to_vec(),
                tokio_tungstenite::tungstenite::Message::Binary(b) => b.to_vec(),
                _ => continue,
            };
            let Ok((value, _)) = Value::parse(&raw) else {
                continue;
            };
            if matches!(value, Value::Push(_)) {
                continue;
            }
            return value;
        }
    }

    async fn cmd(&mut self, args: &[&str]) -> Value {
        self.send(args).await;
        tokio::time::timeout(WITHIN, self.reply())
            .await
            .expect("a reply")
    }
}

/// A WebSocket reader waits like a TCP one, and a transaction sent over a
/// WebSocket is as atomic as one sent over TCP (one implementation).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn websocket_readers_and_transactions_are_atomic_too() {
    let shared = Shared::new(false);
    let (a, b) = ("6og:ws:a", "6og:ws:b");
    let mut txc = WsPipe::connect(&shared, true).await;
    let mut reader = WsPipe::connect(&shared, false).await;
    assert_eq!(txc.cmd(&["MSET", a, "old", b, "old"]).await, ok());
    assert_eq!(txc.cmd(&["MULTI"]).await, ok());
    txc.cmd(&["SET", a, "new"]).await;
    txc.cmd(&["SET", b, "new"]).await;
    let pause = write_pause::after_transaction_write_of(&shared.store, a, &[a, b]);
    txc.send(&["EXEC"]).await;
    reached(&pause);
    reader.send(&["MGET", a, b]).await;
    let during = tokio::time::timeout(Duration::from_millis(300), reader.reply()).await;
    assert!(
        during.is_err(),
        "the WebSocket MGET answered inside the span: {during:?}"
    );
    assert!(
        pause.readers_blocked() > 0,
        "the WebSocket MGET did not wait on the span"
    );
    drop(pause);
    assert_eq!(
        tokio::time::timeout(WITHIN, txc.reply())
            .await
            .expect("EXEC"),
        array(vec![ok(), ok()])
    );
    assert_eq!(
        tokio::time::timeout(WITHIN, reader.reply())
            .await
            .expect("MGET"),
        array(vec![bulk("new"), bulk("new")])
    );
}

// ── Read-your-writes with every observer (G1, design R1) ──────────────────

async fn read_your_writes_with_observers() {
    let path = std::env::temp_dir().join(format!(
        "recached_6og_ryw_{}_{}.aof",
        std::process::id(),
        next_conn_id()
    ));
    let _ = std::fs::remove_file(&path);
    let aof = AofWriter::open(path.clone(), AofSync::Always)
        .await
        .unwrap();
    let shared = Shared::with_aof(false, Some(Arc::new(aof)));
    shared.state.replicas.configure_backlog(1 << 20);
    let _browser = shared
        .state
        .replicas
        .subscribe_pushes(&shared.tx, Routes::Everything)
        .await;
    let mut watcher = Pipe::tcp(&shared);
    assert_eq!(watcher.cmd(&["WATCH", "6og:ryw:unrelated"]).await, ok());
    let mut txc = Pipe::tcp(&shared);
    queue(
        &mut txc,
        &[
            &["SET", "6og:ryw:k", "v", "PX", "100000"],
            &["GET", "6og:ryw:k"],
            &["PEXPIRETIME", "6og:ryw:k"],
            &["SADD", "6og:ryw:s", "m"],
            &["SISMEMBER", "6og:ryw:s", "m"],
            &["APPEND", "6og:ryw:k", "w"],
            &["MGET", "6og:ryw:k", "6og:ryw:none"],
        ],
    )
    .await;
    let Value::Array(Some(items)) = exec(&mut txc).await else {
        panic!("EXEC");
    };
    assert_eq!(items[0], ok());
    assert_eq!(items[1], bulk("v"));
    let Value::Integer(deadline) = items[2] else {
        panic!("{:?}", items[2])
    };
    assert_eq!(items[4], Value::Integer(1));
    assert_eq!(items[6], array(vec![bulk("vw"), nil()]));
    // The AOF has the deadline the primary set, to the millisecond.
    shared.state.aof.as_ref().unwrap().flush().await.unwrap();
    let replayed = KeyValueStore::new();
    replay_aof(&replayed, &path).await.unwrap();
    assert_eq!(
        replayed.execute(Command::PExpireTime("6og:ryw:k".into())),
        Value::Integer(deadline)
    );
    assert_eq!(
        replayed.execute(Command::Get("6og:ryw:k".into())),
        bulk("vw")
    );
    let _ = std::fs::remove_file(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn queued_reads_see_queued_writes_with_every_observer_on_one_worker() {
    read_your_writes_with_observers().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn queued_reads_see_queued_writes_with_every_observer_on_four_workers() {
    read_your_writes_with_observers().await;
}

// ── AOF: one batch, same bytes (G2) ────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_transaction_is_appended_as_one_batch_in_command_order() {
    for sync in [AofSync::Always, AofSync::EverySec, AofSync::No] {
        let path = std::env::temp_dir().join(format!(
            "recached_6og_batch_{}_{}.aof",
            std::process::id(),
            next_conn_id()
        ));
        let _ = std::fs::remove_file(&path);
        let (aof, stats) = AofWriter::open(path.clone(), sync)
            .await
            .unwrap()
            .with_private_stats();
        let shared = Shared::with_aof(false, Some(Arc::new(aof)));
        let mut txc = Pipe::tcp(&shared);
        let batches = stats.batches.load(Ordering::SeqCst);
        queue(
            &mut txc,
            &[
                &["SET", "6og:b:a", "1"],
                &["SET", "6og:b:b", "2"],
                &["SET", "6og:b:b", "3", "NX"],
                &["INCR", "6og:b:a"],
                &["DEL", "6og:b:b"],
            ],
        )
        .await;
        assert_eq!(
            exec(&mut txc).await,
            array(vec![
                ok(),
                ok(),
                nil(),
                Value::Integer(2),
                Value::Integer(1)
            ])
        );
        shared.state.aof.as_ref().unwrap().flush().await.unwrap();
        // One batch for the four effective writes (the flush's own, sealed
        // batch writes nothing more).
        assert_eq!(
            stats.batches.load(Ordering::SeqCst) - batches,
            1,
            "{sync:?}: the transaction's frames were split"
        );
        let bytes = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        // The counter is logged as the value it produced; the DEL last.
        let (first_set, del) = (
            text.find("$3\r\nSET").unwrap(),
            text.find("$3\r\nDEL").unwrap(),
        );
        assert!(
            first_set < del && text.rfind("$3\r\nSET").unwrap() < del,
            "{sync:?}: {text}"
        );
        let replayed = KeyValueStore::new();
        assert_eq!(replay_aof(&replayed, &path).await.unwrap(), 4);
        assert_eq!(replayed.execute(Command::Get("6og:b:a".into())), bulk("2"));
        assert_eq!(replayed.execute(Command::Get("6og:b:b".into())), nil());
        let _ = std::fs::remove_file(&path);
    }
}

// ── WATCH: one coalesced notification per key, the OR of its writes ──────

/// Whether a watcher of `key` (prepared by `prep`) has its `EXEC` aborted by
/// another connection's transaction `tx`.
async fn aborts_a_watcher(shared: &Shared, key: &str, prep: &[&[&str]], tx: &[&[&str]]) -> bool {
    let (mut watcher, mut other) = (Pipe::tcp(shared), Pipe::tcp(shared));
    for command in prep {
        other.cmd(command).await;
    }
    assert_eq!(watcher.cmd(&["WATCH", key]).await, ok());
    queue(&mut other, tx).await;
    let reply = exec(&mut other).await;
    assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
    queue(&mut watcher, &[&["SET", key, "mine"]]).await;
    match exec(&mut watcher).await {
        Value::Array(None) => true,
        Value::Array(Some(_)) => false,
        other => panic!("{other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_watch_is_invalidated_by_any_effective_write_of_the_key() {
    let shared = Shared::new(false);
    type Case<'a> = (&'a str, &'a [&'a [&'a str]], &'a [&'a [&'a str]], bool);
    let cases: &[Case<'_>] = &[
        // A watcher of an absent key: Redis aborts on each of these.
        (
            "set-del",
            &[],
            &[&["SET", "{k}", "a"], &["DEL", "{k}"]],
            true,
        ),
        (
            "set-flushdb",
            &[],
            &[&["SET", "{k}", "a"], &["FLUSHDB"]],
            true,
        ),
        (
            "set-empty-store",
            &[],
            &[&["SET", "{k}", "a"], &["SUNIONSTORE", "{k}", "{k}:none"]],
            true,
        ),
        (
            "set-failed-setnx",
            &[],
            &[&["SET", "{k}", "a"], &["SET", "{k}", "b", "NX"]],
            true,
        ),
        (
            "set-multi-del",
            &[],
            &[
                &["SET", "{k}", "a"],
                &["SET", "{k}:j", "b"],
                &["DEL", "{k}", "{k}:j"],
            ],
            true,
        ),
        (
            "ttl-set-append",
            &[&["SET", "{k}", "x", "EX", "100"]],
            &[&["SET", "{k}", "a", "KEEPTTL"], &["APPEND", "{k}", "b"]],
            true,
        ),
        // Redis signals nothing for an empty STORE onto an absent key.
        (
            "empty-store-absent",
            &[],
            &[&["SUNIONSTORE", "{k}", "{k}:none"]],
            false,
        ),
        // ... and does for one onto a live key, which it deletes.
        (
            "empty-store-live",
            &[&["SADD", "{k}", "m"]],
            &[&["SUNIONSTORE", "{k}", "{k}:none"]],
            true,
        ),
        // A failed SET NX alone changes nothing.
        (
            "failed-setnx-alone",
            &[&["SET", "{k}", "x"]],
            &[&["SET", "{k}", "b", "NX"]],
            false,
        ),
        // A key the transaction does not touch.
        ("untouched", &[], &[&["SET", "{k}:other", "a"]], false),
    ];
    for (label, prep, tx, expected) in cases {
        let key = format!("6og:w:{label}");
        let fill = |commands: &[&[&str]]| -> Vec<Vec<String>> {
            commands
                .iter()
                .map(|c| c.iter().map(|a| a.replace("{k}", &key)).collect())
                .collect()
        };
        let (prep, tx) = (fill(prep), fill(tx));
        fn refs(commands: &[Vec<String>]) -> Vec<Vec<&str>> {
            commands
                .iter()
                .map(|c| c.iter().map(String::as_str).collect())
                .collect()
        }
        let (prep, tx) = (refs(&prep), refs(&tx));
        let prep: Vec<&[&str]> = prep.iter().map(Vec::as_slice).collect();
        let tx: Vec<&[&str]> = tx.iter().map(Vec::as_slice).collect();
        assert_eq!(
            aborts_a_watcher(&shared, &key, &prep, &tx).await,
            *expected,
            "{label}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_watch_is_invalidated_by_a_set_then_an_eviction() {
    let mut shared = Shared::new(false);
    shared.store = Arc::new(KeyValueStore::with_config(
        Some(1),
        None,
        EvictionPolicy::AllKeysLru,
    ));
    // `k` is set, then evicted by the next queued write's key.
    assert!(
        aborts_a_watcher(
            &shared,
            "6og:ev:k",
            &[],
            &[&["SET", "6og:ev:k", "a"], &["SET", "6og:ev:other", "b"]],
        )
        .await
    );
    assert_eq!(shared.store.key_count(), 1);
}

// ── Live queries and delta clients converge (G3) ──────────────────────────

/// What a `CLIENT DELTA ON` live-query client holds: each frame applied to
/// its copy as a client library applies it.
#[derive(Default)]
struct ClientCopy {
    values: HashMap<String, Value>,
    frames_per_key: HashMap<String, usize>,
    deltas_per_key: HashMap<String, usize>,
}

impl ClientCopy {
    async fn drain(&mut self, ws: &mut WsClient) {
        while let Some(raw) = ws.recv_frame(300).await {
            let Ok((Value::Array(Some(items)), _)) = Value::parse(&raw) else {
                continue;
            };
            let text = |v: &Value| match v {
                Value::BulkString(Some(b)) => String::from_utf8_lossy(b).into_owned(),
                other => panic!("{other:?}"),
            };
            let kind = text(&items[0]);
            if kind != "keychange" && kind != "keydelta" {
                continue;
            }
            let key = text(&items[1]);
            *self.frames_per_key.entry(key.clone()).or_default() += 1;
            if kind == "keychange" {
                match &items[2] {
                    Value::BulkString(None) => {
                        self.values.remove(&key);
                    }
                    value => {
                        self.values.insert(key, value.clone());
                    }
                }
                continue;
            }
            *self.deltas_per_key.entry(key.clone()).or_default() += 1;
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
                "rpush" => {
                    let mut now = match entry {
                        Value::Array(Some(items)) => items.clone(),
                        _ => Vec::new(),
                    };
                    now.extend(args.into_iter().map(|arg| Value::BulkString(Some(arg))));
                    *entry = Value::Array(Some(now));
                }
                "lpush" => {
                    let mut now = match entry {
                        Value::Array(Some(items)) => items.clone(),
                        _ => Vec::new(),
                    };
                    for arg in args {
                        now.insert(0, Value::BulkString(Some(arg)));
                    }
                    *entry = Value::Array(Some(now));
                }
                other => panic!("the model does not apply {other}"),
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn delta_and_live_query_clients_converge_on_the_final_state() {
    let srv = spawn_ws_server().await;
    let mut viewer = WsClient::connect(srv.tcp_addr).await;
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(viewer.cmd(&["CLIENT", "DELTA", "ON"]).await, ok());
    viewer.cmd(&["QSUB", "6og:d:*"]).await;
    let mut copy = ClientCopy::default();
    copy.drain(&mut viewer).await;
    assert_eq!(
        writer.cmd(&["RPUSH", "6og:d:s", "a", "b"]).await,
        Value::Integer(2)
    );
    assert_eq!(writer.cmd(&["SET", "6og:d:once", "x"]).await, ok());
    copy.drain(&mut viewer).await;
    copy.frames_per_key.clear();
    copy.deltas_per_key.clear();
    let transactions: &[&[&[&str]]] = &[
        &[&["SET", "6og:d:k", "a"], &["APPEND", "6og:d:k", "b"]],
        &[&["SET", "6og:d:n", "1"], &["INCR", "6og:d:n"]],
        &[
            &["RENAME", "6og:d:s", "6og:d:l"],
            &["LPUSH", "6og:d:l", "x"],
        ],
        &[
            &["MSET", "6og:d:a", "1", "6og:d:b", "2"],
            &["DEL", "6og:d:a"],
        ],
        // A key written once keeps its delta.
        &[&["APPEND", "6og:d:once", "y"], &["SET", "6og:d:other", "z"]],
    ];
    for tx in transactions {
        assert_eq!(writer.cmd(&["MULTI"]).await, ok());
        for command in *tx {
            writer.cmd(command).await;
        }
        let exec = writer.cmd(&["EXEC"]).await;
        assert!(matches!(exec, Value::Array(Some(_))), "{exec:?}");
    }
    copy.drain(&mut viewer).await;
    let server = |key: &str| match srv.store.get_current_with_ttl(key).0 {
        Value::BulkString(None) => None,
        value => Some(value),
    };
    for key in [
        "6og:d:k",
        "6og:d:n",
        "6og:d:s",
        "6og:d:l",
        "6og:d:a",
        "6og:d:b",
        "6og:d:once",
        "6og:d:other",
    ] {
        assert_eq!(
            copy.values.get(key).cloned(),
            server(key),
            "the client's copy of {key} diverged"
        );
    }
    // One notification per key written in a transaction, and no delta for a
    // key written there more than once.
    for key in ["6og:d:k", "6og:d:n", "6og:d:l", "6og:d:a"] {
        assert_eq!(copy.frames_per_key.get(key), Some(&1), "{key}");
        assert_eq!(copy.deltas_per_key.get(key), None, "{key}");
    }
    assert_eq!(copy.deltas_per_key.get("6og:d:once"), Some(&1));
}

// ── Ephemeral state changes in command order (design R6) ──────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_eviction_then_a_claim_in_one_transaction_keeps_the_claim() {
    let mut shared = Shared::new(false);
    shared.store = Arc::new(KeyValueStore::with_config(
        Some(1),
        None,
        EvictionPolicy::AllKeysLru,
    ));
    let (mut owner, mut other) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    assert_eq!(owner.cmd(&["ESET", "6og:e:k", "v1"]).await, ok());
    // `SET other` evicts `k` (forgetting the claim), then `ESET k` claims it
    // again (evicting `other`). The claim must survive the eviction's
    // bookkeeping: it came after it.
    queue(
        &mut owner,
        &[&["SET", "6og:e:other", "x"], &["ESET", "6og:e:k", "v2"]],
    )
    .await;
    assert_eq!(exec(&mut owner).await, array(vec![ok(), ok()]));
    assert_eq!(other.cmd(&["GET", "6og:e:k"]).await, bulk("v2"));
    drop(owner);
    let deadline = Instant::now() + WITHIN;
    while other.cmd(&["EXISTS", "6og:e:k"]).await != Value::Integer(0) {
        assert!(
            Instant::now() < deadline,
            "the owner's claim on 6og:e:k was lost"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

// ── A queued write that panics fails closed (G3, design P2) ───────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_panicking_queued_write_stops_the_transaction_and_fails_closed() {
    let shared = Shared::new(false);
    let (mut txc, mut other) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    queue(
        &mut txc,
        &[
            &["SET", "6og:p:a", "1"],
            &["SET", "6og:p:b", "2"],
            &["SET", "6og:p:c", "3"],
        ],
    )
    .await;
    let armed = transaction_fault::panic_during_mutation_of("6og:p:b");
    let Value::Array(Some(items)) = exec(&mut txc).await else {
        panic!("EXEC");
    };
    drop(armed);
    assert_eq!(items[0], ok());
    assert!(matches!(&items[1], Value::Error(e) if e.contains("internal error")));
    assert!(matches!(&items[2], Value::Error(e) if e.contains("not executed")));
    assert_eq!(other.cmd(&["GET", "6og:p:a"]).await, bulk("1"));
    assert_eq!(other.cmd(&["GET", "6og:p:c"]).await, nil());
    // Nothing is left held: reads and the connection still work, and writes
    // are refused until the state is snapshotted.
    assert!(!shared.state.persistence_is_healthy());
    assert!(matches!(
        other.cmd(&["SET", "6og:p:a", "x"]).await,
        Value::Error(e) if e.starts_with("MISCONF")
    ));
    assert_eq!(txc.cmd(&["PING"]).await, Value::SimpleString("PONG".into()));
}

// ── Notification memory is bounded (design N1) ────────────────────────────

/// Many queued writes to one large value, with a live query that takes full
/// values: the transaction copies the value once (its one coalesced
/// notification), not once per write. Counted with the test binary's
/// allocator on the thread that runs the transaction, against the same
/// transaction with no subscriber (which still pays for the value's growth).
#[test]
fn many_appends_to_a_large_value_copy_it_once() {
    const VALUE: usize = 1 << 20;
    const APPENDS: usize = 1_000;
    let bytes = |with_viewer: bool| -> u64 {
        let shared = Shared::new(false);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let viewer = with_viewer.then(|| {
            runtime.block_on(async {
                let mut viewer = WsPipe::connect(&shared, true).await;
                viewer.cmd(&["QSUB", "6og:mem:*"]).await;
                viewer
            })
        });
        shared.store.execute(Command::Set(
            "6og:mem:k".into(),
            vec![b'v'; VALUE],
            SetOptions::default(),
        ));
        let queue: Vec<Command> = (0..APPENDS)
            .map(|_| Command::Append("6og:mem:k".into(), b"x".to_vec()))
            .collect();
        let (run, bytes) = super::request_path::bytes_allocated_in(|| {
            runtime.block_on(run_transaction_queue(
                queue,
                core_engine::store::clock_ms(),
                &shared.tx,
                next_conn_id(),
                &shared.state,
                &shared.watch_registry,
                &shared.store,
                &shared.pubsub,
            ))
        });
        assert_eq!(run.replies.len(), APPENDS);
        drop(viewer);
        bytes
    };
    let (alone, watched) = (bytes(false), bytes(true));
    // One copy of the value (plus small per-write bookkeeping). A copy per
    // write would be APPENDS × VALUE, cut short only by the subscriber's 8 MiB
    // notification budget overflowing after about eight copies.
    assert!(
        watched.saturating_sub(alone) < 2 * VALUE as u64,
        "{APPENDS} appends to a {VALUE}-byte value allocated {watched} bytes with a \
         live query, {alone} without"
    );
}
