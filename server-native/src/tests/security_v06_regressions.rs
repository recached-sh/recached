//! Independent security regression review of v0.6 (fail-closed multi-tenant
//! sync, the 0.6 Redis commands).
//!
//! Complements `multitenant.rs` rather than repeating it: these cover token
//! replacement against pub/sub registrations and queued writes/publishes,
//! DEDUP retries after a write downgrade, scope classification and push
//! frames for every command added in 0.6, QSUB containment, and what a
//! socket learns before it presents a token.
//!
//! Tests marked "Regression" failed on the code they were written against
//! (`63f4e8e`, dev + the v0.6 CI fixes) and pass with the fix; the rest pin
//! behaviour that already held.

use super::*;

const SECRET: &str = "v06-regression-secret-0123456789abcdef";

fn token(payload: &str) -> String {
    mint_sync_token(SECRET, payload)
}

fn in_secs(n: u64) -> u64 {
    unix_now_secs() + n
}

async fn strict_server() -> TestServer {
    spawn_ws_server_policy(SyncPolicy::strict(SECRET), None, None).await
}

/// A socket scoped by the token body `entries` (an hour, subject `sub`).
async fn scoped(server: &TestServer, entries: &str, sub: &str) -> WsClient {
    let mut c = WsClient::connect(server.tcp_addr).await;
    let tok = token(&format!("{entries}|{}|sub={sub}", in_secs(3600)));
    let reply = c.cmd(&["SYNC", "TOKEN", &tok]).await;
    assert!(
        matches!(reply, Value::Array(Some(_))),
        "token refused: {reply:?}"
    );
    c
}

fn noscope(v: &Value) -> bool {
    matches!(v, Value::Error(e) if e.starts_with("NOSCOPE"))
}

fn read_only(v: &Value) -> bool {
    matches!(v, Value::Error(e) if e.starts_with("NOSCOPE") && e.contains("read-only"))
}

// ── 3. Keyless pushes and classification of the 0.6 commands ─────────────

#[test]
fn v06_commands_are_scoped_with_the_access_they_need() {
    use CommandScope::{KeyLess, Keys};
    let k = |s: &str| s.to_string();
    let cases: Vec<(Command, Vec<(String, Access)>)> = vec![
        (Command::GetDel(k("a")), vec![(k("a"), Access::Write)]),
        (Command::GetEx(k("a"), None), vec![(k("a"), Access::Write)]),
        (Command::ExpireTime(k("a")), vec![(k("a"), Access::Read)]),
        (Command::PExpireTime(k("a")), vec![(k("a"), Access::Read)]),
        (
            Command::HStrLen(k("a"), k("f")),
            vec![(k("a"), Access::Read)],
        ),
        (
            Command::ZPopMin(k("a"), None),
            vec![(k("a"), Access::Write)],
        ),
        (
            Command::ZPopMax(k("a"), Some(2)),
            vec![(k("a"), Access::Write)],
        ),
        (
            Command::RenameNx(k("a"), k("b")),
            vec![(k("a"), Access::Write), (k("b"), Access::Write)],
        ),
        (
            Command::Copy(k("a"), k("b"), true),
            vec![(k("a"), Access::Read), (k("b"), Access::Write)],
        ),
    ];
    for (cmd, want) in cases {
        match command_scope(&cmd) {
            Keys(got) => assert_eq!(got, want, "{cmd:?}"),
            other => panic!("{cmd:?} scoped as {other:?}"),
        }
        // Wrapped in DEDUP, the same requirement applies.
        let wrapped = Command::Dedup("c".into(), 1, Box::new(cmd.clone()));
        assert!(matches!(command_scope(&wrapped), Keys(_)), "{wrapped:?}");
        // No 0.6 write is keyspace-wide: each push is filtered by its keys.
        let push = SyncPush::for_command(0, &cmd, primary_keys(&cmd), Vec::new());
        assert!(!push.keyspace_wide, "{cmd:?}");
    }
    assert!(matches!(
        command_scope(&Command::Echo(b"x".to_vec())),
        KeyLess
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v06_commands_refuse_another_tenants_keys() {
    let server = strict_server().await;
    let mut a = scoped(&server, "rw=tenant:a:*", "alice").await;
    let mut b = scoped(&server, "rw=tenant:b:*,r=catalog:*", "bob").await;
    a.cmd(&["SET", "tenant:a:s", "A-SECRET"]).await;
    a.cmd(&["HSET", "tenant:a:h", "f", "A-SECRET"]).await;
    a.cmd(&["ZADD", "tenant:a:z", "1", "m"]).await;
    a.cmd(&["SET", "tenant:a:t", "x", "EX", "100"]).await;

    for cmd in [
        &["GETDEL", "tenant:a:s"][..],
        &["GETEX", "tenant:a:s"],
        &["GETEX", "tenant:a:s", "PERSIST"],
        &["EXPIRETIME", "tenant:a:t"],
        &["PEXPIRETIME", "tenant:a:t"],
        &["HSTRLEN", "tenant:a:h", "f"],
        &["ZPOPMIN", "tenant:a:z"],
        &["ZPOPMAX", "tenant:a:z", "5"],
        &["RENAMENX", "tenant:a:s", "tenant:b:stolen"],
        &["RENAMENX", "tenant:b:mine", "tenant:a:planted"],
        &["COPY", "tenant:a:s", "tenant:b:stolen"],
        &["COPY", "tenant:b:mine", "tenant:a:planted", "REPLACE"],
    ] {
        let reply = b.cmd(cmd).await;
        assert!(noscope(&reply), "{cmd:?} -> {reply:?}");
        assert!(!format!("{reply:?}").contains("A-SECRET"), "{reply:?}");
    }
    // Nothing of A's was touched.
    assert_eq!(a.cmd(&["GET", "tenant:a:s"]).await, bulk("A-SECRET"));
    assert_eq!(a.cmd(&["ZCARD", "tenant:a:z"]).await, int(1));
    assert_eq!(b.cmd(&["EXISTS", "tenant:b:stolen"]).await, int(0));

    // On a read-only grant: COPY may read the source, but the destructive or
    // TTL-altering 0.6 reads need write.
    server.store.execute(Command::Set(
        "catalog:x".into(),
        "C".into(),
        SetOptions::default(),
    ));
    for cmd in [
        &["GETDEL", "catalog:x"][..],
        &["GETEX", "catalog:x"],
        &["ZPOPMIN", "catalog:x"],
        &["RENAMENX", "catalog:x", "tenant:b:y"],
        &["COPY", "tenant:b:y", "catalog:y"],
    ] {
        let reply = b.cmd(cmd).await;
        assert!(read_only(&reply), "{cmd:?} -> {reply:?}");
    }
    assert_eq!(b.cmd(&["COPY", "catalog:x", "tenant:b:y"]).await, int(1));
    assert_eq!(b.cmd(&["GET", "tenant:b:y"]).await, bulk("C"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v06_write_pushes_reach_the_owning_tenant_only() {
    let server = strict_server().await;
    let mut a = scoped(&server, "rw=tenant:a:*", "alice").await;
    let mut a_peer = scoped(&server, "rw=tenant:a:*", "alice").await;
    let mut b = scoped(&server, "rw=tenant:b:*", "bob").await;
    let mut b_live = scoped(&server, "rw=tenant:b:*", "bob").await;
    assert!(matches!(
        b_live.cmd(&["QSUB", "tenant:b:*"]).await,
        Value::Array(Some(_))
    ));

    a.cmd(&["SET", "tenant:a:s", "A-ONLY"]).await;
    a.cmd(&["SET", "tenant:a:g", "A-ONLY"]).await;
    a.cmd(&["ZADD", "tenant:a:z", "1", "A-MEMBER1", "2", "A-MEMBER2"])
        .await;
    a.cmd(&["GETEX", "tenant:a:s", "EX", "100"]).await;
    a.cmd(&["GETEX", "tenant:a:s", "PERSIST"]).await;
    a.cmd(&["GETDEL", "tenant:a:g"]).await;
    a.cmd(&["ZPOPMIN", "tenant:a:z"]).await;
    a.cmd(&["ZPOPMAX", "tenant:a:z"]).await;
    a.cmd(&["RENAMENX", "tenant:a:s", "tenant:a:r"]).await;
    a.cmd(&["COPY", "tenant:a:r", "tenant:a:c"]).await;

    let leaked = format!(
        "{}{}",
        b.drain_text(250).await,
        b_live.drain_text(250).await
    );
    assert!(
        !leaked.contains("tenant:a") && !leaked.contains("A-"),
        "tenant B saw: {leaked}"
    );
    // The owner's other tab did get them, so the filter is not just closed.
    let seen = a_peer.drain_text(250).await;
    for op in ["PEXPIREAT", "PERSIST", "DEL", "ZREM", "RENAME", "COPY"] {
        assert!(
            seen.contains(op),
            "{op} missing from tenant A's pushes: {seen}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_push_names_no_source_key_the_receiver_cannot_read() {
    // Regression. COPY lists only its destination in `primary_keys`, and the
    // fan-out filter used to forward the push to every socket that can read the
    // destination — but the frame is `COPY <src> <dst> REPLACE`, which names
    // the source. A socket scoped to the destination tenant was told the name
    // of a key it has no grant on. Pushes are now filtered by every key the
    // frame names (`push_keys`).
    let server = strict_server().await;
    // A principal that may read A's template and write B's namespace — a
    // provisioning service, or an admin console.
    let mut bridge = scoped(&server, "r=tenant:a:*,rw=tenant:b:*", "provisioner").await;
    let mut b = scoped(&server, "rw=tenant:b:*", "bob").await;
    server.store.execute(Command::Set(
        "tenant:a:private-template-name".into(),
        "v".into(),
        SetOptions::default(),
    ));
    assert_eq!(
        bridge
            .cmd(&["COPY", "tenant:a:private-template-name", "tenant:b:doc"])
            .await,
        int(1)
    );
    let seen = b.drain_text(300).await;
    assert!(
        !seen.contains("tenant:a:private-template-name"),
        "tenant B was pushed a frame naming tenant A's key: {seen:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn store_family_push_names_no_source_key_the_receiver_cannot_read() {
    // Regression (pre-0.6, same class as COPY). `SUNIONSTORE dst src…` lists only
    // `dst` in `primary_keys` and propagates verbatim, so a socket scoped to
    // `dst` was pushed the names of every source set.
    let server = strict_server().await;
    let mut bridge = scoped(&server, "r=tenant:a:*,rw=tenant:b:*", "provisioner").await;
    let mut b = scoped(&server, "rw=tenant:b:*", "bob").await;
    server.store.execute(Command::SAdd(
        "tenant:a:members-of-secret-project".into(),
        vec!["m".into()],
    ));
    bridge
        .cmd(&[
            "SUNIONSTORE",
            "tenant:b:out",
            "tenant:a:members-of-secret-project",
        ])
        .await;
    let seen = b.drain_text(300).await;
    assert!(
        !seen.contains("tenant:a:members-of-secret-project"),
        "tenant B was pushed a frame naming tenant A's key: {seen:?}"
    );
}

// ── 1. Token replacement vs. registrations and queued work ────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn narrowing_chr_closes_a_socket_holding_a_channel_subscription() {
    let server = strict_server().await;
    let mut c = scoped(&server, "rw=tenant:a:*,chr=tenant:a:*", "alice").await;
    c.cmd(&["SUBSCRIBE", "tenant:a:events"]).await;
    // Same subject, key grants unchanged, channel grant moved elsewhere.
    let narrower = token(&format!(
        "rw=tenant:a:*,chr=tenant:a:other|{}|sub=alice",
        in_secs(3600)
    ));
    // Answered first, then closed: the reply proves the token was taken.
    assert!(matches!(
        c.cmd(&["SYNC", "TOKEN", &narrower]).await,
        Value::Array(Some(_))
    ));
    assert_eq!(c.recv_close(1_000).await, Some(WS_CLOSE_SCOPE_CHANGED));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn narrowing_chr_closes_a_socket_holding_a_pattern_subscription() {
    let server = strict_server().await;
    let mut writer = scoped(&server, "chrw=tenant:a:*", "writer").await;
    let mut c = scoped(&server, "rw=tenant:a:*,chr=tenant:a:*", "alice").await;
    c.cmd(&["PSUBSCRIBE", "tenant:a:*"]).await;
    // `tenant:a:x` does not cover the pattern `tenant:a:*`.
    let narrower = token(&format!(
        "rw=tenant:a:*,chr=tenant:a:x|{}|sub=alice",
        in_secs(3600)
    ));
    // Answered first, then closed: the reply proves the token was taken.
    assert!(matches!(
        c.cmd(&["SYNC", "TOKEN", &narrower]).await,
        Value::Array(Some(_))
    ));
    writer
        .cmd(&["PUBLISH", "tenant:a:y", "AFTER-NARROWING"])
        .await;
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
    let code = tokio::time::timeout(tokio::time::Duration::from_secs(2), read)
        .await
        .unwrap_or(None);
    assert_eq!(code, Some(WS_CLOSE_SCOPE_CHANGED));
    assert!(!seen.contains("AFTER-NARROWING"), "{seen}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_rechecks_a_queued_write_after_a_rw_to_r_downgrade() {
    let server = strict_server().await;
    let mut c = scoped(&server, "rw=tenant:a:*", "alice").await;
    c.cmd(&["SET", "tenant:a:k", "before"]).await;
    assert_eq!(c.cmd(&["MULTI"]).await, ok());
    for cmd in [
        &["SET", "tenant:a:k", "AFTER"][..],
        &["COPY", "tenant:a:k", "tenant:a:k2"],
        &["DEDUP", "c", "9", "GETDEL", "tenant:a:k"],
    ] {
        assert_eq!(c.cmd(cmd).await, Value::SimpleString("QUEUED".into()));
    }
    let ro = token(&format!("r=tenant:a:*|{}|sub=alice", in_secs(3600)));
    assert!(matches!(
        c.cmd(&["SYNC", "TOKEN", &ro]).await,
        Value::Array(Some(_))
    ));
    let reply = c.cmd(&["EXEC"]).await;
    assert!(
        matches!(&reply, Value::Error(e) if e.starts_with("EXECABORT")),
        "{reply:?}"
    );
    assert_eq!(c.cmd(&["GET", "tenant:a:k"]).await, bulk("before"));
    assert_eq!(c.cmd(&["EXISTS", "tenant:a:k2"]).await, int(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_rechecks_a_queued_publish_after_chrw_to_chr_downgrade() {
    let server = strict_server().await;
    let mut listener = scoped(&server, "chr=tenant:a:*", "listener").await;
    listener.cmd(&["SUBSCRIBE", "tenant:a:ch"]).await;
    let mut c = scoped(&server, "rw=tenant:a:*,chrw=tenant:a:*", "alice").await;
    assert_eq!(c.cmd(&["MULTI"]).await, ok());
    assert_eq!(
        c.cmd(&["PUBLISH", "tenant:a:ch", "QUEUED-PUBLISH"]).await,
        Value::SimpleString("QUEUED".into())
    );
    let sub_only = token(&format!(
        "rw=tenant:a:*,chr=tenant:a:*|{}|sub=alice",
        in_secs(3600)
    ));
    assert!(matches!(
        c.cmd(&["SYNC", "TOKEN", &sub_only]).await,
        Value::Array(Some(_))
    ));
    let reply = c.cmd(&["EXEC"]).await;
    assert!(
        matches!(&reply, Value::Error(e) if e.starts_with("EXECABORT")),
        "{reply:?}"
    );
    assert!(!listener.drain_text(200).await.contains("QUEUED-PUBLISH"));
}

// ── 2. DEDUP retries under changed permissions ───────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_dedup_retry_after_losing_write_access_is_answered_without_running() {
    // The documented choice: an applied write reads as applied (`+DUP`) even
    // after the token loses write on its key, and nothing executes. A *new*
    // id is refused as read-only, and that refusal replays as itself.
    let server = strict_server().await;
    let mut c = scoped(&server, "rw=tenant:a:*", "alice").await;
    assert_eq!(
        c.cmd(&["DEDUP", "tab", "1", "INCR", "tenant:a:n"]).await,
        int(1)
    );
    let ro = token(&format!("r=tenant:a:*|{}|sub=alice", in_secs(3600)));
    c.cmd(&["SYNC", "TOKEN", &ro]).await;
    assert_eq!(
        c.cmd(&["DEDUP", "tab", "1", "INCR", "tenant:a:n"]).await,
        Value::SimpleString("DUP".into())
    );
    assert_eq!(c.cmd(&["GET", "tenant:a:n"]).await, bulk("1"));
    let refused = c.cmd(&["DEDUP", "tab", "2", "INCR", "tenant:a:n"]).await;
    assert!(read_only(&refused), "{refused:?}");
    assert_eq!(
        c.cmd(&["DEDUP", "tab", "2", "INCR", "tenant:a:n"]).await,
        refused
    );
    assert_eq!(c.cmd(&["GET", "tenant:a:n"]).await, bulk("1"));

    // Another principal on another socket, same raw client id, sees none of
    // it — neither the +DUP mark nor the remembered refusal.
    let mut bob = scoped(&server, "rw=tenant:b:*", "bob").await;
    assert_eq!(
        bob.cmd(&["DEDUP", "tab", "2", "INCR", "tenant:b:n"]).await,
        int(1)
    );
    // An anonymous socket cannot probe it either.
    let mut anon = WsClient::connect(server.tcp_addr).await;
    let reply = anon.cmd(&["DEDUP", "tab", "1", "INCR", "tenant:a:n"]).await;
    assert!(
        matches!(&reply, Value::Error(e) if e.contains("send SYNC TOKEN")),
        "{reply:?}"
    );
}

// ── 4/5. Disclosure before a token and through QSUB ───────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_token_auth_socket_learns_nothing_before_its_token() {
    let policy = SyncPolicy {
        token_auth: true,
        ..SyncPolicy::strict(SECRET)
    };
    let server = spawn_ws_server_policy(policy, None, Some("resp-admin-password")).await;
    server.store.execute(Command::Set(
        "tenant:a:k".into(),
        "A-VALUE".into(),
        SetOptions::default(),
    ));
    let mut anon = WsClient::connect(server.tcp_addr).await;
    let mut transcript = String::new();
    for cmd in [
        &["HELLO", "3"][..],
        &["HELLO"],
        &["INFO"],
        &["CLIENT", "LIST"],
        &["CONFIG", "GET", "*"],
        &["PUBSUB", "CHANNELS"],
        &["DBSIZE"],
        &["KEYS", "*"],
        &["SCAN", "0"],
        &["LASTSAVE"],
        &["SYNC"],
        &["QSUB", "*"],
        &["GET", "tenant:a:k"],
        &["EXPIRETIME", "tenant:a:k"],
        &["COPY", "tenant:a:k", "x"],
        &["MEMORY", "USAGE", "tenant:a:k"],
        &["DEDUP", "c", "1", "GETDEL", "tenant:a:k"],
    ] {
        let reply = anon.cmd(cmd).await;
        transcript.push_str(&format!("{cmd:?} -> {reply:?}\n"));
        let allowed_empty = matches!(&reply, Value::Array(Some(v)) if v.is_empty());
        assert!(
            matches!(reply, Value::Error(_)) || allowed_empty,
            "{cmd:?} answered before a token: {reply:?}"
        );
    }
    assert!(
        !transcript.contains("A-VALUE")
            && !transcript.contains(env!("CARGO_PKG_VERSION"))
            && !transcript.contains("resp-admin-password"),
        "{transcript}"
    );
    // The key is still there: the refusals did not run anything.
    assert!(matches!(
        server.store.execute(Command::Get("tenant:a:k".into())),
        Value::BulkString(Some(_))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn qsub_wider_than_the_grant_is_refused_without_a_snapshot() {
    let server = strict_server().await;
    server.store.execute(Command::Set(
        "tenant:a:k".into(),
        "A-VALUE".into(),
        SetOptions::default(),
    ));
    server.store.execute(Command::Set(
        "tenant:ab:k".into(),
        "AB-VALUE".into(),
        SetOptions::default(),
    ));
    let mut b = scoped(&server, "rw=tenant:b:*,r=pub:?", "bob").await;
    for pattern in [
        "*",
        "tenant:*",
        "tenant:?:*",
        "tenant:a:*",
        "tenant:a*",
        "tenant:b*",
        "?enant:b:*",
        "pub:*",
        "pub:??",
    ] {
        let reply = b.cmd(&["QSUB", pattern]).await;
        assert!(noscope(&reply), "QSUB {pattern:?} -> {reply:?}");
    }
    // Inside the grant: a snapshot of B's keys only.
    let snap = format!("{:?}", b.cmd(&["QSUB", "tenant:b:x*"]).await);
    assert!(!snap.contains("VALUE"), "{snap}");
    // An exact-equality ambiguous grant is accepted as itself.
    assert!(matches!(
        b.cmd(&["QSUB", "pub:?"]).await,
        Value::Array(Some(_))
    ));
}

// ── 6. Startup configuration ──────────────────────────────────────────────

#[tokio::test]
async fn an_empty_resp_password_does_not_authenticate() {
    // Regression. `RECACHED_PASSWORD=` (set, empty) was read as `Some("")`
    // (main.rs: `std::env::var("RECACHED_PASSWORD").ok()`), so the
    // multitenant profile's `resp_password` fact was true and startup was
    // accepted — while `AUTH ""` then matched it and unlocked the unscoped
    // RESP port. `RECACHED_REPL_PASSWORD` had the same flaw (an empty one
    // passed the public-bind check). Both now go through
    // `credential_from_env`, and `process_auth` matches no empty password.
    let srv = spawn_server_cfg(Some(""), None, false).await;
    let mut c = RespClient::connect(srv.tcp_addr).await;
    srv.store.execute(Command::Set(
        "tenant:a:k".into(),
        "A-VALUE".into(),
        SetOptions::default(),
    ));
    let auth = c.cmd(&["AUTH", ""]).await;
    let got = c.cmd(&["GET", "tenant:a:k"]).await;
    assert!(
        !matches!(auth, Value::SimpleString(_)) && got != bulk("A-VALUE"),
        "AUTH \"\" -> {auth:?}, then GET -> {got:?}"
    );
}

#[test]
fn an_empty_password_in_the_environment_counts_as_unset() {
    assert_eq!(
        credential_from_env("RECACHED_PASSWORD", Some(String::new())),
        None
    );
    assert_eq!(credential_from_env("RECACHED_PASSWORD", None), None);
    assert_eq!(
        credential_from_env("RECACHED_REPL_PASSWORD", Some("s3cret".into())),
        Some("s3cret".into())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_still_reaches_a_socket_that_can_read_both_keys() {
    // The fix filters by every key the frame names; a socket granted both
    // the source and the destination must keep receiving the push.
    let server = strict_server().await;
    let mut writer = scoped(&server, "rw=tenant:a:*", "alice").await;
    let mut reader = scoped(&server, "r=tenant:a:*", "alice-tab-2").await;
    server.store.execute(Command::Set(
        "tenant:a:src".into(),
        "v".into(),
        SetOptions::default(),
    ));
    assert_eq!(
        writer.cmd(&["COPY", "tenant:a:src", "tenant:a:dst"]).await,
        int(1)
    );
    let seen = reader.drain_text(300).await;
    assert!(
        seen.contains("COPY") && seen.contains("tenant:a:dst"),
        "a socket that may read both keys missed the COPY push: {seen:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_live_query_on_the_copy_destination_gets_the_value_but_not_the_source_name() {
    // The other half of the COPY fix: the destination-only socket gets no raw
    // push, but its live query on the destination must still learn the value.
    let server = strict_server().await;
    let mut bridge = scoped(&server, "r=tenant:a:*,rw=tenant:b:*", "provisioner").await;
    let mut b = scoped(&server, "rw=tenant:b:*", "bob").await;
    assert!(matches!(
        b.cmd(&["QSUB", "tenant:b:*"]).await,
        Value::Array(Some(_))
    ));
    server.store.execute(Command::Set(
        "tenant:a:private-template-name".into(),
        "COPIED-VALUE".into(),
        SetOptions::default(),
    ));
    assert_eq!(
        bridge
            .cmd(&["COPY", "tenant:a:private-template-name", "tenant:b:doc"])
            .await,
        int(1)
    );
    let seen = b.drain_text(500).await;
    assert!(
        seen.contains("tenant:b:doc") && seen.contains("COPIED-VALUE"),
        "the live query missed the copied value: {seen:?}"
    );
    assert!(
        !seen.contains("tenant:a:"),
        "the source key leaked: {seen:?}"
    );
}
