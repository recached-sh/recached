//! 6N-C correctness integration: the fixes of PRs 0a–0e working together,
//! in whichever `RECACHED_AOF_FANOUT` mode the suite runs in (CI runs both).
//!
//! Each fix has its own deterministic tests; these cover the interactions
//! the integration gate names: DEDUP with a deferred AOF and a QSUB cut
//! (0a with 0c), and a live query across a token renewal that re-routes
//! the socket (0c with 0e).

use super::dedup_ack_order::{finished_within, node, tcp_write};
use super::qsub_aof_cut::{narrowed, serve_ws};
use super::ws_refusal_barrier::{scoped, serve_strict};
use super::*;
use std::time::Duration;

/// One held AOF batch holding a deferred plain write; then a DEDUP write,
/// its replay and a QSUB, all behind it. Nothing may be answered from the
/// unwritten batch: the DEDUP write (never deferred) waits for its own,
/// later batch, which is performed after the held one; its replay waits on
/// `dedup_order`; `qstate` waits for the cut's tail. Once released, each
/// gets the answer the written state supports.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dedup_and_qsub_never_answer_from_an_unwritten_batch() {
    for policy in [AofSync::EverySec, AofSync::Always] {
        let node = node("int_dedup_qsub", policy).await;
        let addr = serve_ws(&node).await;
        let mut ws = narrowed(addr).await;

        node.aof.hold_batch_containing(b"qk:held");
        let (plain, plain_returned) = tcp_write(
            &node,
            Command::Set("qk:held".into(), b"v".to_vec(), Default::default()),
        );
        node.aof.wait_until_held().await;
        assert!(
            tokio::time::timeout(Duration::from_secs(5), plain_returned)
                .await
                .is_ok(),
            "{policy:?}: the plain write must defer for this test to mean anything"
        );
        let write = Command::Dedup(
            "int-0a0c".into(),
            1,
            Box::new(Command::Incr("qk:ctr".into())),
        );
        let (mut original, _) = tcp_write(&node, write.clone());
        let (mut replay, _) = tcp_write(&node, write);
        ws.send(&["QSUB", "qk:*"]).await;

        assert_eq!(
            ws.recv_frame(300).await,
            None,
            "{policy:?}: qstate from an unwritten batch"
        );
        assert!(
            finished_within(&mut original, Duration::from_millis(50))
                .await
                .is_none()
        );
        assert!(
            finished_within(&mut replay, Duration::from_millis(50))
                .await
                .is_none()
        );
        assert!(node.state.dedup_replay("int-0a0c", 1).is_none());

        node.aof.release_hold();
        // The two identical writes race for `dedup_order`: whichever wins
        // applies it, the other is the replay. Exactly one of each.
        let mut answers = [original.await.unwrap(), replay.await.unwrap()];
        answers.sort_by_key(|v| matches!(v, Value::SimpleString(_)));
        assert_eq!(
            answers,
            [Value::Integer(1), Value::SimpleString("DUP".into())],
            "{policy:?}: not applied exactly once"
        );
        assert_eq!(plain.await.unwrap(), Value::SimpleString("OK".into()));
        let frame = ws.recv_frame(5_000).await.expect("qstate");
        let text = String::from_utf8_lossy(&frame);
        assert!(
            text.contains("qstate") && text.contains("qk:held"),
            "{policy:?}: {text}"
        );
    }
}

/// A strict socket's live query keeps delivering across a token renewal
/// that re-routes it (0e's `route_pushes`), and the socket is counted as a
/// push receiver once throughout.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_live_query_keeps_delivering_across_a_token_renewal() {
    let node = node("int_qsub_renew", AofSync::EverySec).await;
    let addr = serve_strict(&node).await;
    let mut ws = scoped(addr, "alice").await;
    assert_eq!(node.state.replicas.push_receiver_count(), 1);
    let qstate = ws.cmd(&["QSUB", "tenant:alice:*"]).await;
    assert!(!matches!(qstate, Value::Error(_)), "{qstate:?}");

    let mut writer = scoped(addr, "alice").await;
    assert_eq!(writer.cmd(&["SET", "tenant:alice:a", "1"]).await, ok());
    let first = ws
        .recv_any(2_000)
        .await
        .expect("a keychange before renewal");
    assert!(first.contains("tenant:alice:a"), "{first}");

    // Renew with the same grants: re-routed, not re-counted, not closed.
    let renewal = ws
        .cmd(&[
            "SYNC",
            "TOKEN",
            &super::ws_refusal_barrier::token_for("rw=tenant:alice:*", "alice"),
        ])
        .await;
    assert!(!matches!(renewal, Value::Error(_)), "{renewal:?}");
    assert_eq!(
        node.state.replicas.push_receiver_count(),
        2,
        "writer + subscriber, once each"
    );

    assert_eq!(writer.cmd(&["SET", "tenant:alice:b", "2"]).await, ok());
    let mut seen = String::new();
    for _ in 0..4 {
        match ws.recv_any(2_000).await {
            Some(frame) => {
                seen.push_str(&frame);
                if seen.contains("tenant:alice:b") {
                    break;
                }
            }
            None => break,
        }
    }
    assert!(
        seen.contains("tenant:alice:b"),
        "no keychange after renewal: {seen}"
    );
}
