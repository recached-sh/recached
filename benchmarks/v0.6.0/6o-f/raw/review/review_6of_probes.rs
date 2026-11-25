//! Independent review probes for 6O-F (not part of the fix).
//!
//! Each probe drives the real TCP write path with an AOF (or a real
//! primary/replica pair), then replays/applies at a fixed store clock, and
//! asserts the state the ORIGINAL command timeline requires.
//!
//! Probes that are expected to FAIL on e55f7d5 are named `probe_fail_*`.
//! Run: cargo test -p recached --bin recached-server review_6of_probes

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

async fn aof_primary(name: &str) -> (Shared, Arc<AofWriter>, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "recached_review6of_{name}_{}_{}",
        std::process::id(),
        next_conn_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("append.aof");
    let aof = Arc::new(AofWriter::open(path.clone(), AofSync::Always).await.unwrap());
    (Shared::with_aof(false, Some(Arc::clone(&aof))), aof, path)
}

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
        })
        .map(|_| store)
    })
    .join()
    .expect("replay thread")
}

fn state_at(store: &KeyValueStore, key: &str, now: u64) -> (Value, Value) {
    at_instant(now, || {
        (
            store.execute(Command::Get(key.into())),
            store.execute(Command::PExpireTime(key.into())),
        )
    })
}

fn aof_text(path: &Path) -> String {
    String::from_utf8_lossy(&std::fs::read(path).unwrap()).replace("\r\n", " ")
}

// ── R1: counter, then a TTL change, replayed between the two deadlines ─────

/// The sliding-window rate limiter (`INCR k` + `EXPIRE k window` on every
/// hit). The original timeline: at the replay instant the key is live with
/// value 2 and deadline D2. Pre-6O-F (`SET 2 KEEPTTL`) this replayed right;
/// post-6O-F the counter frame recreates an already-expired entry and the
/// later `PEXPIREAT` finds nothing to extend.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_fail_incr_then_extend_replayed_between_deadlines() {
    let (shared, aof, path) = aof_primary("r1").await;
    let mut c = Pipe::tcp(&shared);
    let key = "r6of:r1";
    let d1 = now_unix_ms() + HOUR_MS;
    let d2 = d1 + HOUR_MS;
    assert_eq!(c.cmd(&["SET", key, "1", "PXAT", &d1.to_string()]).await, ok());
    assert_eq!(c.cmd(&["INCR", key]).await, int(2));
    assert_eq!(c.cmd(&["PEXPIREAT", key, &d2.to_string()]).await, int(1));
    aof.flush().await.unwrap();
    eprintln!("AOF: {}", aof_text(&path));
    let at = d1 + 1_000;
    let store = try_replay_at(&path, at).unwrap();
    assert_eq!(
        state_at(&store, key, at),
        (bulk("2"), int(d2 as i64)),
        "the key is live (2, D2) at this instant in the original timeline"
    );
}

/// Same with PERSIST: a key the primary made PERMANENT is lost by a replay
/// after its former deadline.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_fail_incr_then_persist_replayed_after_deadline() {
    let (shared, aof, path) = aof_primary("r1p").await;
    let mut c = Pipe::tcp(&shared);
    let key = "r6of:r1p";
    let d1 = now_unix_ms() + HOUR_MS;
    assert_eq!(c.cmd(&["SET", key, "1", "PXAT", &d1.to_string()]).await, ok());
    assert_eq!(c.cmd(&["INCR", key]).await, int(2));
    assert_eq!(c.cmd(&["PERSIST", key]).await, int(1));
    aof.flush().await.unwrap();
    eprintln!("AOF: {}", aof_text(&path));
    let at = d1 + 1_000;
    let store = try_replay_at(&path, at).unwrap();
    assert_eq!(
        state_at(&store, key, at),
        (bulk("2"), int(-1)),
        "the primary holds 2 permanently"
    );
}

/// Same with the very first rate-limit hit as INCR (no SET): the idiom the
/// fix's own comments cite. `INCR; EXPIRE 1h; INCR; EXPIRE` (2nd EXPIRE
/// modelled as PEXPIREAT D1+1h so no sleep is needed).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_fail_sliding_window_idiom_replayed_after_first_window() {
    let (shared, aof, path) = aof_primary("r1s").await;
    let mut c = Pipe::tcp(&shared);
    let key = "r6of:r1s";
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
    let at = d1 + 1_000;
    let store = try_replay_at(&path, at).unwrap();
    assert_eq!(state_at(&store, key, at), (bulk("2"), int(d2 as i64)));
}

// ── R1 on a lagging replica ────────────────────────────────────────────────

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
                wait_for(&replica.store, "r6of:ready", bulk("1")).await;
                ready.send(()).unwrap();
                wait_for(&replica.store, "r6of:done", bulk("1")).await;
                (
                    replica.store.execute(Command::Get(key.into())),
                    replica.store.execute(Command::PExpireTime(key.into())),
                )
            })
        })
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn probe_fail_replica_applying_incr_then_persist_after_deadline() {
    let primary = node(false, false).await;
    let addr = serve_replicas(&primary).await;
    let mut p = WsClient::connect(primary.ws).await;
    p.cmd(&["SET", "r6of:ready", "1"]).await;
    let key = "r6of:r2";
    let d1 = now_unix_ms() + HOUR_MS;
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let replica = replica_at(addr, d1 + 1_000, key, ready_tx);
    ready_rx.await.unwrap();
    assert_eq!(p.cmd(&["SET", key, "1", "PXAT", &d1.to_string()]).await, ok());
    assert_eq!(p.cmd(&["INCR", key]).await, int(2));
    assert_eq!(p.cmd(&["PERSIST", key]).await, int(1));
    p.cmd(&["SET", "r6of:done", "1"]).await;
    let state = tokio::task::spawn_blocking(move || replica.join().unwrap())
        .await
        .unwrap();
    assert_eq!(state, (bulk("2"), int(-1)), "primary holds 2 permanently");
}

// ── R2: RENAME / LSET of a volatile key replayed after its deadline ────────

/// Pre-existing: the AOF replay fails (server cannot restart).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_fail_rename_of_volatile_key_replayed_after_deadline() {
    let (shared, aof, path) = aof_primary("r3").await;
    let mut c = Pipe::tcp(&shared);
    let d = now_unix_ms() + HOUR_MS;
    assert_eq!(c.cmd(&["SET", "r6of:src", "1", "PXAT", &d.to_string()]).await, ok());
    assert_eq!(c.cmd(&["INCR", "r6of:src"]).await, int(2));
    assert_eq!(c.cmd(&["RENAME", "r6of:src", "r6of:dst"]).await, ok());
    aof.flush().await.unwrap();
    eprintln!("AOF: {}", aof_text(&path));
    let at = d + 1_000;
    let r = try_replay_at(&path, at);
    assert!(r.is_ok(), "replay refused: {:?}", r.err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_fail_lset_of_volatile_list_replayed_after_deadline() {
    let (shared, aof, path) = aof_primary("r3l").await;
    let mut c = Pipe::tcp(&shared);
    let d = now_unix_ms() + HOUR_MS;
    assert_eq!(c.cmd(&["RPUSH", "r6of:l", "a"]).await, int(1));
    assert_eq!(c.cmd(&["PEXPIREAT", "r6of:l", &d.to_string()]).await, int(1));
    assert_eq!(c.cmd(&["LSET", "r6of:l", "0", "b"]).await, ok());
    aof.flush().await.unwrap();
    let r = try_replay_at(&path, d + 1_000);
    assert!(r.is_ok(), "replay refused: {:?}", r.err());
}

// ── R3: absolute deadlines past i64::MAX in other frame families ────────────

async fn replay_now_ok(cmds: &[&[&str]], name: &str) {
    let (shared, aof, path) = aof_primary(name).await;
    let mut c = Pipe::tcp(&shared);
    for cmd in cmds {
        let r = c.cmd(cmd).await;
        assert!(!matches!(r, Value::Error(_)), "{cmd:?} -> {r:?}");
    }
    aof.flush().await.unwrap();
    eprintln!("AOF: {}", aof_text(&path));
    let r = try_replay_at(&path, now_unix_ms());
    assert!(r.is_ok(), "replay refused: {:?}", r.err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_fail_set_exat_i64_max_replays() {
    replay_now_ok(&[&["SET", "r6of:x1", "1", "EXAT", "9223372036854775807"]], "r4a").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_fail_expireat_i64_max_replays() {
    replay_now_ok(
        &[&["SET", "r6of:x2", "1"], &["EXPIREAT", "r6of:x2", "9223372036854775807"]],
        "r4b",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_fail_getex_exat_i64_max_replays() {
    replay_now_ok(
        &[&["SET", "r6of:x3", "1"], &["GETEX", "r6of:x3", "EXAT", "9223372036854775807"]],
        "r4c",
    )
    .await;
}

/// The fixed path: INCR on a key whose deadline saturated (EXAT i64::MAX
/// stores u64::MAX). The counter frame itself clamps; is the whole file
/// replayable? (It is not, because of the SET EXAT frame above.)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_px_i64_max_then_incr_replays() {
    replay_now_ok(
        &[&["SET", "r6of:x4", "1", "PX", "9223372036854775807"], &["INCR", "r6of:x4"]],
        "r4d",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_expire_huge_then_incr_replays() {
    replay_now_ok(
        &[&["SET", "r6of:x5", "1"], &["EXPIRE", "r6of:x5", "9223372036854775"], &["INCR", "r6of:x5"]],
        "r4e",
    )
    .await;
}

// ── R4: things that should hold (positive controls) ────────────────────────

/// SET KEEPTTL with NX/XX/GET variants: frame only when applied, deadline
/// kept, replay at both sides of the deadline.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_set_keepttl_conditions() {
    let (shared, aof, path) = aof_primary("r5").await;
    let mut c = Pipe::tcp(&shared);
    let d = now_unix_ms() + HOUR_MS;
    let ds = d.to_string();
    assert_eq!(c.cmd(&["SET", "r6of:k", "a", "PXAT", &ds]).await, ok());
    assert_eq!(c.cmd(&["SET", "r6of:k", "b", "KEEPTTL", "NX"]).await, nil());
    assert_eq!(c.cmd(&["SET", "r6of:k", "c", "KEEPTTL", "XX", "GET"]).await, bulk("a"));
    assert_eq!(c.cmd(&["SET", "r6of:n", "z", "KEEPTTL", "XX"]).await, nil());
    assert_eq!(c.cmd(&["SET", "r6of:n2", "z", "KEEPTTL", "NX", "GET"]).await, nil());
    aof.flush().await.unwrap();
    let text = aof_text(&path);
    eprintln!("AOF: {text}");
    assert!(!text.contains("KEEPTTL"));
    let s = try_replay_at(&path, d - 1).unwrap();
    assert_eq!(state_at(&s, "r6of:k", d - 1), (bulk("c"), int(d as i64)));
    assert_eq!(state_at(&s, "r6of:n", d - 1).0, nil());
    assert_eq!(state_at(&s, "r6of:n2", d - 1), (bulk("z"), int(-1)));
    let s = try_replay_at(&path, d + 1).unwrap();
    assert_eq!(state_at(&s, "r6of:k", d + 1).0, nil());
    assert_eq!(state_at(&s, "r6of:n2", d + 1), (bulk("z"), int(-1)));
}

/// DEDUP-wrapped counter via the real path: frame inside the envelope
/// carries PXAT.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_dedup_counter_frame() {
    let (shared, aof, path) = aof_primary("r6").await;
    let mut c = Pipe::tcp(&shared);
    let d = now_unix_ms() + HOUR_MS;
    assert_eq!(c.cmd(&["SET", "r6of:dd", "1", "PXAT", &d.to_string()]).await, ok());
    let r = c.cmd(&["DEDUP", "cli", "1", "INCR", "r6of:dd"]).await;
    eprintln!("DEDUP reply {r:?}");
    aof.flush().await.unwrap();
    let text = aof_text(&path);
    eprintln!("AOF: {text}");
    assert!(text.contains(&format!("PXAT ${} {d}", d.to_string().len())), "{text}");
    let s = try_replay_at(&path, d + 1).unwrap();
    assert_eq!(state_at(&s, "r6of:dd", d + 1).0, nil());
}
