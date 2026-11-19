//! 6N-C PR 0b (RV-6, NEW-4): a refused WebSocket DEDUP command takes no
//! global write barrier, and its refusal still lands wholly on one side of
//! every checkpoint and replica cut.
//!
//! A strict socket sending `DEDUP <c> <n> FLUSHDB` is refused as
//! administrative. Recording that refusal took the refused command's
//! stripes — every stripe, with the unordered gate closed — and held them
//! through an AOF append (an fsync under `always`), so any token holder
//! could stall every writer once per message. It now takes only the DEDUP
//! identity's stripe, which still excludes the checkpoint and the
//! sub-replica cut (each holds every stripe), and refusals are paced per
//! connection.

use super::dedup_ack_order::{Node, node, tcp_write};
use super::*;
use std::time::Duration;

const SECRET: &str = "ws-refusal-test-secret-0123456789abcdef";

fn token_for(entries: &str, sub: &str) -> String {
    mint_sync_token(
        SECRET,
        &format!("{entries}|{}|sub={sub}", unix_now_secs() + 3600),
    )
}

/// Serve strict-mode WebSocket clients for `node`.
async fn serve_strict(node: &Node) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (store, state, tx, watches) = (
        Arc::clone(&node.store),
        Arc::clone(&node.state),
        node.tx.clone(),
        Arc::clone(&node.watches),
    );
    tokio::spawn(async move {
        while let Ok((socket, peer)) = listener.accept().await {
            let (s, t, st, wr) = (
                Arc::clone(&store),
                tx.clone(),
                Arc::clone(&state),
                Arc::clone(&watches),
            );
            tokio::spawn(async move {
                handle_ws(
                    socket,
                    s,
                    t,
                    Arc::new(None),
                    next_conn_id(),
                    Arc::new(tokio::sync::Mutex::new(PubSubHub::new())),
                    wr,
                    st,
                    Arc::new(SyncPolicy::strict(SECRET)),
                    Arc::new(None),
                    peer.to_string(),
                )
                .await;
            });
        }
    });
    addr
}

async fn scoped(addr: std::net::SocketAddr, sub: &str) -> WsClient {
    let mut ws = WsClient::connect(addr).await;
    let reply = ws
        .cmd(&[
            "SYNC",
            "TOKEN",
            &token_for(&format!("rw=tenant:{sub}:*"), sub),
        ])
        .await;
    assert!(!matches!(reply, Value::Error(_)), "{reply:?}");
    ws
}

fn refused_flushdb(client: &str, id: u64) -> Vec<String> {
    vec![
        "DEDUP".into(),
        client.into(),
        id.to_string(),
        "FLUSHDB".into(),
    ]
}

async fn send(ws: &mut WsClient, args: &[String]) {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    ws.send(&args).await;
}

fn remembers_refusal(state: &ServerState, client: &str, id: u64) -> bool {
    state
        .dedup_table()
        .into_iter()
        .any(|(c, _, refused)| c == client && refused.iter().any(|(r, _)| *r == id))
}

/// A key on a different stripe from `identity`'s, so a test can show
/// writers there are not held.
fn key_off_stripe(identity: &str) -> String {
    let stripe = |k: &str| {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        k.hash(&mut h);
        h.finish() % 256
    };
    (0..)
        .map(|i| format!("free:{i}"))
        .find(|k| stripe(k) != stripe(identity))
        .unwrap()
}

/// The RV-6 counterexample: while the refusal's AOF append is held (a slow
/// fsync), writers to other keys go on and nothing is flushed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_flushdb_does_not_stall_unrelated_writers() {
    let node = node("refusal_barrier", AofSync::EverySec).await;
    node.store.execute(Command::Set(
        "tenant:alice:keep".into(),
        b"1".to_vec(),
        Default::default(),
    ));
    let addr = serve_strict(&node).await;
    let mut ws = scoped(addr, "alice").await;
    let identity = namespaced_dedup_client("alice", "flusher-0b");
    node.aof.hold_batch_containing(identity.as_bytes());
    send(&mut ws, &refused_flushdb("flusher-0b", 1)).await;
    node.aof.wait_until_held().await;

    // An unrelated writer runs to completion of its command while the
    // refusal is mid-append (before the fix: blocked on every stripe).
    let free = key_off_stripe(&identity);
    let (writer, returned) = tcp_write(
        &node,
        Command::Set(free.clone(), b"v".to_vec(), Default::default()),
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), returned)
            .await
            .expect("an unrelated writer was stalled by the refusal")
            .unwrap(),
        Value::SimpleString("OK".into())
    );
    assert!(
        !remembers_refusal(&node.state, &identity, 1),
        "remembered before its frame was written"
    );
    node.aof.release_hold();
    let reply = ws.recv_frame(5_000).await.expect("the refusal reply");
    assert!(
        String::from_utf8_lossy(&reply).starts_with("-NOSCOPE"),
        "{}",
        String::from_utf8_lossy(&reply)
    );
    assert!(remembers_refusal(&node.state, &identity, 1));
    assert_eq!(
        node.store.execute(Command::Get("tenant:alice:keep".into())),
        Value::BulkString(Some(b"1".to_vec())),
        "the refused FLUSHDB ran"
    );
    assert_eq!(writer.await.unwrap(), Value::SimpleString("OK".into()));
}

/// The keyed branch (6N-C PR 0b review, High): a refused `DEL` naming
/// thousands of keys, one outside the token's scope, used to take one
/// stripe per key — nearly all 256, with the unordered gate closed — across
/// its refusal's append. Held mid-append, it must leave a writer on another
/// stripe, and the all-write barrier itself, free.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_many_key_del_does_not_stall_unrelated_writers() {
    let node = node("refusal_keys", AofSync::EverySec).await;
    let addr = serve_strict(&node).await;
    let mut ws = scoped(addr, "alice").await;
    let identity = namespaced_dedup_client("alice", "deleter-0b");
    node.aof.hold_batch_containing(identity.as_bytes());
    let mut args: Vec<String> = refused_flushdb("deleter-0b", 1);
    args.truncate(3);
    args.push("DEL".into());
    args.extend((0..2_000).map(|i| format!("tenant:alice:k{i}")));
    args.push("tenant:bob:out-of-scope".into());
    send(&mut ws, &args).await;
    node.aof.wait_until_held().await;

    let free = key_off_stripe(&identity);
    let (writer, returned) =
        tcp_write(&node, Command::Set(free, b"v".to_vec(), Default::default()));
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), returned)
            .await
            .expect("an unrelated writer was stalled by a keyed refusal")
            .unwrap(),
        Value::SimpleString("OK".into())
    );
    node.aof.release_hold();
    let reply = ws.recv_frame(5_000).await.expect("the refusal reply");
    assert!(
        String::from_utf8_lossy(&reply).starts_with("-NOSCOPE"),
        "{}",
        String::from_utf8_lossy(&reply)
    );
    assert!(remembers_refusal(&node.state, &identity, 1));
    assert_eq!(writer.await.unwrap(), Value::SimpleString("OK".into()));
}

/// End to end: a checkpoint started while a refusal is being recorded
/// finishes after it, and the refusal survives the checkpoint's AOF
/// truncate in the sidecar it writes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_checkpoint_waits_for_a_refusal_and_keeps_it() {
    let node = node("refusal_save", AofSync::EverySec).await;
    let addr = serve_strict(&node).await;
    let mut ws = scoped(addr, "alice").await;
    let identity = namespaced_dedup_client("alice", "saver-0b");
    node.aof.hold_batch_containing(identity.as_bytes());
    send(&mut ws, &refused_flushdb("saver-0b", 3)).await;
    node.aof.wait_until_held().await;

    let (state, store) = (Arc::clone(&node.state), Arc::clone(&node.store));
    let save = tokio::spawn(async move { state.save(&store).await });
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(!save.is_finished(), "the checkpoint split a refusal");
    node.aof.release_hold();
    assert!(ws.recv_frame(5_000).await.is_some());
    save.await.unwrap().unwrap();

    // What a restart loads: the sidecar the checkpoint wrote.
    let restarted = ServerState::for_tests(
        Arc::new(SnapshotConfig {
            path: node.dir.join("dump.rdb"),
            last_save: AtomicI64::new(0),
            checkpoint_id: AtomicU64::new(0),
        }),
        ReplHub::new(),
        false,
    );
    restarted.load_dedup().await.unwrap();
    assert!(
        remembers_refusal(&restarted, &identity, 3),
        "the refusal was lost across the checkpoint"
    );
}

/// The refusal is a replication frame too. A sub-replica's cut holds every
/// stripe while it reads the DEDUP table and then registers the replica's
/// stream; a refusal that committed and fanned out between the two would
/// reach the replica through neither. Here the table is read first, the
/// refusal is sent, and only then is the replica registered — the losing
/// interleaving — so the refusal must wait for the cut and arrive in the
/// stream. (The shared checkpoint lock 6N-B proposed would not have
/// excluded this cut, which takes no checkpoint lock.)
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refusal_racing_a_replica_cut_reaches_the_replica() {
    let node = node("refusal_cut", AofSync::EverySec).await;
    node.state.replicas.configure_dedup(true);
    node.state.replicas.configure_backlog(1 << 20);
    let addr = serve_strict(&node).await;
    let mut ws = scoped(addr, "alice").await;
    let identity = namespaced_dedup_client("alice", "cutter-0b");

    let barrier = node.state.replicas.lock_all_writes().await;
    let in_table = remembers_refusal(&node.state, &identity, 4);
    assert!(!in_table);
    send(&mut ws, &refused_flushdb("cutter-0b", 4)).await;
    assert_eq!(ws.recv_frame(200).await, None, "refused inside the cut");
    let mut stream = node
        .state
        .replicas
        .attach_test_replica(64, usize::MAX >> 1)
        .await;
    drop(barrier);
    assert!(ws.recv_frame(5_000).await.is_some(), "refusal answered");
    let in_stream = tokio::time::timeout(Duration::from_secs(2), stream.recv())
        .await
        .ok()
        .flatten()
        .is_some_and(|q| {
            q.bytes()
                .windows(identity.len())
                .any(|w| w == identity.as_bytes())
        });
    assert!(
        in_stream,
        "the refusal reached the replica through neither the table nor the stream"
    );
}

/// NEW-4, deterministically: a checkpoint holds every stripe from its DEDUP
/// sidecar to its AOF truncate. A refusal sent inside that window must not
/// be recorded in it — remembered after the sidecar and appended before the
/// truncate, it would be in neither.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refusal_never_lands_inside_a_checkpoint_window() {
    let node = node("refusal_window", AofSync::EverySec).await;
    let addr = serve_strict(&node).await;
    let mut ws = scoped(addr, "alice").await;
    let identity = namespaced_dedup_client("alice", "window-0b");
    let window = node.state.replicas.lock_all_writes().await;
    send(&mut ws, &refused_flushdb("window-0b", 2)).await;
    assert_eq!(ws.recv_frame(300).await, None, "answered inside the window");
    assert!(!remembers_refusal(&node.state, &identity, 2));
    let on_disk = std::fs::read(node.dir.join("append.aof")).unwrap_or_default();
    assert!(
        !on_disk
            .windows(identity.len())
            .any(|w| w == identity.as_bytes()),
        "appended inside the window"
    );
    drop(window);
    assert!(ws.recv_frame(5_000).await.is_some());
    assert!(remembers_refusal(&node.state, &identity, 2));
}

/// A flood of refusals from one socket is paced to `REFUSALS_PER_SECOND`,
/// every refusal is still recorded and answered, and other writers are
/// never held while it lasts. Another tenant's identical client id and id
/// are unaffected.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refusal_flood_is_paced_and_stalls_no_one() {
    let node = node("refusal_flood", AofSync::EverySec).await;
    let addr = serve_strict(&node).await;
    let mut ws = scoped(addr, "alice").await;
    let n = u64::from(crate::connection::REFUSALS_PER_SECOND) * 2 + 6;
    let started = std::time::Instant::now();
    for id in 1..=n {
        send(&mut ws, &refused_flushdb("flood-0b", id)).await;
    }
    let probe = {
        let state = Arc::clone(&node.state);
        tokio::spawn(async move {
            for _ in 0..10 {
                tokio::time::timeout(Duration::from_millis(500), state.replicas.lock_all_writes())
                    .await
                    .expect("a writer was held during the flood");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
    };
    for id in 1..=n {
        let reply = ws.recv_frame(10_000).await.expect("every refusal answered");
        assert!(
            String::from_utf8_lossy(&reply).starts_with("-NOSCOPE"),
            "id {id}"
        );
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(1_800),
        "{n} refusals were not paced: {elapsed:?}"
    );
    probe.await.unwrap();
    // Every one was recorded (the table keeps the latest few per client).
    let identity = namespaced_dedup_client("alice", "flood-0b");
    assert!(remembers_refusal(&node.state, &identity, n));

    // Bob uses the same client id and id: his write is his own.
    let mut bob = scoped(addr, "bob").await;
    let reply = bob
        .cmd(&["DEDUP", "flood-0b", "1", "SET", "tenant:bob:k", "v"])
        .await;
    assert_eq!(
        reply,
        Value::SimpleString("OK".into()),
        "cross-tenant refusal"
    );
}
