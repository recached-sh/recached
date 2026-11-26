//! 6O-H2/H5: the primary logs every expiration it acts on (Expiry Option E1
//! and E2), before the frames that depend on it, on every stream.
//!
//! - A write that finds an expired entry logs `DEL <key>` before its own
//!   frame: in the AOF in the same batch, on replication as the frame
//!   before, to sync clients as the push before, to watchers as a removal.
//! - A write that fails or changes nothing still logs what it removed.
//! - A DEDUP write's `DEL` stays outside its envelope; a duplicate logs
//!   nothing.
//! - Inside `EXEC`, each queued command's `DEL` precedes its frame, in the
//!   transaction's one batch, exactly as running the commands one by one.
//! - The sweeper logs what it removes as bounded `DEL` batches; a replica's
//!   sweeper stays silent.
//! - Eviction keeps its own `DEL`s, after the command; nothing is logged
//!   twice.

use super::mset_atomicity::{Pipe, bulk};
use super::request_path::Shared;
use crate::*;
use core_engine::resp::Value;
use core_engine::store::{EvictionPolicy, KeyValueStore};
use std::time::Duration;

fn ok() -> Value {
    Value::SimpleString("OK".into())
}

/// The records of an AOF or replication byte stream, each as its words.
fn records(bytes: &[u8]) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let (value, used) = Value::parse(&bytes[at..]).expect("whole records");
        at += used;
        let items = match value {
            Value::Push(items) | Value::Array(Some(items)) => items,
            other => panic!("not a record: {other:?}"),
        };
        out.push(
            items
                .into_iter()
                .map(|item| match item {
                    Value::BulkString(Some(b)) => String::from_utf8_lossy(&b).into_owned(),
                    other => format!("{other:?}"),
                })
                .collect(),
        );
    }
    out
}

fn rec(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

struct Logged {
    shared: Shared,
    path: PathBuf,
    stats: &'static crate::aof_stats::AofStats,
    replica: tokio::sync::mpsc::Receiver<QueuedReplFrame>,
}

impl Logged {
    async fn new(sync: AofSync, fanout: AofFanout, store: KeyValueStore) -> Logged {
        let path = std::env::temp_dir().join(format!(
            "recached_6oh_expirylog_{}_{}.aof",
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
        shared.store = Arc::new(store);
        shared.state.replicas.configure_backlog(1 << 20);
        let replica = shared
            .state
            .replicas
            .attach_test_replica(4096, 1 << 26)
            .await;
        Logged {
            shared,
            path,
            stats,
            replica,
        }
    }

    async fn aof(&self) -> Vec<Vec<String>> {
        self.shared
            .state
            .aof
            .as_ref()
            .unwrap()
            .flush()
            .await
            .unwrap();
        records(&std::fs::read(&self.path).unwrap())
    }

    /// Replication frames received so far, each one record.
    fn replicated(&mut self) -> Vec<Vec<String>> {
        let mut out = Vec::new();
        while let Ok(frame) = self.replica.try_recv() {
            let mut one = records(frame.bytes());
            assert_eq!(one.len(), 1, "one record per replication frame");
            out.push(one.remove(0));
        }
        out
    }

    fn batches(&self) -> u64 {
        self.stats.batches.load(Ordering::SeqCst)
    }
}

impl Drop for Logged {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Make `key` expired now: a deadline long past, entry still in the map.
async fn expire(c: &mut Pipe, key: &str) {
    assert_eq!(c.cmd(&["PEXPIREAT", key, "1"]).await, Value::Integer(1));
}

const MODES: [(AofSync, AofFanout); 4] = [
    (AofSync::Always, AofFanout::Inline),
    (AofSync::Always, AofFanout::Coalesced),
    (AofSync::No, AofFanout::Inline),
    (AofSync::No, AofFanout::Coalesced),
];

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_observed_expiry_is_logged_before_the_write_that_found_it() {
    for (sync, fanout) in MODES {
        let label = format!("{sync:?} {fanout:?}");
        let mut log = Logged::new(sync, fanout, KeyValueStore::new()).await;
        let mut c = Pipe::tcp(&log.shared);
        assert_eq!(c.cmd(&["SET", "oh:c", "5"]).await, ok());
        expire(&mut c, "oh:c").await;
        let before = log.batches();
        assert_eq!(c.cmd(&["INCR", "oh:c"]).await, Value::Integer(1), "{label}");
        assert_eq!(
            log.batches() - before,
            1,
            "{label}: DEL and frame in one batch"
        );
        assert_eq!(
            log.aof().await,
            vec![
                rec(&["SET", "oh:c", "5"]),
                rec(&["PEXPIREAT", "oh:c", "1"]),
                rec(&["DEL", "oh:c"]),
                rec(&["SET", "oh:c", "1", "KEEPTTL"]),
            ],
            "{label}: AOF"
        );
        assert_eq!(log.replicated(), log.aof().await, "{label}: replication");
        // Live now: the next write logs no DEL.
        assert_eq!(c.cmd(&["INCR", "oh:c"]).await, Value::Integer(2));
        assert_eq!(
            log.aof().await.last(),
            Some(&rec(&["SET", "oh:c", "2", "KEEPTTL"])),
            "{label}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_write_that_fails_or_changes_nothing_still_logs_what_it_removed() {
    let mut log = Logged::new(AofSync::No, AofFanout::Inline, KeyValueStore::new()).await;
    let mut c = Pipe::tcp(&log.shared);
    for (setup, write, reply_is_error) in [
        (
            vec!["SET", "fx:src", "v"],
            vec!["RENAME", "fx:src", "fx:dst"],
            true,
        ),
        (
            vec!["RPUSH", "fx:list", "a"],
            vec!["LSET", "fx:list", "0", "b"],
            true,
        ),
        (vec!["SET", "fx:p", "v"], vec!["PERSIST", "fx:p"], false),
        (
            vec!["SET", "fx:e", "v"],
            vec!["EXPIRE", "fx:e", "100"],
            false,
        ),
        (
            vec!["HSET", "fx:h", "f", "v"],
            vec!["HDEL", "fx:h", "f"],
            false,
        ),
        (
            vec!["SET", "fx:x", "v"],
            vec!["SET", "fx:x", "w", "XX"],
            false,
        ),
    ] {
        let key = setup[1];
        assert!(!matches!(c.cmd(&setup).await, Value::Error(_)));
        expire(&mut c, key).await;
        let reply = c.cmd(&write).await;
        assert_eq!(
            matches!(reply, Value::Error(_)),
            reply_is_error,
            "{write:?}: {reply:?}"
        );
        assert_eq!(
            log.aof().await.last(),
            Some(&rec(&["DEL", key])),
            "{write:?}: the removal is logged though the write logged nothing"
        );
    }
    let aof = log.aof().await;
    assert_eq!(log.replicated(), aof, "replication follows the AOF");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_dedup_writes_expiry_del_stays_outside_its_envelope() {
    let mut log = Logged::new(AofSync::Always, AofFanout::Inline, KeyValueStore::new()).await;
    let mut c = Pipe::tcp(&log.shared);
    c.cmd(&["SET", "dd:c", "5"]).await;
    expire(&mut c, "dd:c").await;
    assert_eq!(
        c.cmd(&["DEDUP", "cl", "1", "INCR", "dd:c"]).await,
        Value::Integer(1)
    );
    let applied = log.aof().await;
    assert_eq!(
        applied[applied.len() - 2..],
        [
            rec(&["DEL", "dd:c"]),
            rec(&["DEDUP", "cl", "1", "SET", "dd:c", "1", "KEEPTTL"]),
        ]
    );
    // The retry is answered from the mark and logs nothing at all.
    c.cmd(&["DEDUP", "cl", "1", "INCR", "dd:c"]).await;
    assert_eq!(log.aof().await, applied, "a duplicate logged something");
    assert_eq!(c.cmd(&["GET", "dd:c"]).await, bulk("1"));

    // Refused for good: the DEL, then the refusal record.
    c.cmd(&["RPUSH", "dd:l", "a"]).await;
    expire(&mut c, "dd:l").await;
    assert!(matches!(
        c.cmd(&["DEDUP", "cl", "2", "LSET", "dd:l", "0", "b"]).await,
        Value::Error(_)
    ));
    let aof = log.aof().await;
    assert_eq!(aof[aof.len() - 2], rec(&["DEL", "dd:l"]));
    assert_eq!(
        aof[aof.len() - 1][..3],
        rec(&["RECACHED-DEDUP-REFUSED", "cl", "2"])
    );

    // Applied but a no-op: the DEL, then the mark.
    c.cmd(&["HSET", "dd:h", "f", "v"]).await;
    expire(&mut c, "dd:h").await;
    assert_eq!(
        c.cmd(&["DEDUP", "cl", "3", "HDEL", "dd:h", "f"]).await,
        Value::Integer(0)
    );
    let aof = log.aof().await;
    assert_eq!(
        aof[aof.len() - 2..],
        [rec(&["DEL", "dd:h"]), rec(&["RECACHED-DEDUP", "cl", "3"])]
    );
    let _ = log.replicated();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_logs_each_commands_expiry_before_its_frame_as_one_batch() {
    let queue: &[&[&str]] = &[
        &["INCR", "tx:c"],
        &["EXPIREAT", "tx:c", "4000000000"],
        &["APPEND", "tx:s", "tail"],
        &["SUNIONSTORE", "tx:out", "tx:src"],
        &["RENAME", "tx:gone", "tx:to"],
        &["INCR", "tx:c"],
    ];
    let mut streams = Vec::new();
    for in_tx in [false, true] {
        for (sync, fanout) in MODES {
            let mut log = Logged::new(sync, fanout, KeyValueStore::new()).await;
            let mut c = Pipe::tcp(&log.shared);
            for (setup, key) in [
                (vec!["SET", "tx:c", "41"], "tx:c"),
                (vec!["SET", "tx:s", "old"], "tx:s"),
                (vec!["SADD", "tx:src", "m"], "tx:src"),
                (vec!["SET", "tx:gone", "v"], "tx:gone"),
            ] {
                c.cmd(&setup).await;
                expire(&mut c, key).await;
            }
            let start = log.aof().await.len();
            let before = log.batches();
            if in_tx {
                assert_eq!(c.cmd(&["MULTI"]).await, ok());
                for cmd in queue {
                    c.cmd(cmd).await;
                }
                let reply = c.cmd(&["EXEC"]).await;
                assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
                assert_eq!(log.batches() - before, 1, "one batch");
            } else {
                for cmd in queue {
                    c.cmd(cmd).await;
                }
            }
            let aof = log.aof().await[start..].to_vec();
            let repl = log.replicated();
            assert_eq!(repl[repl.len() - aof.len()..], aof[..], "replication = AOF");
            streams.push(aof);
        }
    }
    let want = vec![
        rec(&["DEL", "tx:c"]),
        rec(&["SET", "tx:c", "1", "KEEPTTL"]),
        rec(&["PEXPIREAT", "tx:c", "4000000000000"]),
        rec(&["DEL", "tx:s"]),
        rec(&["APPEND", "tx:s", "tail"]),
        rec(&["DEL", "tx:src"]),
        rec(&["SUNIONSTORE", "tx:out", "tx:src"]),
        rec(&["DEL", "tx:gone"]),
        rec(&["SET", "tx:c", "2", "KEEPTTL"]),
    ];
    for (i, stream) in streams.iter().enumerate() {
        assert_eq!(stream, &want, "run {i}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_sweeper_logs_its_removals_in_bounded_batches() {
    let mut log = Logged::new(AofSync::No, AofFanout::Inline, KeyValueStore::new()).await;
    let mut c = Pipe::tcp(&log.shared);
    // Long keys, so one chunk needs several frames.
    let long = "k".repeat(1_000);
    let mut keys = Vec::new();
    for i in 0..300 {
        let key = format!("sw:{i}:{long}");
        c.cmd(&["SET", &key, "v", "PX", "1"]).await;
        keys.push(key);
    }
    c.cmd(&["SET", "sw:keep", "v"]).await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    let start = log.aof().await.len();
    let mut removed = 0;
    while removed < keys.len() {
        let n = sweep_expired_tick(
            &log.shared.store,
            &log.shared.state,
            &log.shared.watch_registry,
            &log.shared.tx,
        )
        .await;
        assert!(n > 0, "the sweeper stalled at {removed}");
        removed += n;
    }
    let dels = log.aof().await[start..].to_vec();
    let mut logged: Vec<String> = Vec::new();
    for del in &dels {
        assert_eq!(del[0], "DEL");
        let key_bytes: usize = del[1..].iter().map(String::len).sum();
        assert!(
            key_bytes <= EXPIRY_DEL_FRAME_KEY_BYTES,
            "{key_bytes} key bytes in one frame"
        );
        logged.extend(del[1..].iter().cloned());
    }
    assert!(dels.len() > 1, "several frames for 300 KB of keys");
    logged.sort();
    keys.sort();
    assert_eq!(logged, keys, "each removed key logged exactly once");
    let repl = log.replicated();
    assert_eq!(
        repl[repl.len() - dels.len()..],
        dels[..],
        "replicated in order"
    );
    assert_eq!(c.cmd(&["GET", "sw:keep"]).await, bulk("v"));
}

#[test]
fn sweeper_frames_split_by_key_bytes_and_keep_order() {
    let keys: Vec<String> = (0..10)
        .map(|i| format!("{i}{}", "x".repeat(20_000)))
        .collect();
    let frames = expiry_del_frames(&keys);
    let words: Vec<Vec<String>> = frames.iter().flat_map(|f| records(f)).collect();
    assert_eq!(words.len(), 4, "3 keys of 20 KB per 64 KiB frame");
    let flat: Vec<String> = words.iter().flat_map(|w| w[1..].to_vec()).collect();
    assert_eq!(flat, keys);
    // A key longer than the bound still goes, alone.
    let huge = vec!["h".repeat(100_000), "a".into()];
    let words: Vec<Vec<String>> = expiry_del_frames(&huge)
        .iter()
        .flat_map(|f| records(f))
        .collect();
    assert_eq!(words, vec![rec(&["DEL", &huge[0]]), rec(&["DEL", "a"])]);
    assert!(expiry_del_frames(&[]).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_replicas_sweeper_stays_silent() {
    let log = Logged::new(AofSync::No, AofFanout::Inline, KeyValueStore::new()).await;
    let mut c = Pipe::tcp(&log.shared);
    c.cmd(&["SET", "rs:k", "v", "PX", "1"]).await;
    tokio::time::sleep(Duration::from_millis(5)).await;
    let start = log.aof().await.len();
    log.shared.state.is_replica.store(true, Ordering::SeqCst);
    let n = sweep_expired_tick(
        &log.shared.store,
        &log.shared.state,
        &log.shared.watch_registry,
        &log.shared.tx,
    )
    .await;
    assert_eq!(n, 1);
    assert_eq!(
        log.aof().await.len(),
        start,
        "a replica logged its own sweep"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn eviction_keeps_its_own_dels_and_nothing_is_logged_twice() {
    // Key cap 2: `a` expired and `b` live. `SET c` needs a slot: the store
    // reclaims expired `a` (an eviction-path removal, logged after the
    // command). `SET a` on expired `a` instead: purged first, logged before.
    let store = KeyValueStore::with_config(Some(2), None, EvictionPolicy::AllKeysLru);
    let log = Logged::new(AofSync::No, AofFanout::Inline, store).await;
    let mut c = Pipe::tcp(&log.shared);
    c.cmd(&["SET", "ev:a", "1"]).await;
    c.cmd(&["SET", "ev:b", "1"]).await;
    expire(&mut c, "ev:a").await;
    let start = log.aof().await.len();
    assert_eq!(c.cmd(&["SET", "ev:c", "1"]).await, ok());
    assert_eq!(
        log.aof().await[start..],
        [rec(&["SET", "ev:c", "1"]), rec(&["DEL", "ev:a"])],
        "reclaimed by the capacity path: its DEL after the command, once"
    );
    expire(&mut c, "ev:b").await;
    let start = log.aof().await.len();
    assert_eq!(c.cmd(&["SET", "ev:b", "2"]).await, ok());
    assert_eq!(
        log.aof().await[start..],
        [rec(&["DEL", "ev:b"]), rec(&["SET", "ev:b", "2"])],
        "observed by the write: its DEL before the command, once"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_clients_get_the_expiry_del_before_the_write() {
    for in_tx in [false, true] {
        let shared = Shared::new(false);
        let mut sub = shared
            .state
            .replicas
            .subscribe_pushes(&shared.tx, Routes::Everything)
            .await;
        let mut c = Pipe::tcp(&shared);
        c.cmd(&["SET", "ws:c", "5"]).await;
        expire(&mut c, "ws:c").await;
        if in_tx {
            c.cmd(&["MULTI"]).await;
            c.cmd(&["INCR", "ws:c"]).await;
            c.cmd(&["EXEC"]).await;
        } else {
            c.cmd(&["INCR", "ws:c"]).await;
        }
        let mut got = Vec::new();
        while let Ok(Ok(push)) = tokio::time::timeout(Duration::from_millis(200), sub.recv()).await
        {
            got.extend(records(&push.resp));
        }
        assert_eq!(
            got,
            vec![
                rec(&["SET", "ws:c", "5"]),
                rec(&["PEXPIREAT", "ws:c", "1"]),
                rec(&["DEL", "ws:c"]),
                rec(&["SET", "ws:c", "1", "KEEPTTL"]),
            ],
            "in_tx={in_tx}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_sweepers_pushes_respect_tenant_scopes() {
    let shared = Shared::new(false);
    let grants_a = [crate::sync_scopes::Grant::rw("tenant:a:*")];
    let mut a = shared
        .state
        .replicas
        .subscribe_pushes(&shared.tx, Routes::Grants(&grants_a))
        .await;
    let mut c = Pipe::tcp(&shared);
    for key in ["tenant:a:1", "tenant:b:1", "tenant:a:2"] {
        c.cmd(&["SET", key, "v", "PX", "1"]).await;
    }
    tokio::time::sleep(Duration::from_millis(5)).await;
    while let Ok(Ok(_)) = tokio::time::timeout(Duration::from_millis(100), a.recv()).await {}
    sweep_expired_tick(
        &shared.store,
        &shared.state,
        &shared.watch_registry,
        &shared.tx,
    )
    .await;
    let mut got = Vec::new();
    while let Ok(Ok(push)) = tokio::time::timeout(Duration::from_millis(200), a.recv()).await {
        got.extend(records(&push.resp));
    }
    got.sort();
    assert_eq!(
        got,
        vec![rec(&["DEL", "tenant:a:1"]), rec(&["DEL", "tenant:a:2"])],
        "tenant a sees its own expiries, and only those"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_write_that_removes_a_watched_expired_key_is_a_removal_to_its_watcher() {
    // As the sweeper's removal is (watch_expiry.rs): live at WATCH, then
    // removed, aborts; already expired at WATCH, then removed, commits.
    let shared = Shared::new(false);
    let mut other = Pipe::tcp(&shared);
    for (key, live_at_watch) in [("wr:live", true), ("wr:stale", false)] {
        let mut w = Pipe::tcp(&shared);
        w.cmd(&["SET", key, "v"]).await;
        if !live_at_watch {
            expire(&mut w, key).await;
        }
        assert_eq!(w.cmd(&["WATCH", key]).await, ok());
        if live_at_watch {
            expire(&mut other, key).await;
        }
        // Another connection's write only finds it expired, and fails.
        assert!(matches!(
            other.cmd(&["LSET", key, "0", "x"]).await,
            Value::Error(_)
        ));
        assert_eq!(shared.store.key_count(), 0, "{key}: removed by the write");
        w.cmd(&["MULTI"]).await;
        w.cmd(&["SET", "wr:out", "1"]).await;
        let reply = w.cmd(&["EXEC"]).await;
        assert_eq!(
            reply == Value::Array(None),
            live_at_watch,
            "{key}: EXEC {reply:?}"
        );
        other.cmd(&["DEL", "wr:out"]).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_plain_cache_removes_nothing_early() {
    // Nothing observes writes: no DEL could be read, so the store is not
    // asked to remove anything early (and pays nothing for it).
    let shared = Shared::new(false);
    let mut c = Pipe::tcp(&shared);
    c.cmd(&["SET", "pc:k", "v"]).await;
    expire(&mut c, "pc:k").await;
    assert_eq!(c.cmd(&["PERSIST", "pc:k"]).await, Value::Integer(0));
    assert_eq!(
        shared.store.key_count(),
        1,
        "the expired entry is still there"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_del_logs_its_expired_keys_once_and_a_refused_one_still_logs_them() {
    // Its own frame names every key, so no second DEL; refused on a store
    // over its key cap, it has no frame, and the removal is logged alone.
    let store = KeyValueStore::with_config(Some(2), None, EvictionPolicy::NoEviction);
    store.set_follower(true);
    for key in ["dk:a", "dk:b", "dk:c", "dk:d"] {
        store.execute(Command::Set(key.into(), b"v".to_vec(), Default::default()));
    }
    let log = Logged::new(AofSync::No, AofFanout::Inline, store).await;
    let mut c = Pipe::tcp(&log.shared);
    // Only `dk:a` expired: admission finds no other expired key to reclaim.
    expire(&mut c, "dk:a").await;
    let start = log.aof().await.len();
    // Promoted, two keys over the cap: DEL is refused.
    log.shared.store.set_follower(false);
    assert!(matches!(c.cmd(&["DEL", "dk:a"]).await, Value::Error(_)));
    assert_eq!(log.aof().await[start..], [rec(&["DEL", "dk:a"])]);
    // Not enforcing the cap: DEL runs, and its frame is the only record.
    log.shared.store.set_follower(true);
    expire(&mut c, "dk:b").await;
    let start = log.aof().await.len();
    assert_eq!(c.cmd(&["DEL", "dk:b", "dk:c"]).await, Value::Integer(1));
    assert_eq!(log.aof().await[start..], [rec(&["DEL", "dk:b", "dk:c"])]);
}

// ── Review B ─────────────────────────────────────────────────────────────

/// A WebSocket sync client, as a browser SDK connects.
struct Ws {
    ws: tokio_tungstenite::WebSocketStream<tokio::io::DuplexStream>,
}

impl Ws {
    async fn connect(shared: &Shared) -> Ws {
        let (client, server) = tokio::io::duplex(1 << 20);
        tokio::spawn(shared.serve_ws(server));
        let (ws, _) = tokio_tungstenite::client_async("ws://localhost/", client)
            .await
            .expect("handshake");
        Ws { ws }
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

    /// Every frame until `ms` of silence: pushes as their records, replies
    /// as `["reply", …]`.
    async fn drain(&mut self, ms: u64) -> Vec<Vec<String>> {
        use futures_util::StreamExt;
        let mut out = Vec::new();
        loop {
            let next = tokio::time::timeout(Duration::from_millis(ms), self.ws.next()).await;
            let raw = match next {
                Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t)))) => {
                    t.as_bytes().to_vec()
                }
                Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Binary(b)))) => b.to_vec(),
                Ok(Some(Ok(_))) => continue,
                _ => return out,
            };
            match Value::parse(&raw) {
                Ok((Value::Push(_), _)) => out.extend(records(&raw)),
                Ok((other, _)) => out.push(vec!["reply".into(), format!("{other:?}")]),
                Err(_) => {}
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_writer_gets_no_del_of_its_own_write_and_everyone_else_both() {
    // Review B, B-1: the writer's copy applied INCR itself, and never gets
    // its own `SET c 1 KEEPTTL`; a DEL alone would leave it without `c`.
    for in_tx in [false, true] {
        let shared = Shared::new(false);
        let mut writer = Ws::connect(&shared).await;
        let mut other = Ws::connect(&shared).await;
        let mut tcp = Pipe::tcp(&shared);
        tcp.cmd(&["SET", "wb:c", "5"]).await;
        expire(&mut tcp, "wb:c").await;
        let _ = writer.drain(300).await;
        let _ = other.drain(300).await;
        if in_tx {
            writer.send(&["MULTI"]).await;
            writer.send(&["INCR", "wb:c"]).await;
            writer.send(&["EXEC"]).await;
        } else {
            writer.send(&["INCR", "wb:c"]).await;
        }
        let mine = writer.drain(400).await;
        assert!(
            !mine.iter().any(|r| r[0] == "DEL"),
            "in_tx={in_tx}: the writer was pushed a DEL of its own write: {mine:?}"
        );
        assert_eq!(
            other.drain(400).await,
            vec![rec(&["DEL", "wb:c"]), rec(&["SET", "wb:c", "1", "KEEPTTL"])],
            "in_tx={in_tx}: another client gets both, in order"
        );
        assert_eq!(tcp.cmd(&["GET", "wb:c"]).await, bulk("1"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_writes_del_reaches_every_client_the_writer_too() {
    // No frame follows it, so nobody's copy is left without a key the
    // server holds: every client, the writer included, drops the expired one.
    let shared = Shared::new(false);
    let mut writer = Ws::connect(&shared).await;
    let mut other = Ws::connect(&shared).await;
    let mut tcp = Pipe::tcp(&shared);
    tcp.cmd(&["RPUSH", "wf:l", "a"]).await;
    expire(&mut tcp, "wf:l").await;
    let _ = writer.drain(300).await;
    let _ = other.drain(300).await;
    writer.send(&["LSET", "wf:l", "0", "x"]).await;
    let mine = writer.drain(400).await;
    assert!(mine.contains(&rec(&["DEL", "wf:l"])), "{mine:?}");
    assert_eq!(other.drain(400).await, vec![rec(&["DEL", "wf:l"])]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_deferred_aof_wait_keeps_the_del_before_the_frame() {
    // Review B, B-2 (M1): with no replication, a TCP client's write defers
    // its AOF wait to its reply (`append_client_writes`).
    for fanout in [AofFanout::Inline, AofFanout::Coalesced] {
        let path = std::env::temp_dir().join(format!(
            "recached_6oh_deferred_{}_{}.aof",
            std::process::id(),
            next_conn_id()
        ));
        let aof = Arc::new(
            AofWriter::open(path.clone(), AofSync::Always)
                .await
                .unwrap(),
        );
        aof.set_fanout(fanout);
        let shared = Shared::with_aof(false, Some(Arc::clone(&aof)));
        assert!(!shared.state.replicas.is_enabled());
        let mut c = Pipe::tcp(&shared);
        c.cmd(&["SET", "df:c", "5"]).await;
        expire(&mut c, "df:c").await;
        assert_eq!(c.cmd(&["INCR", "df:c"]).await, Value::Integer(1));
        aof.flush().await.unwrap();
        let recs = records(&std::fs::read(&path).unwrap());
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            recs[recs.len() - 2..],
            [rec(&["DEL", "df:c"]), rec(&["SET", "df:c", "1", "KEEPTTL"])],
            "{fanout:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_observed_expiry_forgets_the_ephemeral_claim() {
    // Review B, B-2 (M2): ESET's key expires and another connection writes
    // it afresh. The owner's disconnect must not delete the new key.
    let shared = Shared::new(false);
    let mut owner = Pipe::tcp(&shared);
    let mut other = Pipe::tcp(&shared);
    let _watch = {
        // Something observes writes, so the write runs in a scope.
        let mut w = Pipe::tcp(&shared);
        assert_eq!(w.cmd(&["WATCH", "ec:unrelated"]).await, ok());
        w
    };
    assert_eq!(owner.cmd(&["ESET", "ec:k", "v"]).await, ok());
    expire(&mut other, "ec:k").await;
    other.cmd(&["RPUSH", "ec:k", "a"]).await;
    drop(owner);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        other.cmd(&["LRANGE", "ec:k", "0", "-1"]).await,
        Value::Array(Some(vec![bulk("a")])),
        "the new list was released with the expired claim"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_key_set_then_found_expired_in_exec_invalidates_a_watch_of_it() {
    // Review B, B-2 (M3): `k` absent at WATCH. In another connection's
    // EXEC, `SET k v PXAT 1` creates it (already expired) and `LSET k` then
    // removes it: a set happened, so the watcher must abort, as in Redis.
    let shared = Shared::new(false);
    let mut w = Pipe::tcp(&shared);
    let mut t = Pipe::tcp(&shared);
    assert_eq!(w.cmd(&["WATCH", "sx:k"]).await, ok());
    t.cmd(&["MULTI"]).await;
    t.cmd(&["SET", "sx:k", "v", "PXAT", "1"]).await;
    t.cmd(&["LSET", "sx:k", "0", "x"]).await;
    t.cmd(&["EXEC"]).await;
    w.cmd(&["MULTI"]).await;
    w.cmd(&["SET", "sx:out", "1"]).await;
    assert_eq!(w.cmd(&["EXEC"]).await, Value::Array(None));
}

#[test]
fn sweeper_frames_split_exactly_at_the_bound() {
    // Review B, N-1: exactly 64 KiB of keys is one frame, a byte more two.
    let half = EXPIRY_DEL_FRAME_KEY_BYTES / 2;
    let exact = vec!["a".repeat(half), "b".repeat(half)];
    assert_eq!(expiry_del_frames(&exact).len(), 1);
    let over = vec!["a".repeat(half), "b".repeat(half + 1)];
    assert_eq!(expiry_del_frames(&over).len(), 2);
}
