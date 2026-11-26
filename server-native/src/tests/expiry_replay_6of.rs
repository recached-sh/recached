//! 6O-F: expiring keys through AOF replay and lagging replicas.
//!
//! Frames are applied somewhere else, and later, than where they were made:
//! at a restart, or by a replica after a delay. The AOF replay and the
//! replica evaluate expiry at that later instant, on their own clock. Neither
//! the AOF nor the replication stream records expirations. So a frame that
//! refers to a key's existing state can find that key expired where the
//! original command found it live.
//!
//! **The 6O-F reproductions (`reproduce_*`).** An expiring counter came
//! back **permanent**: it propagates as `SET <n> KEEPTTL`, which keeps
//! nothing on an expired entry. The same held for partial updates
//! (`APPEND`, `HSET`, `RPUSH`), and `LSET` on a volatile list made the replay
//! refuse the file. Fixed by 6O-H (Expiry Option E: the primary logs every
//! expiration it acts on, and the AOF replay applies the expiry-log format
//! without evaluating expiry), so the AOF reproductions now always run. The
//! replica one still runs only with `RECACHED_REPRODUCE_6OF_COUNTER=1`
//! until replicas apply the log the same way.
//!
//! **Regression guards (always run).** A fix that only changes the counter
//! frame was tried (`SET <n> PXAT <deadline>`, branch
//! `experiment/v0.6o-f-counter-pxat-rejected`) and rejected by the 6O-F
//! review. It turns cases that replay correctly today into lost keys, and a
//! `RENAME` after the counter into a replay that refuses the file. These
//! cases pass on `dev` and pin that behaviour for any future fix.
//!
//! **Deadline overflow (fixed here).** A deadline past `i64::MAX` is refused
//! as Redis 7.2.5 refuses it, so that no frame carries one. Records that
//! older servers already wrote are clamped on replay.
//!
//! Time is controlled with the store's clock override
//! (`core_engine::store::at_instant`), not with sleeps. Frames are written by
//! the real TCP path at the real clock, with deadlines an hour ahead. The
//! replay or the replica then runs on a thread whose store clock is fixed
//! before or after them.

use super::mset_atomicity::{Pipe, bulk};
use super::replication_dedup::{follow, node, serve_replicas, wait_for};
use super::request_path::Shared;
use super::{WsClient, next_conn_id};
use crate::*;
use core_engine::cmd::Command;
use core_engine::resp::Value;
use core_engine::store::{KeyValueStore, at_instant};
use std::path::{Path, PathBuf};

const HOUR_MS: u64 = 3_600_000;

fn ok() -> Value {
    Value::SimpleString("OK".into())
}

fn int(n: i64) -> Value {
    Value::Integer(n)
}

fn nil() -> Value {
    Value::BulkString(None)
}

fn reproducing() -> bool {
    let on = std::env::var_os("RECACHED_REPRODUCE_6OF_COUNTER").is_some();
    if !on {
        eprintln!("6O-F reproduction skipped: set RECACHED_REPRODUCE_6OF_COUNTER=1 to run it");
    }
    on
}

/// A primary with an AOF under `policy`, written by the real TCP path.
async fn aof_primary(policy: AofSync, name: &str) -> (Shared, Arc<AofWriter>, Scratch) {
    let path = scratch(name, "append.aof");
    let aof = Arc::new(AofWriter::open(path.to_path_buf(), policy).await.unwrap());
    // As a primary's startup begins a new AOF (6O-H): in the expiry-log
    // format.
    aof.set_file_mode(AofFileMode::Empty);
    aof.begin_expiry_log().await.unwrap();
    (Shared::with_aof(false, Some(Arc::clone(&aof))), aof, path)
}

/// A file in a fresh directory, removed (with the directory) when dropped.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Some(dir) = self.0.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

fn scratch(name: &str, file: &str) -> Scratch {
    let dir = std::env::temp_dir().join(format!(
        "recached_6of_{name}_{}_{}",
        std::process::id(),
        next_conn_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir.join(file))
}

/// Replay `path` into a fresh store on a thread whose store clock reads
/// `now` for the whole replay.
fn try_replay_at(path: &Path, now: u64) -> std::io::Result<KeyValueStore> {
    let path = path.to_path_buf();
    std::thread::spawn(move || {
        let store = KeyValueStore::new();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        at_instant(now, || {
            rt.block_on(crate::persistence::replay_aof(&store, &path))
        })?;
        Ok(store)
    })
    .join()
    .expect("replay thread")
}

fn replay_at(path: &Path, now: u64) -> KeyValueStore {
    try_replay_at(path, now).expect("the AOF replays")
}

/// `GET` and `PEXPIRETIME` of `key`, at `now`.
fn state_at(store: &KeyValueStore, key: &str, now: u64) -> (Value, Value) {
    at_instant(now, || {
        (
            store.execute(Command::Get(key.into())),
            store.execute(Command::PExpireTime(key.into())),
        )
    })
}

/// The fixed-window counter: created with a deadline `T`, then counted while
/// live. Leaves `7` with deadline `T`. Returns `T`.
async fn count_into_a_window(c: &mut Pipe, key: &str) -> u64 {
    let deadline = now_unix_ms() + HOUR_MS;
    let at = deadline.to_string();
    assert_eq!(c.cmd(&["SET", key, "5", "PXAT", &at]).await, ok());
    assert_eq!(c.cmd(&["INCR", key]).await, int(6));
    assert_eq!(c.cmd(&["INCRBY", key, "4"]).await, int(10));
    assert_eq!(c.cmd(&["DECR", key]).await, int(9));
    assert_eq!(c.cmd(&["DECRBY", key, "2"]).await, int(7));
    deadline
}

/// A replica whose store clock reads `now`, following `primary` from a
/// thread of its own. It signals `ready` once it holds the primary's
/// `6of:ready`, then waits for `6of:done` and answers `key`'s state.
fn replica_at(
    primary: std::net::SocketAddr,
    now: u64,
    key: &'static str,
    ready: tokio::sync::oneshot::Sender<()>,
) -> std::thread::JoinHandle<(Value, Value)> {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        at_instant(now, || {
            rt.block_on(async move {
                let replica = node(true, false).await;
                follow(&replica, primary);
                wait_for(&replica.store, "6of:ready", bulk("1")).await;
                ready.send(()).unwrap();
                wait_for(&replica.store, "6of:done", bulk("1")).await;
                (
                    replica.store.execute(Command::Get(key.into())),
                    replica.store.execute(Command::PExpireTime(key.into())),
                )
            })
        })
    })
}

/// Run `writes` on a primary while a replica whose clock reads `now` follows
/// it; `key`'s state on the replica afterwards.
async fn replica_state_after(now: u64, key: &'static str, writes: &[&[&str]]) -> (Value, Value) {
    let primary = node(false, false).await;
    let addr = serve_replicas(&primary).await;
    let mut p = WsClient::connect(primary.ws).await;
    p.cmd(&["SET", "6of:ready", "1"]).await;
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let replica = replica_at(addr, now, key, ready_tx);
    ready_rx.await.unwrap();
    for w in writes {
        let r = p.cmd(w).await;
        assert!(!matches!(r, Value::Error(_)), "{w:?} -> {r:?}");
    }
    p.cmd(&["SET", "6of:done", "1"]).await;
    tokio::task::spawn_blocking(move || replica.join().unwrap())
        .await
        .unwrap()
}

// ── Known failing baseline: resurrection (env-gated) ────────────────────────

async fn counter_replayed_after_the_deadline(policy: AofSync, name: &str) {
    let (shared, aof, path) = aof_primary(policy, name).await;
    let mut c = Pipe::tcp(&shared);
    let key = "6of:a:rate";
    let deadline = count_into_a_window(&mut c, key).await;
    aof.flush().await.unwrap();
    let store = replay_at(&path, deadline + 1_000);
    assert_eq!(
        state_at(&store, key, deadline + 1_000),
        (nil(), int(-2)),
        "an expiring counter replayed after its deadline came back"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reproduce_a_counter_replayed_after_its_deadline_aof_no() {
    counter_replayed_after_the_deadline(AofSync::No, "a_no").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reproduce_a_counter_replayed_after_its_deadline_aof_everysec() {
    counter_replayed_after_the_deadline(AofSync::EverySec, "a_es").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reproduce_a_counter_replayed_after_its_deadline_aof_always() {
    counter_replayed_after_the_deadline(AofSync::Always, "a_al").await;
}

/// The fixed-window idiom: `EXPIRE` on the first hit only.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reproduce_incr_then_expire_once_replayed_after_the_window() {
    let (shared, aof, path) = aof_primary(AofSync::Always, "a_fw").await;
    let mut c = Pipe::tcp(&shared);
    let key = "6of:a:window";
    assert_eq!(c.cmd(&["INCR", key]).await, int(1));
    assert_eq!(c.cmd(&["EXPIRE", key, "3600"]).await, int(1));
    assert_eq!(c.cmd(&["INCR", key]).await, int(2));
    let deadline = match c.cmd(&["PEXPIRETIME", key]).await {
        Value::Integer(at) => at as u64,
        other => panic!("{other:?}"),
    };
    aof.flush().await.unwrap();
    let store = replay_at(&path, deadline + 1_000);
    assert_eq!(state_at(&store, key, deadline + 1_000).0, nil());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reproduce_a_client_set_keepttl_replayed_after_its_deadline() {
    let (shared, aof, path) = aof_primary(AofSync::Always, "a_kt").await;
    let mut c = Pipe::tcp(&shared);
    let key = "6of:a:keepttl";
    let deadline = now_unix_ms() + HOUR_MS;
    assert_eq!(
        c.cmd(&["SET", key, "a", "PXAT", &deadline.to_string()])
            .await,
        ok()
    );
    assert_eq!(c.cmd(&["SET", key, "b", "KEEPTTL"]).await, ok());
    aof.flush().await.unwrap();
    let store = replay_at(&path, deadline + 1_000);
    assert_eq!(state_at(&store, key, deadline + 1_000), (nil(), int(-2)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reproduce_a_replica_applying_a_counter_after_its_deadline() {
    if !reproducing() {
        return;
    }
    let deadline = now_unix_ms() + HOUR_MS;
    let at = deadline.to_string();
    let state = replica_state_after(
        deadline + 1_000,
        "6of:a:replica",
        &[
            &["SET", "6of:a:replica", "5", "PXAT", &at],
            &["INCR", "6of:a:replica"],
            &["DECRBY", "6of:a:replica", "3"],
        ],
    )
    .await;
    assert_eq!(state, (nil(), int(-2)));
}

/// Partial updates have the same exposure (`V0.6O-F-ROOT-CAUSE.md` §4).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reproduce_partial_updates_replayed_after_the_deadline() {
    let (shared, aof, path) = aof_primary(AofSync::Always, "a_delta").await;
    let mut c = Pipe::tcp(&shared);
    let deadline = now_unix_ms() + HOUR_MS;
    let at = deadline.to_string();
    let cases: [(&str, [&[&str]; 2]); 3] = [
        (
            "6of:delta:s",
            [
                &["SET", "6of:delta:s", "a"],
                &["APPEND", "6of:delta:s", "b"],
            ],
        ),
        (
            "6of:delta:h",
            [
                &["HSET", "6of:delta:h", "f", "1"],
                &["HINCRBY", "6of:delta:h", "f", "1"],
            ],
        ),
        (
            "6of:delta:l",
            [
                &["RPUSH", "6of:delta:l", "a"],
                &["RPUSH", "6of:delta:l", "b"],
            ],
        ),
    ];
    for (key, [first, second]) in cases {
        c.cmd(first).await;
        assert_eq!(c.cmd(&["PEXPIREAT", key, &at]).await, int(1));
        c.cmd(second).await;
    }
    aof.flush().await.unwrap();
    let store = replay_at(&path, deadline + 1_000);
    let resurrected: Vec<&str> = cases
        .iter()
        .map(|(key, _)| *key)
        .filter(|key| {
            at_instant(deadline + 1_000, || {
                store.execute(Command::Exists(vec![(*key).into()]))
            }) != int(0)
        })
        .collect();
    assert!(resurrected.is_empty(), "resurrected: {resurrected:?}");
}

/// A restart blocker: `LSET` on a list that expired before the replay fails
/// with `no such key`, and the replay refuses the file.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reproduce_lset_of_a_volatile_list_replayed_after_its_deadline() {
    let (shared, aof, path) = aof_primary(AofSync::Always, "a_lset").await;
    let mut c = Pipe::tcp(&shared);
    let deadline = now_unix_ms() + HOUR_MS;
    assert_eq!(c.cmd(&["RPUSH", "6of:lset", "a"]).await, int(1));
    assert_eq!(
        c.cmd(&["PEXPIREAT", "6of:lset", &deadline.to_string()])
            .await,
        int(1)
    );
    assert_eq!(c.cmd(&["LSET", "6of:lset", "0", "b"]).await, ok());
    aof.flush().await.unwrap();
    let replayed = try_replay_at(&path, deadline + 1_000);
    assert!(replayed.is_ok(), "replay refused: {:?}", replayed.err());
}

// ── Controls: correct on dev ────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_counter_replayed_before_its_deadline_keeps_that_exact_deadline() {
    let (shared, aof, path) = aof_primary(AofSync::Always, "c_before").await;
    let mut c = Pipe::tcp(&shared);
    let key = "6of:c:before";
    let deadline = count_into_a_window(&mut c, key).await;
    aof.flush().await.unwrap();
    let at = deadline - HOUR_MS / 2;
    let store = replay_at(&path, at);
    assert_eq!(state_at(&store, key, at), (bulk("7"), int(deadline as i64)));
    assert_eq!(state_at(&store, key, deadline + 1).0, nil());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_counter_that_restarted_an_expired_key_replays_as_permanent() {
    let (shared, aof, path) = aof_primary(AofSync::Always, "c_restart").await;
    let mut c = Pipe::tcp(&shared);
    let key = "6of:c:restart";
    let past = (now_unix_ms() - 1_000).to_string();
    assert_eq!(c.cmd(&["SET", key, "5", "PXAT", &past]).await, ok());
    assert_eq!(c.cmd(&["INCR", key]).await, int(1));
    aof.flush().await.unwrap();
    let later = now_unix_ms() + 24 * HOUR_MS;
    let store = replay_at(&path, later);
    assert_eq!(state_at(&store, key, later), (bulk("1"), int(-1)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_replica_applying_a_counter_before_its_deadline_keeps_that_deadline() {
    let deadline = now_unix_ms() + HOUR_MS;
    let at = deadline.to_string();
    let state = replica_state_after(
        deadline - 1_000,
        "6of:c:live",
        &[
            &["SET", "6of:c:live", "5", "PXAT", &at],
            &["INCRBY", "6of:c:live", "10"],
        ],
    )
    .await;
    assert_eq!(state, (bulk("15"), int(deadline as i64)));
}

/// A full resynchronisation sends a snapshot, which carries absolute
/// deadlines and leaves out expired entries.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_full_resync_after_the_deadline_does_not_keep_the_counter() {
    let primary = node(false, false).await;
    let addr = serve_replicas(&primary).await;
    let mut p = WsClient::connect(primary.ws).await;
    let key = "6of:c:full";
    let deadline = now_unix_ms() + HOUR_MS;
    let at = deadline.to_string();
    assert_eq!(p.cmd(&["SET", key, "5", "PXAT", &at]).await, ok());
    assert_eq!(p.cmd(&["INCR", key]).await, int(6));
    p.cmd(&["SET", "6of:ready", "1"]).await;
    p.cmd(&["SET", "6of:done", "1"]).await;
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let replica = replica_at(addr, deadline + 1_000, key, ready_tx);
    ready_rx.await.unwrap();
    let state = tokio::task::spawn_blocking(move || replica.join().unwrap())
        .await
        .unwrap();
    assert_eq!(state, (nil(), int(-2)));
}

// ── Regression guards: a counter followed by a deadline change ─────────────
//
// Correct on dev, and wrong under a counter-frame-only fix (6O-F review).

/// The sliding-window rate limiter: `INCR` then `EXPIRE` on every hit,
/// replayed between the first and the extended deadline.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guard_sliding_window_replayed_between_its_deadlines() {
    let (shared, aof, path) = aof_primary(AofSync::Always, "g_slide").await;
    let mut c = Pipe::tcp(&shared);
    let key = "6of:g:slide";
    assert_eq!(c.cmd(&["INCR", key]).await, int(1));
    assert_eq!(c.cmd(&["EXPIRE", key, "3600"]).await, int(1));
    let d1 = match c.cmd(&["PEXPIRETIME", key]).await {
        Value::Integer(n) => n as u64,
        o => panic!("{o:?}"),
    };
    assert_eq!(c.cmd(&["INCR", key]).await, int(2));
    let d2 = d1 + HOUR_MS;
    assert_eq!(c.cmd(&["PEXPIREAT", key, &d2.to_string()]).await, int(1));
    aof.flush().await.unwrap();
    let store = replay_at(&path, d1 + 1_000);
    assert_eq!(
        state_at(&store, key, d1 + 1_000),
        (bulk("2"), int(d2 as i64))
    );
    let store = replay_at(&path, d2 + 1_000);
    assert_eq!(state_at(&store, key, d2 + 1_000).0, nil());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guard_incr_then_extend_replayed_between_its_deadlines() {
    let (shared, aof, path) = aof_primary(AofSync::Always, "g_extend").await;
    let mut c = Pipe::tcp(&shared);
    let key = "6of:g:extend";
    let d1 = now_unix_ms() + HOUR_MS;
    let d2 = d1 + HOUR_MS;
    assert_eq!(
        c.cmd(&["SET", key, "1", "PXAT", &d1.to_string()]).await,
        ok()
    );
    assert_eq!(c.cmd(&["INCR", key]).await, int(2));
    assert_eq!(c.cmd(&["PEXPIREAT", key, &d2.to_string()]).await, int(1));
    aof.flush().await.unwrap();
    let store = replay_at(&path, d1 + 1_000);
    assert_eq!(
        state_at(&store, key, d1 + 1_000),
        (bulk("2"), int(d2 as i64))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guard_incr_then_persist_replayed_after_the_old_deadline() {
    let (shared, aof, path) = aof_primary(AofSync::Always, "g_persist").await;
    let mut c = Pipe::tcp(&shared);
    let key = "6of:g:persist";
    let d1 = now_unix_ms() + HOUR_MS;
    assert_eq!(
        c.cmd(&["SET", key, "1", "PXAT", &d1.to_string()]).await,
        ok()
    );
    assert_eq!(c.cmd(&["INCR", key]).await, int(2));
    assert_eq!(c.cmd(&["PERSIST", key]).await, int(1));
    aof.flush().await.unwrap();
    let store = replay_at(&path, d1 + 1_000);
    assert_eq!(state_at(&store, key, d1 + 1_000), (bulk("2"), int(-1)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn guard_a_replica_applying_incr_then_persist_after_the_old_deadline() {
    let d1 = now_unix_ms() + HOUR_MS;
    let state = replica_state_after(
        d1 + 1_000,
        "6of:g:rpersist",
        &[
            &["SET", "6of:g:rpersist", "1", "PXAT", &d1.to_string()],
            &["INCR", "6of:g:rpersist"],
            &["PERSIST", "6of:g:rpersist"],
        ],
    )
    .await;
    assert_eq!(state, (bulk("2"), int(-1)));
}

/// Under the counter-frame-only fix the replay of this file refused the
/// `RENAME` (`no such key`): the server could not restart.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guard_a_counter_then_rename_replays() {
    let (shared, aof, path) = aof_primary(AofSync::Always, "g_rename").await;
    let mut c = Pipe::tcp(&shared);
    let d = now_unix_ms() + HOUR_MS;
    assert_eq!(
        c.cmd(&["SET", "6of:g:src", "1", "PXAT", &d.to_string()])
            .await,
        ok()
    );
    assert_eq!(c.cmd(&["INCR", "6of:g:src"]).await, int(2));
    assert_eq!(c.cmd(&["RENAME", "6of:g:src", "6of:g:dst"]).await, ok());
    aof.flush().await.unwrap();
    let replayed = try_replay_at(&path, d + 1_000);
    assert!(replayed.is_ok(), "replay refused: {:?}", replayed.err());
}

// ── Deadline overflow (fixed in 6O-F) ───────────────────────────────────────

/// Every expiry form against Redis 7.2.5, measured
/// (`raw/f1-tests/overflow-redis-7.2.5.txt`): a deadline that does not fit a
/// signed 64-bit count of milliseconds is `invalid expire time`. Each case
/// runs against a fresh, existing key `k`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deadlines_past_i64_max_are_refused_as_redis_refuses_them() {
    let shared = Shared::new(false);
    let mut c = Pipe::tcp(&shared);
    let invalid = |cmd: &str| Value::Error(format!("ERR invalid expire time in '{cmd}' command"));
    let cases: [(&[&str], Value); 18] = [
        (
            &["SET", "k", "v", "EXAT", "9223372036854775807"],
            invalid("set"),
        ),
        (&["SET", "k", "v", "EXAT", "9223372036854775"], ok()),
        (
            &["SET", "k", "v", "EXAT", "9223372036854776"],
            invalid("set"),
        ),
        (&["SET", "k", "v", "EX", "9223372036854775"], invalid("set")),
        (&["SET", "k", "v", "EX", "9223372036854776"], invalid("set")),
        (
            &["SET", "k", "v", "PX", "9223372036854775807"],
            invalid("set"),
        ),
        (&["SET", "k", "v", "PXAT", "9223372036854775807"], ok()),
        (&["SETEX", "k", "9223372036854775", "v"], invalid("setex")),
        (
            &["PSETEX", "k", "9223372036854775807", "v"],
            invalid("psetex"),
        ),
        (&["EXPIRE", "k", "9223372036854775"], invalid("expire")),
        (&["EXPIRE", "k", "9223372036854776"], invalid("expire")),
        (&["PEXPIRE", "k", "9223372036854775807"], invalid("pexpire")),
        (
            &["EXPIREAT", "k", "9223372036854775807"],
            invalid("expireat"),
        ),
        (&["EXPIREAT", "k", "9223372036854775"], int(1)),
        (&["PEXPIREAT", "k", "9223372036854775807"], int(1)),
        (
            &["GETEX", "k", "EXAT", "9223372036854775807"],
            invalid("getex"),
        ),
        (&["GETEX", "k", "EX", "9223372036854775"], invalid("getex")),
        (
            &["GETEX", "k", "PX", "9223372036854775807"],
            invalid("getex"),
        ),
    ];
    for (args, want) in cases {
        assert_eq!(c.cmd(&["SET", "k", "v"]).await, ok());
        assert_eq!(c.cmd(args).await, want, "{args:?}");
    }
    // Order against Redis 7.2.5 (6O-F review): an absent or expired key
    // answers GETEX with nil, a list answers WRONGTYPE before any deadline
    // check, and EXPIRE/PEXPIRE on an absent key still refuse a deadline
    // that overflows once the clock is added.
    assert_eq!(
        c.cmd(&["GETEX", "absent", "PX", "9223372036854775000"])
            .await,
        nil()
    );
    assert_eq!(c.cmd(&["SET", "gone", "v", "PX", "1"]).await, ok());
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    assert_eq!(
        c.cmd(&["GETEX", "gone", "PX", "9223372036854775000"]).await,
        nil()
    );
    assert_eq!(c.cmd(&["RPUSH", "list", "v"]).await, int(1));
    let wrongtype =
        Value::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into());
    assert_eq!(
        c.cmd(&["GETEX", "list", "PX", "9223372036854775000"]).await,
        wrongtype
    );
    assert_eq!(
        c.cmd(&["GETEX", "list", "EX", "9223372036854775"]).await,
        wrongtype
    );
    assert_eq!(
        c.cmd(&["EXPIRE", "absent", "9223372036854775"]).await,
        invalid("expire")
    );
    assert_eq!(
        c.cmd(&["PEXPIRE", "absent", "9223372036854775000"]).await,
        invalid("pexpire")
    );
    // SET and EXPIRE check before the key is looked at, as Redis does.
    assert_eq!(c.cmd(&["SET", "k", "v"]).await, ok());
    assert_eq!(
        c.cmd(&["SET", "k", "w", "NX", "EX", "9223372036854776"])
            .await,
        invalid("set")
    );
    assert_eq!(
        c.cmd(&["EXPIRE", "absent", "9223372036854776"]).await,
        invalid("expire")
    );
}

/// Every accepted deadline propagates as a frame the replay parses: the
/// largest of each form, replayed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_largest_accepted_deadlines_replay() {
    let (shared, aof, path) = aof_primary(AofSync::Always, "o_max").await;
    let mut c = Pipe::tcp(&shared);
    for args in [
        &["SET", "6of:o:1", "v", "EXAT", "9223372036854775"][..],
        &["SET", "6of:o:2", "v", "PXAT", "9223372036854775807"],
        &["SET", "6of:o:3", "v"],
        &["EXPIREAT", "6of:o:3", "9223372036854775"],
        &["SET", "6of:o:4", "v"],
        &["PEXPIREAT", "6of:o:4", "9223372036854775807"],
        &["SET", "6of:o:5", "1", "PXAT", "9223372036854775807"],
        &["INCR", "6of:o:5"],
        &["SET", "6of:o:6", "v"],
        &["GETEX", "6of:o:6", "PXAT", "9223372036854775807"],
    ] {
        let r = c.cmd(args).await;
        assert!(!matches!(r, Value::Error(_)), "{args:?} -> {r:?}");
    }
    aof.flush().await.unwrap();
    let store = replay_at(&path, now_unix_ms());
    assert_eq!(
        state_at(&store, "6of:o:5", now_unix_ms()),
        (bulk("2"), int(i64::MAX))
    );
    for key in ["6of:o:1", "6of:o:2", "6of:o:3", "6of:o:4", "6of:o:6"] {
        assert_eq!(
            store.execute(Command::Exists(vec![key.into()])),
            int(1),
            "{key}"
        );
    }
}

fn push(words: &[&str]) -> Vec<u8> {
    let mut out = format!(">{}\r\n", words.len());
    for w in words {
        out.push_str(&format!("${}\r\n{w}\r\n", w.len()));
    }
    out.into_bytes()
}

/// Records that servers before 6O-F wrote for `SET … EXAT` and `EXPIREAT`
/// past `i64::MAX`, bare and inside a DEDUP envelope: before 6O-F the replay
/// refused them, and the server could not restart. Clamped to `i64::MAX`.
#[test]
fn records_with_a_deadline_past_i64_max_written_before_6of_replay() {
    let path = scratch("o_legacy", "append.aof");
    let records = [
        push(&["SET", "6of:l:1", "v", "PXAT", "18446744073709551615"]),
        push(&["SET", "6of:l:2", "v"]),
        push(&["PEXPIREAT", "6of:l:2", "9223372036854775808"]),
        push(&[
            "DEDUP",
            "client-a",
            "7",
            "SET",
            "6of:l:3",
            "v",
            "PXAT",
            "18446744073709551615",
        ]),
    ];
    std::fs::write(&*path, records.concat()).unwrap();
    let store = replay_at(&path, now_unix_ms());
    for key in ["6of:l:1", "6of:l:2", "6of:l:3"] {
        assert_eq!(
            store.execute(Command::PExpireTime(key.into())),
            int(i64::MAX),
            "{key}"
        );
    }
}

#[test]
fn only_overflowing_deadlines_are_clamped() {
    let parsed = |words: &[&str]| {
        let mut v = Value::Array(Some(
            words
                .iter()
                .map(|w| Value::BulkString(Some(w.as_bytes().to_vec())))
                .collect(),
        ));
        crate::persistence::clamp_overflowing_deadline(&mut v);
        v
    };
    let same = |words: &[&str]| {
        assert_eq!(
            parsed(words),
            Value::Array(Some(
                words
                    .iter()
                    .map(|w| Value::BulkString(Some(w.as_bytes().to_vec())))
                    .collect()
            )),
            "{words:?}"
        );
    };
    same(&["SET", "k", "v", "PXAT", "9223372036854775807"]);
    same(&["SET", "k", "v", "PXAT", "-5"]);
    same(&["SET", "k", "v", "PXAT", "12x"]);
    same(&["SET", "k", "18446744073709551615"]);
    same(&["SET", "k", "v", "EX", "18446744073709551615"]);
    same(&["PEXPIREAT", "k", "123"]);
    same(&["HSET", "h", "PXAT", "18446744073709551615"]);
    assert_eq!(
        parsed(&["PEXPIREAT", "k", "99999999999999999999999"]),
        parsed(&["PEXPIREAT", "k", "9223372036854775807"])
    );
}
