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
//! The reproductions failed on the investigation branch (10/10 runs on
//! `1f9232f`), where they ran only with `RECACHED_REPRODUCE_6OE=1`; the fix
//! removed that gate.

use super::mset_atomicity::{Pipe, WITHIN, bulk};
use super::request_path::Shared;
use crate::propagation::exec_pause;
use crate::*;
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
async fn two_queued_reads_never_straddle_an_unordered_mset() {
    two_reads_versus_mset(false, "6oe:a:u:a", "6oe:a:u:b").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_queued_reads_never_straddle_an_ordered_mset() {
    two_reads_versus_mset(true, "6oe:a:o:a", "6oe:a:o:b").await;
}

/// Test B: two separately committed SETs, `SET a new` and then `SET b new`
/// (sent once the first answered). The states a one-at-a-time execution can
/// show are `(old, old)`, `(new, old)` and `(new, new)`; `(old, new)` cannot
/// happen, since `b` became new only after `a` had. A transaction reading
/// `a` then `b` must answer one of the three.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
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
/// answers `[old, old]` or `[new, new]`. Kept as the record of the gap: it
/// fails on `dev` and after 6O-E, so it runs only with
/// `RECACHED_PROBE_6OE_F1=1` (CI runs ignored tests too).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_outside_reader_never_sees_a_transaction_part_way() {
    if std::env::var_os("RECACHED_PROBE_6OE_F1").is_none() {
        eprintln!("F1 probe skipped: set RECACHED_PROBE_6OE_F1=1 to run it");
        return;
    }
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

// ── Test D: histories under load ─────────────────────────────────────────────

/// One completed operation: the generation it wrote or read, and the instants
/// just before its request was sent and just after its reply arrived.
#[derive(Clone, Copy, Debug)]
struct Op {
    generation: u64,
    invoke: Instant,
    respond: Instant,
}

const D_KEYS: [&str; 4] = ["6oe:d:k0", "6oe:d:k1", "6oe:d:k2", "6oe:d:k3"];

/// The transactions the readers queue, over overlapping keys in different
/// orders: single GETs, MGETs, and both.
fn d_reads(round: usize) -> Vec<Vec<&'static str>> {
    let [k0, k1, k2, k3] = D_KEYS;
    let shapes: [Vec<Vec<&str>>; 4] = [
        vec![vec!["GET", k0], vec!["GET", k3]],
        vec![vec!["MGET", k2, k1], vec!["GET", k0]],
        vec![vec!["GET", k1], vec!["MGET", k3, k0, k2]],
        vec![
            vec!["GET", k3],
            vec!["GET", k2],
            vec!["GET", k1],
            vec!["GET", k0],
        ],
    ];
    shapes[round % shapes.len()].clone()
}

fn generation(value: &Value) -> u64 {
    match value {
        Value::BulkString(Some(v)) => std::str::from_utf8(v).unwrap().parse().unwrap(),
        other => panic!("not a generation: {other:?}"),
    }
}

/// Every key the writers write holds the same generation after each write,
/// so the keyspace is one register: a reader's transaction must see one
/// generation in every key it reads, and the history of writes and reads
/// must be linearizable as a register with unique values:
///
/// - the write it read was invoked before the read responded;
/// - no write that began after that write finished completed before the read
///   began (the read would have had to see it, or a later one);
/// - no read that began after another read finished saw an older write
///   than that one (new-then-old inversion).
fn check_register_history(writes: &[Op], reads: &[Op]) {
    use std::collections::HashMap;
    let by_generation: HashMap<u64, Op> = writes.iter().map(|w| (w.generation, *w)).collect();
    let write_of = |r: &Op| -> Op {
        *by_generation
            .get(&r.generation)
            .unwrap_or_else(|| panic!("read generation {} was never written", r.generation))
    };
    let mut writes_by_respond: Vec<Op> = writes.to_vec();
    writes_by_respond.sort_by_key(|w| w.respond);
    for r in reads {
        let w = write_of(r);
        assert!(
            w.invoke < r.respond,
            "read {r:?} saw a write invoked after it: {w:?}"
        );
        // A write w2 with w.respond < w2.invoke and w2.respond < r.invoke
        // would make r's answer stale.
        let finished_before = writes_by_respond.partition_point(|w2| w2.respond < r.invoke);
        if let Some(stale) = writes_by_respond[..finished_before]
            .iter()
            .find(|w2| w.respond < w2.invoke)
        {
            panic!("read {r:?} returned {w:?}, but {stale:?} finished before the read began");
        }
    }
    // New-then-old: sort reads by respond; the latest write-invoke seen by any
    // read that finished before r began must not be after r's write finished.
    let mut by_respond: Vec<(Instant, Instant)> = reads
        .iter()
        .map(|r| (r.respond, write_of(r).invoke))
        .collect();
    by_respond.sort();
    let mut prefix_max: Vec<Instant> = Vec::with_capacity(by_respond.len());
    for (i, (_, invoke)) in by_respond.iter().enumerate() {
        prefix_max.push(if i == 0 {
            *invoke
        } else {
            prefix_max[i - 1].max(*invoke)
        });
    }
    for r in reads {
        let n = by_respond.partition_point(|(respond, _)| *respond < r.invoke);
        if n > 0 {
            let latest = prefix_max[n - 1];
            assert!(
                !(write_of(r).respond < latest),
                "read {r:?} saw a write older than one an earlier read already saw"
            );
        }
    }
}

async fn histories_under_load(ordered: bool) {
    let shared = Shared::new(false);
    let _watcher = if ordered {
        Some(force_ordered(&shared, "6oe:d:unrelated").await)
    } else {
        None
    };
    let start = Instant::now();
    {
        let mut init = Pipe::tcp(&shared);
        let mut args = vec!["MSET"];
        for k in D_KEYS {
            args.extend([k, "0"]);
        }
        assert_eq!(init.cmd(&args).await, ok());
    }
    let initial = Op {
        generation: 0,
        invoke: start,
        respond: Instant::now(),
    };
    let until = Instant::now() + Duration::from_millis(1500);
    let mut tasks = Vec::new();
    // Three writers: two plain MSETs in rotating key orders, and one writing
    // every key with separate SETs inside MULTI/EXEC (a mixed transaction).
    for w in 0..3u64 {
        let mut pipe = Pipe::tcp(&shared);
        tasks.push(tokio::spawn(async move {
            let mut ops = Vec::new();
            let mut i = 0u64;
            while Instant::now() < until {
                i += 1;
                let generation = (w + 1) * 1_000_000 + i;
                let g = generation.to_string();
                let rot = (w as usize + i as usize) % D_KEYS.len();
                let order: Vec<&str> = (0..D_KEYS.len())
                    .map(|j| D_KEYS[(j + rot) % D_KEYS.len()])
                    .collect();
                let invoke = Instant::now();
                if w == 2 {
                    pipe.send(&["MULTI"]).await;
                    for k in &order {
                        pipe.send(&["SET", k, &g]).await;
                    }
                    pipe.send(&["EXEC"]).await;
                    for _ in 0..order.len() + 1 {
                        pipe.reply().await;
                    }
                    let exec = pipe.reply().await;
                    assert!(matches!(exec, Value::Array(Some(_))), "{exec:?}");
                } else {
                    let mut args = vec!["MSET"];
                    for k in &order {
                        args.extend([*k, g.as_str()]);
                    }
                    pipe.send(&args).await;
                    assert_eq!(pipe.reply().await, ok());
                }
                ops.push(Op {
                    generation,
                    invoke,
                    respond: Instant::now(),
                });
            }
            (ops, Vec::new())
        }));
    }
    for r in 0..3usize {
        let mut pipe = Pipe::tcp(&shared);
        tasks.push(tokio::spawn(async move {
            let mut reads = Vec::new();
            let mut round = r;
            while Instant::now() < until {
                round += 1;
                let commands = d_reads(round);
                let invoke = Instant::now();
                pipe.send(&["MULTI"]).await;
                for c in &commands {
                    pipe.send(c).await;
                }
                pipe.send(&["EXEC"]).await;
                for _ in 0..commands.len() + 1 {
                    pipe.reply().await;
                }
                let exec = pipe.reply().await;
                let respond = Instant::now();
                let Value::Array(Some(replies)) = exec else {
                    panic!("EXEC answered {exec:?}");
                };
                let mut seen = Vec::new();
                for reply in &replies {
                    match reply {
                        Value::Array(Some(values)) => seen.extend(values.iter().map(generation)),
                        single => seen.push(generation(single)),
                    }
                }
                assert!(
                    seen.windows(2).all(|p| p[0] == p[1]),
                    "transaction {commands:?} saw several generations: {seen:?}"
                );
                reads.push(Op {
                    generation: seen[0],
                    invoke,
                    respond,
                });
            }
            (Vec::new(), reads)
        }));
    }
    let (mut writes, mut reads) = (vec![initial], Vec::new());
    for task in tasks {
        let (w, r) = tokio::time::timeout(WITHIN, task)
            .await
            .expect("a client finished")
            .expect("no client panicked");
        writes.extend(w);
        reads.extend(r);
    }
    assert!(
        reads.len() > 50 && writes.len() > 50,
        "{} reads, {} writes",
        reads.len(),
        writes.len()
    );
    check_register_history(&writes, &reads);
}

#[tokio::test(flavor = "current_thread")]
async fn histories_under_load_are_linearizable_on_one_worker() {
    histories_under_load(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn histories_under_load_are_linearizable_on_four_workers() {
    histories_under_load(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ordered_histories_under_load_are_linearizable() {
    histories_under_load(true).await;
}

#[test]
fn the_history_checker_refuses_what_it_should() {
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let op = |generation, invoke, respond| Op {
        generation,
        invoke: at(invoke),
        respond: at(respond),
    };
    let writes = [op(0, 0, 1), op(1, 10, 11), op(2, 20, 40)];
    // Fine: reads that overlap or follow the write they saw, and two
    // overlapping reads that saw 2 then 1 while write 2 was in progress.
    check_register_history(&writes, &[op(1, 12, 13), op(2, 15, 25), op(1, 9, 12)]);
    check_register_history(&writes, &[op(2, 25, 30), op(1, 26, 31)]);
    // Stale: write 2 finished (40) before the read began (45).
    assert!(
        std::panic::catch_unwind(|| check_register_history(&writes, &[op(1, 45, 46)])).is_err()
    );
    // Inversion: a read saw 2 (write 2 still in progress), a read that began
    // after it finished saw 1.
    assert!(
        std::panic::catch_unwind(|| check_register_history(
            &writes,
            &[op(2, 25, 26), op(1, 27, 28)]
        ))
        .is_err()
    );
    // A read of a generation never written.
    assert!(std::panic::catch_unwind(|| check_register_history(&writes, &[op(7, 1, 2)])).is_err());
}

// ── Test F: cancellation ─────────────────────────────────────────────────────

/// An ordered MSET of `a` and `b` stopped in the store after writing `a`,
/// holding both keys' replication-order stripes; then an EXEC reading both,
/// which waits for them.
async fn exec_waiting_behind_a_stopped_write(
    shared: &Shared,
    a: &str,
    b: &str,
    txc: &mut Pipe,
    writer: &mut Pipe,
) -> core_engine::store::write_pause::Pause {
    writer.cmd(&["MSET", a, "old", b, "old"]).await;
    let pause = core_engine::store::write_pause::after_mutation_of(&shared.store, a, &[a, b]);
    writer.send(&["MSET", a, "new", b, "new"]).await;
    assert!(
        pause.wait_reached(WITHIN),
        "the MSET never reached its stop"
    );
    queue(txc, &[&["GET", a], &["GET", b]]).await;
    let waits = shared.state.replicas.stripe_waits();
    txc.send(&["EXEC"]).await;
    let deadline = Instant::now() + WITHIN;
    while shared.state.replicas.stripe_waits() == waits {
        assert!(
            Instant::now() < deadline,
            "EXEC never waited for the MSET's stripes"
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    pause
}

async fn gate_reopens(shared: &Shared) {
    let deadline = Instant::now() + WITHIN;
    while shared.state.replicas.unordered.is_closed() {
        assert!(
            Instant::now() < deadline,
            "the unordered gate stayed closed"
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

/// The client disconnects while its EXEC waits for a conflicting writer.
/// The EXEC still runs once the writer finishes (nothing cancels a
/// connection's task on disconnect), its reply cannot be written, and the
/// connection cleans up: its watches are gone, the gate reopens, and other
/// clients carry on.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_client_disconnecting_while_exec_waits_strands_nothing() {
    let shared = Shared::new(false);
    let (a, b, c) = ("6oe:f1:a", "6oe:f1:b", "6oe:f1:watched");
    let _ordered = force_ordered(&shared, "6oe:f1:unrelated").await;
    let (mut txc, mut writer) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    assert_eq!(txc.cmd(&["WATCH", c]).await, ok());
    let pause = exec_waiting_behind_a_stopped_write(&shared, a, b, &mut txc, &mut writer).await;
    drop(txc);
    drop(pause);
    assert_eq!(reply_within(&mut writer, "MSET").await, ok());
    let deadline = Instant::now() + WITHIN;
    while shared.watch_registry.map.lock().await.contains_key(c) {
        assert!(
            Instant::now() < deadline,
            "the disconnected client's WATCH was never released"
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    gate_reopens(&shared).await;
    let mut next = Pipe::tcp(&shared);
    queue(&mut next, &[&["GET", a], &["GET", b]]).await;
    assert_eq!(
        next.cmd(&["EXEC"]).await,
        array(vec![bulk("new"), bulk("new")])
    );
    assert_eq!(writer.cmd(&["MSET", a, "later", b, "later"]).await, ok());
}

/// The connection's task is cancelled (dropped) while its EXEC waits: the
/// stripes it held and the closed gate are released with it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelling_a_waiting_exec_releases_its_guards() {
    let shared = Shared::new(false);
    let (a, b) = ("6oe:f2:a", "6oe:f2:b");
    let _ordered = force_ordered(&shared, "6oe:f2:unrelated").await;
    let ((mut txc, task), mut writer) = (Pipe::tcp_task(&shared), Pipe::tcp(&shared));
    let pause = exec_waiting_behind_a_stopped_write(&shared, a, b, &mut txc, &mut writer).await;
    assert!(
        shared.state.replicas.unordered.is_closed(),
        "the waiting EXEC holds the gate"
    );
    task.abort();
    let _ = task.await;
    drop(pause);
    assert_eq!(reply_within(&mut writer, "MSET").await, ok());
    gate_reopens(&shared).await;
    assert_eq!(writer.cmd(&["MSET", a, "after", b, "after"]).await, ok());
    let mut next = Pipe::tcp(&shared);
    queue(&mut next, &[&["GET", a], &["GET", b]]).await;
    assert_eq!(
        next.cmd(&["EXEC"]).await,
        array(vec![bulk("after"), bulk("after")])
    );
}

// ── Test G: persistence and replication ──────────────────────────────────────

async fn aof_shared(policy: AofSync, name: &str) -> (Shared, Arc<AofWriter>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "recached_6oe_{name}_{}_{}",
        std::process::id(),
        next_conn_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("append.aof");
    let aof = Arc::new(AofWriter::open(path.clone(), policy).await.unwrap());
    (Shared::with_aof(false, Some(Arc::clone(&aof))), aof, path)
}

async fn aof_len(aof: &AofWriter, path: &std::path::Path) -> u64 {
    aof.flush().await.unwrap();
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// A read-only transaction appends nothing to the AOF, under every fsync
/// policy; a mixed one appends its writes and nothing for its reads, and
/// under `always` its reply leaves only once they are on disk.
async fn transactions_and_the_aof(policy: AofSync, name: &str) {
    let (shared, aof, path) = aof_shared(policy, name).await;
    let (a, b) = ("6oe:g:a", "6oe:g:b");
    let mut txc = Pipe::tcp(&shared);
    assert_eq!(txc.cmd(&["MSET", a, "1", b, "1"]).await, ok());
    let before = aof_len(&aof, &path).await;
    queue(
        &mut txc,
        &[&["GET", a], &["MGET", a, b], &["EXISTS", a, b], &["TTL", a]],
    )
    .await;
    let exec = txc.cmd(&["EXEC"]).await;
    assert!(
        matches!(exec, Value::Array(Some(ref r)) if r.len() == 4),
        "{exec:?}"
    );
    assert_eq!(
        aof_len(&aof, &path).await,
        before,
        "a read-only EXEC appended to the AOF"
    );

    queue(&mut txc, &[&["GET", a], &["SET", b, "2"], &["GET", b]]).await;
    assert_eq!(
        txc.cmd(&["EXEC"]).await,
        array(vec![bulk("1"), ok(), bulk("2")])
    );
    if matches!(policy, AofSync::Always) {
        // No early reply: the write is on disk before EXEC answered.
        let on_disk = std::fs::read(&path).unwrap();
        assert!(
            on_disk.len() as u64 > before,
            "EXEC answered before its write reached the AOF"
        );
    }
    let after = aof_len(&aof, &path).await;
    let appended =
        String::from_utf8_lossy(&std::fs::read(&path).unwrap()[before as usize..after as usize])
            .into_owned();
    assert!(
        appended.contains("SET") && appended.contains(b),
        "{appended:?}"
    );
    assert!(
        !appended.contains("GET"),
        "a read reached the AOF: {appended:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn transactions_and_the_aof_no() {
    transactions_and_the_aof(AofSync::No, "no").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn transactions_and_the_aof_everysec() {
    transactions_and_the_aof(AofSync::EverySec, "everysec").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn transactions_and_the_aof_always() {
    transactions_and_the_aof(AofSync::Always, "always").await;
}

/// With a replication stream, a read-only transaction moves the offset by
/// nothing and a mixed one by its writes only.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn transactions_and_the_replication_stream() {
    let shared = Shared::new(false);
    shared.state.replicas.configure_backlog(1 << 20);
    let (a, b) = ("6oe:g:r:a", "6oe:g:r:b");
    let mut txc = Pipe::tcp(&shared);
    txc.cmd(&["MSET", a, "1", b, "1"]).await;
    let before = shared.state.replicas.current_offset();
    assert!(before > 0, "the MSET reached the stream");
    queue(&mut txc, &[&["GET", a], &["MGET", a, b]]).await;
    txc.cmd(&["EXEC"]).await;
    assert_eq!(shared.state.replicas.current_offset(), before);
    queue(
        &mut txc,
        &[&["GET", a], &["SET", b, "2"], &["INCR", a], &["GET", b]],
    )
    .await;
    assert_eq!(
        txc.cmd(&["EXEC"]).await,
        array(vec![bulk("1"), ok(), Value::Integer(2), bulk("2")])
    );
    assert_eq!(
        shared.state.replicas.current_offset(),
        before + 2,
        "one frame per queued write"
    );
}

// ── WebSocket ────────────────────────────────────────────────────────────────

/// Test A on the WebSocket EXEC, which shares `run_transaction_queue` and
/// `lock_transaction` with TCP. A WebSocket client is a push receiver, so
/// every write is ordered.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn websocket_exec_reads_never_straddle_an_mset() {
    let srv = super::spawn_ws_server().await;
    let (a, b) = ("6oe:ws:a", "6oe:ws:b");
    let (mut txc, mut writer) = (
        super::WsClient::connect(srv.tcp_addr).await,
        super::WsClient::connect(srv.tcp_addr).await,
    );
    assert_eq!(writer.cmd(&["MSET", a, "old", b, "old"]).await, ok());
    assert_eq!(txc.cmd(&["MULTI"]).await, ok());
    assert_eq!(txc.cmd(&["GET", a]).await, queued());
    assert_eq!(txc.cmd(&["GET", b]).await, queued());
    let pause = exec_pause::after_command_naming(a);
    txc.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await);
    let waits = srv.state.replicas.stripe_waits();
    writer.send(&["MSET", a, "new", b, "new"]).await;
    let deadline = Instant::now() + WITHIN;
    while srv.state.replicas.stripe_waits() == waits {
        assert!(
            Instant::now() < deadline,
            "the MSET never waited for the transaction"
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    drop(pause);
    let exec = tokio::time::timeout(WITHIN, txc.next_reply())
        .await
        .expect("EXEC reply");
    assert_eq!(exec, array(vec![bulk("old"), bulk("old")]));
    assert_eq!(
        tokio::time::timeout(WITHIN, writer.next_reply())
            .await
            .expect("MSET reply"),
        ok()
    );
}

// ── The transaction's scope covers every key ─────────────────────────────────

/// One sample of every command in the catalog, keys named `k1`, `k2`, ….
/// The test fails when the catalog gains a command this list lacks, so a new
/// command's transaction scope gets reviewed.
const SCOPE_SAMPLES: &[&[&str]] = &[
    &["APPEND", "k1", "v"],
    &["AUTH", "pw"],
    &["BGSAVE"],
    &["COPY", "k1", "k2"],
    &["DBSIZE"],
    &["DECR", "k1"],
    &["DECRBY", "k1", "2"],
    &["DEL", "k1", "k2"],
    &["DISCARD"],
    &["ECHO", "x"],
    &["EXEC"],
    &["EXISTS", "k1", "k2"],
    &["EXPIRE", "k1", "10"],
    &["EXPIREAT", "k1", "10"],
    &["EXPIRETIME", "k1"],
    &["FLUSHDB"],
    &["GET", "k1"],
    &["GETDEL", "k1"],
    &["GETEX", "k1"],
    &["GETEX", "k1", "PX", "10"],
    &["GETRANGE", "k1", "0", "1"],
    &["GETSET", "k1", "v"],
    &["HDEL", "k1", "f"],
    &["HELLO"],
    &["HEXISTS", "k1", "f"],
    &["HGET", "k1", "f"],
    &["HGETALL", "k1"],
    &["HINCRBY", "k1", "f", "1"],
    &["HINCRBYFLOAT", "k1", "f", "1.5"],
    &["HKEYS", "k1"],
    &["HLEN", "k1"],
    &["HMGET", "k1", "f", "g"],
    &["HMSET", "k1", "f", "v"],
    &["HSCAN", "k1", "0"],
    &["HSET", "k1", "f", "v"],
    &["HSETNX", "k1", "f", "v"],
    &["HSTRLEN", "k1", "f"],
    &["HVALS", "k1"],
    &["INCR", "k1"],
    &["INCRBY", "k1", "2"],
    &["INFO"],
    &["KEYS", "*"],
    &["LASTSAVE"],
    &["LINDEX", "k1", "0"],
    &["LLEN", "k1"],
    &["LPOP", "k1"],
    &["LPUSH", "k1", "v"],
    &["LPUSHX", "k1", "v"],
    &["LRANGE", "k1", "0", "1"],
    &["LREM", "k1", "0", "v"],
    &["LSET", "k1", "0", "v"],
    &["LTRIM", "k1", "0", "1"],
    &["MGET", "k1", "k2"],
    &["MSET", "k1", "v", "k2", "v"],
    &["MULTI"],
    &["PERSIST", "k1"],
    &["PEXPIRE", "k1", "10"],
    &["PEXPIREAT", "k1", "10"],
    &["PEXPIRETIME", "k1"],
    &["PING"],
    &["PSETEX", "k1", "10", "v"],
    &["PSUBSCRIBE", "c*"],
    &["PTTL", "k1"],
    &["PUBLISH", "c", "m"],
    &["PUNSUBSCRIBE"],
    &["RENAME", "k1", "k2"],
    &["RENAMENX", "k1", "k2"],
    &["REPLICAOF", "NO", "ONE"],
    &["RPOP", "k1"],
    &["RPUSH", "k1", "v"],
    &["RPUSHX", "k1", "v"],
    &["SADD", "k1", "m"],
    &["SAVE"],
    &["SCAN", "0"],
    &["SCARD", "k1"],
    &["SDIFF", "k1", "k2"],
    &["SDIFFSTORE", "k3", "k1", "k2"],
    &["SET", "k1", "v"],
    &["SETEX", "k1", "10", "v"],
    &["SETNX", "k1", "v"],
    &["SINTER", "k1", "k2"],
    &["SINTERSTORE", "k3", "k1", "k2"],
    &["SISMEMBER", "k1", "m"],
    &["SMEMBERS", "k1"],
    &["SMISMEMBER", "k1", "m", "n"],
    &["SMOVE", "k1", "k2", "m"],
    &["SPOP", "k1"],
    &["SRANDMEMBER", "k1"],
    &["SREM", "k1", "m"],
    &["SSCAN", "k1", "0"],
    &["STRLEN", "k1"],
    &["SUBSCRIBE", "c"],
    &["SUNION", "k1", "k2"],
    &["SUNIONSTORE", "k3", "k1", "k2"],
    &["TTL", "k1"],
    &["TYPE", "k1"],
    &["UNLINK", "k1", "k2"],
    &["UNSUBSCRIBE"],
    &["UNWATCH"],
    &["WATCH", "k1"],
    &["ZADD", "k1", "1", "m"],
    &["ZCARD", "k1"],
    &["ZCOUNT", "k1", "0", "1"],
    &["ZINCRBY", "k1", "1", "m"],
    &["ZMSCORE", "k1", "m", "n"],
    &["ZPOPMAX", "k1"],
    &["ZPOPMIN", "k1"],
    &["ZRANGE", "k1", "0", "1"],
    &["ZRANGEBYSCORE", "k1", "0", "1"],
    &["ZRANK", "k1", "m"],
    &["ZREM", "k1", "m"],
    &["ZREVRANGE", "k1", "0", "1"],
    &["ZREVRANGEBYSCORE", "k1", "1", "0"],
    &["ZREVRANK", "k1", "m"],
    &["ZSCAN", "k1", "0"],
    &["ZSCORE", "k1", "m"],
    &["QUIT"],
    &["CLIENT", "LIST"],
    &["CONFIG", "GET", "x"],
    &["COMMAND"],
    &["CLUSTER", "INFO"],
    &["MEMORY", "USAGE", "k1"],
    &["MODULE", "LIST"],
    &["PUBSUB", "CHANNELS"],
    &["ESET", "k1", "v"],
    &["EADD", "k1", "m"],
    &["JSET", "k1", "$", "1"],
    &["JGET", "k1"],
    &["JMERGE", "k1", "{}"],
    &["RLSET", "k1", "10", "1000"],
    &["RLCHECK", "k1"],
    &["SYNC"],
    &["DEDUP", "c", "1", "SET", "k1", "v"],
    &["QSUB", "k*"],
    &["QUNSUB"],
];

#[test]
fn the_transaction_scope_covers_every_key_a_command_names() {
    use crate::replication::transaction_stripes;
    use crate::sync_scopes::{CommandScope, command_scope};
    let sampled: std::collections::HashSet<String> = SCOPE_SAMPLES
        .iter()
        .map(|s| s[0].to_ascii_lowercase())
        .collect();
    for spec in core_engine::catalog::CATALOG {
        assert!(
            sampled.contains(spec.name),
            "no scope sample for `{}`: add one",
            spec.name
        );
    }
    let mut checked = 0;
    for sample in SCOPE_SAMPLES {
        let value = Value::Array(Some(sample.iter().map(|a| bulk(a)).collect()));
        let Ok(cmd) = Command::from_value(value) else {
            panic!("sample {sample:?} does not parse");
        };
        // Refused inside MULTI (the connection loop answers them before
        // queuing): they never run under a transaction's guards.
        if matches!(
            cmd,
            Command::Watch(_)
                | Command::Unwatch(_)
                | Command::Subscribe(_)
                | Command::Unsubscribe(_)
                | Command::PSubscribe(_)
                | Command::PUnsubscribe(_)
                | Command::QSub(_)
                | Command::QUnsub(_)
                | Command::Multi
                | Command::Exec
                | Command::Discard
        ) {
            continue;
        }
        let stripes = transaction_stripes(std::slice::from_ref(&cmd), std::iter::empty::<&str>());
        let named: Vec<String> = match command_scope(&cmd) {
            CommandScope::Keys(keys) => keys.into_iter().map(|(k, _)| k).collect(),
            CommandScope::KeyLess => Vec::new(),
            CommandScope::Admin => {
                // Keyspace-wide commands must take every stripe.
                if matches!(sample[0], "KEYS" | "SCAN" | "DBSIZE" | "FLUSHDB") {
                    assert!(stripes.is_none(), "{sample:?} must take every stripe");
                }
                Vec::new()
            }
        };
        if let Some(stripes) = stripes {
            for key in named {
                assert!(
                    stripes.contains(&ReplHub::stripe_for(&key)),
                    "{sample:?}: the transaction scope misses key {key}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "only {checked} keys checked");
}

// ── Review follow-ups (round 1) ──────────────────────────────────────────────

/// A write to the *second* read key alone, while EXEC is stopped after the
/// first: `[old, new]` is a state (b changed alone), but not one this
/// transaction may answer once it has read `a`, since the write did not
/// precede the transaction. It must wait for EXEC (review probe P6: a scope
/// of the first command's read keys only passed every other test).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lone_write_to_a_later_read_key_waits_for_the_transaction() {
    let shared = Shared::new(false);
    let (a, b) = ("6oe:p6:a", "6oe:p6:b");
    let (mut txc, mut writer) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    writer.cmd(&["MSET", a, "old", b, "old"]).await;
    queue(&mut txc, &[&["GET", a], &["GET", b]]).await;
    let pause = exec_pause::after_command_naming(a);
    txc.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await);
    let waits = shared.state.replicas.stripe_waits();
    writer.send(&["SET", b, "new"]).await;
    let during = reply_or_waiting(&mut writer, &shared, waits).await;
    drop(pause);
    let exec = reply_within(&mut txc, "EXEC").await;
    if during.is_none() {
        assert_eq!(reply_within(&mut writer, "SET").await, ok());
    }
    assert!(
        during.is_none(),
        "SET b answered while EXEC was between its reads"
    );
    assert_eq!(exec, array(vec![bulk("old"), bulk("old")]));
}

/// A client that sends a read-only transaction and never reads the reply
/// holds nothing once its commands have run: the guards are released before
/// the reply is written (review, High). The reply here is 1 MiB, far past
/// every buffer between the server and the stalled client.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_client_that_never_reads_its_exec_reply_blocks_no_writer() {
    let shared = Shared::new(false);
    let k = "6oe:stall:big";
    let (mut stalled, mut writer) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    assert_eq!(writer.cmd(&["SET", k, &"x".repeat(1 << 20)]).await, ok());
    queue(&mut stalled, &[&["GET", k], &["GET", k]]).await;
    stalled.send(&["EXEC"]).await;
    // Never read again. The writer's SET of the read key must be answered.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        tokio::time::timeout(WITHIN, writer.cmd(&["SET", k, "small"]))
            .await
            .expect("a SET of a key a stalled transaction read"),
        ok()
    );
    assert!(
        !shared.state.replicas.unordered.is_closed(),
        "the gate reopened"
    );
    drop(stalled);
}

/// Queued writes run at the transaction's instant too: `SET k v PX 1000`
/// after the transaction was stopped for 150 ms still counts from the
/// instant, so `PTTL k` in the same transaction answers 1000 (review: running
/// queued writes on the wall clock passed every other test).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn queued_writes_count_relative_deadlines_from_the_instant() {
    let shared = Shared::new(false);
    let (x, k) = ("6oe:wclock:x", "6oe:wclock:k");
    let mut txc = Pipe::tcp(&shared);
    queue(
        &mut txc,
        &[&["GET", x], &["SET", k, "v", "PX", "1000"], &["PTTL", k]],
    )
    .await;
    let pause = exec_pause::after_command_naming(x);
    txc.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await);
    tokio::time::sleep(Duration::from_millis(150)).await;
    drop(pause);
    let exec = reply_within(&mut txc, "EXEC").await;
    assert_eq!(exec, array(vec![nil(), ok(), Value::Integer(1000)]));
}

/// The instant is read once EXEC holds its guards, not when it starts to wait
/// for them: a key that expires while EXEC waits behind a conflicting writer
/// is gone to the transaction, as it would be in Redis, which runs the
/// transaction after that writer (review: reading the clock first passed
/// every other test).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_instant_is_taken_once_the_guards_are_held() {
    let shared = Shared::new(false);
    let (a, b, k) = ("6oe:after:a", "6oe:after:b", "6oe:after:k");
    let _ordered = force_ordered(&shared, "6oe:after:unrelated").await;
    let (mut txc, mut writer) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    assert_eq!(txc.cmd(&["SET", k, "v", "PX", "100"]).await, ok());
    writer.cmd(&["MSET", a, "old", b, "old"]).await;
    let pause = core_engine::store::write_pause::after_mutation_of(&shared.store, a, &[a, b]);
    writer.send(&["MSET", a, "new", b, "new"]).await;
    assert!(pause.wait_reached(WITHIN));
    queue(&mut txc, &[&["GET", a], &["GET", k]]).await;
    let waits = shared.state.replicas.stripe_waits();
    txc.send(&["EXEC"]).await;
    let deadline = Instant::now() + WITHIN;
    while shared.state.replicas.stripe_waits() == waits {
        assert!(Instant::now() < deadline, "EXEC never waited");
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    // k expires while EXEC waits.
    let expired_by = Instant::now() + Duration::from_millis(150);
    tokio::time::sleep(expired_by.saturating_duration_since(Instant::now())).await;
    drop(pause);
    assert_eq!(reply_within(&mut writer, "MSET").await, ok());
    assert_eq!(
        reply_within(&mut txc, "EXEC").await,
        array(vec![bulk("new"), nil()])
    );
}

/// What the AOF records for a queued `SET k v PX 40` is the deadline the
/// store set, counted from the transaction's instant, even when the
/// transaction ran past it before its write was logged (review, Medium: the
/// deadline was read back on the wall clock, found expired, and recomputed
/// from it).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_aof_records_a_queued_writes_deadline_from_the_instant() {
    let (shared, aof, path) = aof_shared(AofSync::No, "deadline").await;
    let (x, k) = ("6oe:aofdl:x", "6oe:aofdl:k");
    let mut txc = Pipe::tcp(&shared);
    let unix_ms = || core_engine::store::clock_ms();
    queue(&mut txc, &[&["GET", x], &["SET", k, "v", "PX", "40"]]).await;
    let pause = exec_pause::after_command_naming(x);
    let sent = unix_ms();
    txc.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await);
    let reached = unix_ms();
    tokio::time::sleep(Duration::from_millis(150)).await;
    drop(pause);
    assert_eq!(
        reply_within(&mut txc, "EXEC").await,
        array(vec![nil(), ok()])
    );
    aof.flush().await.unwrap();
    let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
    let at = text
        .find(k)
        .and_then(|i| {
            let rest = &text[i..];
            let p = rest.to_ascii_uppercase().find("PXAT")?;
            rest[p..]
                .split("\r\n")
                .filter_map(|t| t.parse::<u64>().ok())
                .next()
        })
        .unwrap_or_else(|| panic!("no PXAT for {k} in the AOF: {text:?}"));
    assert!(
        at >= sent + 40 && at <= reached + 41,
        "AOF deadline {at}: the instant was between {sent} and {reached}, so {}..={}",
        sent + 40,
        reached + 41
    );
}

// ── Review follow-ups (round 2) ──────────────────────────────────────────────

/// The WebSocket EXEC releases its guards before sending its reply too: a
/// socket that sends a transaction reading a large value and never reads
/// again does not hold back a writer of that key (review round 2, M14). The
/// reply is 16 MiB, past every socket buffer between the two.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_websocket_client_that_never_reads_its_exec_reply_blocks_no_writer() {
    let srv = super::spawn_ws_server().await;
    let k = "6oe:wsstall:big";
    let (mut stalled, mut writer) = (
        super::WsClient::connect(srv.tcp_addr).await,
        super::WsClient::connect(srv.tcp_addr).await,
    );
    assert_eq!(writer.cmd(&["SET", k, &"x".repeat(4 << 20)]).await, ok());
    assert_eq!(stalled.cmd(&["MULTI"]).await, ok());
    for _ in 0..4 {
        assert_eq!(stalled.cmd(&["GET", k]).await, queued());
    }
    stalled.send(&["EXEC"]).await;
    // `stalled` is never read again.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        tokio::time::timeout(WITHIN, writer.cmd(&["SET", k, "small"]))
            .await
            .expect("a SET of a key a stalled WebSocket transaction read"),
        ok()
    );
    drop(stalled);
}

/// A queued `EADD` decides which members it claims at the transaction's
/// instant, like the `EADD` itself (review round 2, M15): a member already
/// in a set that is live at the instant, but whose deadline passes before
/// the EADD runs, is not this connection's ephemeral member.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_queued_eadd_claims_members_at_the_instant() {
    let shared = Shared::new(false);
    let (x, s) = ("6oe:eadd:x", "6oe:eadd:s");
    let mut txc = Pipe::tcp(&shared);
    assert_eq!(txc.cmd(&["SADD", s, "m"]).await, Value::Integer(1));
    assert_eq!(txc.cmd(&["PEXPIRE", s, "100"]).await, Value::Integer(1));
    queue(&mut txc, &[&["GET", x], &["EADD", s, "m"]]).await;
    let pause = exec_pause::after_command_naming(x);
    txc.send(&["EXEC"]).await;
    assert!(pause.reached(WITHIN).await);
    tokio::time::sleep(Duration::from_millis(150)).await;
    drop(pause);
    let exec = reply_within(&mut txc, "EXEC").await;
    assert_eq!(
        exec,
        array(vec![nil(), Value::Integer(0)]),
        "m was already in s at the instant"
    );
    assert!(
        !shared.state.is_ephemeral_member(s, "m"),
        "a member present at the instant was claimed as this connection's"
    );
}
