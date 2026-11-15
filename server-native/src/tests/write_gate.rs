//! The unordered write fast path and the gate that keeps it sound.
//!
//! A write that nothing observes skips the replication-order stripes (see
//! `UnorderedGate`). Each test here parks such a write *inside* the gate —
//! past its consumer checks, before it touches the store — with the
//! test-only `unordered_pause` hook, then registers a consumer or takes a
//! barrier against it, and checks the one outcome the ordering model allows:
//! the in-flight write completes first and is seen by what comes after.
//!
//! Every one of these fails if the corresponding `close()` is removed (the
//! consumer registers, or the barrier proceeds, while the write is parked).

use super::request_path::Shared;
use crate::*;
use core_engine::resp::Value;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

/// How long a parked write stays parked: long enough that every step a test
/// takes meanwhile lands inside the window.
const PARK: Duration = Duration::from_millis(400);

struct Pipe {
    io: DuplexStream,
    buf: Vec<u8>,
}

impl Pipe {
    /// A TCP-protocol connection to `shared`, served on the current runtime.
    fn tcp(shared: &Shared) -> Pipe {
        let (client, server) = tokio::io::duplex(64 * 1024);
        tokio::spawn(shared.serve(server));
        Pipe {
            io: client,
            buf: Vec::new(),
        }
    }

    async fn send(&mut self, args: &[&str]) {
        let mut req = format!("*{}\r\n", args.len());
        for a in args {
            req.push_str(&format!("${}\r\n{a}\r\n", a.len()));
        }
        self.io.write_all(req.as_bytes()).await.expect("write");
    }

    async fn reply(&mut self) -> Value {
        loop {
            if let Ok((v, n)) = Value::parse(&self.buf) {
                self.buf.drain(..n);
                return v;
            }
            let mut chunk = [0u8; 4096];
            let n = tokio::time::timeout(Duration::from_secs(10), self.io.read(&mut chunk))
                .await
                .expect("reply within 10 s")
                .expect("read");
            assert!(n > 0, "connection closed");
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    async fn cmd(&mut self, args: &[&str]) -> Value {
        self.send(args).await;
        self.reply().await
    }
}

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

/// Disarms the pause hook even when an assertion fails.
struct Pause;
impl Pause {
    fn arm(key: &str) -> Pause {
        crate::propagation::unordered_pause::set(key, PARK);
        Pause
    }
}
impl Drop for Pause {
    fn drop(&mut self) {
        crate::propagation::unordered_pause::clear();
    }
}

/// Start `args` on its own connection and wait until it is parked inside the
/// gate. Returns the connection, whose reply arrives once the write ran.
async fn park(shared: &Shared, args: &[&str]) -> Pipe {
    let mut writer = Pipe::tcp(shared);
    writer.send(args).await;
    // The parked write is synchronous on a worker thread; give it time to get
    // there. Well inside PARK.
    tokio::time::sleep(Duration::from_millis(80)).await;
    writer
}

// The pause hook is process-global; these tests share it, so they take turns.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[test]
fn gate_close_waits_for_a_write_in_flight() {
    let hub = ReplHub::new();
    let gate = &hub.unordered;
    let pass = gate.try_enter().expect("open gate admits a write");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async {
        let closing = gate.close();
        tokio::pin!(closing);
        // Cannot complete while the pass is held...
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut closing)
                .await
                .is_err(),
            "close() returned with a write still in flight"
        );
        assert!(gate.is_closed());
        // ...and the closed gate admits nobody.
        assert!(gate.try_enter().is_none());
        drop(pass);
        let closed = closing.await;
        assert!(gate.try_enter().is_none(), "still closed while held");
        drop(closed);
        assert!(gate.try_enter().is_some(), "reopened on drop");
    });
}

#[test]
fn plain_writes_take_the_unordered_path_only_with_no_consumers() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async {
        let shared = Shared::new(false);
        let store = Arc::clone(&shared.store);
        let state = Arc::clone(&shared.state);
        let tx = shared.tx.clone();
        let wr = Arc::clone(&shared.watch_registry);
        let cmd = core_engine::cmd::Command::Incr("ctr".into());
        let ran = || crate::propagation::try_execute_unordered(&cmd, &state, &wr, &store);
        assert!(ran().is_some(), "standalone write is unordered");
        let receiver = state
            .replicas
            .subscribe_pushes(&tx, Routes::Everything)
            .await;
        assert!(ran().is_none(), "a WebSocket receiver orders writes");
        drop(receiver);
        assert!(ran().is_some());
        let closed = state.replicas.unordered.close().await;
        assert!(ran().is_none(), "a closed gate orders writes");
        drop(closed);
        state.replicas.configure_backlog(1024);
        assert!(ran().is_none(), "a replication backlog orders writes");
    });
}

/// WATCH must not take effect while a write it could miss is in flight: the
/// write either completes before WATCH answers, or is seen and aborts EXEC.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn watch_registered_during_an_unordered_write_still_protects_exec() {
    let _serial = SERIAL.lock().await;
    let shared = Shared::new(false);
    let _pause = Pause::arm("cas:k");
    let mut writer = park(&shared, &["SET", "cas:k", "from-writer"]).await;

    let mut client = Pipe::tcp(&shared);
    assert_eq!(
        client.cmd(&["WATCH", "cas:k"]).await,
        Value::SimpleString("OK".into())
    );
    let seen = client.cmd(&["GET", "cas:k"]).await;
    // Let the parked write finish, whatever WATCH did.
    assert_eq!(writer.reply().await, Value::SimpleString("OK".into()));
    client.cmd(&["MULTI"]).await;
    client.cmd(&["GET", "cas:k"]).await;
    client.cmd(&["SET", "cas:k", "from-exec"]).await;
    match client.cmd(&["EXEC"]).await {
        // Aborted: the write landed after WATCH and was noticed.
        Value::Array(None) => {}
        // Committed: then nothing may have changed the key since WATCH.
        Value::Array(Some(results)) => assert_eq!(
            results.first(),
            Some(&seen),
            "EXEC committed although cas:k changed after WATCH (saw {seen:?} before)"
        ),
        other => panic!("EXEC: {other:?}"),
    }
}

/// The all-write barrier (snapshot, replica attach, active expiry, QSUB's
/// initial state) must not be taken over a half-done unordered write.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_all_write_barrier_waits_for_an_unordered_write() {
    let _serial = SERIAL.lock().await;
    let shared = Shared::new(false);
    let _pause = Pause::arm("bar:k");
    let mut writer = park(&shared, &["INCR", "bar:k"]).await;

    let barrier = shared.state.replicas.lock_all_writes().await;
    // Whatever the barrier now captures must include the write.
    assert_eq!(
        shared
            .store
            .execute(core_engine::cmd::Command::Get("bar:k".into())),
        bulk("1"),
        "barrier acquired while an unordered write was still in flight"
    );
    drop(barrier);
    assert_eq!(writer.reply().await, Value::Integer(1));
}

/// EXEC isolates its keys from other writers: an unordered write in flight
/// finishes before the transaction starts.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn exec_does_not_overlap_an_unordered_write() {
    let _serial = SERIAL.lock().await;
    let shared = Shared::new(false);
    let _pause = Pause::arm("tx:k");
    let mut writer = park(&shared, &["INCR", "tx:k"]).await;

    let mut client = Pipe::tcp(&shared);
    client.cmd(&["MULTI"]).await;
    client.cmd(&["GET", "tx:k"]).await;
    client.cmd(&["INCR", "tx:k"]).await;
    let results = client.cmd(&["EXEC"]).await;
    assert_eq!(
        results,
        Value::Array(Some(vec![bulk("1"), Value::Integer(2)])),
        "EXEC ran while an unordered INCR on its key was in flight"
    );
    assert_eq!(writer.reply().await, Value::Integer(1));
}

/// A WebSocket receiver subscribes with the gate closed: a write in flight at
/// that moment completes before the socket is live.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn websocket_subscription_waits_for_an_unordered_write() {
    let _serial = SERIAL.lock().await;
    let shared = Shared::new(false);
    let _pause = Pause::arm("ws:k");
    let mut writer = park(&shared, &["SET", "ws:k", "v"]).await;

    let (client, server) = tokio::io::duplex(64 * 1024);
    tokio::spawn(shared.serve_ws(server));
    let (mut ws, _) = tokio_tungstenite::client_async("ws://localhost/", client)
        .await
        .expect("handshake");
    use futures_util::{SinkExt, StreamExt};
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        "*1\r\n$4\r\nPING\r\n".into(),
    ))
    .await
    .expect("send");
    let _pong = ws.next().await.expect("reply").expect("frame");
    assert_eq!(
        shared
            .store
            .execute(core_engine::cmd::Command::Get("ws:k".into())),
        bulk("v"),
        "WebSocket subscribed while an unordered write was still in flight"
    );
    assert_eq!(writer.reply().await, Value::SimpleString("OK".into()));
}

/// An ephemeral claim closes the gate: a plain SET in flight on the claimed
/// key lands before the claim, so the claim (not the SET) owns the key and
/// the SET is not deleted when the claiming connection goes away later.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_ephemeral_claim_does_not_overlap_an_unordered_write() {
    let _serial = SERIAL.lock().await;
    let shared = Shared::new(false);
    let _pause = Pause::arm("eph:k");
    let mut writer = park(&shared, &["SET", "eph:k", "plain"]).await;

    let mut claimer = Pipe::tcp(&shared);
    assert_eq!(
        claimer.cmd(&["ESET", "eph:k", "ephemeral"]).await,
        Value::SimpleString("OK".into())
    );
    assert_eq!(writer.reply().await, Value::SimpleString("OK".into()));
    // The SET finished first, so ESET's value is current and its claim holds.
    assert_eq!(
        shared
            .store
            .execute(core_engine::cmd::Command::Get("eph:k".into())),
        bulk("ephemeral"),
        "ESET claimed while a plain SET on its key was still in flight"
    );
}

/// Many workers hammering one key through the unordered path lose nothing,
/// and CAS transactions racing them stay exact.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_hot_key_writes_and_cas_stay_exact() {
    let _serial = SERIAL.lock().await;
    let shared = Arc::new(Shared::new(false));
    const WRITERS: usize = 8;
    const PER_WRITER: usize = 2_000;
    let mut tasks = Vec::new();
    for _ in 0..WRITERS {
        let shared = Arc::clone(&shared);
        tasks.push(tokio::spawn(async move {
            let mut p = Pipe::tcp(&shared);
            for _ in 0..PER_WRITER {
                p.send(&["INCR", "hot"]).await;
            }
            for _ in 0..PER_WRITER {
                assert!(matches!(p.reply().await, Value::Integer(_)));
            }
        }));
    }
    // CAS increments of a second key racing plain INCRs of it.
    let cas_shared = Arc::clone(&shared);
    let cas = tokio::spawn(async move {
        let mut p = Pipe::tcp(&cas_shared);
        let mut committed = 0i64;
        for _ in 0..200 {
            p.cmd(&["WATCH", "cas"]).await;
            let cur = match p.cmd(&["GET", "cas"]).await {
                Value::BulkString(Some(b)) => String::from_utf8(b).unwrap().parse::<i64>().unwrap(),
                _ => 0,
            };
            p.cmd(&["MULTI"]).await;
            let next = (cur + 1).to_string();
            p.cmd(&["SET", "cas", &next]).await;
            if matches!(p.cmd(&["EXEC"]).await, Value::Array(Some(_))) {
                committed += 1;
            }
        }
        committed
    });
    let plain_shared = Arc::clone(&shared);
    let plain = tokio::spawn(async move {
        let mut p = Pipe::tcp(&plain_shared);
        for _ in 0..500 {
            p.cmd(&["INCR", "cas"]).await;
        }
    });
    for t in tasks {
        t.await.expect("writer");
    }
    let committed = cas.await.expect("cas");
    plain.await.expect("plain");
    assert_eq!(
        shared
            .store
            .execute(core_engine::cmd::Command::Get("hot".into())),
        bulk(&(WRITERS * PER_WRITER).to_string())
    );
    // A committed CAS adds exactly one to the value it read, and an aborted
    // one adds nothing, so the key is exactly the sum of both kinds.
    assert_eq!(
        shared
            .store
            .execute(core_engine::cmd::Command::Get("cas".into())),
        bulk(&(committed + 500).to_string())
    );
}

/// Active expiry keeps up with more than one chunk of expired keys per tick
/// (it used to remove at most 256 a second, so a TTL-churn load grew the
/// keyspace without bound), and stops after one chunk when a sample shows
/// little to remove.
#[tokio::test]
async fn one_expiry_tick_removes_far_more_than_one_chunk() {
    use core_engine::cmd::{SetExpiry, SetOptions};
    let shared = Shared::new(false);
    let px = |ms| SetOptions {
        expiry: Some(SetExpiry::Px(ms)),
        ..SetOptions::default()
    };
    for i in 0..5_000 {
        shared
            .store
            .execute(Command::Set(format!("gone:{i}"), b"v".to_vec(), px(1)));
    }
    for i in 0..1_000 {
        shared.store.execute(Command::Set(
            format!("live:{i}"),
            b"v".to_vec(),
            px(3_600_000),
        ));
    }
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let removed = sweep_expired_tick(&shared.store, &shared.state, &shared.watch_registry).await;
    // Several chunks in one tick, where one chunk was the old ceiling. (The
    // exact count depends on how many keys the time budget covers, which a
    // debug build running beside other tests makes much smaller.)
    assert!(
        removed >= 4 * EXPIRY_CHUNK,
        "one tick removed only {removed} of 5,000 expired keys"
    );
    assert_eq!(shared.store.key_count(), 6_000 - removed);

    // The rest go on later ticks: a sample under a quarter expired ends a
    // tick early (as in Redis), but every tick examines at least a chunk.
    let mut ticks = 0;
    while shared.store.key_count() > 1_000 && ticks < 200 {
        sweep_expired_tick(&shared.store, &shared.state, &shared.watch_registry).await;
        ticks += 1;
    }
    assert_eq!(
        shared.store.key_count(),
        1_000,
        "expired keys left after {ticks} ticks"
    );
    let started = std::time::Instant::now();
    assert_eq!(
        sweep_expired_tick(&shared.store, &shared.state, &shared.watch_registry).await,
        0
    );
    assert!(
        started.elapsed() < EXPIRY_TICK_BUDGET,
        "a fruitless tick ran its whole budget"
    );
}
