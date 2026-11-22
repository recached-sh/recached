//! 6O-B: the shared TCP request path — cross-connection reply coalescing,
//! and the overflow arm armed only once it can fire. The gate's lock-free
//! empty check is covered by the AOF gate's own tests (`write_gate`,
//! `request_path`'s replies-after-AOF check, `persistence`'s gate tests): a
//! check that missed a queued ticket would release a reply early.

use super::request_path::Shared;
use super::*;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, DuplexStream, ReadBuf};

/// `ENABLED` is process-wide: the tests that switch it, or depend on it, run
/// one at a time.
static SWITCH: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn switch_lock() -> std::sync::MutexGuard<'static, ()> {
    SWITCH.lock().unwrap_or_else(|p| p.into_inner())
}

// ── the back-off state machine ────────────────────────────────────────────────

#[test]
fn two_yields_that_gain_nothing_back_off_for_the_backoff_count() {
    let _g = switch_lock();
    reply_coalesce::reset();
    let probe = reply_coalesce::begin().expect("a fresh worker yields");
    assert!(!reply_coalesce::end(probe), "nobody ran: a miss");
    assert_eq!(reply_coalesce::state(), (0, 1));
    let probe = reply_coalesce::begin().expect("one miss does not back off");
    assert!(!reply_coalesce::end(probe));
    assert_eq!(
        reply_coalesce::state(),
        (reply_coalesce::BACKOFF, 0),
        "the second miss in a row backs off"
    );
    for i in 0..reply_coalesce::BACKOFF {
        assert!(
            reply_coalesce::begin().is_none(),
            "flush {i} of the back-off is immediate"
        );
    }
    assert!(
        reply_coalesce::begin().is_some(),
        "after the back-off the next flush probes again"
    );
    reply_coalesce::reset();
}

#[test]
fn a_yield_another_connection_ran_during_is_a_gain_and_clears_the_misses() {
    let _g = switch_lock();
    reply_coalesce::reset();
    let probe = reply_coalesce::begin().unwrap();
    assert!(!reply_coalesce::end(probe));
    let probe = reply_coalesce::begin().unwrap();
    reply_coalesce::note_batch(); // another connection's read batch
    assert!(reply_coalesce::end(probe), "a batch ran during the yield");
    assert_eq!(reply_coalesce::state(), (0, 0), "a gain clears the miss");
    // So a later single miss does not back off.
    let probe = reply_coalesce::begin().unwrap();
    assert!(!reply_coalesce::end(probe));
    assert_eq!(reply_coalesce::state(), (0, 1));
    reply_coalesce::reset();
}

#[test]
fn switched_off_never_yields() {
    let _g = switch_lock();
    reply_coalesce::reset();
    reply_coalesce::set_enabled(false);
    let none = reply_coalesce::begin();
    reply_coalesce::set_enabled(true);
    assert!(none.is_none());
    assert!(reply_coalesce::begin().is_some());
    reply_coalesce::reset();
}

// ── coalescing on the real connection loop ────────────────────────────────────

/// The server's end of an in-memory socket that records, at the first byte
/// the server writes, how many of `keys` the store holds: the commands that
/// had run by the time this connection's reply left.
struct Recording {
    inner: DuplexStream,
    store: Arc<KeyValueStore>,
    keys: Vec<String>,
    seen: Arc<std::sync::Mutex<Vec<usize>>>,
    recorded: bool,
}

impl AsyncRead for Recording {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for Recording {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if !self.recorded {
            self.recorded = true;
            let present = self
                .keys
                .iter()
                .filter(|k| self.store.get_current(k) != Value::BulkString(None))
                .count();
            self.seen.lock().unwrap().push(present);
        }
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// `CONNS` connections, each with one `SET` already waiting, served on one
/// current-thread worker: for each, how many of the `SET`s had run when its
/// first reply byte was written, in the order the replies left.
async fn first_reply_sees(conns: usize) -> Vec<usize> {
    let shared = Shared::new(false);
    let keys: Vec<String> = (0..conns).map(|i| format!("coalesce:{i}")).collect();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut clients = Vec::new();
    let mut servers = Vec::new();
    for key in &keys {
        let (mut client, server) = tokio::io::duplex(64 * 1024);
        let frame = format!("*3\r\n$3\r\nSET\r\n${}\r\n{key}\r\n$1\r\nv\r\n", key.len());
        client.write_all(frame.as_bytes()).await.unwrap();
        let socket = Recording {
            inner: server,
            store: Arc::clone(&shared.store),
            keys: keys.clone(),
            seen: Arc::clone(&seen),
            recorded: false,
        };
        servers.push(tokio::spawn(handle_tcp(
            socket,
            Arc::clone(&shared.store),
            shared.tx.clone(),
            Arc::new(None),
            Arc::clone(&shared.pubsub),
            Arc::clone(&shared.watch_registry),
            Arc::clone(&shared.state),
            "test".to_string(),
        )));
        clients.push(client);
    }
    for client in &mut clients {
        let mut reply = [0u8; 5];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"+OK\r\n");
    }
    drop(clients);
    for server in servers {
        let _ = server.await;
    }
    let seen = seen.lock().unwrap().clone();
    seen
}

#[tokio::test]
async fn replies_of_connections_ready_together_leave_after_all_their_commands_ran() {
    let _g = switch_lock();
    reply_coalesce::reset();
    // Several passes in a row on this thread: the yields must keep finding
    // company (the connections record their batches), or the second pass
    // would already be backed off and reply one by one.
    for pass in 0..4 {
        let coalesced = first_reply_sees(3).await;
        assert_eq!(
            coalesced,
            vec![3, 3, 3],
            "pass {pass}: every reply waits for the other ready connections' commands"
        );
    }

    // Control: with coalescing off the first reply leaves after only its own
    // command, so the assertion above is about the yield, not the harness.
    reply_coalesce::reset();
    reply_coalesce::set_enabled(false);
    let immediate = first_reply_sees(3).await;
    reply_coalesce::set_enabled(true);
    reply_coalesce::reset();
    assert_eq!(
        immediate[0], 1,
        "uncoalesced, the first reply left alone: {immediate:?}"
    );
}

#[tokio::test]
async fn a_lone_connection_backs_off_and_still_gets_every_reply_in_order() {
    let _g = switch_lock();
    reply_coalesce::reset();
    let server = spawn_server().await;
    let mut c = RespClient::connect(server.tcp_addr).await;
    // Far more round trips than MISSES + BACKOFF: the back-off cycles, and
    // every reply still arrives, in order, with the right content.
    for i in 0..200 {
        let key = format!("lone:{i}");
        assert_eq!(
            c.cmd(&["SET", &key, &i.to_string()]).await,
            Value::SimpleString("OK".into())
        );
        assert_eq!(
            c.cmd(&["GET", &key]).await,
            Value::BulkString(Some(i.to_string().into_bytes()))
        );
    }
}

// ── the overflow arm ──────────────────────────────────────────────────────────

/// The arm skipped until the connection can be sent an overflow must still
/// fire once it can: a subscriber whose queue overflows is disconnected.
#[tokio::test]
async fn a_subscriber_whose_queue_overflows_is_disconnected() {
    let server = spawn_server().await;
    let mut sub = RespClient::connect(server.tcp_addr).await;
    let ack = sub.cmd(&["SUBSCRIBE", "news"]).await;
    assert!(matches!(ack, Value::Array(Some(_))), "subscribed: {ack:?}");
    let mut publisher = RespClient::connect(server.tcp_addr).await;
    // One message over the subscriber's 8 MiB queue budget overflows it.
    let big = "x".repeat(9 * 1024 * 1024);
    let _ = publisher.cmd(&["PUBLISH", "news", &big]).await;
    let mut buf = [0u8; 4096];
    let closed = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            match sub.stream.read(&mut buf).await {
                Ok(0) | Err(_) => return true,
                Ok(_) => {}
            }
        }
    })
    .await;
    assert_eq!(
        closed,
        Ok(true),
        "the overflowed subscriber was disconnected"
    );
}

/// A connection that never subscribed or watched keeps working, with
/// pipelined commands and a reply of every kind on the path.
#[tokio::test]
async fn a_connection_that_never_subscribes_answers_pipelined_commands() {
    let server = spawn_server().await;
    let mut c = RespClient::connect(server.tcp_addr).await;
    let mut req = Vec::new();
    for i in 0..64 {
        req.extend_from_slice(
            format!("*3\r\n$3\r\nSET\r\n$4\r\nk{i:03}\r\n$1\r\nv\r\n").as_bytes(),
        );
        req.extend_from_slice(format!("*2\r\n$4\r\nINCR\r\n$4\r\nn{i:03}\r\n").as_bytes());
    }
    c.stream.write_all(&req).await.unwrap();
    for _ in 0..64 {
        assert_eq!(c.cmd(&[]).await, Value::SimpleString("OK".into()));
        assert_eq!(c.cmd(&[]).await, Value::Integer(1));
    }
}
