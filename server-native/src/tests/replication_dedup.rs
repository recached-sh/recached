//! Replication-aware deduplication (v0.6 Phase 6B).
//!
//! A real primary and replica, each serving WebSocket clients, connected by
//! the real replication handshake. The property under test: once a replica has
//! applied a DEDUP write, promoting it does not let a client's retry of that
//! write run again — whether the replica learned it from the live stream, a
//! full synchronization, or a partial one; and whether the outcome was a
//! change, a no-op, or a permanent refusal.

use super::*;

struct Node {
    store: Arc<KeyValueStore>,
    state: Arc<ServerState>,
    tx: broadcast::Sender<SyncMsg>,
    ws: std::net::SocketAddr,
}

/// A node with its own store, state and WebSocket listener (open sync mode).
async fn node(is_replica: bool, dedup: bool) -> Node {
    let hub = ReplHub::new();
    hub.configure_dedup(dedup);
    // A primary only fans out once replication is enabled.
    hub.configure_backlog(1 << 20);
    let state = ServerState::for_tests(
        Arc::new(SnapshotConfig {
            path: tmp_path("repl_dedup.rdb"),
            last_save: AtomicI64::new(0),
            checkpoint_id: AtomicU64::new(0),
        }),
        hub,
        is_replica,
    );
    let store = Arc::new(KeyValueStore::new());
    let (tx, _rx) = broadcast::channel::<SyncMsg>(256);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ws = listener.local_addr().unwrap();
    let (s, t, st) = (Arc::clone(&store), tx.clone(), Arc::clone(&state));
    let pubsub: SharedPubSub = Arc::new(tokio::sync::Mutex::new(PubSubHub::new()));
    let watch: WatchRegistry = WatchHub::new();
    tokio::spawn(async move {
        while let Ok((socket, peer)) = listener.accept().await {
            let (s, t, st, ps, wr) = (
                Arc::clone(&s),
                t.clone(),
                Arc::clone(&st),
                Arc::clone(&pubsub),
                Arc::clone(&watch),
            );
            tokio::spawn(async move {
                handle_ws(
                    socket,
                    s,
                    t,
                    Arc::new(None),
                    next_conn_id(),
                    ps,
                    wr,
                    st,
                    Arc::new(SyncPolicy::open()),
                    Arc::new(None),
                    peer.to_string(),
                )
                .await;
            });
        }
    });
    Node {
        store,
        state,
        tx,
        ws,
    }
}

/// Serve replicas from `primary`; returns the replication address.
async fn serve_replicas(primary: &Node) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (store, state) = (Arc::clone(&primary.store), Arc::clone(&primary.state));
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let (store, state) = (Arc::clone(&store), Arc::clone(&state));
            tokio::spawn(async move {
                let hub = Arc::clone(&state.replicas);
                let _ = handle_replica(
                    socket,
                    store,
                    state,
                    hub,
                    None,
                    DEFAULT_REPL_CHANNEL_CAPACITY,
                    IpAddr::from([127, 0, 0, 1]),
                    ReplAuthThrottle::new(),
                )
                .await;
            });
        }
    });
    addr
}

/// Follow `primary_addr` from `replica` until the link closes or `ms` pass.
async fn follow_for(
    replica: &Node,
    primary_addr: std::net::SocketAddr,
    resume: &mut ReplResume,
    ms: u64,
) {
    let mut socket = TcpStream::connect(primary_addr).await.unwrap();
    let _ = tokio::time::timeout(
        tokio::time::Duration::from_millis(ms),
        sync_from_primary(
            &mut socket,
            &replica.store,
            None,
            &replica.tx,
            &replica.state,
            resume,
        ),
    )
    .await;
}

/// Follow in the background for the rest of the test.
fn follow(replica: &Node, primary_addr: std::net::SocketAddr) {
    let (store, state, tx) = (
        Arc::clone(&replica.store),
        Arc::clone(&replica.state),
        replica.tx.clone(),
    );
    tokio::spawn(async move {
        let mut resume = ReplResume::default();
        let mut socket = TcpStream::connect(primary_addr).await.unwrap();
        let _ = sync_from_primary(&mut socket, &store, None, &tx, &state, &mut resume).await;
    });
}

async fn wait_for(store: &KeyValueStore, key: &str, want: Value) {
    for _ in 0..200 {
        if store.execute(Command::Get(key.into())) == want {
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    panic!(
        "replica never reached {key} = {want:?}; has {:?}",
        store.execute(Command::Get(key.into()))
    );
}

/// The replica has applied every frame the primary has sent: a sentinel
/// written last has arrived.
async fn caught_up(primary: &mut WsClient, replica: &Node, tag: &str) {
    primary.cmd(&["SET", "sentinel", tag]).await;
    wait_for(&replica.store, "sentinel", bulk(tag)).await;
}

fn dup() -> Value {
    Value::SimpleString("DUP".into())
}

// ── The live stream ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_promoted_replica_does_not_rerun_an_applied_write() {
    // Regression: v0.5 replicated the bare INCR, so the promoted replica had
    // the data but not the mark, and the client's retry (its reply was lost
    // when the primary died) ran again: 2 instead of 1.
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    follow(&replica, serve_replicas(&primary).await);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut p = WsClient::connect(primary.ws).await;
    assert_eq!(p.cmd(&["DEDUP", "c1", "1", "INCR", "n"]).await, int(1));
    assert_eq!(
        p.cmd(&["DEDUP", "c1", "2", "APPEND", "log", "x"]).await,
        int(1)
    );
    caught_up(&mut p, &replica, "t1").await;
    assert!(
        replica
            .state
            .replicas
            .upstream_dedup
            .load(Ordering::Acquire)
    );

    replica.state.promote_to_primary();
    let mut r = WsClient::connect(replica.ws).await;
    assert_eq!(r.cmd(&["DEDUP", "c1", "1", "INCR", "n"]).await, dup());
    assert_eq!(
        r.cmd(&["DEDUP", "c1", "2", "APPEND", "log", "x"]).await,
        dup()
    );
    assert_eq!(r.cmd(&["GET", "n"]).await, bulk("1"));
    assert_eq!(r.cmd(&["GET", "log"]).await, bulk("x"));
    // The next write is new, and runs.
    assert_eq!(r.cmd(&["DEDUP", "c1", "3", "INCR", "n"]).await, int(2));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn with_dedup_replication_off_a_retry_may_run_again_as_documented() {
    // RECACHED_REPL_DEDUP=off keeps the 0.5 stream. This pins the limit the
    // documentation states for it — and shows the test above detects it.
    let primary = node(false, false).await;
    let replica = node(true, false).await;
    follow(&replica, serve_replicas(&primary).await);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut p = WsClient::connect(primary.ws).await;
    assert_eq!(p.cmd(&["DEDUP", "c1", "1", "INCR", "n"]).await, int(1));
    caught_up(&mut p, &replica, "t1").await;
    assert!(
        !replica
            .state
            .replicas
            .upstream_dedup
            .load(Ordering::Acquire)
    );

    replica.state.promote_to_primary();
    let mut r = WsClient::connect(replica.ws).await;
    assert_eq!(r.cmd(&["DEDUP", "c1", "1", "INCR", "n"]).await, int(2));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_no_op_write_replicates_its_mark() {
    // SETNX on an existing key changes nothing, so it travels as a mark-only
    // record. Without it, a retry after the key was deleted would set it.
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    follow(&replica, serve_replicas(&primary).await);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut p = WsClient::connect(primary.ws).await;
    p.cmd(&["SET", "k", "original"]).await;
    assert_eq!(
        p.cmd(&["DEDUP", "c1", "1", "SETNX", "k", "retry"]).await,
        int(0)
    );
    p.cmd(&["DEL", "k"]).await;
    caught_up(&mut p, &replica, "t1").await;

    replica.state.promote_to_primary();
    let mut r = WsClient::connect(replica.ws).await;
    assert_eq!(
        r.cmd(&["DEDUP", "c1", "1", "SETNX", "k", "retry"]).await,
        dup()
    );
    assert_eq!(r.cmd(&["GET", "k"]).await, nil());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_permanent_refusal_replicates_and_is_answered_again() {
    // Refused (WRONGTYPE) at id 2, then id 3 applied: id 2 sits below the
    // mark. Without the refusal record the promoted replica would answer its
    // retry +DUP — telling the client a refused write was applied.
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    follow(&replica, serve_replicas(&primary).await);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut p = WsClient::connect(primary.ws).await;
    assert_eq!(
        p.cmd(&["DEDUP", "c1", "1", "HSET", "h", "f", "v"]).await,
        int(1)
    );
    let refused = p.cmd(&["DEDUP", "c1", "2", "INCR", "h"]).await;
    assert!(matches!(&refused, Value::Error(e) if e.starts_with("WRONGTYPE")));
    assert_eq!(p.cmd(&["DEDUP", "c1", "3", "SET", "x", "y"]).await, ok());
    caught_up(&mut p, &replica, "t1").await;

    replica.state.promote_to_primary();
    let mut r = WsClient::connect(replica.ws).await;
    assert_eq!(r.cmd(&["DEDUP", "c1", "2", "INCR", "h"]).await, refused);
    assert_eq!(r.cmd(&["DEDUP", "c1", "3", "SET", "x", "y"]).await, dup());
}

// ── Synchronization ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_full_synchronization_carries_the_dedup_table() {
    let primary = node(false, true).await;
    let mut p = WsClient::connect(primary.ws).await;
    // Written before the replica exists: it only sees the snapshot.
    assert_eq!(p.cmd(&["DEDUP", "c1", "7", "INCR", "n"]).await, int(1));
    p.cmd(&["HSET", "h", "f", "v"]).await;
    let refused = p.cmd(&["DEDUP", "c1", "9", "INCR", "h"]).await;
    assert!(matches!(refused, Value::Error(_)));
    assert_eq!(p.cmd(&["DEDUP", "c1", "10", "SET", "x", "y"]).await, ok());

    let replica = node(true, true).await;
    follow(&replica, serve_replicas(&primary).await);
    caught_up(&mut p, &replica, "t1").await;

    replica.state.promote_to_primary();
    let mut r = WsClient::connect(replica.ws).await;
    assert_eq!(r.cmd(&["DEDUP", "c1", "7", "INCR", "n"]).await, dup());
    assert!(matches!(
        r.cmd(&["DEDUP", "c1", "9", "INCR", "h"]).await,
        Value::Error(e) if e.starts_with("WRONGTYPE")
    ));
    assert_eq!(r.cmd(&["GET", "n"]).await, bulk("1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_partial_resynchronization_carries_marks_from_the_backlog() {
    let primary = node(false, true).await;
    let addr = serve_replicas(&primary).await;
    let replica = node(true, true).await;
    let mut p = WsClient::connect(primary.ws).await;
    let mut resume = ReplResume::default();
    follow_for(&replica, addr, &mut resume, 300).await;

    // While the replica is away, the primary applies a DEDUP write; it waits
    // in the backlog.
    assert_eq!(p.cmd(&["DEDUP", "c1", "1", "INCR", "n"]).await, int(1));
    // A key only this replica holds: a full synchronization would wipe it, so
    // its survival proves the reconnect resumed from the backlog.
    replica.store.execute(Command::Set(
        "replica-only".into(),
        "1".into(),
        SetOptions::default(),
    ));
    follow_for(&replica, addr, &mut resume, 300).await;
    assert_eq!(
        replica.store.execute(Command::Get("replica-only".into())),
        bulk("1"),
        "the reconnect was a full synchronization, not a partial one"
    );
    wait_for(&replica.store, "n", bulk("1")).await;

    replica.state.promote_to_primary();
    let mut r = WsClient::connect(replica.ws).await;
    assert_eq!(r.cmd(&["DEDUP", "c1", "1", "INCR", "n"]).await, dup());
}

// ── What the replica's own clients see ──────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_replicas_browser_peers_are_pushed_the_bare_write() {
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    follow(&replica, serve_replicas(&primary).await);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let mut watcher = WsClient::connect(replica.ws).await;
    watcher.cmd(&["PING"]).await;

    let mut p = WsClient::connect(primary.ws).await;
    p.cmd(&["DEDUP", "secret-client-id", "1", "SET", "k", "v"])
        .await;
    let push = watcher
        .recv_push(2_000)
        .await
        .expect("no push reached the replica's peer");
    assert!(push.contains("SET") || push.contains("set"), "{push}");
    assert!(
        !push.contains("DEDUP") && !push.contains("secret-client-id"),
        "envelope leaked to a peer: {push}"
    );
}

// ── Compatibility ───────────────────────────────────────────────────────────

#[tokio::test]
async fn a_dedup_primary_refuses_a_replica_that_cannot_read_dedup_frames() {
    // What a 0.5 replica does: echo the advertised id, offer no `accept`.
    let primary = node(false, true).await;
    let addr = serve_replicas(&primary).await;
    let mut sock = TcpStream::connect(addr).await.unwrap();
    let mut magic = [0u8; 4];
    sock.read_exact(&mut magic).await.unwrap();
    let mut len = [0u8; 2];
    sock.read_exact(&mut len).await.unwrap();
    let mut advertised = vec![0u8; usize::from(u16::from_le_bytes(len))];
    sock.read_exact(&mut advertised).await.unwrap();
    let advertised = String::from_utf8(advertised).unwrap();
    assert!(advertised.ends_with(";caps=dedup1"), "{advertised}");
    sock.write_all(&(advertised.len() as u16).to_le_bytes())
        .await
        .unwrap();
    sock.write_all(advertised.as_bytes()).await.unwrap();
    sock.write_all(&0u64.to_le_bytes()).await.unwrap();
    let mut mode = [0u8; 1];
    sock.read_exact(&mut mode).await.unwrap();
    // Not 'F' or 'P', which a 0.5 replica would act on: it reports an
    // invalid mode and retries, and nothing it cannot read is ever sent.
    assert_eq!(mode[0], b'E');
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_0_6_replica_follows_a_primary_without_the_capability() {
    // A 0.5 primary advertises a bare run id. Upgrading replicas first must
    // work: the replica follows in the 0.5 format and reports that its marks
    // are not replicated.
    let primary = node(false, false).await;
    let replica = node(true, true).await;
    follow(&replica, serve_replicas(&primary).await);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let mut p = WsClient::connect(primary.ws).await;
    p.cmd(&["SET", "k", "v"]).await;
    wait_for(&replica.store, "k", bulk("v")).await;
    assert!(
        !replica
            .state
            .replicas
            .upstream_dedup
            .load(Ordering::Acquire)
    );
}

#[test]
fn handshake_ids_parse_their_parameters() {
    let bare = HandshakeId::parse("abc123");
    assert_eq!(bare.run_id, "abc123");
    let offered = HandshakeId::parse("abc123;caps=dedup1");
    assert_eq!(offered.run_id, "abc123");
    let accepted = HandshakeId::parse(";accept=other,dedup1");
    assert_eq!(accepted.run_id, "");
    // A capability is matched exactly, never as a prefix.
    let near = HandshakeId::parse("abc;accept=dedup10");
    assert_eq!(near.run_id, "abc");
    for (raw, key, want) in [
        ("abc123;caps=dedup1", "caps", true),
        ("abc123;caps=dedup1", "accept", false),
        (";accept=other,dedup1", "accept", true),
        ("abc;accept=dedup10", "accept", false),
        ("abc123", "caps", false),
    ] {
        assert_eq!(
            HandshakeId::parse(raw).has(key, REPL_CAP_DEDUP),
            want,
            "{raw} {key}"
        );
    }
}
