//! 6O-E: a transaction's queued commands, isolated from concurrent writes.
//!
//! Redis runs `EXEC` as one step: no other client's command runs between two
//! of a transaction's queued commands, and every queued command sees the
//! clock at one instant. These tests stop an `EXEC` right after one of its
//! queued commands (the test-only `exec_pause` hook, with every guard the
//! transaction holds still held), send a conflicting write from another
//! connection, and check what the transaction answered against the states a
//! one-at-a-time execution could have produced.
//!
//! A writer that must not run inside the transaction is seen waiting on a
//! replication-order stripe (`ReplHub::stripe_waits`, test builds), so no
//! test infers "blocked" from a timeout.
//!
//! The reproductions are `#[ignore]`d on the investigation branch, where they
//! fail; the fix removes the `ignore`.

use super::mset_atomicity::{Pipe, WITHIN, bulk};
use super::request_path::Shared;
use crate::propagation::exec_pause;
use core_engine::resp::Value;
use std::time::{Duration, Instant};

fn ok() -> Value {
    Value::SimpleString("OK".into())
}

fn queued() -> Value {
    Value::SimpleString("QUEUED".into())
}

fn array(items: Vec<Value>) -> Value {
    Value::Array(Some(items))
}

fn nil() -> Value {
    Value::BulkString(None)
}

/// Open a transaction on `pipe` and queue `commands`.
async fn queue(pipe: &mut Pipe, commands: &[&[&str]]) {
    assert_eq!(pipe.cmd(&["MULTI"]).await, ok());
    for command in commands {
        assert_eq!(pipe.cmd(command).await, queued(), "{command:?}");
    }
}

/// The reply to a write already sent on `pipe`, if it arrives before the
/// writer is seen waiting on a replication-order stripe (`None` once it is).
async fn reply_or_waiting(pipe: &mut Pipe, shared: &Shared, waits_before: usize) -> Option<Value> {
    let deadline = Instant::now() + WITHIN;
    while Instant::now() < deadline {
        if shared.state.replicas.stripe_waits() > waits_before {
            return None;
        }
        if let Ok(reply) = tokio::time::timeout(Duration::from_millis(2), pipe.reply()).await {
            return Some(reply);
        }
    }
    panic!("the writer neither answered nor waited for the transaction");
}

async fn reply_within(pipe: &mut Pipe, what: &str) -> Value {
    tokio::time::timeout(WITHIN, pipe.reply())
        .await
        .unwrap_or_else(|_| panic!("no {what} reply"))
}

/// Make every write take the ordered path, as on a server with an AOF,
/// replicas or browser clients: an unrelated WATCH on its own connection.
async fn force_ordered(shared: &Shared, key: &str) -> Pipe {
    let mut watcher = Pipe::tcp(shared);
    assert_eq!(watcher.cmd(&["WATCH", key]).await, ok());
    watcher
}

/// Test A: `MULTI GET a GET b EXEC`, stopped after `GET a`, while another
/// connection runs `MSET a new b new`. Only `[old, old]` and `[new, new]`
/// are states of the keyspace; `[old, new]` means the MSET ran between the
/// two queued reads.
async fn two_reads_versus_mset(ordered: bool, a: &str, b: &str) {
    let shared = Shared::new(false);
    let _watcher = if ordered {
        Some(force_ordered(&shared, &format!("{a}:unrelated")).await)
    } else {
        None
    };
    let (mut txc, mut writer) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    assert_eq!(writer.cmd(&["MSET", a, "old", b, "old"]).await, ok());
    queue(&mut txc, &[&["GET", a], &["GET", b]]).await;
    let pause = exec_pause::after_command_naming(a);
    txc.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await, "EXEC never reached GET {a}");
    let waits = shared.state.replicas.stripe_waits();
    writer.send(&["MSET", a, "new", b, "new"]).await;
    let during = reply_or_waiting(&mut writer, &shared, waits).await;
    drop(pause);
    let exec = reply_within(&mut txc, "EXEC").await;
    let wrote = match during.clone() {
        Some(reply) => reply,
        None => reply_within(&mut writer, "MSET").await,
    };
    assert_eq!(wrote, ok());
    let (old, new) = (
        array(vec![bulk("old"), bulk("old")]),
        array(vec![bulk("new"), bulk("new")]),
    );
    assert!(
        exec == old || exec == new,
        "EXEC answered {exec:?}: the MSET ran between its two reads \
         (it answered while the transaction was stopped: {})",
        during.is_some()
    );
    assert!(
        during.is_none(),
        "the MSET answered while EXEC was between its reads"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "6O-E0 reproduction: fails until EXEC isolates its queued reads"]
async fn two_queued_reads_never_straddle_an_unordered_mset() {
    two_reads_versus_mset(false, "6oe:a:u:a", "6oe:a:u:b").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "6O-E0 reproduction: fails until EXEC isolates its queued reads"]
async fn two_queued_reads_never_straddle_an_ordered_mset() {
    two_reads_versus_mset(true, "6oe:a:o:a", "6oe:a:o:b").await;
}

/// Test B: two separately committed SETs, `SET a new` and then `SET b new`
/// (sent once the first answered). The states a one-at-a-time execution can
/// show are `(old, old)`, `(new, old)` and `(new, new)`; `(old, new)` cannot
/// happen, since `b` became new only after `a` had. A transaction reading
/// `a` then `b` must answer one of the three.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "6O-E0 reproduction: fails until EXEC isolates its queued reads"]
async fn two_queued_reads_see_a_state_two_sets_passed_through() {
    let shared = Shared::new(false);
    let (a, b) = ("6oe:b:a", "6oe:b:b");
    let (mut txc, mut writer) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    writer.cmd(&["MSET", a, "old", b, "old"]).await;
    queue(&mut txc, &[&["GET", a], &["GET", b]]).await;
    let pause = exec_pause::after_command_naming(a);
    txc.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await);
    let waits = shared.state.replicas.stripe_waits();
    writer.send(&["SET", a, "new"]).await;
    let first = reply_or_waiting(&mut writer, &shared, waits).await;
    // Only once SET a has answered is SET b sent: it then follows it.
    let second_sent = first.is_some();
    let second = if second_sent {
        writer.send(&["SET", b, "new"]).await;
        reply_or_waiting(&mut writer, &shared, waits).await
    } else {
        None
    };
    drop(pause);
    let exec = reply_within(&mut txc, "EXEC").await;
    if first.is_none() {
        assert_eq!(reply_within(&mut writer, "SET a").await, ok());
    }
    if second_sent && second.is_none() {
        assert_eq!(reply_within(&mut writer, "SET b").await, ok());
    }
    let legal = [
        array(vec![bulk("old"), bulk("old")]),
        array(vec![bulk("new"), bulk("old")]),
        array(vec![bulk("new"), bulk("new")]),
    ];
    assert!(
        legal.contains(&exec),
        "EXEC answered {exec:?}, a state the keyspace never had"
    );
}

/// Test C: a mixed transaction. `GET a`, `SET b mine`, `GET b`, `GET c`,
/// stopped after `GET a`, while another connection runs `MSET a new c new`
/// and then `SET b theirs`. The transaction sees its own `SET b` and neither
/// external write; both external writes land after it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "6O-E0 reproduction: fails until EXEC isolates its queued reads"]
async fn a_mixed_transaction_sees_its_own_writes_and_no_one_elses() {
    let shared = Shared::new(false);
    let (a, b, c) = ("6oe:c:a", "6oe:c:b", "6oe:c:c");
    let (mut txc, mut writer) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    writer.cmd(&["MSET", a, "old", b, "old", c, "old"]).await;
    queue(
        &mut txc,
        &[&["GET", a], &["SET", b, "mine"], &["GET", b], &["GET", c]],
    )
    .await;
    let pause = exec_pause::after_command_naming(a);
    txc.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await);
    let waits = shared.state.replicas.stripe_waits();
    writer.send(&["MSET", a, "new", c, "new"]).await;
    let during = reply_or_waiting(&mut writer, &shared, waits).await;
    drop(pause);
    let exec = reply_within(&mut txc, "EXEC").await;
    if during.is_none() {
        assert_eq!(reply_within(&mut writer, "MSET").await, ok());
    }
    assert_eq!(writer.cmd(&["SET", b, "theirs"]).await, ok());
    assert_eq!(
        exec,
        array(vec![bulk("old"), ok(), bulk("mine"), bulk("old")]),
        "the transaction saw another connection's MSET part-way through"
    );
    assert_eq!(
        writer.cmd(&["MGET", a, b, c]).await,
        array(vec![bulk("new"), bulk("theirs"), bulk("new")])
    );
}

/// Test E: WATCH and isolation together. A transaction watching `a` and
/// reading `a` then `b` commits (no watched key changed before EXEC), and a
/// write to both that arrives while it runs waits for it.
///
/// Not a reproduction: EXEC already takes its watched keys' stripes, and
/// this write names `a`, so it passes before the fix too. It guards that
/// the fix keeps the watched keys in the transaction's scope.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_watched_transaction_is_isolated_while_it_runs() {
    let shared = Shared::new(false);
    let (a, b) = ("6oe:e:a", "6oe:e:b");
    let (mut txc, mut writer) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    writer.cmd(&["MSET", a, "old", b, "old"]).await;
    assert_eq!(txc.cmd(&["WATCH", a]).await, ok());
    queue(&mut txc, &[&["GET", a], &["GET", b]]).await;
    let pause = exec_pause::after_command_naming(a);
    txc.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await);
    let waits = shared.state.replicas.stripe_waits();
    writer.send(&["MSET", b, "new", a, "new"]).await;
    let during = reply_or_waiting(&mut writer, &shared, waits).await;
    drop(pause);
    let exec = reply_within(&mut txc, "EXEC").await;
    if during.is_none() {
        assert_eq!(reply_within(&mut writer, "MSET").await, ok());
    }
    assert_eq!(exec, array(vec![bulk("old"), bulk("old")]));
}

/// The clock: Redis evaluates every queued command at the instant EXEC
/// began (7.2.5: `SET a v PX 100`, `MULTI`, `GET a`, `DEBUG SLEEP 0.2`,
/// `GET a`, `PTTL a`, `EXEC` answers `v`, `v`, `97`). A key that expires
/// while the transaction runs must not be live to one queued read and gone
/// to the next.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "6O-E0 reproduction: fails until EXEC reads the clock once"]
async fn every_queued_command_sees_the_clock_at_one_instant() {
    let shared = Shared::new(false);
    let a = "6oe:clock:a";
    let mut txc = Pipe::tcp(&shared);
    let set_at = Instant::now();
    assert_eq!(txc.cmd(&["SET", a, "v", "PX", "150"]).await, ok());
    queue(
        &mut txc,
        &[&["GET", a], &["GET", a], &["PTTL", a], &["EXISTS", a]],
    )
    .await;
    let pause = exec_pause::after_command_naming(a);
    txc.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await);
    // Past the deadline, with the transaction stopped after its first read.
    let deadline = set_at + Duration::from_millis(250);
    tokio::time::sleep(deadline.saturating_duration_since(Instant::now())).await;
    drop(pause);
    let exec = reply_within(&mut txc, "EXEC").await;
    let Value::Array(Some(items)) = &exec else {
        panic!("EXEC answered {exec:?}");
    };
    if items[0] == nil() {
        // The first read already ran after the deadline (a very slow host):
        // nothing to check.
        return;
    }
    assert_eq!(items[0], bulk("v"));
    assert_eq!(
        items[1],
        bulk("v"),
        "the key was live to one queued read and expired to the next: {exec:?}"
    );
    assert!(
        matches!(items[2], Value::Integer(ms) if ms > 0),
        "PTTL: {exec:?}"
    );
    assert_eq!(items[3], Value::Integer(1), "EXISTS: {exec:?}");
}

/// Control: without a concurrent writer, the same transactions answer the
/// sequential results (on every build).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn transactions_answer_sequentially_without_a_concurrent_writer() {
    let shared = Shared::new(false);
    let (a, b, c) = ("6oe:ctl:a", "6oe:ctl:b", "6oe:ctl:c");
    let mut txc = Pipe::tcp(&shared);
    txc.cmd(&["MSET", a, "old", b, "old", c, "old"]).await;
    queue(&mut txc, &[&["GET", a], &["GET", b]]).await;
    assert_eq!(
        txc.cmd(&["EXEC"]).await,
        array(vec![bulk("old"), bulk("old")])
    );
    queue(
        &mut txc,
        &[&["GET", a], &["SET", b, "mine"], &["GET", b], &["GET", c]],
    )
    .await;
    assert_eq!(
        txc.cmd(&["EXEC"]).await,
        array(vec![bulk("old"), ok(), bulk("mine"), bulk("old")])
    );
    // The stop itself changes no answer: armed, reached, released.
    queue(&mut txc, &[&["GET", a], &["GET", b]]).await;
    let pause = exec_pause::after_command_naming(a);
    txc.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await);
    drop(pause);
    assert_eq!(
        reply_within(&mut txc, "EXEC").await,
        array(vec![bulk("old"), bulk("mine")])
    );
}

/// Finding F1 (recorded, out of scope for 6O-E): a transaction's writes are
/// visible part-way to a reader outside it. `MULTI SET a new SET b new EXEC`
/// stopped between the two SETs; another connection's `MGET a b`. Redis
/// answers `[old, old]` or `[new, new]`. Kept `#[ignore]`d as the record of
/// the gap: it fails on `dev` and after 6O-E.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "F1: a transaction's writes are not atomic to outside readers (documented, out of scope)"]
async fn an_outside_reader_never_sees_a_transaction_part_way() {
    let shared = Shared::new(false);
    let (a, b) = ("6oe:f1:a", "6oe:f1:b");
    let (mut txc, mut reader) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    txc.cmd(&["MSET", a, "old", b, "old"]).await;
    queue(&mut txc, &[&["SET", a, "new"], &["SET", b, "new"]]).await;
    let pause = exec_pause::after_command_naming(a);
    txc.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await);
    let seen = reader.cmd(&["MGET", a, b]).await;
    drop(pause);
    reply_within(&mut txc, "EXEC").await;
    assert!(
        seen == array(vec![bulk("old"), bulk("old")])
            || seen == array(vec![bulk("new"), bulk("new")]),
        "an outside MGET saw the transaction part-way: {seen:?}"
    );
}
