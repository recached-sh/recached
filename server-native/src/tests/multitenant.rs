//! Adversarial multi-tenant isolation tests (v0.6 Phase 6A).
//!
//! Every test runs real WebSocket connections against a real server, with two
//! independent tenants — `tenant:a:*` and `tenant:b:*` — and asserts that one
//! cannot read, write, subscribe to, or be pushed the other's data, along each
//! path data can leave the server: direct replies, mutation pushes, live-query
//! snapshots and keychanges, WATCH notifications, pub/sub, transactions, DEDUP
//! replay, token renewal and expiry.
//!
//! Tests marked "regression" fail against v0.5.1 (verified by running them on
//! the baseline build `7cdbd0d`); the rest pin behaviour that already held.

use super::*;

const SECRET: &str = "multitenant-test-secret-0123456789abcdef";

/// `payload` is the token body exactly as minted: `entries|exp|claims`.
fn token(payload: &str) -> String {
    mint_sync_token(SECRET, payload)
}

fn in_secs(n: u64) -> u64 {
    unix_now_secs() + n
}

/// A token for tenant `t` (`a` or `b`) with subject `sub`, valid an hour.
fn tenant_token(t: &str, sub: &str) -> String {
    token(&format!(
        "rw=tenant:{t}:*,chrw=tenant:{t}:*|{}|sub={sub}",
        in_secs(3600)
    ))
}

async fn strict_server() -> TestServer {
    spawn_ws_server_policy(SyncPolicy::strict(SECRET), None, None).await
}

/// A connection scoped as tenant `t`.
async fn tenant(server: &TestServer, t: &str, sub: &str) -> WsClient {
    let mut c = WsClient::connect(server.tcp_addr).await;
    let reply = c.cmd(&["SYNC", "TOKEN", &tenant_token(t, sub)]).await;
    assert!(
        matches!(reply, Value::Array(Some(_))),
        "token refused: {reply:?}"
    );
    c
}

fn is_noscope(v: &Value) -> bool {
    matches!(v, Value::Error(e) if e.starts_with("NOSCOPE"))
}

impl WsClient {
    /// Read frames for up to `ms` until the server closes the socket, and
    /// return the close code it sent. `None` if it stayed open.
    pub(super) async fn recv_close(&mut self, ms: u64) -> Option<u16> {
        let fut = async {
            loop {
                match self.ws.next().await {
                    Some(Ok(Message::Close(Some(frame)))) => return Some(u16::from(frame.code)),
                    Some(Ok(Message::Close(None))) | None | Some(Err(_)) => return Some(0),
                    Some(Ok(_)) => continue,
                }
            }
        };
        tokio::time::timeout(tokio::time::Duration::from_millis(ms), fut)
            .await
            .ok()
            .flatten()
    }

    /// Every text frame received within `ms`, concatenated.
    pub(super) async fn drain_text(&mut self, ms: u64) -> String {
        let mut out = String::new();
        while let Some(frame) = self.recv_any(ms).await {
            out.push_str(&frame);
        }
        out
    }
}

// ── Direct access ───────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tenants_cannot_read_or_write_each_others_keys() {
    let server = strict_server().await;
    let mut a = tenant(&server, "a", "alice").await;
    let mut b = tenant(&server, "b", "bob").await;

    assert_eq!(a.cmd(&["SET", "tenant:a:secret", "s3"]).await, ok());
    assert!(is_noscope(&b.cmd(&["GET", "tenant:a:secret"]).await));
    assert!(is_noscope(&b.cmd(&["SET", "tenant:a:secret", "x"]).await));
    assert!(is_noscope(&b.cmd(&["DEL", "tenant:a:secret"]).await));
    // Multi-key commands need every key: one foreign key refuses the lot.
    assert!(is_noscope(
        &b.cmd(&["MGET", "tenant:b:own", "tenant:a:secret"]).await
    ));
    assert!(is_noscope(
        &b.cmd(&["RENAME", "tenant:a:secret", "tenant:b:stolen"])
            .await
    ));
    assert!(is_noscope(
        &b.cmd(&["SUNIONSTORE", "tenant:b:out", "tenant:a:secret"])
            .await
    ));
    // Keyspace-wide reads are refused outright.
    for cmd in [
        &["KEYS", "*"][..],
        &["SCAN", "0"],
        &["DBSIZE"],
        &["INFO"],
        &["CLIENT", "LIST"],
        &["PUBSUB", "CHANNELS"],
    ] {
        assert!(is_noscope(&b.cmd(cmd).await), "{cmd:?} was allowed");
    }
    assert_eq!(a.cmd(&["GET", "tenant:a:secret"]).await, bulk("s3"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefix_grants_stop_at_the_namespace_boundary() {
    // `tenant:a:*` must not reach `tenant:ab:*` — the delimiter is part of the
    // grant. And a grant written without one is a different, wider grant.
    let server = strict_server().await;
    let mut a = tenant(&server, "a", "alice").await;
    assert!(is_noscope(&a.cmd(&["SET", "tenant:ab:x", "1"]).await));
    assert!(is_noscope(&a.cmd(&["GET", "tenant:a"]).await));
    // Live queries must sit inside the grant, not merely match it.
    for pattern in ["*", "tenant:*", "tenant:a*", "tenant:?:*", "tenant:a:*:*x"] {
        let reply = a.cmd(&["QSUB", pattern]).await;
        if pattern == "tenant:a:*:*x" {
            assert!(
                matches!(reply, Value::Array(Some(_))),
                "{pattern}: {reply:?}"
            );
        } else {
            assert!(is_noscope(&reply), "QSUB {pattern} was allowed: {reply:?}");
        }
    }
}

// ── Pushes and live queries ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mutations_and_snapshots_stay_inside_the_tenant() {
    let server = strict_server().await;
    let mut a = tenant(&server, "a", "alice").await;
    let mut b = tenant(&server, "b", "bob").await;
    let mut b_live = tenant(&server, "b", "bob").await;
    assert!(matches!(
        b_live.cmd(&["QSUB", "tenant:b:*"]).await,
        Value::Array(Some(_))
    ));

    a.cmd(&["SET", "tenant:a:k", "A-ONLY"]).await;
    a.cmd(&["HSET", "tenant:a:h", "f", "A-ONLY"]).await;
    a.cmd(&["EXPIRE", "tenant:a:k", "100"]).await;
    a.cmd(&["DEL", "tenant:a:h"]).await;
    let leaked = format!(
        "{}{}",
        b.drain_text(200).await,
        b_live.drain_text(200).await
    );
    assert!(
        !leaked.contains("A-ONLY") && !leaked.contains("tenant:a"),
        "{leaked}"
    );

    // A fresh snapshot of B's own pattern holds none of A's keys.
    let snap = format!("{:?}", b.cmd(&["QSUB", "tenant:b:*"]).await);
    assert!(!snap.contains("tenant:a"), "{snap}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_keyless_push_reaches_scoped_connections_only_when_keyspace_wide() {
    // Regression: an empty key list used to mean "visible to every scope".
    // A write missing from `primary_keys` would then have pushed its value to
    // every tenant. Only FLUSHDB may be keyless now.
    let server = strict_server().await;
    let mut b = tenant(&server, "b", "bob").await;
    let push = |keyspace_wide: bool, marker: &str| SyncPush {
        origin: 0,
        keys: Vec::new(),
        resp: resp_push(&[b"set", b"tenant:a:k", marker.as_bytes()]),
        keyspace_wide,
    };
    let leaked = SyncPush::for_command(
        0,
        &Command::Set("tenant:a:k".into(), "x".into(), SetOptions::default()),
        Vec::new(),
        Vec::new(),
    );
    assert!(!leaked.keyspace_wide, "SET must never be keyspace-wide");
    assert!(!forwards_push(
        &push(false, "LEAK"),
        7,
        Some(&[Grant::rw("tenant:b:*")]),
        true,
        &HashSet::new(),
        &HashSet::new(),
    ));
    assert!(forwards_push(
        &push(true, "FLUSH"),
        7,
        Some(&[Grant::rw("tenant:b:*")]),
        true,
        &HashSet::new(),
        &HashSet::new(),
    ));
    drop(b.drain_text(50).await);
    let flush = SyncPush::for_command(
        0,
        &Command::FlushDb,
        primary_keys(&Command::FlushDb),
        Vec::new(),
    );
    assert!(flush.keyspace_wide && flush.keys.is_empty());
}

#[test]
fn every_write_names_its_keys_or_is_keyspace_wide() {
    // The fail-closed filter above makes a write missing from
    // `primary_keys` invisible to scoped connections rather than visible to
    // all of them — but invisible is still wrong. This catches the omission
    // at the point it is made.
    for (cmd, _) in all_commands() {
        if !is_write_command(&cmd) {
            continue;
        }
        let push = SyncPush::for_command(0, &cmd, primary_keys(&cmd), Vec::new());
        assert!(
            !push.keys.is_empty() || push.keyspace_wide,
            "{cmd:?} is a write that names no key: add it to primary_keys"
        );
    }
}

// ── Pub/Sub ─────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pubsub_channels_are_authorized_by_channel_grants() {
    // Regression: on v0.5.1 any strict-mode socket — even one that never
    // presented a token — could SUBSCRIBE or PSUBSCRIBE to any channel and
    // PUBLISH on any channel.
    let server = strict_server().await;

    let mut anon = WsClient::connect(server.tcp_addr).await;
    for cmd in [
        &["SUBSCRIBE", "tenant:a:events"][..],
        &["PSUBSCRIBE", "*"],
        &["PUBLISH", "tenant:a:events", "forged"],
    ] {
        let reply = anon.cmd(cmd).await;
        assert!(
            matches!(&reply, Value::Error(e) if e.contains("send SYNC TOKEN")),
            "anonymous {cmd:?}: {reply:?}"
        );
    }

    let mut a = tenant(&server, "a", "alice").await;
    let mut a_pub = tenant(&server, "a", "alice").await;
    let mut b = tenant(&server, "b", "bob").await;
    assert!(is_noscope(&b.cmd(&["SUBSCRIBE", "tenant:a:events"]).await));
    assert!(is_noscope(&b.cmd(&["PSUBSCRIBE", "tenant:*"]).await));
    assert!(is_noscope(&b.cmd(&["PSUBSCRIBE", "*"]).await));
    assert!(is_noscope(
        &b.cmd(&["PUBLISH", "tenant:a:events", "forged"]).await
    ));

    // Inside the grant it works, and B sees nothing of A's channel.
    assert!(matches!(
        a.cmd(&["SUBSCRIBE", "tenant:a:events"]).await,
        Value::Array(Some(_)) | Value::Push(_)
    ));
    assert!(matches!(
        b.cmd(&["PSUBSCRIBE", "tenant:b:*"]).await,
        Value::Array(Some(_)) | Value::Push(_)
    ));
    assert_eq!(
        a_pub
            .cmd(&["PUBLISH", "tenant:a:events", "A-PRIVATE"])
            .await,
        int(1)
    );
    assert!(a.drain_text(200).await.contains("A-PRIVATE"));
    assert!(!b.drain_text(200).await.contains("A-PRIVATE"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_public_broadcast_channel_is_granted_explicitly_and_subscribe_only() {
    let server = strict_server().await;
    let tok = token(&format!(
        "rw=tenant:b:*,chr=announcements|{}|sub=bob",
        in_secs(60)
    ));
    let mut b = WsClient::connect(server.tcp_addr).await;
    b.cmd(&["SYNC", "TOKEN", &tok]).await;
    assert!(matches!(
        b.cmd(&["SUBSCRIBE", "announcements"]).await,
        Value::Array(Some(_)) | Value::Push(_)
    ));
    let reply = b.cmd(&["PUBLISH", "announcements", "spoof"]).await;
    assert!(
        matches!(&reply, Value::Error(e) if e.contains("subscribe-only")),
        "{reply:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn open_pubsub_keeps_the_pre_0_6_behaviour_when_asked_for() {
    let policy = SyncPolicy {
        pubsub: PubSubPolicy::Open,
        ..SyncPolicy::strict(SECRET)
    };
    let server = spawn_ws_server_policy(policy, None, None).await;
    let mut anon = WsClient::connect(server.tcp_addr).await;
    assert!(matches!(
        anon.cmd(&["SUBSCRIBE", "anything"]).await,
        Value::Array(Some(_)) | Value::Push(_)
    ));
}

// ── Transactions and WATCH ──────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_rechecks_queued_commands_against_the_token_now_in_force() {
    // Regression: commands were scope-checked only when queued, so a client
    // could queue `GET tenant:a:secret` under tenant A's token, swap to
    // tenant B's, and EXEC would still read A's key.
    let server = strict_server().await;
    let mut a = tenant(&server, "a", "alice").await;
    a.cmd(&["SET", "tenant:a:secret", "A-ONLY"]).await;

    assert_eq!(a.cmd(&["MULTI"]).await, ok());
    assert_eq!(
        a.cmd(&["GET", "tenant:a:secret"]).await,
        Value::SimpleString("QUEUED".into())
    );
    // Same subject, holds no live query/watch/channel: nothing is revoked,
    // so the socket stays open and the swap is accepted.
    let swap = token(&format!("rw=tenant:b:*|{}|sub=alice", in_secs(60)));
    assert!(matches!(
        a.cmd(&["SYNC", "TOKEN", &swap]).await,
        Value::Array(Some(_))
    ));
    let reply = a.cmd(&["EXEC"]).await;
    assert!(
        matches!(&reply, Value::Error(e) if e.starts_with("EXECABORT")),
        "EXEC under the new token returned {reply:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn watch_on_another_tenants_key_is_refused() {
    let server = strict_server().await;
    let mut b = tenant(&server, "b", "bob").await;
    assert!(is_noscope(&b.cmd(&["WATCH", "tenant:a:balance"]).await));
}

// ── Token lifecycle ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn narrowing_the_token_closes_a_connection_that_held_the_old_scope() {
    // Regression: v0.5.1 replaced the grants but kept the live query (and any
    // WATCH) registered, so after a user switched from tenant A to tenant B
    // the socket kept receiving tenant A's keychanges, values included.
    let server = strict_server().await;
    let mut writer = tenant(&server, "a", "writer").await;
    let mut c = tenant(&server, "a", "alice").await;
    assert!(matches!(
        c.cmd(&["QSUB", "tenant:a:*"]).await,
        Value::Array(Some(_))
    ));
    assert!(matches!(
        c.cmd(&["SYNC", "TOKEN", &tenant_token("b", "alice")]).await,
        Value::Array(Some(_))
    ));
    writer.cmd(&["SET", "tenant:a:after", "A-AFTER-SWAP"]).await;
    let mut seen = String::new();
    let read = async {
        loop {
            match c.ws.next().await {
                Some(Ok(Message::Close(frame))) => break frame.map(|f| u16::from(f.code)),
                Some(Ok(Message::Text(t))) => seen.push_str(&t),
                Some(Ok(_)) => continue,
                _ => break None,
            }
        }
    };
    // Bounded: a connection that stays open is the failure being tested for.
    let code = tokio::time::timeout(tokio::time::Duration::from_secs(2), read)
        .await
        .unwrap_or(None);
    assert_eq!(code, Some(WS_CLOSE_SCOPE_CHANGED));
    assert!(!seen.contains("A-AFTER-SWAP"), "old scope leaked: {seen}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_watch_outside_the_new_token_also_forces_resynchronisation() {
    let server = strict_server().await;
    let mut c = tenant(&server, "a", "alice").await;
    assert_eq!(c.cmd(&["WATCH", "tenant:a:balance"]).await, ok());
    c.cmd(&["SYNC", "TOKEN", &tenant_token("b", "alice")]).await;
    assert_eq!(c.recv_close(1_000).await, Some(WS_CLOSE_SCOPE_CHANGED));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_subject_on_the_same_socket_forces_resynchronisation() {
    let server = strict_server().await;
    let mut c = tenant(&server, "a", "alice").await;
    c.cmd(&["SYNC", "TOKEN", &tenant_token("a", "mallory")])
        .await;
    assert_eq!(c.recv_close(1_000).await, Some(WS_CLOSE_SCOPE_CHANGED));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn renewing_with_an_equal_or_wider_token_keeps_the_connection() {
    let server = strict_server().await;
    let mut writer = tenant(&server, "a", "writer").await;
    let mut c = tenant(&server, "a", "alice").await;
    assert!(matches!(
        c.cmd(&["QSUB", "tenant:a:*"]).await,
        Value::Array(Some(_))
    ));
    let wider = token(&format!(
        "rw=tenant:a:*,r=catalog:*|{}|sub=alice",
        in_secs(3600)
    ));
    assert!(matches!(
        c.cmd(&["SYNC", "TOKEN", &wider]).await,
        Value::Array(Some(_))
    ));
    writer.cmd(&["SET", "tenant:a:k", "still-live"]).await;
    let got = c.recv_keychange(1_000).await;
    assert_eq!(got.map(|(k, _)| k).as_deref(), Some("tenant:a:k"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_expired_token_closes_the_connection_and_stops_delivery() {
    // Regression: expiry was checked only when the token was presented, so a
    // connection kept its scopes, and kept receiving pushes, forever.
    let server = strict_server().await;
    let mut writer = tenant(&server, "a", "writer").await;
    let mut c = WsClient::connect(server.tcp_addr).await;
    let short = token(&format!("rw=tenant:a:*|{}|sub=alice", in_secs(2)));
    assert!(matches!(
        c.cmd(&["SYNC", "TOKEN", &short]).await,
        Value::Array(Some(_))
    ));
    assert!(matches!(
        c.cmd(&["QSUB", "tenant:a:*"]).await,
        Value::Array(Some(_))
    ));
    // Within the lifetime: delivered.
    writer.cmd(&["SET", "tenant:a:k", "before"]).await;
    assert!(c.recv_keychange(1_000).await.is_some());
    // The token runs out somewhere in the next ~2 s; the socket must close.
    let code = c.recv_close(4_000).await;
    assert_eq!(code, Some(WS_CLOSE_TOKEN_EXPIRED));
    writer.cmd(&["SET", "tenant:a:k", "AFTER-EXPIRY"]).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unrepresentably_distant_expiry_does_not_panic_the_connection() {
    // The deadline is `now + (exp - now)` and `exp` comes from the token. On
    // Linux and macOS the largest possible value still fits the clock, so this
    // pins behaviour (the socket stays usable) rather than a reproduced panic.
    let server = strict_server().await;
    let mut c = WsClient::connect(server.tcp_addr).await;
    let far = token(&format!("rw=tenant:a:*|{}|sub=alice", u64::MAX));
    assert!(matches!(
        c.cmd(&["SYNC", "TOKEN", &far]).await,
        Value::Array(Some(_))
    ));
    assert_eq!(c.cmd(&["SET", "tenant:a:k", "v"]).await, ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn renewal_before_expiry_extends_the_connection() {
    let server = strict_server().await;
    let mut c = WsClient::connect(server.tcp_addr).await;
    let short = token(&format!("rw=tenant:a:*|{}|sub=alice", in_secs(2)));
    c.cmd(&["SYNC", "TOKEN", &short]).await;
    assert!(matches!(
        c.cmd(&["SYNC", "TOKEN", &tenant_token("a", "alice")]).await,
        Value::Array(Some(_))
    ));
    // Past the first token's expiry the socket is still open and scoped.
    tokio::time::sleep(tokio::time::Duration::from_millis(3_100)).await;
    assert_eq!(c.cmd(&["SET", "tenant:a:k", "v"]).await, ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconnecting_with_an_expired_token_is_refused_and_writes_are_deferred() {
    let server = strict_server().await;
    let mut c = WsClient::connect(server.tcp_addr).await;
    let expired = token(&format!("rw=tenant:a:*|{}|sub=alice", unix_now_secs()));
    let reply = c.cmd(&["SYNC", "TOKEN", &expired]).await;
    assert!(
        matches!(&reply, Value::Error(e) if e.contains("token expired")),
        "{reply:?}"
    );
    // The client's queued write meets the transient refusal, so it is kept
    // and replayed once the application supplies a fresh token.
    let reply = c
        .cmd(&["DEDUP", "c-1", "1", "SET", "tenant:a:k", "v"])
        .await;
    let Value::Error(e) = &reply else {
        panic!("{reply:?}")
    };
    assert!(core_engine::resp::is_transient_refusal(e), "{e}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_bad_tokens_disconnect_the_socket() {
    let server = strict_server().await;
    let mut c = WsClient::connect(server.tcp_addr).await;
    let forged = mint_sync_token("not-the-secret", "rw=*");
    for _ in 0..(MAX_AUTH_FAILURES - 1) {
        assert!(matches!(
            c.cmd(&["SYNC", "TOKEN", &forged]).await,
            Value::Error(_)
        ));
    }
    c.ws.send(Message::Text(
        format!(
            "*3\r\n$4\r\nSYNC\r\n$5\r\nTOKEN\r\n${}\r\n{}\r\n",
            forged.len(),
            forged
        )
        .into(),
    ))
    .await
    .unwrap();
    assert!(c.recv_close(1_000).await.is_some(), "socket stayed open");
}

// ── DEDUP replay ────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dedup_state_is_namespaced_by_token_subject() {
    // Regression: DEDUP client ids are client-chosen and were global. A second
    // principal reusing (or learning) a client id could read that client's
    // remembered refusals — which name the refused key — and could raise its
    // high-water mark so the victim's next writes were answered +DUP and
    // silently dropped.
    let server = strict_server().await;
    let mut alice = tenant(&server, "a", "alice").await;
    let mut bob = tenant(&server, "b", "bob").await;

    // Alice: one applied write (id 5) and one permanent refusal (id 6).
    assert_eq!(
        alice
            .cmd(&["DEDUP", "shared-id", "5", "INCR", "tenant:a:n"])
            .await,
        int(1)
    );
    let refused = alice
        .cmd(&[
            "DEDUP",
            "shared-id",
            "6",
            "SET",
            "outside:secret-key-name",
            "x",
        ])
        .await;
    assert!(is_noscope(&refused));

    // Bob reuses Alice's client id. His id 6 must execute as his own write,
    // not replay Alice's refusal (which echoed her key name).
    let reply = bob
        .cmd(&["DEDUP", "shared-id", "6", "SET", "tenant:b:k", "v"])
        .await;
    assert_eq!(
        reply,
        ok(),
        "bob was answered from alice's state: {reply:?}"
    );

    // Bob raises "shared-id"'s mark far ahead; Alice's next write still runs.
    bob.cmd(&["DEDUP", "shared-id", "1000000", "SET", "tenant:b:k", "v"])
        .await;
    assert_eq!(
        alice
            .cmd(&["DEDUP", "shared-id", "7", "INCR", "tenant:a:n"])
            .await,
        int(2),
        "alice's write was suppressed by bob's mark"
    );
    // Alice's own retry of id 5 is still a duplicate, across a renewed token.
    let renewed = tenant_token("a", "alice");
    alice.cmd(&["SYNC", "TOKEN", &renewed]).await;
    assert_eq!(
        alice
            .cmd(&["DEDUP", "shared-id", "5", "INCR", "tenant:a:n"])
            .await,
        Value::SimpleString("DUP".into())
    );
}

#[test]
fn namespaced_dedup_ids_are_stable_short_and_distinct() {
    let a = namespaced_dedup_client("alice", "c-1");
    assert_eq!(a, namespaced_dedup_client("alice", "c-1"));
    assert_ne!(a, namespaced_dedup_client("bob", "c-1"));
    assert_ne!(a, namespaced_dedup_client("alice", "c-2"));
    // The subject/client boundary is unambiguous.
    assert_ne!(
        namespaced_dedup_client("ab", "c"),
        namespaced_dedup_client("a", "bc")
    );
    assert_eq!(a.len(), 41);
    assert!(a.starts_with('~') && a[1..].bytes().all(|b| b.is_ascii_hexdigit()));
    // Fits the DEDUP id limit, so it survives AOF replay's parser.
    assert!(a.len() <= 64);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clients_cannot_send_the_servers_namespaced_dedup_ids() {
    let server = strict_server().await;
    let mut bob = tenant(&server, "b", "bob").await;
    let forged = namespaced_dedup_client("alice", "c");
    let reply = bob
        .cmd(&["DEDUP", &forged, "1", "SET", "tenant:b:k", "v"])
        .await;
    assert!(
        matches!(&reply, Value::Error(e) if e.contains("reserved")),
        "{reply:?}"
    );
}

// ── Token-as-credential ─────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_auth_lets_a_browser_connect_without_the_resp_password() {
    let policy = SyncPolicy {
        token_auth: true,
        ..SyncPolicy::strict(SECRET)
    };
    let server = spawn_ws_server_policy(policy, None, Some("resp-admin-password")).await;
    let mut c = WsClient::connect(server.tcp_addr).await;
    // Before a token: connection-describing commands work, server details and
    // data do not.
    assert!(matches!(
        c.cmd(&["HELLO", "3"]).await,
        Value::Error(e) if e.starts_with("NOAUTH")
    ));
    assert!(matches!(
        c.cmd(&["GET", "tenant:a:k"]).await,
        Value::Error(e) if e.contains("send SYNC TOKEN")
    ));
    assert!(matches!(
        c.cmd(&["SYNC", "TOKEN", &tenant_token("a", "alice")]).await,
        Value::Array(Some(_))
    ));
    assert_eq!(c.cmd(&["SET", "tenant:a:k", "v"]).await, ok());
    assert!(is_noscope(&c.cmd(&["GET", "tenant:b:k"]).await));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn password_auth_still_gates_the_socket_by_default() {
    let server = spawn_ws_server_policy(
        SyncPolicy::strict(SECRET),
        None,
        Some("resp-admin-password"),
    )
    .await;
    let mut c = WsClient::connect(server.tcp_addr).await;
    assert!(matches!(
        c.cmd(&["SYNC", "TOKEN", &tenant_token("a", "alice")]).await,
        Value::Error(e) if e.starts_with("NOAUTH")
    ));
}

// ── Token contract ──────────────────────────────────────────────────────────

#[test]
fn token_claims_and_clock_boundaries() {
    let strict = SyncPolicy::strict(SECRET);
    let now = 1_700_000_000;
    let at = |payload: &str, policy: &SyncPolicy, now: u64| {
        verify_sync_token_at(policy, &token(payload), now)
    };

    // Expiry is exclusive: valid through exp-1, expired at exp.
    assert!(at(&format!("rw=a:*|{now}"), &strict, now - 1).is_ok());
    assert_eq!(
        at(&format!("rw=a:*|{now}"), &strict, now).unwrap_err(),
        "token expired"
    );

    // The v0.5 two-segment form still verifies, with no claims.
    let t = at(&format!("cart:*|{}", now + 10), &strict, now).unwrap();
    assert_eq!(t.subject, None);
    assert_eq!(t.expires_at, Some(now + 10));

    // Claims, channel grants, and an empty expiry ahead of claims.
    let t = at(
        "r=cat:*,chr=news,chrw=room:1:*||sub=u-1;aud=app",
        &strict,
        now,
    )
    .unwrap();
    assert_eq!(t.grants, vec![Grant::ro("cat:*")]);
    assert_eq!(t.channels, vec![Grant::ro("news"), Grant::rw("room:1:*")]);
    assert_eq!(t.subject.as_deref(), Some("u-1"));
    assert_eq!(t.expires_at, None);

    for (payload, why) in [
        ("rw=a:*||bogus=1", "unknown claim"),
        ("rw=a:*||sub=x;sub=y", "repeated claim"),
        ("rw=a:*||sub=has space", "malformed claim value"),
        ("rw=a:*||sub", "malformed claim"),
        ("rw=a:*|soon|sub=x", "malformed expiry"),
        ("chr=", "token grants an empty pattern"),
    ] {
        assert_eq!(at(payload, &strict, now).unwrap_err(), why, "{payload}");
    }

    let demanding = SyncPolicy {
        require_expiry: true,
        require_subject: true,
        max_ttl_secs: Some(3600),
        audience: Some("app".into()),
        ..SyncPolicy::strict(SECRET)
    };
    let good = format!("rw=a:*|{}|sub=u;aud=app", now + 3600);
    assert!(at(&good, &demanding, now).is_ok());
    for (payload, why) in [
        ("rw=a:*||sub=u;aud=app".to_string(), "no expiry"),
        (format!("rw=a:*|{}|aud=app", now + 60), "no sub claim"),
        (format!("rw=a:*|{}|sub=u;aud=app", now + 3601), "exceeds"),
        (format!("rw=a:*|{}|sub=u;aud=other", now + 60), "audience"),
        (format!("rw=a:*|{}|sub=u", now + 60), "audience"),
    ] {
        let err = at(&payload, &demanding, now).unwrap_err();
        assert!(err.contains(why), "{payload}: {err}");
    }
}

#[test]
fn a_v0_6_token_fails_closed_on_the_v0_5_parser() {
    // What a 0.5 server does with a 0.6 three-segment payload: it splits once
    // on '|' and parses the remainder as the expiry, which fails.
    let payload = "rw=a:*|1700000000|sub=u";
    let (_, expiry) = payload.split_once('|').unwrap();
    assert!(expiry.parse::<u64>().is_err());
}
