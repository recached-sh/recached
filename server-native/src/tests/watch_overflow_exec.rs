//! v0.6.0 release blocker (6O-B review, finding 2, High, pre-existing):
//! a WATCH whose first change overflows the connection's notification
//! budget must not let EXEC commit.
//!
//! The first notification for a watched key carries the key's whole value.
//! Over the 8 MiB budget, `WatchSubscriber::notify` signals the connection's
//! overflow channel and drops the registration, and nothing reaches the
//! watch queue that EXEC drains. The overflow arm disconnects the
//! connection, but `select!` is unbiased: an EXEC in the same read batch, or
//! in a batch the read arm won the race for, committed although the key had
//! changed. EXEC now checks the overflow channel under its write guards and
//! disconnects instead of committing, as the overflow arm would have.
//!
//! Every test asserts the outcome that matters, whichever arm wins: no
//! successful EXEC reply, and the transaction's write not applied.

use super::*;

/// Over the 8 MiB notification budget on its own.
const BIG: usize = 9 * 1024 * 1024;
/// APPEND chunks that build a `BIG` value under any frame limit.
const CHUNK: usize = 256 * 1024;

fn frame(args: &[&str]) -> Vec<u8> {
    let mut out = format!("*{}\r\n", args.len()).into_bytes();
    for a in args {
        out.extend_from_slice(format!("${}\r\n{a}\r\n", a.len()).as_bytes());
    }
    out
}

fn frames(cmds: &[&[&str]]) -> Vec<u8> {
    cmds.iter().flat_map(|c| frame(c)).collect()
}

/// Store a `BIG` value at `key` through `c`, before anyone watches it.
async fn store_big(c: &mut RespClient, key: &str) {
    let big = "x".repeat(BIG);
    assert_eq!(
        c.cmd(&["SET", key, &big]).await,
        Value::SimpleString("OK".into())
    );
}

/// Read replies from `c` until the server closes the connection or `secs`
/// pass; returns every reply read, and whether it closed.
async fn replies_until_close(c: &mut RespClient, secs: u64) -> (Vec<Value>, bool) {
    let mut replies = Vec::new();
    let mut raw = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    let closed = tokio::time::timeout(std::time::Duration::from_secs(secs), async {
        loop {
            match c.stream.read(&mut buf).await {
                Ok(0) | Err(_) => return,
                Ok(n) => raw.extend_from_slice(&buf[..n]),
            }
        }
    })
    .await
    .is_ok();
    let mut at = 0;
    while let Ok((v, n)) = Value::parse(&raw[at..]) {
        replies.push(v);
        at += n;
    }
    (replies, closed)
}

/// Whether `v` is a successful EXEC reply: an array, not nil.
fn is_committed_exec(v: &Value) -> bool {
    matches!(v, Value::Array(Some(items)) if items.iter().all(|i| !matches!(i, Value::Error(_))) && !items.is_empty())
}

fn assert_not_committed(replies: &[Value], store: &KeyValueStore, marker: &str, what: &str) {
    assert!(
        !replies.iter().any(is_committed_exec),
        "{what}: EXEC committed after the watched key's change overflowed: {replies:?}"
    );
    assert_eq!(
        store.get_current(marker),
        Value::BulkString(None),
        "{what}: the transaction's write was applied"
    );
}

// ── TCP ───────────────────────────────────────────────────────────────────────

/// The reviewer's case: WATCH, the connection's own change that overflows,
/// and the transaction all in one read batch.
#[tokio::test]
async fn watch_overflow_and_exec_in_one_batch_never_commit() {
    let server = spawn_server().await;
    let mut setup = RespClient::connect(server.tcp_addr).await;
    store_big(&mut setup, "k").await;
    let mut c = RespClient::connect(server.tcp_addr).await;
    let req = frames(&[
        &["WATCH", "k"],
        &["APPEND", "k", "x"],
        &["MULTI"],
        &["SET", "marker", "1"],
        &["EXEC"],
    ]);
    c.stream.write_all(&req).await.unwrap();
    let (replies, closed) = replies_until_close(&mut c, 10).await;
    assert_not_committed(&replies, &server.store, "marker", "one batch");
    assert!(closed, "the overflowed connection was disconnected");
}

/// The first change comes from another connection and the EXEC arrives in a
/// later batch: whichever of the read and overflow arms `select!` polls
/// first, EXEC does not commit.
#[tokio::test]
async fn an_overflow_from_another_writer_before_exec_never_commits() {
    let server = spawn_server().await;
    let mut writer = RespClient::connect(server.tcp_addr).await;
    for round in 0..8 {
        let key = format!("k{round}");
        let marker = format!("marker{round}");
        let mut c = RespClient::connect(server.tcp_addr).await;
        assert_eq!(
            c.cmd(&["WATCH", &key]).await,
            Value::SimpleString("OK".into())
        );
        // The watched key's first change is over the budget on its own.
        store_big(&mut writer, &key).await;
        let req = frames(&[&["MULTI"], &["SET", &marker, "1"], &["EXEC"]]);
        // The socket may already be closed by the overflow arm.
        let _ = c.stream.write_all(&req).await;
        let (replies, closed) = replies_until_close(&mut c, 10).await;
        assert_not_committed(&replies, &server.store, &marker, &format!("round {round}"));
        assert!(closed, "round {round}: disconnected");
    }
}

/// WATCH and UNWATCH within one batch with the overflow between them, then
/// a transaction: the connection is condemned by the overflow, so it is
/// disconnected and its transaction does not run, as before when the
/// overflow arm won.
#[tokio::test]
async fn watch_unwatch_in_one_batch_then_exec_is_disconnected_without_committing() {
    let server = spawn_server().await;
    let mut setup = RespClient::connect(server.tcp_addr).await;
    store_big(&mut setup, "k").await;
    let mut c = RespClient::connect(server.tcp_addr).await;
    let req = frames(&[
        &["WATCH", "k"],
        &["APPEND", "k", "x"],
        &["UNWATCH"],
        &["MULTI"],
        &["SET", "marker", "1"],
        &["EXEC"],
    ]);
    c.stream.write_all(&req).await.unwrap();
    let (replies, closed) = replies_until_close(&mut c, 10).await;
    assert_not_committed(&replies, &server.store, "marker", "watch/unwatch");
    assert!(closed);
}

/// Several watched keys, one of which overflows: the transaction does not
/// commit.
#[tokio::test]
async fn one_overflowed_key_among_several_watched_never_commits() {
    let server = spawn_server().await;
    let mut setup = RespClient::connect(server.tcp_addr).await;
    store_big(&mut setup, "b").await;
    let mut c = RespClient::connect(server.tcp_addr).await;
    let req = frames(&[
        &["WATCH", "a", "b", "c"],
        &["APPEND", "b", "x"],
        &["MULTI"],
        &["SET", "marker", "1"],
        &["EXEC"],
    ]);
    c.stream.write_all(&req).await.unwrap();
    let (replies, closed) = replies_until_close(&mut c, 10).await;
    assert_not_committed(&replies, &server.store, "marker", "several keys");
    assert!(closed);
}

/// Unchanged: an ordinary conflict (a small change, queued) aborts with a
/// nil reply and the connection stays usable; an unconflicted transaction
/// commits.
#[tokio::test]
async fn ordinary_conflicts_still_abort_and_clean_transactions_commit() {
    let server = spawn_server().await;
    let mut c = RespClient::connect(server.tcp_addr).await;
    let mut other = RespClient::connect(server.tcp_addr).await;
    assert_eq!(
        c.cmd(&["WATCH", "k"]).await,
        Value::SimpleString("OK".into())
    );
    assert_eq!(
        other.cmd(&["SET", "k", "small"]).await,
        Value::SimpleString("OK".into())
    );
    assert_eq!(c.cmd(&["MULTI"]).await, Value::SimpleString("OK".into()));
    assert_eq!(
        c.cmd(&["SET", "marker", "1"]).await,
        Value::SimpleString("QUEUED".into())
    );
    assert_eq!(
        c.cmd(&["EXEC"]).await,
        Value::Array(None),
        "a conflict aborts"
    );
    assert_eq!(server.store.get_current("marker"), Value::BulkString(None));

    assert_eq!(
        c.cmd(&["WATCH", "k"]).await,
        Value::SimpleString("OK".into())
    );
    assert_eq!(c.cmd(&["MULTI"]).await, Value::SimpleString("OK".into()));
    assert_eq!(
        c.cmd(&["SET", "marker", "2"]).await,
        Value::SimpleString("QUEUED".into())
    );
    assert_eq!(
        c.cmd(&["EXEC"]).await,
        Value::Array(Some(vec![Value::SimpleString("OK".into())])),
        "no conflict: commits"
    );
    assert_eq!(
        server.store.get_current("marker"),
        Value::BulkString(Some(b"2".to_vec()))
    );
}

/// Check-and-set increments under concurrent writers whose values are each
/// over the notification budget: every overflow either disconnects the
/// watcher or aborts its EXEC, so no increment is lost. The counter equals
/// the number of committed transactions exactly.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_check_and_set_with_overflowing_values_loses_no_update() {
    let server = spawn_server().await;
    let pad = "p".repeat(BIG);
    let mut setup = RespClient::connect(server.tcp_addr).await;
    assert_eq!(
        setup.cmd(&["SET", "ctr", &format!("0:{pad}")]).await,
        Value::SimpleString("OK".into())
    );
    let addr = server.tcp_addr;
    let mut workers = Vec::new();
    for w in 0..3 {
        let pad = pad.clone();
        workers.push(tokio::spawn(async move {
            let _ = w;
            let mut committed = 0u64;
            let mut attempts = 0u64;
            while committed < 3 && attempts < 60 {
                attempts += 1;
                let mut c = RespClient::connect(addr).await;
                // Another worker's write may overflow this watcher and
                // disconnect it at any point: then start over.
                let req = frames(&[&["WATCH", "ctr"], &["GET", "ctr"]]);
                if c.stream.write_all(&req).await.is_err() {
                    continue;
                }
                let (replies, _) = replies_until_close_or(&mut c, 3, 2).await;
                let current = match replies.as_slice() {
                    [Value::SimpleString(ok), Value::BulkString(Some(v))] if ok == "OK" => {
                        v.clone()
                    }
                    _ => continue,
                };
                let n: u64 = std::str::from_utf8(&current)
                    .unwrap()
                    .split(':')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap();
                let next = format!("{}:{pad}", n + 1);
                let req = frames(&[&["MULTI"], &["SET", "ctr", &next], &["EXEC"]]);
                if c.stream.write_all(&req).await.is_err() {
                    continue;
                }
                let (replies, _) = replies_until_close_or(&mut c, 3, 3).await;
                if replies.iter().any(is_committed_exec) {
                    committed += 1;
                }
            }
            committed
        }));
    }
    let mut committed = 0;
    for w in workers {
        committed += w.await.unwrap();
    }
    let got = match server.store.get_current("ctr") {
        Value::BulkString(Some(v)) => v,
        other => panic!("ctr {other:?}"),
    };
    let n: u64 = std::str::from_utf8(&got)
        .unwrap()
        .split(':')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(n, committed, "every committed increment, and only those");
}

/// Like [`replies_until_close`], but also stops once `want` replies arrived.
async fn replies_until_close_or(c: &mut RespClient, secs: u64, want: usize) -> (Vec<Value>, bool) {
    let mut raw = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    let count = |raw: &[u8]| {
        let mut at = 0;
        let mut out = Vec::new();
        while let Ok((v, n)) = Value::parse(&raw[at..]) {
            out.push(v);
            at += n;
        }
        out
    };
    let closed = tokio::time::timeout(std::time::Duration::from_secs(secs), async {
        loop {
            match c.stream.read(&mut buf).await {
                Ok(0) | Err(_) => return true,
                Ok(n) => {
                    raw.extend_from_slice(&buf[..n]);
                    if count(&raw).len() >= want {
                        return false;
                    }
                }
            }
        }
    })
    .await
    .unwrap_or(false);
    (count(&raw), closed)
}

// ── WebSocket ─────────────────────────────────────────────────────────────────

/// Build a `BIG` value at `key` through `c` in `CHUNK`-sized APPENDs.
async fn ws_store_big(c: &mut WsClient, key: &str) {
    let chunk = "y".repeat(CHUNK);
    for _ in 0..BIG / CHUNK {
        assert!(matches!(
            c.cmd(&["APPEND", key, &chunk]).await,
            Value::Integer(_)
        ));
    }
}

/// Send a command to `c`, which the server may already have closed for the
/// overflow (the WS loop polls the overflow arm while idle).
async fn ws_send_or_closed(c: &mut WsClient, args: &[&str]) {
    let mut req = format!("*{}\r\n", args.len());
    for a in args {
        req.push_str(&format!("${}\r\n{}\r\n", a.len(), a));
    }
    let _ = c.ws.send(Message::Text(req.into())).await;
}

/// Whether the WS server closed `c` within `ms`, with every text frame it
/// sent first.
async fn ws_frames_until_close(c: &mut WsClient, ms: u64) -> (Vec<String>, bool) {
    let mut frames = Vec::new();
    let fut = async {
        loop {
            match c.ws.next().await {
                Some(Ok(Message::Text(t))) => frames.push(t.to_string()),
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return,
                Some(Ok(_)) => {}
            }
        }
    };
    let closed = tokio::time::timeout(std::time::Duration::from_millis(ms), fut)
        .await
        .is_ok();
    (frames, closed)
}

/// The WebSocket handler has the same EXEC path: an overflow from the
/// watched key's first change never lets its EXEC commit.
#[tokio::test]
async fn ws_watch_overflow_before_exec_never_commits() {
    let server = spawn_ws_server_policy(SyncPolicy::open(), None, None).await;
    for round in 0..4 {
        let key = format!("wk{round}");
        let marker = format!("wmarker{round}");
        let mut setup = WsClient::connect(server.tcp_addr).await;
        ws_store_big(&mut setup, &key).await;
        let mut c = WsClient::connect(server.tcp_addr).await;
        assert_eq!(
            c.cmd(&["WATCH", &key]).await,
            Value::SimpleString("OK".into())
        );
        // One byte more: its notification carries the whole value.
        assert!(matches!(
            setup.cmd(&["APPEND", &key, "z"]).await,
            Value::Integer(_)
        ));
        ws_send_or_closed(&mut c, &["MULTI"]).await;
        ws_send_or_closed(&mut c, &["SET", &marker, "1"]).await;
        ws_send_or_closed(&mut c, &["EXEC"]).await;
        let (frames, closed) = ws_frames_until_close(&mut c, 10_000).await;
        assert!(
            !frames.iter().any(|f| f.starts_with("*1\r\n+OK")),
            "round {round}: EXEC committed: {:?}",
            frames
                .iter()
                .map(|f| &f[..f.len().min(40)])
                .collect::<Vec<_>>()
        );
        assert_eq!(
            server.store.get_current(&marker),
            Value::BulkString(None),
            "round {round}: the transaction's write was applied"
        );
        assert!(closed, "round {round}: disconnected");
    }
}

/// Tenant isolation: tenant A's overflowed WATCH condemns A's connection
/// only. Tenant B's transaction on its own watched key commits, and A's does
/// not.
#[tokio::test]
async fn one_tenants_overflow_leaves_another_tenants_transaction_alone() {
    const SECRET: &str = "watch-overflow-test-secret-0123456789abcdef";
    let server = spawn_ws_server_policy(SyncPolicy::strict(SECRET), None, None).await;
    let tenant = |t: &str| {
        mint_sync_token(
            SECRET,
            &format!(
                "rw=tenant:{t}:*,chrw=tenant:{t}:*|{}|sub=user-{t}",
                unix_now_secs() + 3600
            ),
        )
    };
    async fn scoped(server: &TestServer, token: &str) -> WsClient {
        let mut c = WsClient::connect(server.tcp_addr).await;
        let reply = c.cmd(&["SYNC", "TOKEN", token]).await;
        assert!(
            matches!(reply, Value::Array(Some(_))),
            "token refused: {reply:?}"
        );
        c
    }
    let mut a_writer = scoped(&server, &tenant("a")).await;
    let mut a = scoped(&server, &tenant("a")).await;
    let mut b = scoped(&server, &tenant("b")).await;
    ws_store_big(&mut a_writer, "tenant:a:k").await;
    assert_eq!(
        b.cmd(&["WATCH", "tenant:b:k"]).await,
        Value::SimpleString("OK".into())
    );
    assert_eq!(
        a.cmd(&["WATCH", "tenant:a:k"]).await,
        Value::SimpleString("OK".into())
    );
    assert!(matches!(
        a_writer.cmd(&["APPEND", "tenant:a:k", "z"]).await,
        Value::Integer(_)
    ));

    assert_eq!(b.cmd(&["MULTI"]).await, Value::SimpleString("OK".into()));
    assert_eq!(
        b.cmd(&["SET", "tenant:b:m", "1"]).await,
        Value::SimpleString("QUEUED".into())
    );
    assert_eq!(
        b.cmd(&["EXEC"]).await,
        Value::Array(Some(vec![Value::SimpleString("OK".into())])),
        "tenant b's transaction is untouched by tenant a's overflow"
    );

    ws_send_or_closed(&mut a, &["MULTI"]).await;
    ws_send_or_closed(&mut a, &["SET", "tenant:a:m", "1"]).await;
    ws_send_or_closed(&mut a, &["EXEC"]).await;
    let (_, closed) = ws_frames_until_close(&mut a, 10_000).await;
    assert!(closed, "tenant a's overflowed connection was disconnected");
    assert_eq!(
        server.store.get_current("tenant:a:m"),
        Value::BulkString(None)
    );
}

/// The WebSocket handler runs one command per message, so its exposure is
/// the race alone: WATCH, the connection's own overflowing APPEND, then
/// MULTI, SET and EXEC sent back to back. After the APPEND both the overflow
/// arm and the next buffered message are ready, and before the fix EXEC
/// committed whenever the three messages won the race in a row: measured on
/// `dev` `be3c90e`, 4 to 8 rounds in 2,000 (detected in 4 runs of 4; a run
/// misses it with probability about 0.2%). Now no round commits.
#[tokio::test]
async fn ws_pipelined_messages_after_an_overflow_never_commit() {
    let server = spawn_ws_server_policy(SyncPolicy::open(), None, None).await;
    let mut setup = WsClient::connect(server.tcp_addr).await;
    ws_store_big(&mut setup, "k").await;
    const ROUNDS: usize = 2_000;
    let mut committed = 0;
    for round in 0..ROUNDS {
        let marker = format!("marker{round}");
        let mut c = WsClient::connect(server.tcp_addr).await;
        ws_send_or_closed(&mut c, &["WATCH", "k"]).await;
        ws_send_or_closed(&mut c, &["APPEND", "k", "x"]).await;
        ws_send_or_closed(&mut c, &["MULTI"]).await;
        ws_send_or_closed(&mut c, &["SET", &marker, "1"]).await;
        ws_send_or_closed(&mut c, &["EXEC"]).await;
        let (frames, closed) = ws_frames_until_close(&mut c, 10_000).await;
        assert!(
            closed,
            "round {round}: the overflowed connection was disconnected"
        );
        if frames.iter().any(|f| f.starts_with("*1\r\n+OK"))
            || server.store.get_current(&marker) != Value::BulkString(None)
        {
            committed += 1;
        }
    }
    assert_eq!(
        committed, 0,
        "rounds whose EXEC committed after the overflow"
    );
}
