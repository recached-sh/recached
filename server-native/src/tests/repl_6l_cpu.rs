//! Replication CPU changes (v0.6 Phase 6L-D) end to end.
//!
//! R1 moved the backlog and replica queues under one synchronous lock; R2
//! shares each write's frame between the AOF caller, the replica queues and
//! the WebSocket push, and builds a push's WebSocket message once for every
//! connection. Neither may change what any consumer receives. These tests
//! drive concurrent writers (plain and DEDUP) through real connections into a
//! primary with two replicas, reattach replicas partially and fully while the
//! writes continue, and pin the WebSocket bytes against the previous
//! per-connection encoding.

use super::replication_dedup::*;
use super::*;

const KEYS: usize = 64;

/// Writers through real WebSocket connections: SET and INCR on shared keys,
/// and DEDUP-wrapped INCRs under a client id of their own.
fn spawn_writers(primary: &Node, writers: usize, each: usize) -> Vec<tokio::task::JoinHandle<()>> {
    (0..writers)
        .map(|w| {
            let addr = primary.ws;
            tokio::spawn(async move {
                let mut c = WsClient::connect(addr).await;
                for i in 0..each {
                    let key = format!("k{}", (w * 7 + i) % KEYS);
                    match i % 3 {
                        0 => assert_eq!(c.cmd(&["SET", &key, &format!("w{w}i{i}")]).await, ok()),
                        1 => {
                            c.cmd(&["INCR", &format!("n{}", i % 8)]).await;
                        }
                        _ => {
                            let id = (i / 3 + 1).to_string();
                            let reply = c
                                .cmd(&["DEDUP", &format!("c{w}"), &id, "INCR", &format!("d{w}")])
                                .await;
                            assert!(matches!(reply, Value::Integer(_)), "{reply:?}");
                        }
                    }
                }
            })
        })
        .collect()
}

fn assert_converged(primary: &Node, replica: &Node, what: &str) {
    let mut keys: Vec<String> = primary
        .store
        .snapshot()
        .into_iter()
        .map(|e| e.key)
        .collect();
    keys.sort();
    let mut replica_keys: Vec<String> = replica
        .store
        .snapshot()
        .into_iter()
        .map(|e| e.key)
        .collect();
    replica_keys.sort();
    assert_eq!(keys, replica_keys, "{what}: key sets differ");
    for k in &keys {
        assert_eq!(
            primary.store.get_current(k),
            replica.store.get_current(k),
            "{what}: {k} differs"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_writers_replicate_identically_to_two_replicas() {
    let primary = node(false, true).await;
    let (a, b) = (node(true, true).await, node(true, true).await);
    let addr = serve_replicas(&primary).await;
    follow(&a, addr);
    follow(&b, addr);
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    for w in spawn_writers(&primary, 8, 300) {
        w.await.unwrap();
    }
    let mut p = WsClient::connect(primary.ws).await;
    caught_up(&mut p, &a, "a1").await;
    caught_up(&mut p, &b, "b1").await;
    assert_converged(&primary, &a, "replica a");
    assert_converged(&primary, &b, "replica b");

    // DEDUP identities travelled with their writes: a promoted replica
    // answers a retry the primary applied with +DUP.
    a.state.promote_to_primary(&a.store).await;
    let mut r = WsClient::connect(a.ws).await;
    assert_eq!(
        r.cmd(&["DEDUP", "c3", "1", "INCR", "d3"]).await,
        Value::SimpleString("DUP".into())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn replicas_reattach_partially_and_fully_while_writes_continue() {
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    let steady = node(true, true).await;
    let addr = serve_replicas(&primary).await;
    follow(&steady, addr);

    let writers = spawn_writers(&primary, 4, 600);
    // Attach, drop, resume from the same offset (partial), then start over
    // (full snapshot under load), then follow for good — all mid-stream.
    let mut resume = ReplResume::default();
    follow_for(&replica, addr, &mut resume, 150).await;
    follow_for(&replica, addr, &mut resume, 150).await;
    let mut fresh = ReplResume::default();
    follow_for(&replica, addr, &mut fresh, 150).await;
    follow(&replica, addr);
    for w in writers {
        w.await.unwrap();
    }
    // One last write, applied by both replicas before either is compared:
    // a sentinel per replica raced, since writing the second one left the
    // first replica a write behind the primary it was compared with.
    let mut p = WsClient::connect(primary.ws).await;
    caught_up(&mut p, &replica, "done").await;
    wait_for(&steady.store, "sentinel", bulk("done")).await;
    assert_converged(&primary, &replica, "reattached replica");
    assert_converged(&primary, &steady, "steady replica");
}

/// The next two text or binary frames `c` is sent.
async fn two_frames(c: &mut WsClient) -> Vec<Message> {
    let mut out = Vec::new();
    while out.len() < 2 {
        match tokio::time::timeout(tokio::time::Duration::from_secs(2), c.ws.next()).await {
            Ok(Some(Ok(m @ (Message::Text(_) | Message::Binary(_))))) => out.push(m),
            Ok(Some(Ok(_))) => continue,
            _ => break,
        }
    }
    out
}

/// The per-connection encoding before 6L-D R2, kept as the reference.
fn previous_ws_message(bytes: &[u8]) -> Message {
    match std::str::from_utf8(bytes) {
        Ok(text) => Message::Text(text.into()),
        Err(_) => Message::Binary(bytes.to_vec().into()),
    }
}

#[test]
fn a_shared_push_message_is_what_each_connection_built_before() {
    use rand::{Rng, SeedableRng};
    let mut rng = rand::rngs::StdRng::seed_from_u64(61);
    let mut payloads: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"*3\r\n$3\r\nSET\r\n$1\r\nk\r\n$1\r\nv\r\n".to_vec(),
        "*3\r\n$3\r\nSET\r\n$1\r\nk\r\n$4\r\nh\u{e9}\u{2603}\r\n"
            .as_bytes()
            .to_vec(),
        vec![0xff, 0xfe, b'x'],
        vec![0xe2, 0x98],
    ];
    for _ in 0..2_000 {
        let len = rng.random_range(0..64);
        let ascii = rng.random_ratio(1, 2);
        payloads.push(
            (0..len)
                .map(|_| {
                    if ascii {
                        rng.random_range(0x20..0x7f)
                    } else {
                        rng.random()
                    }
                })
                .collect(),
        );
    }
    let cmd = Command::Set("k".into(), "v".into(), SetOptions::default());
    for payload in payloads {
        let push = SyncPush::for_command(0, &cmd, vec!["k".into()], payload.clone());
        let expected = previous_ws_message(&payload);
        // The first connection builds it; every later one gets the same.
        assert_eq!(push_message(&push), expected, "{payload:?}");
        assert_eq!(
            push_message(&push),
            expected,
            "{payload:?} (second connection)"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_browser_gets_the_same_text_and_binary_push_bytes() {
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    follow(&replica, serve_replicas(&primary).await);
    let (mut x, mut y) = (
        WsClient::connect(primary.ws).await,
        WsClient::connect(primary.ws).await,
    );
    for c in [&mut x, &mut y] {
        assert_eq!(c.cmd(&["PING"]).await, Value::SimpleString("PONG".into()));
    }
    let mut w = WsClient::connect(primary.ws).await;
    assert_eq!(w.cmd(&["SET", "t", "plain"]).await, ok());
    let mut raw = b"*3\r\n$3\r\nSET\r\n$1\r\nb\r\n$3\r\n".to_vec();
    raw.extend_from_slice(&[0xff, 0xfe, b'x']);
    raw.extend_from_slice(b"\r\n");
    assert_eq!(w.cmd_binary(raw).await, ok());

    let (fx, fy) = (two_frames(&mut x).await, two_frames(&mut y).await);
    assert_eq!(fx.len(), 2, "{fx:?}");
    assert_eq!(fx, fy, "two browsers, byte-identical pushes");
    assert!(
        matches!(&fx[0], Message::Text(t) if t.contains("plain")),
        "{:?}",
        fx[0]
    );
    assert!(
        matches!(&fx[1], Message::Binary(b) if b.windows(3).any(|w| w == [0xff, 0xfe, b'x'])),
        "{:?}",
        fx[1]
    );
    let mut p = WsClient::connect(primary.ws).await;
    caught_up(&mut p, &replica, "z").await;
    assert_converged(&primary, &replica, "replica");
}
