//! 6O-I4: a capable replica applies a transaction group atomically
//! (`benchmarks/v0.6.0/6o-i/V0.6O-I-PROTOCOL-DESIGN.md` §6.4–§6.5).
//!
//! The replica runs a whole group through `KeyValueStore::apply_transaction`,
//! the primary's own boundary: its readers wait for, or validate across, the
//! group's marked stripes, and never see part of it. Then it logs the group
//! (one group in its own AOF) and relays it (one group to its sub-replicas).
//! A member that fails ends the link and forces a full synchronization.

use super::replica_expirylog_6oh::{aof_words, files, replica_with_files};
use super::replication_dedup::{Node, node, serve_replicas, wait_for};
use super::*;
use core_engine::resp::Value;
use core_engine::store::{transaction_fault, write_pause};
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

fn array(items: Vec<Value>) -> Value {
    Value::Array(Some(items))
}

async fn exec(client: &mut WsClient, writes: &[&[&str]]) {
    client.cmd(&["MULTI"]).await;
    for w in writes {
        client.cmd(w).await;
    }
    let reply = client.cmd(&["EXEC"]).await;
    assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
}

/// Follow `primary_addr`, reconnecting, as `run_repl_client` does, on a
/// thread of its own: a group stopped inside its synchronous apply then
/// blocks no worker that other tasks are queued on (6O-G lesson).
fn follow_on_own_thread(replica: &Node, primary_addr: std::net::SocketAddr) {
    let (store, state, tx, watches) = (
        Arc::clone(&replica.store),
        Arc::clone(&replica.state),
        replica.tx.clone(),
        Arc::clone(&replica.watches),
    );
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(async move {
                let mut resume = ReplResume::default();
                loop {
                    if let Ok(mut socket) = TcpStream::connect(primary_addr).await
                        && let Err(e) = sync_from_primary(
                            &mut socket,
                            &store,
                            None,
                            &tx,
                            &watches,
                            &state,
                            &mut resume,
                        )
                        .await
                        && e.kind() == std::io::ErrorKind::InvalidData
                    {
                        resume = ReplResume::default();
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            });
    });
}

// ── Readers wait for the whole group ─────────────────────────────────────

/// The replica stops inside a group's apply, after its first member, with
/// the group's stripes held and marked. Every kind of read of its keys from
/// the replica's store (the read paths its clients' commands take) waits for
/// the whole group, and then sees all of it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn readers_of_a_replica_wait_for_the_whole_group() {
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    follow_on_own_thread(&replica, serve_replicas(&primary).await);
    let mut client = WsClient::connect(primary.ws).await;
    type Read = (&'static str, fn(&str, &str) -> Command, Value);
    let reads: [Read; 6] = [
        ("get-first", |a, _| Command::Get(a.into()), bulk("new")),
        ("get-later", |_, b| Command::Get(b.into()), bulk("newer")),
        (
            "mget",
            |a, b| Command::MGet(vec![b.into(), a.into()]),
            array(vec![bulk("newer"), bulk("new")]),
        ),
        (
            "exists",
            |a, b| Command::Exists(vec![a.into(), b.into()]),
            Value::Integer(2),
        ),
        (
            "type",
            |_, b| Command::Type(b.into()),
            Value::SimpleString("string".into()),
        ),
        (
            "strlen",
            |_, b| Command::Strlen(b.into()),
            Value::Integer(5),
        ),
    ];
    for (label, read, want) in reads {
        let (a, b) = (format!("6oi:r:{label}:a"), format!("6oi:r:{label}:b"));
        // `b` absent before: EXISTS counts 1 only if part-applied.
        client.cmd(&["SET", &a, "old"]).await;
        wait_for(&replica.store, &a, bulk("old")).await;
        let pause = write_pause::after_transaction_write_of(&replica.store, &a, &[&a, &b]);
        exec(&mut client, &[&["SET", &a, "new"], &["SET", &b, "newer"]]).await;
        assert!(
            pause.wait_reached(Duration::from_secs(5)),
            "{label}: the replica never stopped inside the group"
        );
        let done = Arc::new(AtomicBool::new(false));
        let reader = {
            let (store, done, cmd) = (Arc::clone(&replica.store), Arc::clone(&done), read(&a, &b));
            std::thread::spawn(move || {
                let got = store.execute(cmd);
                done.store(true, Ordering::SeqCst);
                got
            })
        };
        let blocked =
            pause.wait_reader_blocked_or(Duration::from_secs(5), || done.load(Ordering::SeqCst));
        assert!(
            blocked && !done.load(Ordering::SeqCst),
            "{label}: a reader of the replica answered while the group was part-applied"
        );
        drop(pause);
        assert_eq!(reader.join().unwrap(), want, "{label}");
    }
}

/// The reproduction of `V0.6O-I-BASELINE.md` §1.2, extended: several
/// readers of the replica, store reads and a client's, while the primary
/// commits overlapping transactions, never see part of one, nor (reading
/// `a` then `b`) a newer `a` than `b`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_replica_reader_never_sees_part_of_an_exec() {
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    follow_on_own_thread(&replica, serve_replicas(&primary).await);
    let mut client = WsClient::connect(primary.ws).await;
    client.cmd(&["MSET", "acct:a", "0", "acct:b", "0"]).await;
    wait_for(&replica.store, "acct:b", bulk("0")).await;
    // A replica slower than its link: it pauses after each frame or group.
    replica
        .state
        .replicas
        .test_apply_delay_us
        .store(20_000, Ordering::Relaxed);
    let stop = Arc::new(AtomicBool::new(false));
    let mut readers = Vec::new();
    for _ in 0..3 {
        let (store, stop) = (Arc::clone(&replica.store), Arc::clone(&stop));
        readers.push(std::thread::spawn(move || {
            let mut wrong = Vec::new();
            let int = |v: &Value| match v {
                Value::BulkString(Some(b)) => String::from_utf8_lossy(b).parse::<u64>().unwrap(),
                other => panic!("{other:?}"),
            };
            while !stop.load(Ordering::Relaxed) {
                let got = store.execute(Command::MGet(vec!["acct:a".into(), "acct:b".into()]));
                if let Value::Array(Some(items)) = &got
                    && items[0] != items[1]
                {
                    wrong.push(format!("MGET {got:?}"));
                }
                let a = int(&store.execute(Command::Get("acct:a".into())));
                let b = int(&store.execute(Command::Get("acct:b".into())));
                if a > b {
                    wrong.push(format!("GET a = {a}, then GET b = {b}"));
                }
            }
            wrong
        }));
    }
    let ws_stop = Arc::clone(&stop);
    let ws = replica.ws;
    let ws_reader = tokio::spawn(async move {
        let mut reader = WsClient::connect(ws).await;
        let mut wrong = Vec::new();
        while !ws_stop.load(Ordering::Relaxed) {
            let got = reader.cmd(&["MGET", "acct:a", "acct:b"]).await;
            if let Value::Array(Some(items)) = &got
                && items[0] != items[1]
            {
                wrong.push(format!("client MGET {got:?}"));
            }
        }
        wrong
    });
    // Overlapping transactions from two clients, and an unrelated writer.
    let mut other = WsClient::connect(primary.ws).await;
    for round in 1..=20u32 {
        let v = round.to_string();
        exec(
            &mut client,
            &[&["SET", "acct:a", &v], &["SET", "acct:b", &v]],
        )
        .await;
        other.cmd(&["SET", "acct:unrelated", &v]).await;
    }
    wait_for(&replica.store, "acct:b", bulk("20")).await;
    stop.store(true, Ordering::Relaxed);
    let mut wrong: Vec<String> = readers
        .into_iter()
        .flat_map(|r| r.join().unwrap())
        .collect();
    wrong.extend(ws_reader.await.unwrap());
    assert!(
        wrong.is_empty(),
        "{} part-applied observation(s), e.g. {:?}",
        wrong.len(),
        wrong.first()
    );
}

// ── Effects: the replica's own AOF, its sub-replicas ─────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_replica_logs_each_group_as_one_group_and_restarts_whole() {
    let dir = files("6oi_replica_group");
    let primary = node(false, true).await;
    let replica = replica_with_files(&dir).await;
    follow_on_own_thread(&replica, serve_replicas(&primary).await);
    let mut client = WsClient::connect(primary.ws).await;
    client.cmd(&["SET", "rg:x", "gone", "PX", "1"]).await;
    tokio::time::sleep(Duration::from_millis(5)).await;
    exec(
        &mut client,
        &[
            &["SET", "rg:a", "1"],
            &["APPEND", "rg:x", "new"],
            &["SET", "rg:b", "1"],
        ],
    )
    .await;
    wait_for(&replica.store, "rg:b", bulk("1")).await;
    let words: Vec<String> = aof_words(&dir).into_iter().map(|(_, w)| w).collect();
    let begin = format!(
        "RECACHED-CHECKPOINT {}",
        Control::TxnBegin { count: 4 }.id()
    );
    let commit = format!(
        "RECACHED-CHECKPOINT {}",
        Control::TxnCommit { count: 4 }.id()
    );
    let at = words.iter().position(|w| *w == begin).expect("a group");
    assert_eq!(
        words[at..=at + 5],
        [
            begin,
            "SET rg:a 1".into(),
            "DEL rg:x".into(),
            "APPEND rg:x new".into(),
            "SET rg:b 1".into(),
            commit,
        ],
        "{words:?}"
    );
    // Restarted from its files alone, it has the whole group.
    let store = KeyValueStore::new();
    let loaded = load_snapshot_checkpoint(&store, &dir.0.join("dump.rdb"))
        .await
        .unwrap();
    replay_aof_format(
        &store,
        &dir.0.join("append.aof"),
        loaded.aof_checkpoint,
        false,
    )
    .await
    .unwrap();
    for (key, want) in [("rg:a", "1"), ("rg:x", "new"), ("rg:b", "1")] {
        assert_eq!(store.execute(Command::Get(key.into())), bulk(want), "{key}");
    }
}

/// A → B → C: B relays each group, applied, as a group (with its header,
/// to a capable sub-replica); C's readers never see part of it either.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_replica_relays_each_group_to_its_sub_replicas_as_a_group() {
    let a = node(false, true).await;
    let b = node(true, true).await;
    let c = node(true, true).await;
    follow_on_own_thread(&b, serve_replicas(&a).await);
    let b_addr = serve_replicas(&b).await;
    follow_on_own_thread(&c, b_addr);
    let mut client = WsClient::connect(a.ws).await;
    client.cmd(&["SET", "ch:a", "0"]).await;
    wait_for(&c.store, "ch:a", bulk("0")).await;
    // After B's own full sync, which resets its stream (6O-G).
    let mut rx = b.state.replicas.attach_test_replica(1 << 16, 1 << 30).await;
    let pause = write_pause::after_transaction_write_of(&c.store, "ch:a", &["ch:a", "ch:b"]);
    exec(&mut client, &[&["SET", "ch:a", "1"], &["SET", "ch:b", "1"]]).await;
    assert!(
        pause.wait_reached(Duration::from_secs(5)),
        "C applies it as a group"
    );
    let done = Arc::new(AtomicBool::new(false));
    let reader = {
        let (store, done) = (Arc::clone(&c.store), Arc::clone(&done));
        std::thread::spawn(move || {
            let got = store.execute(Command::MGet(vec!["ch:a".into(), "ch:b".into()]));
            done.store(true, Ordering::SeqCst);
            got
        })
    };
    assert!(pause.wait_reader_blocked_or(Duration::from_secs(5), || done.load(Ordering::SeqCst)));
    drop(pause);
    assert_eq!(reader.join().unwrap(), array(vec![bulk("1"), bulk("1")]));
    // What B queued for its sub-replicas: the SET, then the two frames as
    // one group with consecutive offsets.
    let mut frames = Vec::new();
    while let Ok(queued) = rx.try_recv() {
        frames.push((queued.offset(), queued.group()));
    }
    let grouped: Vec<_> = frames.iter().filter(|(_, g)| g.is_some()).collect();
    assert_eq!(grouped.len(), 2, "{frames:?}");
    let g = grouped[0].1.unwrap();
    assert_eq!(
        (g.count, grouped[0].0, grouped[1].0),
        (2, g.first, g.first + 1)
    );
}

// ── Failure: a member that fails, or panics ──────────────────────────────

/// A primary speaking the protocol by hand: one connection.
async fn fake_primary_link(listener: &TcpListener) -> (TcpStream, String) {
    let (mut sock, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .expect("the replica connects")
        .unwrap();
    let advertised = "fake-run;caps=expirylog1,txn1";
    sock.write_all(REPL_MAGIC).await.unwrap();
    sock.write_all(&(advertised.len() as u16).to_le_bytes())
        .await
        .unwrap();
    sock.write_all(advertised.as_bytes()).await.unwrap();
    let mut len = [0u8; 2];
    sock.read_exact(&mut len).await.unwrap();
    let mut id = vec![0u8; usize::from(u16::from_le_bytes(len))];
    sock.read_exact(&mut id).await.unwrap();
    let mut offset = [0u8; 8];
    sock.read_exact(&mut offset).await.unwrap();
    let run = String::from_utf8(id).unwrap();
    (sock, run.split(';').next().unwrap_or_default().to_string())
}

async fn full_sync(sock: &mut TcpStream, boundary: u64) {
    sock.write_all(b"F").await.unwrap();
    sock.write_all(&boundary.to_le_bytes()).await.unwrap();
    let empty = rmp_serde::to_vec(&Vec::<SnapshotEntry>::new()).unwrap();
    sock.write_all(&(empty.len() as u32).to_le_bytes())
        .await
        .unwrap();
    sock.write_all(&empty).await.unwrap();
    let mut ack = [0u8; 8];
    sock.read_exact(&mut ack).await.unwrap();
}

async fn send(sock: &mut TcpStream, offset: u64, bytes: &[u8]) {
    let mut out = offset.to_le_bytes().to_vec();
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
    let _ = sock.write_all(&out).await;
}

fn frame(words: &[&str]) -> Vec<u8> {
    let parts: Vec<&[u8]> = words.iter().map(|w| w.as_bytes()).collect();
    resp_push(&parts)
}

async fn send_group(sock: &mut TcpStream, first: u64, frames: &[Vec<u8>]) {
    let header = group_header(ReplGroup {
        first,
        count: frames.len() as u32,
        bytes: frames.iter().map(|f| f.len() as u64).sum(),
    });
    let _ = sock.write_all(&header).await;
    for (i, f) in frames.iter().enumerate() {
        send(sock, first + i as u64, f).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_member_that_fails_stops_the_group_and_forces_a_full_sync() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let replica = node(true, false).await;
    follow_on_own_thread(&replica, listener.local_addr().unwrap());
    let (mut sock, _) = fake_primary_link(&listener).await;
    full_sync(&mut sock, 10).await;
    send(&mut sock, 11, &frame(&["SET", "f:s", "text"])).await;
    // INCR of a string that is not a number: the replica diverged.
    send_group(
        &mut sock,
        12,
        &[
            frame(&["SET", "f:a", "1"]),
            frame(&["INCR", "f:s"]),
            frame(&["SET", "f:after", "1"]),
        ],
    )
    .await;
    let (_sock, run) = fake_primary_link(&listener).await;
    assert_eq!(run, "", "a full synchronization is asked for");
    // The member after the failing one did not run.
    assert_eq!(
        replica.store.execute(Command::Get("f:after".into())),
        Value::BulkString(None)
    );
    // Without an AOF there is nothing to protect: no MISCONF latch.
    assert!(replica.state.persistence_is_healthy());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_member_that_panics_is_contained_and_forces_a_full_sync() {
    let dir = files("6oi_replica_panic");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let replica = replica_with_files(&dir).await;
    follow_on_own_thread(&replica, listener.local_addr().unwrap());
    let (mut sock, _) = fake_primary_link(&listener).await;
    full_sync(&mut sock, 10).await;
    let armed = transaction_fault::panic_during_mutation_of("6oi:panic:b");
    send_group(
        &mut sock,
        11,
        &[
            frame(&["SET", "6oi:panic:a", "1"]),
            frame(&["SET", "6oi:panic:b", "1"]),
            frame(&["SET", "6oi:panic:c", "1"]),
        ],
    )
    .await;
    let (_sock, run) = fake_primary_link(&listener).await;
    drop(armed);
    assert_eq!(run, "", "a full synchronization is asked for");
    assert_eq!(
        replica.store.execute(Command::Get("6oi:panic:c".into())),
        Value::BulkString(None),
        "nothing after the panic ran"
    );
    // Its AOF no longer describes its keyspace: latched until a checkpoint.
    assert!(!replica.state.persistence_is_healthy());
    let words = aof_words(&dir);
    assert!(
        !words.iter().any(|(_, w)| w.starts_with("SET 6oi:panic:")),
        "nothing of the group was logged: {words:?}"
    );
}

/// Review A R2-N2: the group bound charges 128 bytes per frame for the
/// `Vec` that holds it and its parsed command's fixed part.
#[test]
fn a_frames_fixed_cost_fits_the_per_frame_charge() {
    let fixed = std::mem::size_of::<Vec<u8>>() + std::mem::size_of::<Command>();
    assert!(
        fixed <= REPL_QUEUE_FRAME_OVERHEAD,
        "{fixed} bytes per frame against a charge of {REPL_QUEUE_FRAME_OVERHEAD}"
    );
}

// ── Review C H1: notifications coalesced as the primary's ────────────────

fn text(v: &Value) -> String {
    match v {
        Value::BulkString(Some(b)) => String::from_utf8_lossy(b).into_owned(),
        Value::SimpleString(s) => s.clone(),
        other => format!("{other:?}"),
    }
}

/// A delta-subscribed live query on a replica, while the primary runs
/// `MULTI; SET k a; APPEND k b; EXEC`: the client's copy of `k` must end as
/// the replica's ("ab"). Announcing each member after the whole group sent
/// the final value, then the APPEND's delta ("abb").
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_delta_live_query_on_a_replica_converges_after_a_group() {
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    follow_on_own_thread(&replica, serve_replicas(&primary).await);
    let mut client = WsClient::connect(primary.ws).await;
    client.cmd(&["SET", "dl:seed", "x"]).await;
    wait_for(&replica.store, "dl:seed", bulk("x")).await;
    let mut viewer = WsClient::connect(replica.ws).await;
    viewer.send(&["CLIENT", "DELTA", "ON"]).await;
    viewer.send(&["QSUB", "dl:k*"]).await;
    while viewer.recv_frame(300).await.is_some() {}
    exec(
        &mut client,
        &[&["SET", "dl:k", "a"], &["APPEND", "dl:k", "b"]],
    )
    .await;
    wait_for(&replica.store, "dl:k", bulk("ab")).await;
    let mut model: Option<Vec<u8>> = None;
    let mut seen = Vec::new();
    while let Some(raw) = viewer.recv_frame(500).await {
        let Ok((v, _)) = Value::parse(&raw) else {
            continue;
        };
        let items = match &v {
            Value::Array(Some(items)) | Value::Push(items) => items.clone(),
            _ => continue,
        };
        if items.len() < 3 {
            continue;
        }
        seen.push(format!("{v:?}"));
        let kind = text(&items[0]);
        if kind == "keychange" && text(&items[1]) == "dl:k" {
            model = match &items[2] {
                Value::BulkString(Some(b)) => Some(b.clone()),
                _ => None,
            };
        } else if kind == "keydelta" && text(&items[1]) == "dl:k" && text(&items[2]) == "append" {
            let mut now = model.clone().unwrap_or_default();
            if let Value::BulkString(Some(b)) = &items[3] {
                now.extend_from_slice(b);
            }
            model = Some(now);
        }
    }
    assert_eq!(
        model.map(|b| String::from_utf8_lossy(&b).into_owned()),
        Some("ab".to_string()),
        "frames: {seen:#?}"
    );
}

/// A group that creates a key and then deletes it invalidates a replica
/// client's `WATCH` of that (absent) key, as the primary's `EXEC` would.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_group_that_sets_then_removes_a_key_invalidates_a_replica_watch() {
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    follow_on_own_thread(&replica, serve_replicas(&primary).await);
    let mut client = WsClient::connect(primary.ws).await;
    client.cmd(&["SADD", "ws:src", "m"]).await;
    for _ in 0..200 {
        if replica.store.execute(Command::SCard("ws:src".into())) == Value::Integer(1) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let mut watcher = WsClient::connect(replica.ws).await;
    assert_eq!(
        watcher.cmd(&["WATCH", "ws:dst"]).await,
        Value::SimpleString("OK".into())
    );
    exec(
        &mut client,
        &[
            &["SADD", "ws:src", "n"],
            &["SINTERSTORE", "ws:dst", "ws:src"],
            &["DEL", "ws:dst"],
        ],
    )
    .await;
    for _ in 0..200 {
        if replica.store.execute(Command::SCard("ws:src".into())) == Value::Integer(2) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    watcher.cmd(&["MULTI"]).await;
    watcher.cmd(&["GET", "ws:other"]).await;
    assert_eq!(
        watcher.cmd(&["EXEC"]).await,
        Value::Array(None),
        "the WATCH of ws:dst was invalidated"
    );
}

// ── Reviews C M1 / D D-1: a failed group converges every copy ────────────

/// A → B → C. B diverged from A on one key; a group of A fails on B after
/// its first member applied there. B resets its stream, so C resynchronizes
/// in full from B and ends with what B holds, even when B is promoted before
/// it reconnects to A.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_group_on_a_middle_replica_resets_its_sub_replicas() {
    let a = node(false, true).await;
    let b = node(true, true).await;
    let c = node(true, true).await;
    follow_on_own_thread(&b, serve_replicas(&a).await);
    follow_on_own_thread(&c, serve_replicas(&b).await);
    let mut client = WsClient::connect(a.ws).await;
    client.cmd(&["SET", "fg:pre", "0"]).await;
    wait_for(&c.store, "fg:pre", bulk("0")).await;
    // B (and, relayed, C) diverge from A on fg:list.
    apply_repl_frame_for_tests(
        &b.store,
        &b.state,
        &b.tx,
        &b.watches,
        &resp_push(&[b"SET", b"fg:list", b"str"]),
    )
    .await
    .unwrap();
    wait_for(&c.store, "fg:list", bulk("str")).await;
    let b_run = b.state.replicas.run_id();
    exec(
        &mut client,
        &[&["SET", "fg:k1", "v"], &["LPUSH", "fg:list", "x"]],
    )
    .await;
    wait_for(&b.store, "fg:k1", bulk("v")).await;
    b.state.promote_to_primary(&b.store).await;
    assert_ne!(b.state.replicas.run_id(), b_run, "B reset its stream");
    // C converges on B's keyspace: the applied prefix included.
    wait_for(&c.store, "fg:k1", bulk("v")).await;
}

/// With an AOF, a failed group latches persistence unhealthy (its file no
/// longer describes the keyspace); the members before the failure stay in
/// the store until the full sync replaces it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_group_with_an_aof_latches_and_keeps_its_prefix_until_the_full_sync() {
    let dir = files("6oi_replica_fail_aof");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let replica = replica_with_files(&dir).await;
    follow_on_own_thread(&replica, listener.local_addr().unwrap());
    let (mut sock, _) = fake_primary_link(&listener).await;
    full_sync(&mut sock, 10).await;
    send(&mut sock, 11, &frame(&["SET", "fa:s", "text"])).await;
    send_group(
        &mut sock,
        12,
        &[
            frame(&["SET", "fa:a", "1"]),
            frame(&["INCR", "fa:s"]),
            frame(&["SET", "fa:after", "1"]),
        ],
    )
    .await;
    let (_sock, run) = fake_primary_link(&listener).await;
    assert_eq!(run, "");
    assert_eq!(
        replica.store.execute(Command::Get("fa:a".into())),
        bulk("1")
    );
    assert_eq!(
        replica.store.execute(Command::Get("fa:after".into())),
        Value::BulkString(None)
    );
    assert!(!replica.state.persistence_is_healthy());
}
