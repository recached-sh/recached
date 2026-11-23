//! 6O-E2: WATCH and expiry, against Redis 7.2.5's rules.
//!
//! Redis aborts `EXEC` when a key that was live at `WATCH` has expired since
//! (`isWatchedKeyExpired`), even though no command touched it. Redis also
//! treats the removal of a key that was absent or already expired at
//! `WATCH` as no change (`touchWatchedKey`'s `expired` flag,
//! `touchAllWatchedKeysInDb`): a `DEL` or `FLUSHDB` of such a key, or the
//! deletion of an already-expired one, does not abort. Every case here was
//! checked against Redis 7.2.5 (`benchmarks/v0.6.0/6o-e/tools/watch_expiry_diff.py`).
//!
//! The in-process server runs no sweeper, so expiry here is lazy unless a
//! test runs `sweep_expired_tick` itself. Deadlines are tens of
//! milliseconds, and tests wait for them by polling the store, not by
//! sleeping a fixed time.

use super::mset_atomicity::{Pipe, WITHIN, bulk};
use super::request_path::Shared;
use crate::propagation::exec_pause;
use crate::*;
use core_engine::resp::Value;
use std::time::{Duration, Instant};

fn ok() -> Value {
    Value::SimpleString("OK".into())
}

/// `MULTI`, `SET <k>:other 1`, `EXEC`: true when it committed.
async fn commits(pipe: &mut Pipe, k: &str) -> bool {
    assert_eq!(pipe.cmd(&["MULTI"]).await, ok());
    let other = format!("{k}:other");
    assert_eq!(
        pipe.cmd(&["SET", &other, "1"]).await,
        Value::SimpleString("QUEUED".into())
    );
    match pipe.cmd(&["EXEC"]).await {
        Value::Array(None) => false,
        Value::Array(Some(_)) => true,
        other => panic!("EXEC answered {other:?}"),
    }
}

fn live(store: &KeyValueStore, key: &str) -> bool {
    matches!(
        store.execute(Command::Exists(vec![key.to_string()])),
        Value::Integer(1)
    )
}

/// Wait until `key` has expired (by the store's clock), bounded.
async fn expired(store: &KeyValueStore, key: &str) {
    let deadline = Instant::now() + WITHIN;
    while live(store, key) {
        assert!(Instant::now() < deadline, "{key} never expired");
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

/// `SET key v PX ms`.
async fn set_px(pipe: &mut Pipe, key: &str, ms: u64) {
    assert_eq!(
        pipe.cmd(&["SET", key, "v", "PX", &ms.to_string()]).await,
        ok()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_watched_key_that_expires_lazily_aborts_exec() {
    let shared = Shared::new(false);
    let mut s = Pipe::tcp(&shared);
    let k = "6oe:wx:lazy";
    set_px(&mut s, k, 30).await;
    assert_eq!(s.cmd(&["WATCH", k]).await, ok());
    expired(&shared.store, k).await;
    assert!(
        !commits(&mut s, k).await,
        "Redis aborts: the key was live at WATCH"
    );
    // The WATCH is gone after the aborted EXEC: the next one commits.
    assert!(commits(&mut s, k).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn watching_a_key_already_expired_commits() {
    let shared = Shared::new(false);
    let mut s = Pipe::tcp(&shared);
    let k = "6oe:wx:before";
    set_px(&mut s, k, 20).await;
    expired(&shared.store, k).await;
    assert_eq!(s.cmd(&["WATCH", k]).await, ok());
    assert!(
        commits(&mut s, k).await,
        "Redis commits: nothing changed after WATCH"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_key_not_yet_expired_at_exec_commits() {
    let shared = Shared::new(false);
    let mut s = Pipe::tcp(&shared);
    let k = "6oe:wx:later";
    set_px(&mut s, k, 60_000).await;
    assert_eq!(s.cmd(&["WATCH", k]).await, ok());
    assert!(commits(&mut s, k).await);
}

/// The sweeper removing a key that was live at WATCH aborts; removing one
/// that had already expired at WATCH does not.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sweeper_removals_follow_the_same_rules() {
    let shared = Shared::new(false);
    let (mut live, mut stale) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let (kl, ks) = ("6oe:wx:sweep:live", "6oe:wx:sweep:stale");
    set_px(&mut stale, ks, 20).await;
    expired(&shared.store, ks).await;
    assert_eq!(stale.cmd(&["WATCH", ks]).await, ok());
    set_px(&mut live, kl, 30).await;
    assert_eq!(live.cmd(&["WATCH", kl]).await, ok());
    expired(&shared.store, kl).await;
    let removed = sweep_expired_tick(&shared.store, &shared.state, &shared.watch_registry).await;
    assert!(removed >= 2, "the sweeper removed {removed}");
    assert!(!commits(&mut live, kl).await, "live at WATCH, then swept");
    assert!(
        commits(&mut stale, ks).await,
        "already expired at WATCH, then swept"
    );
}

/// A key that expires while `EXEC` runs: the transaction runs at the instant
/// its guards were complete, when the key was live, so it commits and its
/// reads see the key; the same key expiring before `EXEC` aborts it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn expiry_during_exec_is_judged_at_the_transactions_instant() {
    let shared = Shared::new(false);
    let mut s = Pipe::tcp(&shared);
    let k = "6oe:wx:during";
    set_px(&mut s, k, 80).await;
    assert_eq!(s.cmd(&["WATCH", k]).await, ok());
    assert_eq!(s.cmd(&["MULTI"]).await, ok());
    s.cmd(&["GET", k]).await;
    s.cmd(&["GET", k]).await;
    let pause = exec_pause::after_command_naming(k);
    s.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await);
    expired(&shared.store, k).await;
    drop(pause);
    let exec = tokio::time::timeout(WITHIN, s.reply())
        .await
        .expect("EXEC reply");
    match exec {
        // EXEC began after the deadline (a very slow host): it aborted.
        Value::Array(None) => {}
        other => assert_eq!(other, Value::Array(Some(vec![bulk("v"), bulk("v")]))),
    }
}

/// Explicit deletion, recreation and TTL changes, each against a key that was
/// live, absent, or already expired at WATCH.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deletion_recreation_and_ttl_changes() {
    let shared = Shared::new(false);
    let (mut s, mut o) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let cases: [(&str, bool); 8] = [
        ("live-deleted", false),
        ("absent-deleted", true),
        ("expired-deleted", true),
        ("absent-created", false),
        ("expired-recreated", false),
        ("live-ttl-changed", false),
        ("live-persisted", false),
        ("live-expire-in-past", false),
    ];
    for (case, expect_commit) in cases {
        let k = format!("6oe:wx:{case}");
        let k = k.as_str();
        match case {
            "live-deleted" | "live-expire-in-past" => {
                s.cmd(&["SET", k, "v"]).await;
            }
            "expired-deleted" | "expired-recreated" => {
                set_px(&mut s, k, 20).await;
                expired(&shared.store, k).await;
            }
            "live-ttl-changed" | "live-persisted" => set_px(&mut s, k, 60_000).await,
            _ => {}
        }
        assert_eq!(s.cmd(&["WATCH", k]).await, ok());
        match case {
            "live-deleted" | "absent-deleted" | "expired-deleted" => {
                o.cmd(&["DEL", k]).await;
            }
            "absent-created" | "expired-recreated" => {
                o.cmd(&["SET", k, "w"]).await;
            }
            "live-ttl-changed" => {
                o.cmd(&["PEXPIRE", k, "120000"]).await;
            }
            "live-persisted" => {
                o.cmd(&["PERSIST", k]).await;
            }
            "live-expire-in-past" => {
                o.cmd(&["PEXPIREAT", k, "1"]).await;
            }
            _ => unreachable!(),
        }
        assert_eq!(commits(&mut s, k).await, expect_commit, "case {case}");
    }
}

/// FLUSHDB invalidates a WATCH only of a key that existed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn flushdb_aborts_only_watches_of_keys_that_existed() {
    let shared = Shared::new(false);
    let (mut absent, mut live, mut o) =
        (Pipe::tcp(&shared), Pipe::tcp(&shared), Pipe::tcp(&shared));
    let (ka, kl) = ("6oe:wx:flush:absent", "6oe:wx:flush:live");
    live.cmd(&["SET", kl, "v"]).await;
    assert_eq!(absent.cmd(&["WATCH", ka]).await, ok());
    assert_eq!(live.cmd(&["WATCH", kl]).await, ok());
    assert_eq!(o.cmd(&["FLUSHDB"]).await, ok());
    assert!(
        commits(&mut absent, ka).await,
        "Redis commits: the key did not exist"
    );
    assert!(!commits(&mut live, kl).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn several_watched_keys_abort_if_any_one_expired() {
    let shared = Shared::new(false);
    let mut s = Pipe::tcp(&shared);
    let (k1, k2, k3) = ("6oe:wx:multi:1", "6oe:wx:multi:2", "6oe:wx:multi:3");
    set_px(&mut s, k1, 30).await;
    s.cmd(&["SET", k2, "v"]).await;
    assert_eq!(s.cmd(&["WATCH", k1, k2, k3]).await, ok());
    expired(&shared.store, k1).await;
    assert!(!commits(&mut s, k1).await);
    // None of them changing: commits.
    set_px(&mut s, k1, 60_000).await;
    assert_eq!(s.cmd(&["WATCH", k1, k2, k3]).await, ok());
    assert!(commits(&mut s, k1).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unwatch_and_discard_clear_an_expired_watch() {
    let shared = Shared::new(false);
    let mut s = Pipe::tcp(&shared);
    let (ku, kd) = ("6oe:wx:unwatch", "6oe:wx:discard");
    set_px(&mut s, ku, 30).await;
    assert_eq!(s.cmd(&["WATCH", ku]).await, ok());
    assert_eq!(s.cmd(&["UNWATCH"]).await, ok());
    expired(&shared.store, ku).await;
    assert!(commits(&mut s, ku).await, "UNWATCH before expiry");
    set_px(&mut s, kd, 30).await;
    assert_eq!(s.cmd(&["WATCH", kd]).await, ok());
    expired(&shared.store, kd).await;
    assert_eq!(s.cmd(&["MULTI"]).await, ok());
    assert_eq!(s.cmd(&["DISCARD"]).await, ok());
    assert!(
        commits(&mut s, kd).await,
        "DISCARD after expiry clears the WATCH"
    );
}

/// Many WATCH/EXEC cycles on expiring keys leave no registration behind.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn repeated_watch_cycles_leak_nothing() {
    let shared = Shared::new(false);
    let mut s = Pipe::tcp(&shared);
    let k = "6oe:wx:cycles";
    for i in 0..300 {
        // Even cycles let the key expire; odd ones change its TTL, with a
        // deadline long enough that it is live at WATCH on a loaded host.
        set_px(&mut s, k, if i % 2 == 0 { 5 } else { 60_000 }).await;
        assert_eq!(s.cmd(&["WATCH", k]).await, ok());
        if i % 2 == 0 {
            expired(&shared.store, k).await;
            assert!(!commits(&mut s, k).await, "cycle {i}");
        } else {
            s.cmd(&["PERSIST", k]).await;
            assert!(!commits(&mut s, k).await, "cycle {i}: PERSIST is a change");
            s.cmd(&["DEL", k]).await;
        }
    }
    assert!(shared.watch_registry.map.lock().await.is_empty());
    assert_eq!(shared.watch_registry.watched_keys.load(Ordering::SeqCst), 0);
}

/// The same rules on the WebSocket EXEC.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn websocket_watch_follows_the_expiry_rules() {
    let srv = super::spawn_ws_server().await;
    let (mut s, mut o) = (
        super::WsClient::connect(srv.tcp_addr).await,
        super::WsClient::connect(srv.tcp_addr).await,
    );
    async fn ws_commits(c: &mut super::WsClient, k: &str) -> bool {
        assert_eq!(c.cmd(&["MULTI"]).await, ok());
        c.cmd(&["SET", &format!("{k}:other"), "1"]).await;
        !matches!(c.cmd(&["EXEC"]).await, Value::Array(None))
    }
    let (kl, ka) = ("6oe:wx:ws:lazy", "6oe:wx:ws:absent");
    assert_eq!(s.cmd(&["SET", kl, "v", "PX", "30"]).await, ok());
    assert_eq!(s.cmd(&["WATCH", kl]).await, ok());
    expired(&srv.store, kl).await;
    assert!(
        !ws_commits(&mut s, kl).await,
        "live at WATCH, expired lazily"
    );
    assert_eq!(s.cmd(&["WATCH", ka]).await, ok());
    o.cmd(&["DEL", ka]).await;
    assert!(ws_commits(&mut s, ka).await, "DEL of a key absent at WATCH");
}

/// Tenants: a scoped WebSocket client's WATCH aborts on its own key's
/// expiry, is not disturbed by another tenant's writes, and cannot watch
/// another tenant's key at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tenant_watches_are_isolated() {
    use super::ws_refusal_barrier::{scoped, serve_strict};
    let node = super::dedup_ack_order::node("6oe-wx-tenant", AofSync::No).await;
    let addr = serve_strict(&node).await;
    let (mut a, mut b) = (scoped(addr, "a").await, scoped(addr, "b").await);
    let (ka, kb) = ("tenant:a:k", "tenant:b:k");
    assert_eq!(a.cmd(&["SET", ka, "v", "PX", "30"]).await, ok());
    assert_eq!(a.cmd(&["WATCH", ka]).await, ok());
    assert!(
        matches!(b.cmd(&["WATCH", ka]).await, Value::Error(_)),
        "another tenant's key"
    );
    assert_eq!(b.cmd(&["SET", kb, "x"]).await, ok());
    expired(&node.store, ka).await;
    assert_eq!(a.cmd(&["MULTI"]).await, ok());
    a.cmd(&["SET", "tenant:a:other", "1"]).await;
    assert_eq!(a.cmd(&["EXEC"]).await, Value::Array(None));
    // Unexpired, and only tenant b writing: commits.
    assert_eq!(a.cmd(&["SET", ka, "v"]).await, ok());
    assert_eq!(a.cmd(&["WATCH", ka]).await, ok());
    assert_eq!(b.cmd(&["SET", kb, "y"]).await, ok());
    assert_eq!(a.cmd(&["MULTI"]).await, ok());
    a.cmd(&["SET", "tenant:a:other", "2"]).await;
    assert!(matches!(a.cmd(&["EXEC"]).await, Value::Array(Some(_))));
}

// ── Review follow-ups ────────────────────────────────────────────────────────
//
// A key absent at WATCH and then created must abort EXEC however short its
// TTL, as in Redis (`SET` signals the key; the WATCH entry is not
// `expired`). The first version decided "no change" by reading the key when
// the notification was delivered, after the write's AOF stage, and a key
// created with a short TTL had already expired by then (review, High).

/// Review probe P1, deterministic: the creation runs inside another
/// connection's transaction, which is stopped for 30 ms after its first
/// command, so the key it creates with `PX 5` has expired by the time its
/// notification is delivered.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_creation_announced_after_its_deadline_still_aborts() {
    let shared = Shared::new(false);
    let (mut w, mut o) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let (k, first) = ("6oe:wx:p1:k", "6oe:wx:p1:first");
    assert_eq!(w.cmd(&["WATCH", k]).await, ok());
    assert_eq!(o.cmd(&["MULTI"]).await, ok());
    o.cmd(&["SET", first, "1"]).await;
    o.cmd(&["SET", k, "v", "PX", "5"]).await;
    let pause = exec_pause::after_command_naming(first);
    o.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await);
    tokio::time::sleep(Duration::from_millis(30)).await;
    drop(pause);
    assert!(matches!(
        tokio::time::timeout(WITHIN, o.reply()).await.expect("EXEC"),
        Value::Array(Some(_))
    ));
    assert!(
        !commits(&mut w, k).await,
        "a key created after WATCH is a change"
    );
}

/// Review probe P2: creating the key with a deadline already past is a
/// change too (Redis 7.2.5 aborts).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn creating_an_absent_key_already_expired_still_aborts() {
    let shared = Shared::new(false);
    let (mut w, mut o) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let k = "6oe:wx:p2";
    assert_eq!(w.cmd(&["WATCH", k]).await, ok());
    assert_eq!(o.cmd(&["SET", k, "v", "PXAT", "1"]).await, ok());
    assert!(!commits(&mut w, k).await);
}

/// Review probes P3/P4: short TTLs from another connection and from the
/// watcher itself, many times. Every one aborts.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn creations_with_short_ttls_always_abort() {
    let shared = Shared::new(false);
    let (mut w, mut o) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    for i in 0..100 {
        let k = format!("6oe:wx:p3:{i}");
        assert_eq!(w.cmd(&["WATCH", &k]).await, ok());
        if i % 2 == 0 {
            o.cmd(&["SET", &k, "v", "PX", "1"]).await;
        } else {
            w.cmd(&["PSETEX", &k, "1", "v"]).await;
        }
        tokio::time::sleep(Duration::from_millis(3)).await;
        assert!(!commits(&mut w, &k).await, "round {i}");
    }
}

/// `DEL` of several keys, one of them the watched key that was absent at
/// WATCH: nothing changed for the watcher (Redis signals only keys it
/// deleted); the other key was really deleted.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_multi_key_del_that_misses_the_watched_key_commits() {
    let shared = Shared::new(false);
    let (mut w, mut o) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let (k, other) = ("6oe:wx:pdel:k", "6oe:wx:pdel:other");
    o.cmd(&["SET", other, "v"]).await;
    assert_eq!(w.cmd(&["WATCH", k]).await, ok());
    assert_eq!(o.cmd(&["DEL", other, k]).await, Value::Integer(1));
    assert!(commits(&mut w, k).await);
}

// ── Review round 2 ───────────────────────────────────────────────────────────

/// A write that lands while WATCH registers (after the registration is
/// published, before the key is read) is seen one way or the other: the
/// writer notifies the new registration, or the read sees the key live. A
/// later DEL then aborts EXEC, as in Redis under either order. Reading the
/// key before publishing left a window in which the write was announced to
/// nobody and the key recorded absent, so the DEL committed (review round 2,
/// R1).
async fn a_write_during_registration(expire: bool) {
    let shared = Shared::new(false);
    let k = if expire {
        "6oe:wx:reg:expire"
    } else {
        "6oe:wx:reg:del"
    };
    let (mut w, mut o) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let pause = crate::watch::register_pause::when_registering(k);
    w.send(&["WATCH", k]).await;
    let deadline = Instant::now() + WITHIN;
    while !pause.reached() {
        assert!(Instant::now() < deadline, "WATCH never registered");
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    if expire {
        o.send(&["SET", k, "v", "PX", "60"]).await;
    } else {
        o.send(&["SET", k, "v"]).await;
    }
    while !live(&shared.store, k) {
        assert!(Instant::now() < deadline, "the SET never reached the store");
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    drop(pause);
    assert_eq!(
        tokio::time::timeout(WITHIN, w.reply())
            .await
            .expect("WATCH"),
        ok()
    );
    assert_eq!(
        tokio::time::timeout(WITHIN, o.reply()).await.expect("SET"),
        ok()
    );
    if expire {
        expired(&shared.store, k).await;
    } else {
        assert_eq!(o.cmd(&["DEL", k]).await, Value::Integer(1));
    }
    assert!(
        !commits(&mut w, k).await,
        "a write during WATCH's registration was lost"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_write_during_registration_then_a_del_aborts() {
    a_write_during_registration(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_write_during_registration_then_expiry_aborts() {
    a_write_during_registration(true).await;
}

/// An `*STORE` with an empty result into a key absent at WATCH stores
/// nothing: no change, as in Redis (review round 2, R2). Into a key live at
/// WATCH it deletes the key: a change.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_empty_store_is_a_removal() {
    let shared = Shared::new(false);
    let (mut w, mut o) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let (absent, live_key) = ("6oe:wx:store:absent", "6oe:wx:store:live");
    o.cmd(&["SADD", "6oe:wx:store:a", "x"]).await;
    o.cmd(&["SET", live_key, "v"]).await;
    assert_eq!(w.cmd(&["WATCH", absent]).await, ok());
    assert_eq!(
        o.cmd(&[
            "SINTERSTORE",
            absent,
            "6oe:wx:store:a",
            "6oe:wx:store:missing"
        ])
        .await,
        Value::Integer(0)
    );
    assert!(
        commits(&mut w, absent).await,
        "an empty STORE into an absent key"
    );
    assert_eq!(w.cmd(&["WATCH", live_key]).await, ok());
    assert_eq!(
        o.cmd(&["SUNIONSTORE", live_key, "6oe:wx:store:missing"])
            .await,
        Value::Integer(0)
    );
    assert!(
        !commits(&mut w, live_key).await,
        "an empty STORE deleted a live key"
    );
}
