//! Live-query snapshots after the 6K-A prefix plan (`MatchPlan`).
//!
//! The store now finds a QSUB's initial state from the ordered key index
//! instead of walking the whole keyspace. These tests hold the end-to-end
//! contract that has to survive that change: a client's snapshot plus every
//! keychange after it equals the store's final state for its pattern — no key
//! missed in the snapshot, no write lost between snapshot and registration —
//! while other clients write and delete keys under it; expired keys are not
//! in a snapshot; and a tenant is never shown another tenant's keys.

use super::*;

const SECRET: &str = "live-query-6k-secret-0123456789abcdef";

/// `pattern`'s keys and values as the store holds them now, in the encoding
/// QSUB and keychange frames use.
fn store_state(store: &KeyValueStore, pattern: &str) -> BTreeMap<String, Value> {
    store
        .matching_key_values(pattern, usize::MAX)
        .into_iter()
        .collect()
}

/// A `qstate` reply as a map.
pub(super) fn qstate_map(reply: &Value, pattern: &str) -> BTreeMap<String, Value> {
    let Value::Array(Some(items)) = reply else {
        panic!("QSUB {pattern}: expected qstate, got {reply:?}");
    };
    assert_eq!(items.first(), Some(&bulk("qstate")), "{reply:?}");
    assert_eq!(items.get(1), Some(&bulk(pattern)), "{reply:?}");
    let mut out = BTreeMap::new();
    for pair in items[2..].chunks(2) {
        let [Value::BulkString(Some(key)), value] = pair else {
            panic!("malformed qstate pair: {pair:?}");
        };
        let key = String::from_utf8_lossy(key).into_owned();
        assert!(
            out.insert(key.clone(), value.clone()).is_none(),
            "{key} twice in one snapshot"
        );
    }
    out
}

/// Apply keychanges to `local` until none arrives for `quiet_ms`.
async fn drain_keychanges(
    client: &mut WsClient,
    local: &mut BTreeMap<String, Value>,
    quiet_ms: u64,
) {
    while let Some((key, value)) = client.recv_keychange(quiet_ms).await {
        if value == nil() {
            local.remove(&key);
        } else {
            local.insert(key, value);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn snapshot_plus_keychanges_converge_while_writers_run() {
    let srv = spawn_ws_server().await;
    // Background keyspace the prefix plan must skip, and initial state it
    // must find.
    for i in 0..2_000 {
        srv.store.execute(Command::Set(
            format!("bg:{i}"),
            "x".into(),
            SetOptions::default(),
        ));
    }
    for i in 0..40 {
        srv.store.execute(Command::Set(
            format!("t:a:{i}"),
            "0".into(),
            SetOptions::default(),
        ));
    }

    let stop = Arc::new(AtomicBool::new(false));
    let mut writers = Vec::new();
    for w in 0..3u64 {
        let stop = Arc::clone(&stop);
        let addr = srv.tcp_addr;
        writers.push(tokio::spawn(async move {
            let mut c = WsClient::connect(addr).await;
            let mut i = w;
            while !stop.load(Ordering::Relaxed) {
                let key = format!("t:a:{}", i % 60);
                match i % 7 {
                    0 => {
                        c.cmd(&["DEL", &key]).await;
                    }
                    1 => {
                        // Similar prefix, other tenant: never in t:a:*.
                        c.cmd(&["SET", &format!("t:ab:{}", i % 60), "no"]).await;
                    }
                    2 => {
                        c.cmd(&["APPEND", &key, "+"]).await;
                    }
                    3 | 4 => {
                        c.cmd(&["INCR", &key]).await;
                    }
                    _ => {
                        c.cmd(&["SET", &key, &format!("v{i}")]).await;
                    }
                }
                i += 3;
            }
        }));
    }

    // Observers subscribe at staggered moments, each into the middle of the
    // write stream.
    let mut observers = Vec::new();
    for _ in 0..6 {
        tokio::time::sleep(Duration::from_millis(40)).await;
        let mut client = WsClient::connect(srv.tcp_addr).await;
        let reply = client.cmd(&["QSUB", "t:a:*"]).await;
        let local = qstate_map(&reply, "t:a:*");
        assert!(
            local.keys().all(|k| k.starts_with("t:a:")),
            "snapshot reached past its prefix: {local:?}"
        );
        observers.push((client, local));
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    stop.store(true, Ordering::Relaxed);
    for w in writers {
        w.await.unwrap();
    }
    let expected = store_state(&srv.store, "t:a:*");
    for (n, (mut client, mut local)) in observers.into_iter().enumerate() {
        drain_keychanges(&mut client, &mut local, 400).await;
        assert_eq!(local, expected, "observer {n} diverged from the store");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tenant_snapshot_holds_only_its_own_keys_among_similar_prefixes() {
    let srv = spawn_ws_server_policy(SyncPolicy::strict(SECRET), None, None).await;
    for (key, value) in [
        ("tenant:a", "bare"),
        ("tenant:a:1", "A1"),
        ("tenant:a:book:1", "A-BOOK"),
        ("tenant:ab:1", "AB1"),
        ("tenant:ab:book:1", "AB-BOOK"),
        ("tenant:b:1", "B1"),
        ("tenant:", "root"),
    ] {
        srv.store.execute(Command::Set(
            key.into(),
            value.into(),
            SetOptions::default(),
        ));
    }
    for i in 0..5_000 {
        srv.store.execute(Command::Set(
            format!("tenant:z{i}:k"),
            "Z".into(),
            SetOptions::default(),
        ));
    }
    let mut a = WsClient::connect(srv.tcp_addr).await;
    let tok = mint_sync_token(
        SECRET,
        &format!("rw=tenant:a:*|{}|sub=alice", unix_now_secs() + 3600),
    );
    assert!(matches!(
        a.cmd(&["SYNC", "TOKEN", &tok]).await,
        Value::Array(Some(_))
    ));
    let snap = qstate_map(&a.cmd(&["QSUB", "tenant:a:*"]).await, "tenant:a:*");
    assert_eq!(
        snap.keys().map(String::as_str).collect::<Vec<_>>(),
        ["tenant:a:1", "tenant:a:book:1"]
    );
    let books = qstate_map(
        &a.cmd(&["QSUB", "tenant:a:book:?"]).await,
        "tenant:a:book:?",
    );
    assert_eq!(books.keys().collect::<Vec<_>>(), ["tenant:a:book:1"]);
    let exact = qstate_map(&a.cmd(&["QSUB", "tenant:a:1"]).await, "tenant:a:1");
    assert_eq!(exact.get("tenant:a:1"), Some(&bulk("A1")));
    // Wider than the grant, though its prefix is the grant's: still refused.
    for wider in ["tenant:a*", "tenant:*", "*"] {
        let reply = a.cmd(&["QSUB", wider]).await;
        assert!(
            matches!(&reply, Value::Error(e) if e.starts_with("NOSCOPE")),
            "QSUB {wider} -> {reply:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn snapshot_of_a_pattern_without_a_prefix_is_unchanged() {
    // `*:book` and `?x` cannot use the index; they still walk every key and
    // must return exactly what they did.
    let srv = spawn_ws_server().await;
    for key in ["a:book", "b:book", "a:bookx", "ax", "bx", "xx", "abx"] {
        srv.store
            .execute(Command::Set(key.into(), key.into(), SetOptions::default()));
    }
    let mut c = WsClient::connect(srv.tcp_addr).await;
    for pattern in ["*:book", "?x", "*", "a*"] {
        let snap = qstate_map(&c.cmd(&["QSUB", pattern]).await, pattern);
        assert_eq!(snap, store_state(&srv.store, pattern), "{pattern}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expired_keys_are_absent_from_a_prefix_snapshot() {
    let srv = spawn_ws_server().await;
    let mut c = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(c.cmd(&["SET", "e:live", "v"]).await, ok());
    assert_eq!(c.cmd(&["SET", "e:dead", "v", "PX", "1"]).await, ok());
    tokio::time::sleep(Duration::from_millis(20)).await;
    let mut q = WsClient::connect(srv.tcp_addr).await;
    let snap = qstate_map(&q.cmd(&["QSUB", "e:*"]).await, "e:*");
    assert_eq!(snap.keys().collect::<Vec<_>>(), ["e:live"]);
}

/// One `INFO recached` field, read over the WebSocket.
async fn recached_info(c: &mut WsClient, field: &str) -> u64 {
    let Value::BulkString(Some(body)) = c.cmd(&["INFO", "recached"]).await else {
        panic!("INFO recached did not return a bulk string");
    };
    let body = String::from_utf8_lossy(&body).into_owned();
    body.lines()
        .find_map(|line| line.strip_prefix(&format!("{field}:")))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or_else(|| panic!("{field} missing from {body}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_write_checks_only_the_live_queries_that_could_match_it() {
    // 6K-B: with ~1,000 tenant live queries registered, a write to an
    // unrelated key checks none of them and a write to one tenant's key
    // checks that tenant's pattern only — and is still delivered.
    let srv = spawn_ws_server().await;
    let mut subscribers = Vec::new();
    for conn in 0..16 {
        let mut c = WsClient::connect(srv.tcp_addr).await;
        for q in 0..62 {
            let pattern = format!("tenant:{}:*", conn * 62 + q);
            assert!(matches!(
                c.cmd(&["QSUB", &pattern]).await,
                Value::Array(Some(_))
            ));
        }
        subscribers.push(c);
    }
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(recached_info(&mut writer, "live_queries").await, 992);
    assert_eq!(recached_info(&mut writer, "qsub_route_prefixes").await, 992);

    let before = recached_info(&mut writer, "qsub_route_candidates").await;
    assert_eq!(writer.cmd(&["SET", "unrelated:1", "v"]).await, ok());
    assert_eq!(
        recached_info(&mut writer, "qsub_route_candidates").await,
        before,
        "an unrelated write checked live-query patterns"
    );

    assert_eq!(writer.cmd(&["SET", "tenant:70:k", "v"]).await, ok());
    assert_eq!(
        recached_info(&mut writer, "qsub_route_candidates").await,
        before + 1,
        "tenant:70:k is a candidate for tenant:70:* alone (not tenant:7*:)"
    );
    // tenant:70 is connection 1's (62..124).
    let (key, value) = subscribers[1]
        .recv_keychange(1_000)
        .await
        .expect("the matching live query was notified");
    assert_eq!((key.as_str(), value), ("tenant:70:k", bulk("v")));
    assert!(subscribers[0].recv_keychange(200).await.is_none());

    // Unsubscribing unindexes: the same write then checks nothing.
    assert_eq!(subscribers[1].cmd(&["QUNSUB", "tenant:70:*"]).await, ok());
    let before = recached_info(&mut writer, "qsub_route_candidates").await;
    assert_eq!(writer.cmd(&["SET", "tenant:70:k", "w"]).await, ok());
    assert_eq!(
        recached_info(&mut writer, "qsub_route_candidates").await,
        before
    );
    assert_eq!(recached_info(&mut writer, "live_queries").await, 991);
    drop(subscribers);
}
