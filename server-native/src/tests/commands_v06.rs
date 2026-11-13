//! Server integration for the 0.6 commands (Phase 6C): what each new write
//! replicates, what it pushes to browser peers, how it interacts with WATCH,
//! and how sync scopes classify it. Command semantics are covered by the
//! engine tests and against Redis by `tests/redis_diff.rs`.

use super::replication_dedup::{caught_up, follow, node, serve_replicas};
use super::*;

/// Everything a reader can observe about `key`, for comparing two stores.
fn digest(store: &KeyValueStore, key: &str) -> String {
    let k = || key.to_string();
    let ty = store.execute(Command::Type(k()));
    let body = match &ty {
        Value::SimpleString(t) if t == "string" => store.execute(Command::Get(k())),
        Value::SimpleString(t) if t == "hash" => {
            let Value::Array(Some(items)) = store.execute(Command::HGetAll(k())) else {
                panic!()
            };
            let mut pairs: Vec<_> = items.chunks(2).map(|p| format!("{p:?}")).collect();
            pairs.sort();
            Value::BulkString(Some(pairs.join(",").into_bytes()))
        }
        Value::SimpleString(t) if t == "list" => store.execute(Command::LRange(k(), 0, -1)),
        Value::SimpleString(t) if t == "zset" => store.execute(Command::ZRange(k(), 0, -1, true)),
        _ => Value::BulkString(None),
    };
    format!(
        "{ty:?} {body:?} exp={:?}",
        store.execute(Command::PExpireTime(k()))
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_new_write_replicates_and_pushes_a_primitive_frame() {
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    follow(&replica, serve_replicas(&primary).await);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let mut peer = WsClient::connect(primary.ws).await;
    peer.cmd(&["PING"]).await;
    let mut p = WsClient::connect(primary.ws).await;

    for setup in [
        &["SET", "gd", "v"][..],
        &["SET", "gx", "v"],
        &["SET", "gp", "v", "EX", "500"],
        &["SET", "gpast", "v"],
        &["SET", "rn", "v"],
        &["HSET", "cp", "f", "v"],
        &["PEXPIREAT", "cp", "4102444800000"],
        &["ZADD", "zp", "1", "a", "2", "b", "3", "c", "4", "d"],
    ] {
        p.cmd(setup).await;
    }
    caught_up(&mut p, &replica, "setup").await;
    drop(peer.drain_text(200).await);

    assert_eq!(p.cmd(&["GETDEL", "gd"]).await, bulk("v"));
    assert_eq!(p.cmd(&["GETEX", "gx", "EX", "600"]).await, bulk("v"));
    assert_eq!(p.cmd(&["GETEX", "gp", "PERSIST"]).await, bulk("v"));
    assert_eq!(p.cmd(&["GETEX", "gpast", "PXAT", "1"]).await, bulk("v"));
    assert_eq!(p.cmd(&["RENAMENX", "rn", "rn2"]).await, int(1));
    assert_eq!(p.cmd(&["COPY", "cp", "cp2"]).await, int(1));
    assert!(matches!(
        p.cmd(&["ZPOPMIN", "zp", "2"]).await,
        Value::Array(Some(_))
    ));
    assert!(matches!(
        p.cmd(&["ZPOPMAX", "zp"]).await,
        Value::Array(Some(_))
    ));
    // No-ops broadcast nothing.
    assert_eq!(p.cmd(&["GETDEL", "missing"]).await, nil());
    assert_eq!(p.cmd(&["RENAMENX", "rn2", "cp"]).await, int(0));
    caught_up(&mut p, &replica, "writes").await;

    for key in ["gd", "gx", "gp", "gpast", "rn", "rn2", "cp", "cp2", "zp"] {
        assert_eq!(
            digest(&replica.store, key),
            digest(&primary.store, key),
            "replica diverged on {key}"
        );
    }

    // Browser peers are sent primitives every 0.5 client already replays,
    // except COPY, which replays verbatim like the STORE family.
    let pushed = peer.drain_text(300).await.to_ascii_uppercase();
    for frame in ["DEL", "PEXPIREAT", "PERSIST", "RENAME", "COPY", "ZREM"] {
        assert!(pushed.contains(frame), "no {frame} frame pushed: {pushed}");
    }
    for name in ["GETDEL", "GETEX", "RENAMENX", "ZPOPMIN", "ZPOPMAX"] {
        assert!(!pushed.contains(name), "{name} pushed verbatim: {pushed}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_watched_key_taken_by_getdel_or_zpop_aborts_the_transaction() {
    for take in [&["GETDEL", "w"][..], &["ZPOPMIN", "wz"]] {
        let srv = spawn_ws_server().await;
        let mut a = WsClient::connect(srv.tcp_addr).await;
        let mut b = WsClient::connect(srv.tcp_addr).await;
        a.cmd(&["SET", "w", "v"]).await;
        a.cmd(&["ZADD", "wz", "1", "m"]).await;
        let key = take[1];
        assert_eq!(a.cmd(&["WATCH", key]).await, ok());
        assert!(!matches!(b.cmd(take).await, Value::Error(_)));
        assert_eq!(a.cmd(&["MULTI"]).await, ok());
        a.cmd(&["SET", "other", "x"]).await;
        assert_eq!(
            a.cmd(&["EXEC"]).await,
            Value::Array(None),
            "{take:?} did not dirty WATCH"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_scopes_classify_the_new_commands_per_key() {
    const SECRET: &str = "scope-secret-0123456789abcdef0123456789";
    let srv = spawn_ws_server_policy(SyncPolicy::strict(SECRET), None, None).await;
    let tok = mint_sync_token(SECRET, "rw=mine:*,r=shared:*");
    let mut c = WsClient::connect(srv.tcp_addr).await;
    assert!(matches!(
        c.cmd(&["SYNC", "TOKEN", &tok]).await,
        Value::Array(Some(_))
    ));
    srv.store.execute(Command::Set(
        "shared:k".into(),
        "v".into(),
        SetOptions::default(),
    ));
    srv.store.execute(Command::Set(
        "theirs:k".into(),
        "v".into(),
        SetOptions::default(),
    ));
    srv.store.execute(Command::ZAdd(
        "shared:z".into(),
        Default::default(),
        vec![(1.0, "m".into())],
    ));
    let read_only = |v: &Value| matches!(v, Value::Error(e) if e.contains("read-only"));
    let outside = |v: &Value| matches!(v, Value::Error(e) if e.contains("outside"));

    // COPY reads its source: a read grant on it is enough.
    assert_eq!(c.cmd(&["COPY", "shared:k", "mine:copy"]).await, int(1));
    // ...but not one outside every grant, and never into a read-only key.
    assert!(outside(&c.cmd(&["COPY", "theirs:k", "mine:x"]).await));
    assert!(read_only(
        &c.cmd(&["COPY", "mine:copy", "shared:new"]).await
    ));
    // GETDEL, GETEX (even without an option), ZPOP*, RENAMENX all write.
    assert!(read_only(&c.cmd(&["GETDEL", "shared:k"]).await));
    assert!(read_only(&c.cmd(&["GETEX", "shared:k"]).await));
    assert!(read_only(&c.cmd(&["GETEX", "shared:k", "PERSIST"]).await));
    assert!(read_only(&c.cmd(&["ZPOPMIN", "shared:z"]).await));
    assert!(read_only(
        &c.cmd(&["RENAMENX", "shared:k", "mine:moved"]).await
    ));
    assert!(outside(&c.cmd(&["GETDEL", "theirs:k"]).await));
    // The reads need only read.
    assert_eq!(c.cmd(&["EXPIRETIME", "shared:k"]).await, int(-1));
    // A string, so Redis's WRONGTYPE: scope let the read through.
    assert!(matches!(
        c.cmd(&["HSTRLEN", "shared:k", "f"]).await,
        Value::Error(e) if e.starts_with("WRONGTYPE")
    ));
    assert!(outside(&c.cmd(&["EXPIRETIME", "theirs:k"]).await));
    // Within the read-write grant, everything works.
    assert_eq!(c.cmd(&["SET", "mine:a", "1"]).await, ok());
    assert_eq!(c.cmd(&["RENAMENX", "mine:a", "mine:b"]).await, int(1));
    assert_eq!(c.cmd(&["GETDEL", "mine:b"]).await, bulk("1"));
}

#[tokio::test]
async fn a_replica_refuses_the_new_writes() {
    let srv = spawn_ws_server().await;
    srv.state.is_replica.store(true, Ordering::Release);
    let mut c = WsClient::connect(srv.tcp_addr).await;
    for cmd in [
        &["GETDEL", "k"][..],
        &["GETEX", "k"],
        &["RENAMENX", "a", "b"],
        &["COPY", "a", "b"],
        &["ZPOPMIN", "z"],
        &["ZPOPMAX", "z"],
    ] {
        assert!(
            matches!(c.cmd(cmd).await, Value::Error(e) if e.starts_with("READONLY")),
            "{cmd:?}"
        );
    }
    assert_eq!(c.cmd(&["EXPIRETIME", "k"]).await, int(-2));
    assert_eq!(c.cmd(&["HSTRLEN", "k", "f"]).await, int(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn relative_expiry_reaches_replicas_to_the_millisecond() {
    // The propagated deadline was re-derived from a second clock reading taken
    // after the store's, so a replica (and the AOF, and browser peers) could
    // hold a deadline a millisecond or two later than the primary's. Any one
    // write rarely straddles a millisecond; hundreds of every relative form do.
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    follow(&replica, serve_replicas(&primary).await);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let mut p = WsClient::connect(primary.ws).await;
    let mut keys = Vec::new();
    for i in 0..120 {
        let (a, b, c, d, e) = (
            format!("set:{i}"),
            format!("setex:{i}"),
            format!("expire:{i}"),
            format!("getex:{i}"),
            format!("pexpire:{i}"),
        );
        p.cmd(&["SET", &a, "v", "PX", "900000"]).await;
        p.cmd(&["SETEX", &b, "900", "v"]).await;
        p.cmd(&["SET", &c, "v"]).await;
        p.cmd(&["EXPIRE", &c, "900"]).await;
        p.cmd(&["SET", &d, "v"]).await;
        p.cmd(&["GETEX", &d, "PX", "900000"]).await;
        p.cmd(&["SET", &e, "v"]).await;
        p.cmd(&["PEXPIRE", &e, "900000"]).await;
        keys.extend([a, b, c, d, e]);
    }
    caught_up(&mut p, &replica, "expiries").await;
    let drifted: Vec<_> = keys
        .iter()
        .filter_map(|k| {
            let want = primary.store.execute(Command::PExpireTime(k.clone()));
            let got = replica.store.execute(Command::PExpireTime(k.clone()));
            (want != got).then(|| format!("{k}: primary {want:?} replica {got:?}"))
        })
        .collect();
    assert!(
        drifted.is_empty(),
        "{} deadlines drifted:\n{}",
        drifted.len(),
        drifted.join("\n")
    );
}

#[test]
fn the_exact_deadline_overrides_the_recomputed_one() {
    let frame = broadcast_for_at(
        &Command::Expire("k".into(), 5),
        &Value::Integer(1),
        1_000_000,
        Some(1_004_999),
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&frame).contains("1004999"));
    assert_eq!(
        relative_expiry_key(&Command::Set(
            "k".into(),
            "v".into(),
            SetOptions {
                expiry: Some(SetExpiry::Exat(5)),
                ..Default::default()
            }
        )),
        None,
        "absolute expiries need no read-back"
    );
}
