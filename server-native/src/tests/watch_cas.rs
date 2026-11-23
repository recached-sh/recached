//! 6O-E3: a TCP `WATCH` is compare-and-set only.
//!
//! Its notifications set the connection's invalidated flag and queue
//! nothing: no copy of the value, no charge against the 8 MiB notification
//! budget, no slot in the 256-item queue. A change of any size, or any
//! number of changes, aborts `EXEC` with a nil reply and leaves the
//! connection usable. Redis does the same: WATCH is a dirty flag
//! (`CLIENT_DIRTY_CAS`). The overflow-safety tests of #157 are in
//! `watch_overflow_exec.rs`.

use super::mset_atomicity::{Pipe, WITHIN};
use super::request_path::Shared;
use core_engine::resp::Value;
use std::time::{Duration, Instant};

const BIG: usize = 9 * 1024 * 1024;

fn ok() -> Value {
    Value::SimpleString("OK".into())
}

async fn exec_once(pipe: &mut Pipe, marker: &str) -> Value {
    assert_eq!(pipe.cmd(&["MULTI"]).await, ok());
    pipe.cmd(&["SET", marker, "1"]).await;
    pipe.cmd(&["EXEC"]).await
}

/// Bytes charged to `conn_id`'s watch registration of `key`, if registered.
async fn charged(shared: &Shared, key: &str) -> Option<usize> {
    let reg = shared.watch_registry.map.lock().await;
    reg.get(key)
        .and_then(|subs| subs.first())
        .map(|sub| sub.pending_bytes())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_change_over_the_budget_aborts_exec_and_keeps_the_connection() {
    let shared = Shared::new(false);
    let (mut s, mut o) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let k = "6oe:cas:big";
    assert_eq!(s.cmd(&["WATCH", k]).await, ok());
    assert_eq!(o.cmd(&["SET", k, &"x".repeat(BIG)]).await, ok());
    assert_eq!(
        charged(&shared, k).await,
        Some(0),
        "nothing charged for the change"
    );
    assert_eq!(exec_once(&mut s, "6oe:cas:big:m").await, Value::Array(None));
    assert_eq!(
        s.cmd(&["GET", "6oe:cas:big:m"]).await,
        Value::BulkString(None)
    );
    assert_eq!(s.cmd(&["PING"]).await, Value::SimpleString("PONG".into()));
    // Clean afterwards: the next transaction commits.
    assert_eq!(s.cmd(&["WATCH", k]).await, ok());
    assert!(matches!(
        exec_once(&mut s, "6oe:cas:big:m").await,
        Value::Array(Some(_))
    ));
}

/// More changes than the queue has slots: a TCP watcher used to overflow
/// the 256-item queue and be disconnected. Now nothing is queued.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn more_changes_than_the_queue_holds_still_just_abort() {
    let shared = Shared::new(false);
    let (mut s, mut o) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let k = "6oe:cas:many";
    assert_eq!(s.cmd(&["WATCH", k]).await, ok());
    for i in 0..600 {
        o.cmd(&["SET", k, &i.to_string()]).await;
    }
    assert_eq!(charged(&shared, k).await, Some(0));
    assert_eq!(
        exec_once(&mut s, "6oe:cas:many:m").await,
        Value::Array(None)
    );
    assert_eq!(s.cmd(&["PING"]).await, Value::SimpleString("PONG".into()));
}

/// Several writers of large values to watched keys at once, against several
/// watchers: every watcher's EXEC aborts, none is disconnected, nothing is
/// charged.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_large_writers_abort_every_watcher() {
    let shared = Shared::new(false);
    let keys = ["6oe:cas:cw:1", "6oe:cas:cw:2", "6oe:cas:cw:3"];
    let mut watchers = Vec::new();
    for _ in 0..3 {
        let mut w = Pipe::tcp(&shared);
        assert_eq!(w.cmd(&["WATCH", keys[0], keys[1], keys[2]]).await, ok());
        watchers.push(w);
    }
    let mut writers = Vec::new();
    for (i, k) in keys.iter().enumerate() {
        let mut p = Pipe::tcp(&shared);
        let k = k.to_string();
        writers.push(tokio::spawn(async move {
            let big = format!("{i}{}", "y".repeat(BIG));
            assert_eq!(p.cmd(&["SET", &k, &big]).await, ok());
            assert_eq!(
                p.cmd(&["APPEND", &k, "z"]).await,
                Value::Integer(big.len() as i64 + 1)
            );
        }));
    }
    for w in writers {
        w.await.unwrap();
    }
    for k in keys {
        let reg = shared.watch_registry.map.lock().await;
        assert!(reg[k].iter().all(|sub| sub.pending_bytes() == 0), "{k}");
    }
    for (i, w) in watchers.iter_mut().enumerate() {
        assert_eq!(
            exec_once(w, &format!("6oe:cas:cw:m{i}")).await,
            Value::Array(None),
            "watcher {i}"
        );
        assert_eq!(w.cmd(&["PING"]).await, Value::SimpleString("PONG".into()));
    }
}

/// A large change to a key nobody watches leaves the transaction alone.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unrelated_large_change_does_not_abort() {
    let shared = Shared::new(false);
    let (mut s, mut o) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    assert_eq!(s.cmd(&["SET", "6oe:cas:mine", "v"]).await, ok());
    assert_eq!(s.cmd(&["WATCH", "6oe:cas:mine"]).await, ok());
    assert_eq!(
        o.cmd(&["SET", "6oe:cas:other", &"x".repeat(BIG)]).await,
        ok()
    );
    assert!(matches!(
        exec_once(&mut s, "6oe:cas:unrel:m").await,
        Value::Array(Some(_))
    ));
}

/// A watcher's connection task dropped while registered: its registrations
/// are released at the next change (the receiver is gone), as a failed send
/// used to release them; a disconnected client's are released by its
/// cleanup.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn registrations_of_gone_connections_are_released() {
    let shared = Shared::new(false);
    let (ka, kd) = ("6oe:cas:gone:aborted", "6oe:cas:gone:disconnected");
    let (mut aborted, task) = Pipe::tcp_task(&shared);
    assert_eq!(aborted.cmd(&["WATCH", ka]).await, ok());
    let mut disconnected = Pipe::tcp(&shared);
    assert_eq!(disconnected.cmd(&["WATCH", kd]).await, ok());
    task.abort();
    let _ = task.await;
    drop(disconnected);
    let mut o = Pipe::tcp(&shared);
    let deadline = Instant::now() + WITHIN;
    loop {
        o.cmd(&["SET", ka, "x"]).await;
        let reg = shared.watch_registry.map.lock().await;
        if !reg.contains_key(ka) && !reg.contains_key(kd) {
            break;
        }
        drop(reg);
        assert!(
            Instant::now() < deadline,
            "a gone connection's WATCH stayed registered"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    assert!(shared.watch_registry.is_empty());
}

/// A WebSocket watcher is still a push consumer: it is pushed the change and
/// its EXEC aborts. Subscriptions keep their full payloads.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn websocket_watchers_are_still_pushed_their_changes() {
    let srv = super::spawn_ws_server().await;
    let (mut s, mut o) = (
        super::WsClient::connect(srv.tcp_addr).await,
        super::WsClient::connect(srv.tcp_addr).await,
    );
    let k = "6oe:cas:ws";
    assert_eq!(s.cmd(&["WATCH", k]).await, ok());
    assert_eq!(o.cmd(&["SET", k, "pushed-value"]).await, ok());
    let mut seen = false;
    let deadline = Instant::now() + WITHIN;
    while !seen && Instant::now() < deadline {
        if let Some(frame) = s.recv_any(200).await {
            seen = frame.contains("keychange") && frame.contains("pushed-value");
        }
    }
    assert!(
        seen,
        "the WebSocket watcher was pushed the change with its value"
    );
    assert_eq!(s.cmd(&["MULTI"]).await, ok());
    s.cmd(&["SET", "6oe:cas:ws:m", "1"]).await;
    assert_eq!(s.cmd(&["EXEC"]).await, Value::Array(None));
}
