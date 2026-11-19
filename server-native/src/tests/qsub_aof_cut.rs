//! 6N-C PR 0c (RV-8): a QSUB `qstate` never shows a write before that write
//! reached its AOF stage.
//!
//! A TCP write with no observer defers its AOF wait to its connection: its
//! value is in the store before its frame is written. A QSUB on a socket not
//! routed for that key took its all-write barrier, read the store and sent
//! `qstate` at once — so under `always` a browser could be shown a value
//! that a failed `fdatasync` then took down with the process. QSUB now
//! records the AOF queue's tail at its cut and waits for it, with every
//! stripe released, before sending `qstate`.

use super::dedup_ack_order::{Node, finished_within, node, tcp_write};
use super::*;
use std::time::Duration;

/// Serve WebSocket clients for `node` (open sync mode).
async fn serve_ws(node: &Node) -> std::net::SocketAddr {
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
                    Arc::new(SyncPolicy::open()),
                    Arc::new(None),
                    peer.to_string(),
                )
                .await;
            });
        }
    });
    addr
}

/// A socket routed for `other:*` only, so a write to `qk:*` has no push
/// candidate and (with no live query yet) defers.
async fn narrowed(addr: std::net::SocketAddr) -> WsClient {
    let mut ws = WsClient::connect(addr).await;
    let reply = ws.cmd(&["SYNC", "other:*"]).await;
    assert!(!matches!(reply, Value::Error(_)), "{reply:?}");
    ws
}

/// Start a deferred TCP `SET` held before its AOF write; returns once the
/// command returned (it deferred) and the performer is held.
async fn held_deferred_set(node: &Node, key: &str, value: &str) -> tokio::task::JoinHandle<Value> {
    node.aof.hold_batch_containing(key.as_bytes());
    let (task, returned) = tcp_write(
        node,
        Command::Set(key.into(), value.as_bytes().to_vec(), Default::default()),
    );
    node.aof.wait_until_held().await;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), returned)
            .await
            .expect("the SET must take the deferred path for this test to mean anything")
            .unwrap(),
        Value::SimpleString("OK".into())
    );
    task
}

/// The RV-8 counterexample, under each policy: `qstate` waits for the held
/// batch and then shows the write.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn qstate_waits_for_a_deferred_write_in_its_snapshot() {
    for policy in [AofSync::No, AofSync::EverySec, AofSync::Always] {
        let node = node("qsub_cut", policy).await;
        let addr = serve_ws(&node).await;
        let mut ws = narrowed(addr).await;
        let set = held_deferred_set(&node, "qk:1", "v1").await;

        ws.send(&["QSUB", "qk:*"]).await;
        assert_eq!(
            ws.recv_frame(300).await,
            None,
            "{policy:?}: qstate was sent while the write it shows was unwritten"
        );
        node.aof.release_hold();
        let frame = ws
            .recv_frame(5_000)
            .await
            .expect("qstate after the batch was written");
        let text = String::from_utf8_lossy(&frame);
        assert!(
            text.contains("qstate") && text.contains("qk:1") && text.contains("v1"),
            "{policy:?}: {text}"
        );
        assert_eq!(set.await.unwrap(), Value::SimpleString("OK".into()));
    }
}

/// The wait holds no stripe: while QSUBs wait on a stalled disk, the
/// all-write barrier — every stripe, the strongest thing a writer can need
/// — is free, and repeated QSUB/QUNSUB from several sockets do not change
/// that. Once the disk recovers every QSUB gets its state.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn waiting_qsubs_do_not_hold_writers_during_a_disk_stall() {
    let node = node("qsub_stall", AofSync::EverySec).await;
    let addr = serve_ws(&node).await;
    let mut sockets = Vec::new();
    for _ in 0..3 {
        sockets.push(narrowed(addr).await);
    }
    let set = held_deferred_set(&node, "qk:stall", "v").await;
    for (i, ws) in sockets.iter_mut().enumerate() {
        ws.send(&["QSUB", "qk:*"]).await;
        if i == 0 {
            // One socket also subscribes and unsubscribes another pattern.
            ws.send(&["QUNSUB", "qk:*"]).await;
            ws.send(&["QSUB", "qk:*"]).await;
        }
    }
    for round in 0..5 {
        let barrier = tokio::time::timeout(
            Duration::from_secs(1),
            node.state.replicas.lock_all_writes(),
        )
        .await;
        assert!(
            barrier.is_ok(),
            "round {round}: a waiting QSUB holds write stripes"
        );
        drop(barrier);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    for ws in &mut sockets {
        assert_eq!(ws.recv_frame(100).await, None, "qstate before the write");
    }
    node.aof.release_hold();
    for (i, ws) in sockets.iter_mut().enumerate() {
        let first = ws.recv_frame(5_000).await.expect("first reply");
        assert!(
            String::from_utf8_lossy(&first).contains("qstate"),
            "socket {i}"
        );
        if i == 0 {
            assert_eq!(ws.recv_frame(5_000).await.as_deref(), Some(&b"+OK\r\n"[..]));
            let again = ws.recv_frame(5_000).await.expect("second qstate");
            assert!(String::from_utf8_lossy(&again).contains("qk:stall"));
        }
    }
    assert_eq!(set.await.unwrap(), Value::SimpleString("OK".into()));
}

/// Under `always`, the held batch then fails its `fdatasync`: the writer
/// stops, and the socket is closed without ever being sent `qstate`.
/// (In production the process exits; in-process tests latch instead.)
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_always_failure_never_emits_the_qstate() {
    let node = node("qsub_fail", AofSync::Always).await;
    let addr = serve_ws(&node).await;
    let mut ws = narrowed(addr).await;
    node.aof
        .inject_fault(AofFault::FsyncFails(std::io::ErrorKind::Other));
    let mut set = held_deferred_set(&node, "qk:doomed", "never").await;
    ws.send(&["QSUB", "qk:*"]).await;
    assert_eq!(ws.recv_frame(200).await, None);
    node.aof.release_hold();
    // Closed with nothing sent: `recv_frame` skips the close frame.
    assert_eq!(
        ws.recv_frame(3_000).await,
        None,
        "a qstate showing an undurable write was sent"
    );
    let reply = finished_within(&mut set, Duration::from_secs(3))
        .await
        .expect("the doomed write's connection settles");
    assert!(
        matches!(&reply, Value::Error(e) if e.starts_with("WITHHELD")),
        "the doomed write was acknowledged: {reply:?}"
    );
}

/// With no AOF, and with nothing pending, QSUB answers at once as before.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nothing_pending_means_no_wait() {
    let node = node("qsub_idle", AofSync::EverySec).await;
    node.store.execute(Command::Set(
        "qk:idle".into(),
        b"v".to_vec(),
        Default::default(),
    ));
    let addr = serve_ws(&node).await;
    let mut ws = narrowed(addr).await;
    assert!(node.aof.tail().is_none(), "nothing queued");
    ws.send(&["QSUB", "qk:*"]).await;
    let frame = ws.recv_frame(1_000).await.expect("immediate qstate");
    assert!(String::from_utf8_lossy(&frame).contains("qk:idle"));
}
