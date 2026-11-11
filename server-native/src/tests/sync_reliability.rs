//! v0.5 synchronization reliability on the server: duplicate suppression
//! that survives a crash, remembered refusals, the dedup barrier, push
//! framing, and writes after `SUBSCRIBE`.
//!
//! "Crash" here means dropping the server state without a checkpoint and
//! recovering from the files alone, exactly as startup does: snapshot, dedup
//! sidecars, then the AOF. The real-process version — `kill -9` between
//! runs of the binary — is `tests/restart.rs`.

use super::*;
use std::path::Path;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "recached_syncrel_{}_{}_{}",
        name,
        std::process::id(),
        next_conn_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A server state persisting into `dir`, with an AOF that fsyncs every
/// append — so what the test reads back is what a crash would leave.
async fn durable_state(dir: &Path) -> Arc<ServerState> {
    let aof = AofWriter::open(dir.join("append.aof"), AofSync::Always)
        .await
        .unwrap();
    Arc::new(ServerState {
        snap: Arc::new(SnapshotConfig {
            path: dir.join("dump.rdb"),
            last_save: AtomicI64::new(now_unix_secs()),
            checkpoint_id: AtomicU64::new(0),
        }),
        aof: Some(Arc::new(aof)),
        replicas: ReplHub::new(),
        is_replica: AtomicBool::new(false),
        dedup: std::sync::Mutex::new(HashMap::new()),
        ephemeral: std::sync::Mutex::new(HashMap::new()),
        ephemeral_members: std::sync::Mutex::new(HashMap::new()),
        dedup_dirty: std::sync::atomic::AtomicBool::new(false),
        dedup_order: tokio::sync::Mutex::new(()),
        save_lock: tokio::sync::Mutex::new(()),
        persistence_healthy: AtomicBool::new(true),
        persistence_failures: AtomicU64::new(0),
    })
}

/// Recover a store and state from `dir` the way `main` does at startup.
async fn recover(dir: &Path) -> (KeyValueStore, Arc<ServerState>) {
    let store = KeyValueStore::new();
    let loaded = load_snapshot_checkpoint(&store, &dir.join("dump.rdb"))
        .await
        .unwrap();
    let (_, recovered) =
        replay_aof_recovering(&store, &dir.join("append.aof"), loaded.aof_checkpoint)
            .await
            .unwrap();
    let state = durable_state(dir).await;
    state
        .snap
        .checkpoint_id
        .store(loaded.aof_checkpoint, Ordering::Release);
    state.load_dedup().await.unwrap();
    state.merge_recovered_dedup(recovered);
    (store, state)
}

fn dedup(client: &str, id: u64, inner: Command) -> Command {
    Command::Dedup(client.into(), id, Box::new(inner))
}

async fn write(state: &ServerState, store: &KeyValueStore, cmd: &Command) -> Value {
    let tx = broadcast::channel::<SyncMsg>(8).0;
    let watches = WatchHub::new();
    execute_ordered_write(cmd, &tx, 1, state, &watches, store).await
}

fn get(store: &KeyValueStore, key: &str) -> Value {
    store.execute(Command::Get(key.into()))
}

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

fn dup() -> Value {
    Value::SimpleString("DUP".into())
}

// ── TEST 3 / TEST 4: crash consistency ───────────────────────────────────────

#[tokio::test]
async fn test_04_an_incr_and_its_mark_survive_a_crash_before_any_checkpoint() {
    let dir = scratch("incr_crash");
    {
        let state = durable_state(&dir).await;
        let store = KeyValueStore::new();
        let incr = dedup("phone", 7, Command::IncrBy("visits".into(), 5));
        assert_eq!(write(&state, &store, &incr).await, Value::Integer(5));
        // The reply is lost and the server dies before it checkpoints.
    }
    let aof = std::fs::read(dir.join("append.aof")).unwrap();
    assert!(
        String::from_utf8_lossy(&aof).contains("DEDUP"),
        "the AOF records the write inside its envelope"
    );

    let (store, state) = recover(&dir).await;
    assert_eq!(get(&store, "visits"), bulk("5"));
    // TEST 3: the client retries the INCR whose reply it never saw.
    let retry = dedup("phone", 7, Command::IncrBy("visits".into(), 5));
    assert_eq!(write(&state, &store, &retry).await, dup());
    assert_eq!(get(&store, "visits"), bulk("5"), "applied once, not twice");
    // A genuinely new write still applies.
    let next = dedup("phone", 8, Command::IncrBy("visits".into(), 1));
    assert_eq!(write(&state, &store, &next).await, Value::Integer(6));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_03_every_non_idempotent_write_is_suppressed_on_retry_after_a_crash() {
    let dir = scratch("nonidempotent");
    let writes = || {
        vec![
            Command::Incr("c".into()),
            Command::Decr("d".into()),
            Command::IncrBy("e".into(), 3),
            Command::Append("s".into(), b"x".to_vec()),
            Command::LPush("l".into(), vec![b"a".to_vec()]),
            Command::RPush("l".into(), vec![b"b".to_vec()]),
            Command::HIncrBy("h".into(), "f".into(), 2),
            Command::JMerge("j".into(), r#"{"n":1}"#.into()),
            Command::SAdd("set".into(), vec!["m".into()]),
            Command::ZIncrBy("z".into(), 1.5, "m".into()),
        ]
    };
    {
        let state = durable_state(&dir).await;
        let store = KeyValueStore::new();
        for (i, cmd) in writes().into_iter().enumerate() {
            let r = write(&state, &store, &dedup("c1", i as u64 + 1, cmd)).await;
            assert!(!matches!(r, Value::Error(_)), "{r:?}");
        }
    }
    let (store, state) = recover(&dir).await;
    let before = store.snapshot();
    for (i, cmd) in writes().into_iter().enumerate() {
        assert_eq!(
            write(&state, &store, &dedup("c1", i as u64 + 1, cmd)).await,
            dup(),
            "write {i} ran twice"
        );
    }
    let mut after = store.snapshot();
    let mut before = before;
    before.sort_by(|a, b| a.key.cmp(&b.key));
    after.sort_by(|a, b| a.key.cmp(&b.key));
    assert_eq!(
        rmp_serde::to_vec(&before).unwrap(),
        rmp_serde::to_vec(&after).unwrap()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_write_that_changed_nothing_keeps_its_mark_across_a_crash() {
    // `HDEL` of a field that is not there has no mutation to log; without its
    // own record a retry after a crash would run where the first try did not.
    let dir = scratch("noop_mark");
    {
        let state = durable_state(&dir).await;
        let store = KeyValueStore::new();
        let noop = dedup("c", 3, Command::HDel("h".into(), vec!["f".into()]));
        assert_eq!(write(&state, &store, &noop).await, Value::Integer(0));
    }
    let (store, state) = recover(&dir).await;
    let retry = dedup("c", 3, Command::HDel("h".into(), vec!["f".into()]));
    assert_eq!(write(&state, &store, &retry).await, dup());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_torn_aof_tail_loses_the_write_and_its_mark_together() {
    let dir = scratch("torn_tail");
    {
        let state = durable_state(&dir).await;
        let store = KeyValueStore::new();
        let incr = dedup("c", 1, Command::IncrBy("n".into(), 2));
        assert_eq!(write(&state, &store, &incr).await, Value::Integer(2));
    }
    // The power fails mid-append: the record is cut short.
    let path = dir.join("append.aof");
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() - 3]).unwrap();

    let (store, state) = recover(&dir).await;
    assert_eq!(get(&store, "n"), Value::BulkString(None));
    // Neither survived, so the retry applies — once.
    let retry = dedup("c", 1, Command::IncrBy("n".into(), 2));
    assert_eq!(write(&state, &store, &retry).await, Value::Integer(2));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_checkpoint_interrupted_after_its_marks_recovers_consistently() {
    // The marks are written before the snapshot. Here the snapshot cannot be
    // renamed into place (a directory sits at its path), so the checkpoint
    // fails after the marks landed — the crash window between the two files.
    let dir = scratch("interrupted_checkpoint");
    {
        let state = durable_state(&dir).await;
        let store = KeyValueStore::new();
        let incr = dedup("c", 4, Command::IncrBy("n".into(), 7));
        assert_eq!(write(&state, &store, &incr).await, Value::Integer(7));
        std::fs::create_dir_all(dir.join("dump.rdb").join("occupied")).unwrap();
        assert!(state.save(&store).await.is_err());
        assert!(dir.join("dump.dedup").exists(), "marks were written first");
    }
    std::fs::remove_dir_all(dir.join("dump.rdb")).unwrap();

    // No snapshot; marks ahead of it; the AOF still holds every write.
    let (store, state) = recover(&dir).await;
    assert_eq!(get(&store, "n"), bulk("7"));
    let retry = dedup("c", 4, Command::IncrBy("n".into(), 7));
    assert_eq!(write(&state, &store, &retry).await, dup());
    assert_eq!(get(&store, "n"), bulk("7"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn marks_and_data_agree_after_a_completed_checkpoint_and_later_writes() {
    let dir = scratch("checkpoint_then_writes");
    {
        let state = durable_state(&dir).await;
        let store = KeyValueStore::new();
        write(
            &state,
            &store,
            &dedup("c", 1, Command::IncrBy("n".into(), 1)),
        )
        .await;
        state.save(&store).await.unwrap();
        write(
            &state,
            &store,
            &dedup("c", 2, Command::IncrBy("n".into(), 10)),
        )
        .await;
    }
    let (store, state) = recover(&dir).await;
    assert_eq!(get(&store, "n"), bulk("11"));
    for id in [1, 2] {
        let retry = dedup("c", id, Command::IncrBy("n".into(), 100));
        assert_eq!(write(&state, &store, &retry).await, dup(), "id {id}");
    }
    assert_eq!(get(&store, "n"), bulk("11"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn an_aof_written_before_0_5_still_replays() {
    let dir = scratch("legacy_aof");
    std::fs::write(
        dir.join("append.aof"),
        resp_push(&[b"SET", b"k", b"v"])
            .into_iter()
            .chain(resp_push(&[b"SET", b"n", b"3", b"KEEPTTL"]))
            .collect::<Vec<u8>>(),
    )
    .unwrap();
    let (store, _) = recover(&dir).await;
    assert_eq!(get(&store, "k"), bulk("v"));
    assert_eq!(get(&store, "n"), bulk("3"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ── remembered refusals ──────────────────────────────────────────────────────

#[tokio::test]
async fn a_refused_write_replayed_below_the_mark_is_refused_again_even_after_a_restart() {
    let dir = scratch("refusal_memory");
    let refused = || dedup("c", 1, Command::IncrBy("text".into(), 1));
    {
        let state = durable_state(&dir).await;
        let store = KeyValueStore::new();
        store.execute(Command::Set(
            "text".into(),
            b"abc".to_vec(),
            Default::default(),
        ));
        assert!(matches!(
            write(&state, &store, &refused()).await,
            Value::Error(_)
        ));
        // A later write is accepted, so the refused id is now below the mark.
        write(
            &state,
            &store,
            &dedup(
                "c",
                2,
                Command::Set("ok".into(), b"1".to_vec(), Default::default()),
            ),
        )
        .await;
        // Its replay is the refusal, not a duplicate.
        assert!(matches!(
            write(&state, &store, &refused()).await,
            Value::Error(e) if e.contains("not an integer")
        ));
    }
    // After a crash, from the AOF record.
    let (store, state) = recover(&dir).await;
    assert!(matches!(
        write(&state, &store, &refused()).await,
        Value::Error(_)
    ));
    // After a checkpoint, from the refusals sidecar.
    state.save(&store).await.unwrap();
    let (store, state) = recover(&dir).await;
    assert!(matches!(
        write(&state, &store, &refused()).await,
        Value::Error(_)
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_refused_write_retried_above_the_mark_runs_again_and_forgets_the_refusal() {
    let dir = scratch("refusal_retry");
    let state = durable_state(&dir).await;
    let store = KeyValueStore::new();
    store.execute(Command::Set(
        "n".into(),
        b"abc".to_vec(),
        Default::default(),
    ));
    let incr = dedup("c", 5, Command::IncrBy("n".into(), 1));
    assert!(matches!(
        write(&state, &store, &incr).await,
        Value::Error(_)
    ));
    store.execute(Command::Del(vec!["n".into()]));
    // Nothing later was accepted, so the id is still new and may succeed.
    assert_eq!(write(&state, &store, &incr).await, Value::Integer(1));
    assert_eq!(write(&state, &store, &incr).await, dup());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn remembered_refusals_are_bounded_per_client() {
    let dir = scratch("refusal_bound");
    let state = durable_state(&dir).await;
    for id in 0..(DEDUP_REFUSALS_PER_CLIENT as u64 + 10) {
        state.remember_dedup_refusal("c", id, "ERR nope");
    }
    let map = state.dedup.lock().unwrap();
    assert_eq!(map["c"].refused.len(), DEDUP_REFUSALS_PER_CLIENT);
    assert_eq!(
        map["c"].refused.front().map(|r| r.0),
        Some(10),
        "oldest forgotten"
    );
    drop(map);
    let _ = std::fs::remove_dir_all(&dir);
}

// ── WebSocket: barrier, push framing, subscribe, scope ───────────────────────

fn dedup_args<'a>(client: &'a str, id: &'a str, inner: &[&'a str]) -> Vec<&'a str> {
    let mut v = vec!["DEDUP", client, id];
    v.extend_from_slice(inner);
    v
}

#[tokio::test]
async fn writes_after_subscribe_are_accepted_over_websocket() {
    // The RESP2 subscribe-mode rule refused them for good, so the browser SDK
    // dropped every write queued after `subscribe()`.
    let srv = spawn_ws_server().await;
    let mut c = WsClient::connect(srv.tcp_addr).await;
    let ack = c.cmd(&["SUBSCRIBE", "news"]).await;
    assert!(matches!(ack, Value::Array(_)), "{ack:?}");
    assert_eq!(
        c.cmd(&dedup_args("tab", "1", &["SET", "after", "yes"]))
            .await,
        Value::SimpleString("OK".into())
    );
    assert_eq!(get(&srv.store, "after"), bulk("yes"));
}

#[tokio::test]
async fn the_dedup_barrier_defers_every_later_write_after_a_deferred_one() {
    let srv = spawn_ws_server().await;
    let mut c = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(
        c.cmd(&["CLIENT", "DEDUPBARRIER", "ON"]).await,
        Value::SimpleString("OK".into())
    );
    srv.state
        .persistence_healthy
        .store(false, Ordering::Release);
    assert!(matches!(
        c.cmd(&dedup_args("tab", "10", &["SET", "a", "1"])).await,
        Value::Error(e) if e.starts_with("MISCONF")
    ));
    // Persistence recovers, but the next write must not jump the queue: it
    // would raise the mark past id 10, whose replay would then be skipped.
    srv.state.persistence_healthy.store(true, Ordering::Release);
    assert!(matches!(
        c.cmd(&dedup_args("tab", "11", &["SET", "b", "1"])).await,
        Value::Error(e) if e.starts_with("TRYAGAIN")
    ));
    assert_eq!(get(&srv.store, "b"), Value::BulkString(None));

    // The reconnect replays both, in order, and both apply.
    let mut again = WsClient::connect(srv.tcp_addr).await;
    for (id, key) in [("10", "a"), ("11", "b")] {
        assert_eq!(
            again.cmd(&dedup_args("tab", id, &["SET", key, "1"])).await,
            Value::SimpleString("OK".into())
        );
    }
}

#[tokio::test]
async fn without_the_barrier_a_later_write_still_applies() {
    // 0.4 behaviour, kept for clients that do not opt in.
    let srv = spawn_ws_server().await;
    let mut c = WsClient::connect(srv.tcp_addr).await;
    srv.state
        .persistence_healthy
        .store(false, Ordering::Release);
    assert!(matches!(
        c.cmd(&dedup_args("old", "10", &["SET", "a", "1"])).await,
        Value::Error(_)
    ));
    srv.state.persistence_healthy.store(true, Ordering::Release);
    assert_eq!(
        c.cmd(&dedup_args("old", "11", &["SET", "b", "1"])).await,
        Value::SimpleString("OK".into())
    );
}

#[tokio::test]
async fn push_framing_sends_keychanges_as_pushes() {
    let srv = spawn_ws_server().await;
    let mut c = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(
        c.cmd(&["CLIENT", "PUSHFRAMES", "ON"]).await,
        Value::SimpleString("OK".into())
    );
    let snapshot = c.cmd(&["QSUB", "doc:*"]).await;
    assert!(
        matches!(snapshot, Value::Array(Some(_))),
        "qstate stays a reply"
    );
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    writer.cmd(&["SET", "doc:1", "v"]).await;
    let frame = c.recv_any(2_000).await.expect("a keychange");
    assert!(frame.starts_with(">3\r\n$9\r\nkeychange"), "{frame:?}");
}

#[tokio::test]
async fn without_push_framing_keychanges_stay_arrays() {
    let srv = spawn_ws_server().await;
    let mut c = WsClient::connect(srv.tcp_addr).await;
    c.cmd(&["QSUB", "doc:*"]).await;
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    writer.cmd(&["SET", "doc:1", "v"]).await;
    let frame = c.recv_any(2_000).await.expect("a keychange");
    assert!(frame.starts_with("*3\r\n$9\r\nkeychange"), "{frame:?}");
}

#[tokio::test]
async fn test_13_an_applied_write_reads_as_applied_after_the_token_narrowed() {
    let secret = "s3cret";
    let srv = spawn_ws_server_cfg(Some(secret.to_string())).await;
    let mut before = WsClient::connect(srv.tcp_addr).await;
    before
        .cmd(&["SYNC", "TOKEN", &mint_sync_token(secret, "cart:*")])
        .await;
    assert_eq!(
        before
            .cmd(&dedup_args("phone", "3", &["INCRBY", "cart:n", "1"]))
            .await,
        Value::Integer(1)
    );
    // The reply was lost; the app reconnects with a token for other keys.
    let mut after = WsClient::connect(srv.tcp_addr).await;
    after
        .cmd(&["SYNC", "TOKEN", &mint_sync_token(secret, "other:*")])
        .await;
    assert_eq!(
        after
            .cmd(&dedup_args("phone", "3", &["INCRBY", "cart:n", "1"]))
            .await,
        dup(),
        "applied, so reported applied — not refused"
    );
    // A new write to the key is still out of scope.
    assert!(matches!(
        after
            .cmd(&dedup_args("phone", "4", &["INCRBY", "cart:n", "1"]))
            .await,
        Value::Error(e) if e.starts_with("NOSCOPE")
    ));
    assert_eq!(get(&srv.store, "cart:n"), bulk("1"));
}

#[tokio::test]
async fn test_07_an_out_of_scope_refusal_is_remembered_below_the_mark() {
    let secret = "s3cret";
    let srv = spawn_ws_server_cfg(Some(secret.to_string())).await;
    let token = mint_sync_token(secret, "cart:*");
    let mut c = WsClient::connect(srv.tcp_addr).await;
    c.cmd(&["SYNC", "TOKEN", &token]).await;
    assert!(matches!(
        c.cmd(&dedup_args("phone", "1", &["SET", "admin:1", "x"])).await,
        Value::Error(e) if e.starts_with("NOSCOPE")
    ));
    c.cmd(&dedup_args("phone", "2", &["SET", "cart:1", "x"]))
        .await;
    // Both replies lost; the replay of the refused one is refused again.
    let mut again = WsClient::connect(srv.tcp_addr).await;
    again.cmd(&["SYNC", "TOKEN", &token]).await;
    assert!(matches!(
        again.cmd(&dedup_args("phone", "1", &["SET", "admin:1", "x"])).await,
        Value::Error(e) if e.starts_with("NOSCOPE")
    ));
    assert_eq!(
        again
            .cmd(&dedup_args("phone", "2", &["SET", "cart:1", "x"]))
            .await,
        dup()
    );
}

#[test]
fn the_aof_envelope_wraps_the_broadcast_frame_exactly() {
    let frame = resp_push(&[b"SET", b"k", b"\x00\xff"]);
    let wrapped = dedup_wrap("c", 42, &frame);
    let (value, used) = Value::parse(&wrapped).unwrap();
    assert_eq!(used, wrapped.len());
    let Value::Push(items) = value else {
        panic!("a push frame");
    };
    let cmd = Command::from_value(Value::Array(Some(items))).unwrap();
    assert_eq!(
        cmd,
        Command::Dedup(
            "c".into(),
            42,
            Box::new(Command::Set("k".into(), vec![0, 255], Default::default()))
        )
    );
}

#[tokio::test]
async fn a_duplicate_suppressed_binary_write_is_accepted_over_websocket() {
    // Every browser `setBytes()` and mobile binary `set()` arrives like this.
    // Before 0.5 the parser checked the whole envelope against DEDUP's own
    // (empty) value positions and refused it for good.
    let srv = spawn_ws_server().await;
    let mut c = WsClient::connect(srv.tcp_addr).await;
    let binary: &[u8] = &[0x00, 0xff, 0x1b, 0x80];
    let mut frame =
        b"*6\r\n$5\r\nDEDUP\r\n$3\r\ntab\r\n$1\r\n1\r\n$3\r\nSET\r\n$3\r\nbin\r\n$4\r\n".to_vec();
    frame.extend_from_slice(binary);
    frame.extend_from_slice(b"\r\n");
    assert_eq!(
        c.cmd_binary(frame.clone()).await,
        Value::SimpleString("OK".into())
    );
    assert_eq!(
        get(&srv.store, "bin"),
        Value::BulkString(Some(binary.to_vec()))
    );
    // And its replay is recognised.
    assert_eq!(c.cmd_binary(frame).await, dup());
}

#[tokio::test]
async fn a_client_pipelining_a_long_replay_is_not_disconnected_by_its_own_echoes() {
    // A reconnect replays the whole outbox at once. Every write is broadcast,
    // and the writer's own socket has to drain those pushes (it skips its
    // own) — which the connection loop, choosing the socket arm five times in
    // six, did slower than they arrived: past ~600 writes the channel lagged
    // and the server closed the socket "sync stream gap" mid-replay.
    let srv = spawn_ws_server().await;
    let mut c = WsClient::connect(srv.tcp_addr).await;
    const WRITES: usize = 3_000;
    for i in 0..WRITES {
        let id = (i + 1).to_string();
        let key = format!("replay:{i}");
        let mut frame = format!(
            "*6\r\n$5\r\nDEDUP\r\n$3\r\ntab\r\n${}\r\n{id}\r\n",
            id.len()
        );
        frame.push_str(&format!(
            "$3\r\nSET\r\n${}\r\n{key}\r\n$1\r\nv\r\n",
            key.len()
        ));
        c.ws.feed(Message::Text(frame.into())).await.unwrap();
    }
    c.ws.flush().await.unwrap();
    for i in 0..WRITES {
        match tokio::time::timeout(std::time::Duration::from_secs(10), c.ws.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => assert_eq!(t.as_str(), "+OK\r\n", "reply {i}"),
            other => panic!("reply {i}: {other:?}"),
        }
    }
    assert_eq!(
        get(&srv.store, &format!("replay:{}", WRITES - 1)),
        bulk("v")
    );
}

// ── v0.5.1 audit regressions ─────────────────────────────────────────────────

fn set(key: &str) -> Command {
    Command::Set(key.into(), b"1".to_vec(), Default::default())
}

#[tokio::test]
async fn a_refusal_before_any_accepted_write_does_not_mark_id_zero_applied() {
    // A remembered refusal created the client's entry with a default mark of
    // 0, which claims wire id 0 was applied. A client without the barrier
    // whose first write (id 0) was deferred and whose second was refused for
    // good then had its replay of id 0 answered `+DUP` — retired, never run.
    let dir = scratch("refusal_id_zero");
    {
        let state = durable_state(&dir).await;
        let store = KeyValueStore::new();
        store.execute(set("text"));
        store.execute(Command::Append("text".into(), b"x".to_vec()));
        state.persistence_healthy.store(false, Ordering::Release);
        assert!(matches!(
            write(&state, &store, &dedup("fresh", 0, set("first"))).await,
            Value::Error(e) if e.starts_with("MISCONF")
        ));
        state.persistence_healthy.store(true, Ordering::Release);
        assert!(matches!(
            write(&state, &store, &dedup("fresh", 1, Command::HSet("text".into(), vec![("f".into(), b"v".to_vec())]))).await,
            Value::Error(e) if e.starts_with("WRONGTYPE")
        ));
        // The reconnect replays id 0: it was never applied, so it must run.
        assert_eq!(
            write(&state, &store, &dedup("fresh", 0, set("first"))).await,
            Value::SimpleString("OK".into())
        );
    }
    let _ = std::fs::remove_dir_all(&dir);

    // The same after a crash (refusal from the AOF) and after a checkpoint
    // (refusal from the sidecar).
    for checkpoint in [false, true] {
        let dir = scratch(if checkpoint {
            "refusal_id_zero_sidecar"
        } else {
            "refusal_id_zero_aof"
        });
        {
            let state = durable_state(&dir).await;
            let store = KeyValueStore::new();
            store.execute(set("text"));
            assert!(matches!(
                write(
                    &state,
                    &store,
                    &dedup(
                        "fresh",
                        1,
                        Command::HSet("text".into(), vec![("f".into(), b"v".to_vec())])
                    )
                )
                .await,
                Value::Error(_)
            ));
            if checkpoint {
                state.save(&store).await.unwrap();
            }
        }
        let (store, state) = recover(&dir).await;
        assert_eq!(
            write(&state, &store, &dedup("fresh", 0, set("first"))).await,
            Value::SimpleString("OK".into()),
            "checkpoint={checkpoint}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tokio::test]
async fn a_retry_that_succeeded_after_a_refusal_reads_as_applied_after_a_crash() {
    // Live, the success forgets the refusal. Recovery merged every AOF mark
    // first and every AOF refusal after, so the refusal came back and the
    // replay of a write that *was* applied was answered with its old error.
    let dir = scratch("refusal_then_success_crash");
    {
        let state = durable_state(&dir).await;
        let store = KeyValueStore::new();
        store.execute(Command::Set(
            "n".into(),
            b"abc".to_vec(),
            Default::default(),
        ));
        let incr = dedup("c", 5, Command::IncrBy("n".into(), 1));
        assert!(matches!(
            write(&state, &store, &incr).await,
            Value::Error(_)
        ));
        store.execute(Command::Del(vec!["n".into()]));
        // Retried while still above the mark: runs and succeeds.
        assert_eq!(write(&state, &store, &incr).await, Value::Integer(1));
        assert_eq!(write(&state, &store, &incr).await, dup());
    }
    let (store, state) = recover(&dir).await;
    assert_eq!(get(&store, "n"), bulk("1"));
    assert_eq!(
        write(
            &state,
            &store,
            &dedup("c", 5, Command::IncrBy("n".into(), 1))
        )
        .await,
        dup(),
        "applied before the crash, so applied after it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_write_refused_twice_then_applied_reads_as_applied_after_a_crash() {
    // Each refusal of the same write leaves its own AOF record, and the
    // success that follows clears all of them live. Recovery must cancel
    // every one, not only the latest.
    let dir = scratch("refused_twice_then_success_crash");
    {
        let state = durable_state(&dir).await;
        let store = KeyValueStore::new();
        store.execute(Command::Set(
            "n".into(),
            b"abc".to_vec(),
            Default::default(),
        ));
        let incr = dedup("c", 5, Command::IncrBy("n".into(), 1));
        for _ in 0..2 {
            assert!(matches!(
                write(&state, &store, &incr).await,
                Value::Error(_)
            ));
        }
        store.execute(Command::Del(vec!["n".into()]));
        assert_eq!(write(&state, &store, &incr).await, Value::Integer(1));
    }
    let (store, state) = recover(&dir).await;
    assert_eq!(get(&store, "n"), bulk("1"));
    assert_eq!(
        write(
            &state,
            &store,
            &dedup("c", 5, Command::IncrBy("n".into(), 1))
        )
        .await,
        dup(),
        "applied before the crash, so applied after it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_torn_aof_tail_does_not_poison_the_next_restart() {
    // Startup skipped the torn record but left its bytes in place, and the
    // AOF is opened for append — so every later write landed behind the
    // fragment, and the *next* restart refused to start ("AOF corrupted").
    let dir = scratch("torn_tail_twice");
    {
        let state = durable_state(&dir).await;
        let store = KeyValueStore::new();
        write(
            &state,
            &store,
            &dedup("c", 1, Command::IncrBy("n".into(), 2)),
        )
        .await;
        write(
            &state,
            &store,
            &dedup("c", 2, Command::IncrBy("n".into(), 3)),
        )
        .await;
    }
    let path = dir.join("append.aof");
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() - 3]).unwrap();

    // First restart: the torn write is dropped, and the retry is logged.
    {
        let (store, state) = recover(&dir).await;
        assert_eq!(get(&store, "n"), bulk("2"));
        assert_eq!(
            write(
                &state,
                &store,
                &dedup("c", 2, Command::IncrBy("n".into(), 3))
            )
            .await,
            Value::Integer(5)
        );
    }
    // Second restart: everything since the first is still there.
    let (store, state) = recover(&dir).await;
    assert_eq!(get(&store, "n"), bulk("5"));
    assert_eq!(
        write(
            &state,
            &store,
            &dedup("c", 2, Command::IncrBy("n".into(), 3))
        )
        .await,
        dup()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_remembered_refusal_keeps_a_bounded_error() {
    // An out-of-scope refusal echoes the key, which may be 512 KB: 64 of them
    // per client id, under any number of ids, held outside maxmemory and
    // rewritten into the sidecar at every checkpoint while writes are blocked.
    let dir = scratch("refusal_error_bound");
    let state = durable_state(&dir).await;
    let reason = format!(
        "NOSCOPE key '{}' is outside this connection's sync scopes",
        "é".repeat(256 * 1024)
    );
    state.remember_dedup_refusal("c", 1, &reason);
    let kept = state.dedup.lock().unwrap()["c"].refused[0].1.clone();
    assert!(kept.len() <= DEDUP_REFUSAL_ERROR_BYTES, "{}", kept.len());
    assert!(kept.starts_with("NOSCOPE key 'éé"), "keeps the class");
    assert!(!core_engine::resp::is_transient_refusal(&kept));
    // A short error is kept verbatim.
    state.remember_dedup_refusal("c", 2, "WRONGTYPE nope");
    assert_eq!(
        state.dedup.lock().unwrap()["c"].refused[1].1,
        "WRONGTYPE nope"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn an_out_of_scope_keyspace_wide_dedup_write_is_remembered_below_the_mark() {
    let secret = "s3cret";
    let srv = spawn_ws_server_cfg(Some(secret.to_string())).await;
    let token = mint_sync_token(secret, "cart:*");
    let mut c = WsClient::connect(srv.tcp_addr).await;
    c.cmd(&["SYNC", "TOKEN", &token]).await;
    assert!(matches!(
        c.cmd(&dedup_args("phone", "1", &["FLUSHDB"])).await,
        Value::Error(e) if e.starts_with("NOSCOPE")
    ));
    assert_eq!(
        c.cmd(&dedup_args("phone", "2", &["SET", "cart:1", "x"]))
            .await,
        Value::SimpleString("OK".into())
    );
    // Both replies lost: the replay of the refused write is refused again,
    // not answered `+DUP` and retired as applied.
    let mut again = WsClient::connect(srv.tcp_addr).await;
    again.cmd(&["SYNC", "TOKEN", &token]).await;
    assert!(matches!(
        again.cmd(&dedup_args("phone", "1", &["FLUSHDB"])).await,
        Value::Error(e) if e.starts_with("NOSCOPE")
    ));
}

#[tokio::test]
async fn a_connection_without_a_sync_token_cannot_read_dedup_marks() {
    // Strict mode: the token is the authorization. Answering replays before
    // a token was presented let any anonymous socket that knew a client id
    // read its mark and its remembered refusals — which echo key names from
    // another tenant's scope.
    let secret = "s3cret";
    let srv = spawn_ws_server_cfg(Some(secret.to_string())).await;
    let mut owner = WsClient::connect(srv.tcp_addr).await;
    owner
        .cmd(&["SYNC", "TOKEN", &mint_sync_token(secret, "cart:*")])
        .await;
    owner
        .cmd(&dedup_args("victim", "1", &["SET", "secret:plan", "x"]))
        .await;
    owner
        .cmd(&dedup_args("victim", "2", &["SET", "cart:1", "x"]))
        .await;

    let mut anon = WsClient::connect(srv.tcp_addr).await;
    for id in ["1", "2"] {
        let reply = anon
            .cmd(&dedup_args("victim", id, &["SET", "x", "y"]))
            .await;
        assert!(
            matches!(&reply, Value::Error(e) if e.contains("send SYNC TOKEN")),
            "id {id}: {reply:?}"
        );
    }
}

#[tokio::test]
async fn an_out_of_scope_refusal_is_recorded_under_the_write_barrier() {
    // A checkpoint blocks writes while it writes the sidecars and the
    // snapshot and truncates the AOF. A refusal recorded outside that barrier
    // could miss the sidecar and land in the AOF just before the truncate —
    // lost from both, so after a crash its replay read as applied.
    let secret = "s3cret";
    let srv = spawn_ws_server_cfg(Some(secret.to_string())).await;
    let mut c = WsClient::connect(srv.tcp_addr).await;
    c.cmd(&["SYNC", "TOKEN", &mint_sync_token(secret, "cart:*")])
        .await;
    let checkpoint = srv.state.replicas.lock_all_writes().await;
    let req =
        "*6\r\n$5\r\nDEDUP\r\n$5\r\nphone\r\n$1\r\n1\r\n$3\r\nSET\r\n$7\r\nadmin:1\r\n$1\r\nx\r\n";
    c.ws.send(Message::Text(req.into())).await.unwrap();
    assert_eq!(
        c.recv_any(300).await,
        None,
        "the refusal waits for the checkpoint"
    );
    drop(checkpoint);
    let reply = c.recv_any(2_000).await.expect("the refusal");
    assert!(reply.starts_with("-NOSCOPE"), "{reply}");
}
