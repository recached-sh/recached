//! Replica correctness (v0.6 Phase 6L-A).
//!
//! **Full resynchronisation.** The replica replaces its keyspace wholesale.
//! Keys deleted on the primary while the link was down are simply gone: no
//! DEL says so. Every WebSocket client holding a local copy or a live-query
//! snapshot is closed with the sync-gap code so that it rebuilds, and a
//! `WATCH` before the resync aborts the `EXEC` after it. A *partial*
//! resynchronisation replays ordered frames and closes nobody.
//!
//! **Capped replicas.** A replica follows its primary's evictions and makes
//! none of its own, so it never diverges from the primary or refuses a
//! replicated write. Its caps take effect on promotion.

use super::live_query_6k::qstate_map;
use super::replication_dedup::*;
use super::*;
use std::collections::BTreeMap;

const SECRET: &str = "replica-6l-test-secret-0123456789abcdef";

fn token_for(entries: &str) -> String {
    mint_sync_token(SECRET, &format!("{entries}|{}", unix_now_secs() + 3600))
}

/// Seed the primary, then bring the replica up to date over one link that
/// is dropped after `ms`. Returns the resume point, which a later
/// [`follow_for`] given the same value continues from (a partial
/// resynchronisation), or which a fresh `ReplResume::default()` discards (a
/// full one).
async fn first_sync(replica: &Node, primary_addr: std::net::SocketAddr) -> ReplResume {
    let mut resume = ReplResume::default();
    follow_for(replica, primary_addr, &mut resume, 400).await;
    resume
}

fn bulk_map(pairs: &[(&str, &str)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), bulk(v)))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_full_resync_closes_clients_with_the_sync_gap_code_and_they_rebuild() {
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    let addr = serve_replicas(&primary).await;
    let mut p = WsClient::connect(primary.ws).await;
    p.cmd(&["SET", "item:1", "a"]).await;
    p.cmd(&["SET", "item:2", "b"]).await;
    first_sync(&replica, addr).await;
    wait_for(&replica.store, "item:2", bulk("b")).await;

    let mut c = WsClient::connect(replica.ws).await;
    let snap = qstate_map(&c.cmd(&["QSUB", "item:*"]).await, "item:*");
    assert_eq!(snap, bulk_map(&[("item:1", "a"), ("item:2", "b")]));

    // While the link is down: a deletion and a creation the replica has no
    // frames for.
    p.cmd(&["DEL", "item:1"]).await;
    p.cmd(&["SET", "item:3", "c"]).await;

    // A fresh resume point: the primary must send a full snapshot.
    let mut fresh = ReplResume::default();
    let (_, code) = tokio::join!(
        follow_for(&replica, addr, &mut fresh, 600),
        c.recv_close(3_000)
    );
    assert_eq!(
        code,
        Some(WS_CLOSE_SYNC_GAP),
        "a client of a fully resynchronised replica must be told to rebuild"
    );
    assert_eq!(replica.store.get_current("item:1"), Value::BulkString(None));

    // The reconnect rebuilds from the new keyspace.
    let mut again = WsClient::connect(replica.ws).await;
    let snap = qstate_map(&again.cmd(&["QSUB", "item:*"]).await, "item:*");
    assert_eq!(snap, bulk_map(&[("item:2", "b"), ("item:3", "c")]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_partial_resync_keeps_clients_and_delivers_what_they_missed() {
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    let addr = serve_replicas(&primary).await;
    let mut p = WsClient::connect(primary.ws).await;
    p.cmd(&["SET", "item:1", "a"]).await;
    let mut resume = first_sync(&replica, addr).await;
    wait_for(&replica.store, "item:1", bulk("a")).await;

    let mut c = WsClient::connect(replica.ws).await;
    let snap = qstate_map(&c.cmd(&["QSUB", "item:*"]).await, "item:*");
    assert_eq!(snap, bulk_map(&[("item:1", "a")]));

    p.cmd(&["DEL", "item:1"]).await;
    p.cmd(&["SET", "item:2", "b"]).await;
    // The same resume point: ordered frames, not a snapshot.
    follow_for(&replica, addr, &mut resume, 600).await;

    let mut seen = BTreeMap::new();
    while let Some((key, value)) = c.recv_keychange(500).await {
        seen.insert(key, value);
    }
    assert_eq!(
        seen.get("item:1"),
        Some(&Value::BulkString(None)),
        "{seen:?}"
    );
    assert_eq!(seen.get("item:2"), Some(&bulk("b")), "{seen:?}");
    assert_eq!(
        c.recv_close(300).await,
        None,
        "a partial resync closed a client"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tenants_rebuild_only_their_own_keys_after_a_full_resync() {
    // Strict multi-tenant replica: the gap close is the same for every
    // tenant, and each reconnect, under a renewed token, sees only its own
    // keys.
    let primary = node(false, true).await;
    let replica = node_with(true, true, KeyValueStore::new(), SyncPolicy::strict(SECRET)).await;
    let addr = serve_replicas(&primary).await;
    let mut p = WsClient::connect(primary.ws).await;
    for (k, v) in [("t:a:1", "a1"), ("t:b:1", "b1")] {
        p.cmd(&["SET", k, v]).await;
    }
    first_sync(&replica, addr).await;
    wait_for(&replica.store, "t:b:1", bulk("b1")).await;

    let mut tenants = Vec::new();
    for t in ["a", "b"] {
        let mut c = WsClient::connect(replica.ws).await;
        let reply = c
            .cmd(&["SYNC", "TOKEN", &token_for(&format!("t:{t}:*"))])
            .await;
        assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
        let pattern = format!("t:{t}:*");
        qstate_map(&c.cmd(&["QSUB", &pattern]).await, &pattern);
        tenants.push(c);
    }

    p.cmd(&["SET", "t:a:2", "a2"]).await;
    p.cmd(&["DEL", "t:b:1"]).await;
    let mut fresh = ReplResume::default();
    let [mut a, mut b]: [WsClient; 2] = tenants.try_into().ok().unwrap();
    let (_, code_a, code_b) = tokio::join!(
        follow_for(&replica, addr, &mut fresh, 600),
        a.recv_close(3_000),
        b.recv_close(3_000)
    );
    assert_eq!(
        (code_a, code_b),
        (Some(WS_CLOSE_SYNC_GAP), Some(WS_CLOSE_SYNC_GAP))
    );

    for (t, want) in [
        ("a", bulk_map(&[("t:a:1", "a1"), ("t:a:2", "a2")])),
        ("b", BTreeMap::new()),
    ] {
        let mut c = WsClient::connect(replica.ws).await;
        // A renewed token, as a client's reconnect fetches.
        let reply = c
            .cmd(&["SYNC", "TOKEN", &token_for(&format!("t:{t}:*"))])
            .await;
        assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
        let pattern = format!("t:{t}:*");
        let snap = qstate_map(&c.cmd(&["QSUB", &pattern]).await, &pattern);
        assert_eq!(snap, want, "tenant {t}");
        // Out of scope stays out of scope.
        let other = if t == "a" { "t:b:*" } else { "t:a:*" };
        let refused = c.cmd(&["QSUB", other]).await;
        assert!(matches!(refused, Value::Error(_)), "{refused:?}");
    }
}

#[tokio::test]
async fn a_replaced_keyspace_is_announced_to_watchers_with_current_values() {
    // WATCH before a full resync must abort the EXEC after it, as any write
    // to the key would; and the frame carries the key's value as it now
    // stands, never a nil for a key that still exists.
    let store = KeyValueStore::new();
    store.execute(Command::Set(
        "w".into(),
        "new".into(),
        SetOptions::default(),
    ));
    let registry: WatchRegistry = WatchHub::new();
    let (overflow, _overflow_rx) = mpsc::channel(1);
    let (subscriber, mut rx) = notification_channel(7, overflow, Arc::new(AtomicBool::new(false)));
    {
        let mut reg = registry.map.lock().await;
        reg.entry("w".to_string()).or_default().push(subscriber);
        registry.sync_len(&reg);
    }

    notify_keyspace_replaced(&registry, &store).await;
    let notif = rx.try_recv().expect("the watcher is told");
    assert_eq!(notif.key, "w");
    let frame = encode_notification(&notif, false);
    assert!(
        frame.windows(3).any(|w| w == b"new"),
        "the current value, not a nil: {:?}",
        String::from_utf8_lossy(&frame)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_capped_replica_follows_its_primary_instead_of_evicting() {
    // Primary uncapped, replica capped at 2 keys: before 6L the replica
    // evicted on its own, an INCR of an evicted key restarted from 0 there
    // alone, and its live queries were never told.
    let primary = node(false, true).await;
    let replica = node_with(
        true,
        true,
        KeyValueStore::with_config(Some(2), None, EvictionPolicy::AllKeysRandom),
        SyncPolicy::open(),
    )
    .await;
    follow(&replica, serve_replicas(&primary).await);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut q = WsClient::connect(replica.ws).await;
    qstate_map(&q.cmd(&["QSUB", "k:*"]).await, "k:*");

    let mut p = WsClient::connect(primary.ws).await;
    for i in 0..5 {
        p.cmd(&["SET", &format!("k:{i}"), "v"]).await;
    }
    for _ in 0..3 {
        p.cmd(&["INCR", "k:n"]).await;
    }
    caught_up(&mut p, &replica, "t1").await;

    for i in 0..5 {
        assert_eq!(replica.store.get_current(&format!("k:{i}")), bulk("v"));
    }
    assert_eq!(replica.store.get_current("k:n"), bulk("3"));
    assert_eq!(replica.store.evicted_count(), 0);
    while let Some((key, value)) = q.recv_keychange(300).await {
        assert_ne!(value, Value::BulkString(None), "{key} reported deleted");
    }

    // Promotion turns the caps back on: the next new key makes room.
    replica.state.promote_to_primary(&replica.store);
    let mut r = WsClient::connect(replica.ws).await;
    assert_eq!(r.cmd(&["SET", "after", "x"]).await, ok());
    assert_eq!(r.cmd(&["DBSIZE"]).await, int(2));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_noeviction_replica_never_refuses_a_replicated_write() {
    // Before 6L the refusal failed the link, and the full resync that
    // followed hit the same refusal: the replica never caught up.
    let primary = node(false, true).await;
    let replica = node_with(
        true,
        true,
        KeyValueStore::with_config(None, Some(256), EvictionPolicy::NoEviction),
        SyncPolicy::open(),
    )
    .await;
    follow(&replica, serve_replicas(&primary).await);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut p = WsClient::connect(primary.ws).await;
    let big = "x".repeat(4_096);
    assert_eq!(p.cmd(&["SET", "big", &big]).await, ok());
    caught_up(&mut p, &replica, "t1").await;
    assert_eq!(replica.store.get_current("big"), bulk(&big));
}
