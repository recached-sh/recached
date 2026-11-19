//! 6N-C PR 0e (RV-12): only a WebSocket connection that is routed something
//! counts as a push receiver.
//!
//! A socket used to be counted from the moment it connected, before AUTH. An
//! unauthenticated client could therefore hold every writer on the server
//! off the unordered fast path — a lock convoy on a cache-only deployment —
//! without being able to read or write anything. Now a socket is counted
//! when it is first routed (AUTH in open mode, a token in strict mode), with
//! the gate closed, and uncounted when it goes away.

use super::*;
use std::time::Duration;

const SECRET: &str = "ws-fastpath-test-secret-0123456789abcdef";

fn token_for(entries: &str) -> String {
    mint_sync_token(SECRET, &format!("{entries}|{}", unix_now_secs() + 3600))
}

/// Whether a plain write from a TCP client would take the unordered path
/// now. The test servers have no watcher, AOF or replication.
fn fast_path(srv: &TestServer) -> bool {
    try_execute_unordered(
        &Command::Incr("fastpath:probe".into()),
        &srv.state,
        &WatchHub::new(),
        &srv.store,
    )
    .is_some()
}

fn receivers(srv: &TestServer) -> usize {
    srv.state.replicas.push_receiver_count()
}

/// The count settles asynchronously after a socket closes (its task has to
/// notice): wait for it.
async fn settles_to(srv: &TestServer, want: usize) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while receivers(srv) != want {
        assert!(
            std::time::Instant::now() < deadline,
            "push receivers stuck at {}, want {want}",
            receivers(srv)
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unauthenticated_socket_leaves_writes_on_the_fast_path() {
    let srv = spawn_ws_server_policy(SyncPolicy::open(), None, Some("pw")).await;
    let mut anon = WsClient::connect(srv.tcp_addr).await;
    // A round trip, so the connection is certainly set up.
    assert!(matches!(
        anon.cmd(&["GET", "x"]).await,
        Value::Error(e) if e.starts_with("NOAUTH")
    ));
    assert_eq!(receivers(&srv), 0, "an unauthenticated socket was counted");
    assert!(
        fast_path(&srv),
        "an unauthenticated socket disabled the fast path"
    );

    // A failed AUTH changes nothing.
    assert!(matches!(anon.cmd(&["AUTH", "nope"]).await, Value::Error(_)));
    assert_eq!(receivers(&srv), 0);

    // Authenticated, it is routed everything: counted, and pushed writes.
    assert_eq!(anon.cmd(&["AUTH", "pw"]).await, ok());
    assert_eq!(receivers(&srv), 1);
    assert!(!fast_path(&srv), "a routed receiver must order writes");
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(writer.cmd(&["AUTH", "pw"]).await, ok());
    assert_eq!(receivers(&srv), 2);
    assert_eq!(writer.cmd(&["SET", "seen", "v"]).await, ok());
    let push = anon
        .recv_push(2_000)
        .await
        .expect("routed socket is pushed");
    assert!(push.contains("seen"), "{push}");

    drop(anon);
    drop(writer);
    settles_to(&srv, 0).await;
    assert!(fast_path(&srv), "disconnect released the accounting");
}

/// Strict mode: authenticated but without a token, a socket is routed
/// nothing and not counted; a token routes and counts it once; a renewal or
/// narrowing does not count it again; a narrowing that revokes held
/// subscriptions closes it and releases the count.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_strict_socket_counts_once_routed_by_a_token() {
    let srv = spawn_ws_server_policy(SyncPolicy::strict(SECRET), None, Some("pw")).await;
    let mut ws = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(ws.cmd(&["AUTH", "pw"]).await, ok());
    assert_eq!(
        receivers(&srv),
        0,
        "authenticated without a token: routed nothing"
    );
    assert!(fast_path(&srv));

    let reply = ws
        .cmd(&["SYNC", "TOKEN", &token_for("rw=tenant:a:*")])
        .await;
    assert!(!matches!(reply, Value::Error(_)), "{reply:?}");
    assert_eq!(receivers(&srv), 1);
    assert!(!fast_path(&srv));

    // Renewal and narrowing re-route without a second count.
    for grants in ["rw=tenant:a:*", "rw=tenant:a:x:*"] {
        let reply = ws.cmd(&["SYNC", "TOKEN", &token_for(grants)]).await;
        assert!(!matches!(reply, Value::Error(_)), "{reply:?}");
        assert_eq!(receivers(&srv), 1, "{grants}: counted twice");
    }
    // The narrowed routes are in force: a write outside them is not pushed,
    // one inside is.
    let mut other = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(other.cmd(&["AUTH", "pw"]).await, ok());
    let reply = other
        .cmd(&["SYNC", "TOKEN", &token_for("rw=tenant:a:*")])
        .await;
    assert!(!matches!(reply, Value::Error(_)), "{reply:?}");
    assert_eq!(receivers(&srv), 2);
    assert_eq!(other.cmd(&["SET", "tenant:a:y:1", "no"]).await, ok());
    assert_eq!(other.cmd(&["SET", "tenant:a:x:1", "yes"]).await, ok());
    let push = ws
        .recv_push(2_000)
        .await
        .expect("pushed inside the narrowed scope");
    assert!(
        push.contains("tenant:a:x:1") && !push.contains("tenant:a:y:1"),
        "{push}"
    );

    // A QSUB the next token does not cover is revoked: closed, uncounted.
    ws.cmd(&["QSUB", "tenant:a:x:*"]).await;
    ws.send(&["SYNC", "TOKEN", &token_for("rw=tenant:b:*")])
        .await;
    let _reply = ws.recv_frame(2_000).await;
    settles_to(&srv, 1).await;
    drop(other);
    settles_to(&srv, 0).await;
}

/// Many sockets connecting, authenticating (or not) and going away at once
/// leave no count behind.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_auth_and_disconnect_leave_no_count_behind() {
    let srv = spawn_ws_server_policy(SyncPolicy::open(), None, Some("pw")).await;
    let addr = srv.tcp_addr;
    let tasks: Vec<_> = (0..40)
        .map(|i| {
            tokio::spawn(async move {
                let mut ws = WsClient::connect(addr).await;
                match i % 4 {
                    // Authenticates and stays a moment.
                    0 => {
                        assert_eq!(ws.cmd(&["AUTH", "pw"]).await, ok());
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                    // Sends AUTH and goes away without waiting for the reply.
                    1 => ws.send(&["AUTH", "pw"]).await,
                    // Fails AUTH.
                    2 => {
                        let _ = ws.cmd(&["AUTH", "wrong"]).await;
                    }
                    // Never authenticates.
                    _ => {}
                }
            })
        })
        .collect();
    for task in tasks {
        task.await.unwrap();
    }
    settles_to(&srv, 0).await;
    assert!(fast_path(&srv));
}
