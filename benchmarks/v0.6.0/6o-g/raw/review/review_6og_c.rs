//! Review C probes (6O-G panic containment and convergence). Not for commit.

use super::mset_atomicity::{Pipe, bulk};
use super::request_path::Shared;
use crate::*;
use core_engine::resp::Value;
use core_engine::store::{EvictionPolicy, transaction_fault};
use std::time::Duration;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

type Ws = WebSocketStream<tokio::io::DuplexStream>;

fn ok() -> Value {
    Value::SimpleString("OK".into())
}

async fn ws(shared: &Shared) -> Ws {
    let (client, server) = tokio::io::duplex(64 * 1024);
    tokio::spawn(shared.serve_ws(server));
    let (ws, _) = tokio_tungstenite::client_async("ws://localhost/", client)
        .await
        .expect("ws handshake");
    ws
}

fn resp(args: &[&str]) -> String {
    let mut req = format!("*{}\r\n", args.len());
    for a in args {
        req.push_str(&format!("${}\r\n{a}\r\n", a.len()));
    }
    req
}

async fn ws_send(ws: &mut Ws, args: &[&str]) {
    ws.send(Message::Text(resp(args).into())).await.unwrap();
}

/// The next text/binary reply, parsed.
async fn ws_reply(ws: &mut Ws) -> Value {
    loop {
        match tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .expect("a reply")
        {
            Some(Ok(Message::Text(t))) => return Value::parse(t.as_bytes()).unwrap().0,
            Some(Ok(Message::Binary(b))) => return Value::parse(&b).unwrap().0,
            Some(Ok(_)) => continue,
            other => panic!("closed while waiting for a reply: {other:?}"),
        }
    }
}

async fn ws_cmd(ws: &mut Ws, args: &[&str]) -> Value {
    ws_send(ws, args).await;
    ws_reply(ws).await
}

/// `Some(code)` if the socket is closed (with that close code, or 0 for a
/// plain EOF/error) within `ms`; `None` if it stays open. Skips data frames.
async fn closed_within(ws: &mut Ws, ms: u64) -> Option<u16> {
    let fut = async {
        loop {
            match ws.next().await {
                Some(Ok(Message::Close(Some(frame)))) => return u16::from(frame.code),
                Some(Ok(Message::Close(None))) => return 0,
                Some(Ok(_)) => continue,
                _ => return 0,
            }
        }
    };
    tokio::time::timeout(Duration::from_millis(ms), fut).await.ok()
}

// ── C-1: the WebSocket EXEC whose reply cannot be sent skips the resync ──────

/// A WebSocket client runs the EXEC whose write panics, and goes away before
/// its reply is written. `ws_send!` then `break`s before
/// `resynchronise_clients`, so no other sync client is ever closed: their
/// copies never learn of the part-applied write.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn c1_other_sync_clients_are_resynchronised_even_if_the_exec_client_left() {
    let shared = Shared::new(false);
    let mut victim = ws(&shared).await;
    assert_eq!(
        ws_cmd(&mut victim, &["PING"]).await,
        Value::SimpleString("PONG".into())
    );

    let mut exec_client = ws(&shared).await;
    assert_eq!(ws_cmd(&mut exec_client, &["MULTI"]).await, ok());
    ws_cmd(&mut exec_client, &["SET", "rc1:a", "1"]).await;
    ws_cmd(&mut exec_client, &["SET", "rc1:b", "2"]).await;
    let armed = transaction_fault::panic_during_mutation_of("rc1:b");
    let pause = crate::propagation::exec_pause::after_command_naming("rc1:a");
    ws_send(&mut exec_client, &["EXEC"]).await;
    assert!(pause.reached(Duration::from_secs(10)).await, "EXEC never paused");
    // The client goes away while EXEC is in phase 2.
    drop(exec_client);
    tokio::time::sleep(Duration::from_millis(50)).await;
    drop(pause);
    drop(armed);

    // The part-applied write happened.
    assert_eq!(
        shared.store.execute(Command::Get("rc1:b".into())),
        bulk("2")
    );
    assert!(!shared.state.persistence_is_healthy());
    let closed = closed_within(&mut victim, 3_000).await;
    assert_eq!(
        closed,
        Some(WS_CLOSE_SYNC_GAP),
        "another sync client was never told to resynchronise after the part-applied write"
    );
}

// ── C-2: a TCP EXEC client that does not read holds back the resync ─────────

/// The resync of every WebSocket client waits for the TCP EXEC reply to be
/// written. A client that does not read a large reply holds it back for as
/// long as it likes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn c2_resync_does_not_wait_for_the_exec_clients_reader() {
    let shared = Shared::new(false);
    let big = "x".repeat(1 << 20);
    let mut setup = Pipe::tcp(&shared);
    assert_eq!(setup.cmd(&["SET", "rc2:big", &big]).await, ok());
    let mut victim = ws(&shared).await;
    assert_eq!(
        ws_cmd(&mut victim, &["PING"]).await,
        Value::SimpleString("PONG".into())
    );

    let mut txc = Pipe::tcp(&shared);
    assert_eq!(txc.cmd(&["MULTI"]).await, ok());
    txc.cmd(&["GET", "rc2:big"]).await;
    txc.cmd(&["SET", "rc2:k", "v"]).await;
    let armed = transaction_fault::panic_during_mutation_of("rc2:k");
    txc.send(&["EXEC"]).await;
    // The client does not read for a while.
    let closed_before_read = closed_within(&mut victim, 2_000).await;
    drop(armed);
    let reply = tokio::time::timeout(Duration::from_secs(10), txc.reply())
        .await
        .expect("EXEC reply");
    assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
    let closed_after_read = if closed_before_read.is_none() {
        closed_within(&mut victim, 2_000).await
    } else {
        closed_before_read
    };
    eprintln!("C-2: closed_before_read={closed_before_read:?} closed_after_read={closed_after_read:?}");
    assert_eq!(
        closed_before_read,
        Some(WS_CLOSE_SYNC_GAP),
        "sync clients were left on their diverged copy until the EXEC client read its reply"
    );
}

// ── C-3: the executing WebSocket client is closed with the sync-gap code ─────

/// What the 6O-G convergence test means to check with `recv_frame(..) ==
/// None` (which a 2 s silence also satisfies): the client gets its reply,
/// and then a close with the sync-gap code.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn c3_the_executing_ws_client_gets_its_reply_then_the_sync_gap_close() {
    let shared = Shared::new(false);
    let mut client = ws(&shared).await;
    assert_eq!(ws_cmd(&mut client, &["MULTI"]).await, ok());
    ws_cmd(&mut client, &["SET", "rc3:a", "1"]).await;
    ws_cmd(&mut client, &["SET", "rc3:b", "2"]).await;
    let armed = transaction_fault::panic_during_mutation_of("rc3:b");
    let reply = ws_cmd(&mut client, &["EXEC"]).await;
    drop(armed);
    let Value::Array(Some(items)) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(items[0], ok());
    assert_eq!(
        closed_within(&mut client, 3_000).await,
        Some(WS_CLOSE_SYNC_GAP)
    );
    let _ = CloseCode::Normal;
}

// ── C-4: eviction victims of the panicking write are never announced ────────

/// On a capped store the panicking write evicts a key before its mutation.
/// The victim list dies with the unwind: no DEL frame (covered by MISCONF and
/// the full resync), but also no WATCH notification — `converge` announces
/// only `primary_keys(cmd)`. After `SAVE` restores writes, a client that
/// WATCHed the victim commits although the key was removed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn c4_a_watch_on_a_key_the_panicking_write_evicted_is_invalidated() {
    let mut shared = Shared::new(false);
    shared.store = Arc::new(KeyValueStore::with_config(
        Some(1),
        None,
        EvictionPolicy::AllKeysLru,
    ));
    let mut setup = Pipe::tcp(&shared);
    assert_eq!(setup.cmd(&["SET", "rc4:victim", "v"]).await, ok());

    let mut watcher = Pipe::tcp(&shared);
    assert_eq!(watcher.cmd(&["WATCH", "rc4:victim"]).await, ok());

    let mut txc = Pipe::tcp(&shared);
    assert_eq!(txc.cmd(&["MULTI"]).await, ok());
    txc.cmd(&["SET", "rc4:new", "n"]).await;
    let armed = transaction_fault::panic_during_mutation_of("rc4:new");
    let reply = txc.cmd(&["EXEC"]).await;
    drop(armed);
    assert!(matches!(&reply, Value::Array(Some(items)) if matches!(&items[0], Value::Error(_))), "{reply:?}");
    assert_eq!(
        shared.store.execute(Command::Get("rc4:victim".into())),
        Value::BulkString(None),
        "the victim was evicted by the part-applied write"
    );
    // The operator restores writes.
    assert_eq!(setup.cmd(&["SAVE"]).await, ok());
    assert!(shared.state.persistence_is_healthy());

    assert_eq!(watcher.cmd(&["MULTI"]).await, ok());
    watcher.cmd(&["SET", "rc4:other", "1"]).await;
    let committed = watcher.cmd(&["EXEC"]).await;
    assert_eq!(
        committed,
        Value::Array(None),
        "a WATCH on a key removed by the part-applied write still committed"
    );
}

// ── C-5: a poisoned ephemeral map, the case 6O-G says it recovers ─────────────

fn poison<T: Send>(mutex: &std::sync::Mutex<T>) {
    std::thread::scope(|scope| {
        let _ = scope
            .spawn(|| {
                let _guard = mutex.lock();
                panic!("review-c: poison the ephemeral map");
            })
            .join();
    });
    assert!(mutex.is_poisoned());
}

/// Disconnect cleanup still reads the key map with `lock().map(..)
/// .unwrap_or_default()` (`ephemeral_claims_for`), so after a poison it sees
/// no claims and returns early: the key is never deleted.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c5a_disconnect_cleanup_after_a_poisoned_ephemeral_map() {
    let shared = Shared::new(false);
    let conn = 987_654_321u64;
    shared.state.claim_ephemeral("rc5:presence", conn);
    shared.store.execute(Command::Set(
        "rc5:presence".into(),
        b"here".to_vec(),
        SetOptions::default(),
    ));
    poison(&shared.state.ephemeral);
    release_ephemeral_state(
        conn,
        &shared.tx,
        &shared.state,
        &shared.watch_registry,
        &shared.store,
    )
    .await;
    assert_eq!(
        shared.store.execute(Command::Get("rc5:presence".into())),
        Value::BulkString(None),
        "the ephemeral key outlived its only holder"
    );
}

/// `SAVE` — the only way out of the 6O-G `MISCONF` — panics on a poisoned
/// ephemeral map (`durable_snapshot`'s `.expect`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c5b_save_after_a_poisoned_ephemeral_map() {
    let shared = Shared::new(false);
    poison(&shared.state.ephemeral_members);
    let (state, store) = (Arc::clone(&shared.state), Arc::clone(&shared.store));
    let saved = tokio::spawn(async move { state.save(&store).await }).await;
    assert!(
        matches!(saved, Ok(Ok(()))),
        "SAVE did not succeed: panicked = {}",
        saved.as_ref().err().is_some_and(|e| e.is_panic())
    );
}

// ── C-6: a second panic during the repair escapes every containment ──────────

/// `repair_after_panic` runs outside `catch_unwind`, and the server's
/// `fail_closed`/`converge` run only once `apply_transaction` returns. If the
/// repair itself panics (the same corrupted value the first panic came from,
/// read again by `rebuild_metadata`), the panic unwinds out of EXEC: the
/// connection task dies, and the server never fails closed, never rotates
/// the replication run id, never resynchronises clients. Simulated with the
/// store's own repair observer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn c6_a_panic_in_the_repair_still_fails_closed() {
    let shared = Shared::new(false);
    let run_id = shared.state.replicas.run_id();
    let _observer = transaction_fault::observe_repair(&shared.store, |_| {
        panic!("review-c: the repair hits the corrupted value again")
    });
    let mut txc = Pipe::tcp(&shared);
    assert_eq!(txc.cmd(&["MULTI"]).await, ok());
    txc.cmd(&["SET", "rc6:a", "1"]).await;
    txc.cmd(&["SET", "rc6:b", "2"]).await;
    let armed = transaction_fault::panic_during_mutation_of("rc6:b");
    txc.send(&["EXEC"]).await;
    let reply = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(3), txc.reply()).await
    })
    .await;
    drop(armed);
    eprintln!("C-6: EXEC reply task outcome: {:?}", reply.as_ref().map(|r| r.as_ref().map(|_| "a reply").map_err(|_| "timeout")).map_err(|e| e.is_panic()));
    assert_eq!(
        shared.store.execute(Command::Get("rc6:b".into())),
        bulk("2"),
        "the part-applied write is visible"
    );
    let mut other = Pipe::tcp(&shared);
    let write = other.cmd(&["SET", "rc6:c", "3"]).await;
    eprintln!(
        "C-6: healthy={} write={write:?} run_id_rotated={}",
        shared.state.persistence_is_healthy(),
        shared.state.replicas.run_id() != run_id
    );
    assert!(
        !shared.state.persistence_is_healthy(),
        "a part-applied write is in the keyspace and writes are still accepted ({write:?})"
    );
}
