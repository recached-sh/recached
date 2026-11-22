//! 6O-D: MSET's atomicity as ordinary readers see it.
//!
//! Redis runs `MSET` as one step: no reader sees some of its keys written and
//! others not. These tests stop an MSET right after its first mutation
//! ([`write_pause`]), with its write-order stripes held, and read the
//! keyspace from another thread while it is stopped:
//!
//! - an `MGET` of its keys must not answer a mix of old and new values;
//! - two reads that complete one after the other must not see the MSET's
//!   first key new and a later key old — no single point in time had that
//!   state. Each such history is checked with [`linearization`], a brute-force
//!   linearizability check over the recorded invocation and response order.
//!
//! The reader either answers during the stop (which is correct only with the
//! old values) or waits for the write, which the hook counts; the test then
//! lets the write finish and checks what the reader answered.

use super::*;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

const WITHIN: Duration = Duration::from_secs(20);

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

fn mset(pairs: &[(&str, &str)]) -> Command {
    Command::MSet(
        pairs
            .iter()
            .map(|(key, val)| (key.to_string(), val.as_bytes().to_vec()))
            .collect(),
    )
}

fn mget(keys: &[&str]) -> Command {
    Command::MGet(keys.iter().map(|key| key.to_string()).collect())
}

// ── Histories and a linearizability check ─────────────────────────────────

/// One completed operation of a history over string registers: the values it
/// wrote (atomically, all at once) and the values it observed (`None`: the
/// key was absent). `invoke` and `respond` are ticks of one shared clock, so
/// `a.respond < b.invoke` means `a` finished before `b` began.
#[derive(Clone, Debug)]
pub(super) struct Op {
    pub(super) name: String,
    pub(super) invoke: u64,
    pub(super) respond: u64,
    pub(super) writes: Vec<(String, Option<String>)>,
    pub(super) reads: Vec<(String, Option<String>)>,
}

type State = BTreeMap<String, Option<String>>;

/// Search positions already known to lead nowhere: (operations done, state).
type Failed = HashSet<(u64, Vec<(String, Option<String>)>)>;

/// A sequential order of `ops` that respects their real-time order (an
/// operation that finished before another began comes first) and in which
/// every read observes the state the writes before it produced — or `None`
/// when there is none, i.e. the history is not linearizable.
///
/// Exhaustive with memoization; meant for the few-operation histories these
/// tests build.
pub(super) fn linearization(initial: &State, ops: &[Op]) -> Option<Vec<usize>> {
    assert!(ops.len() <= 64, "history too long for the checker");
    fn search(
        ops: &[Op],
        done: u64,
        state: &State,
        order: &mut Vec<usize>,
        failed: &mut Failed,
    ) -> bool {
        if done.count_ones() as usize == ops.len() {
            return true;
        }
        let memo = (done, state.clone().into_iter().collect::<Vec<_>>());
        if failed.contains(&memo) {
            return false;
        }
        for (i, op) in ops.iter().enumerate() {
            if done & (1 << i) != 0 {
                continue;
            }
            // Minimal in real time: nothing still pending finished before it began.
            let blocked = ops
                .iter()
                .enumerate()
                .any(|(j, other)| j != i && done & (1 << j) == 0 && other.respond < op.invoke);
            if blocked {
                continue;
            }
            let observed_ok = op
                .reads
                .iter()
                .all(|(key, val)| state.get(key).cloned().flatten() == *val);
            if !observed_ok {
                continue;
            }
            let mut next = state.clone();
            for (key, val) in &op.writes {
                next.insert(key.clone(), val.clone());
            }
            order.push(i);
            if search(ops, done | (1 << i), &next, order, failed) {
                return true;
            }
            order.pop();
        }
        failed.insert(memo);
        false
    }
    let mut order = Vec::new();
    let mut failed = HashSet::new();
    search(ops, 0, initial, &mut order, &mut failed).then_some(order)
}

/// Records operations against one shared clock.
#[derive(Default)]
pub(super) struct Recorder {
    clock: AtomicU64,
    ops: Mutex<Vec<Op>>,
}

impl Recorder {
    /// Run `op`, recording its invocation and response ticks around it, with
    /// the values it writes and the values `op` reports observing.
    pub(super) fn run(
        &self,
        name: &str,
        writes: &[(&str, Option<&str>)],
        op: impl FnOnce() -> Vec<(String, Option<String>)>,
    ) {
        let invoke = self.clock.fetch_add(1, Ordering::SeqCst);
        let reads = op();
        let respond = self.clock.fetch_add(1, Ordering::SeqCst);
        self.ops
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(Op {
                name: name.to_string(),
                invoke,
                respond,
                writes: writes
                    .iter()
                    .map(|(key, val)| (key.to_string(), val.map(str::to_string)))
                    .collect(),
                reads,
            });
    }

    pub(super) fn history(&self) -> Vec<Op> {
        let mut ops = self
            .ops
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        ops.sort_by_key(|op| op.invoke);
        ops
    }
}

pub(super) fn describe(ops: &[Op]) -> String {
    ops.iter()
        .map(|op| {
            format!(
                "  [{:>2}..{:>2}] {}  writes {:?}  observed {:?}",
                op.invoke, op.respond, op.name, op.writes, op.reads
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn state(pairs: &[(&str, Option<&str>)]) -> State {
    pairs
        .iter()
        .map(|(key, val)| (key.to_string(), val.map(str::to_string)))
        .collect()
}

#[test]
fn the_checker_accepts_and_refuses_what_it_should() {
    let initial = state(&[("a", Some("old")), ("b", Some("old"))]);
    let op = |name: &str, invoke, respond, writes: &[(&str, &str)], reads: &[(&str, &str)]| Op {
        name: name.to_string(),
        invoke,
        respond,
        writes: writes
            .iter()
            .map(|(k, v)| (k.to_string(), Some(v.to_string())))
            .collect(),
        reads: reads
            .iter()
            .map(|(k, v)| (k.to_string(), Some(v.to_string())))
            .collect(),
    };
    let w = |invoke, respond| op("MSET", invoke, respond, &[("a", "new"), ("b", "new")], &[]);
    // Reads overlapping the write, in sequence: new then new, old then new,
    // old then old are all fine.
    for (first, second) in [("new", "new"), ("old", "new"), ("old", "old")] {
        let ops = [
            w(0, 9),
            op("GET a", 1, 2, &[], &[("a", first)]),
            op("GET b", 3, 4, &[], &[("b", second)]),
        ];
        assert!(linearization(&initial, &ops).is_some(), "{first}/{second}");
    }
    // New, then old: no point in time had a new and b old.
    let ops = [
        w(0, 9),
        op("GET a", 1, 2, &[], &[("a", "new")]),
        op("GET b", 3, 4, &[], &[("b", "old")]),
    ];
    assert!(linearization(&initial, &ops).is_none());
    // The same two reads *overlapping* each other are fine: GET b may take
    // effect first.
    let ops = [
        w(0, 9),
        op("GET a", 1, 4, &[], &[("a", "new")]),
        op("GET b", 2, 3, &[], &[("b", "old")]),
    ];
    assert!(linearization(&initial, &ops).is_some());
    // One MGET observing a mix is never fine.
    let ops = [
        w(0, 9),
        op("MGET a b", 1, 2, &[], &[("a", "new"), ("b", "old")]),
    ];
    assert!(linearization(&initial, &ops).is_none());
    // A read after the write completed must see it.
    let ops = [w(0, 1), op("GET b", 2, 3, &[], &[("b", "old")])];
    assert!(linearization(&initial, &ops).is_none());
}

// ── Deterministic interleavings ───────────────────────────────────────────

/// What the reader thread answered, and whether it did so while the MSET was
/// still stopped.
struct Observed<T> {
    answer: T,
    during_stop: bool,
    reader_waited: bool,
}

/// Stop `write` (an MSET) right after it mutates `stop_after`, run `read` on
/// another thread, and let the write go once the reader has either answered
/// or is waiting for it.
fn read_during_stopped_mset<T: Send + Sync>(
    store: &KeyValueStore,
    write: Command,
    stop_after: &str,
    readers_of: &[&str],
    read: impl FnOnce() -> T + Send,
) -> Observed<T> {
    let pause = write_pause::after_mutation_of(stop_after, readers_of);
    let answer: OnceLock<T> = OnceLock::new();
    let mut during_stop = false;
    let mut reader_waited = false;
    std::thread::scope(|scope| {
        let writer = scope.spawn(|| store.execute(write));
        assert!(
            pause.wait_reached(WITHIN),
            "the MSET never reached its stop"
        );
        let answer = &answer;
        let reader = scope.spawn(move || {
            let _ = answer.set(read());
        });
        reader_waited = pause.wait_reader_blocked_or(WITHIN, || answer.get().is_some());
        during_stop = answer.get().is_some();
        assert!(
            reader_waited || during_stop,
            "the reader neither answered nor waited for the write"
        );
        drop(pause);
        assert_eq!(writer.join().unwrap(), Value::SimpleString("OK".into()));
        reader.join().unwrap();
    });
    Observed {
        answer: answer.into_inner().expect("the reader answered"),
        during_stop,
        reader_waited,
    }
}

/// Test A: an MGET of an MSET's keys, while the MSET has written its first
/// key and not the others, must answer all old (it takes effect first) or,
/// having waited, all new — never a mix.
#[test]
#[ignore = "6O-D0 reproduction of the pre-existing MSET reader-atomicity defect; fixed in 6O-D1"]
fn an_mget_never_sees_part_of_an_mset() {
    let store = KeyValueStore::new();
    let keys = ["atom:mget:a", "atom:mget:b", "atom:mget:c"];
    store.execute(mset(&[
        (keys[0], "old"),
        (keys[1], "old"),
        (keys[2], "old"),
    ]));
    let seen = read_during_stopped_mset(
        &store,
        mset(&[(keys[0], "new"), (keys[1], "new"), (keys[2], "new")]),
        keys[0],
        &keys,
        || store.execute(mget(&keys)),
    );
    let old = Value::Array(Some(vec![bulk("old"); 3]));
    let new = Value::Array(Some(vec![bulk("new"); 3]));
    assert!(
        seen.answer == old || seen.answer == new,
        "MGET saw part of an MSET: {:?} (answered while the MSET was stopped: {}, waited: {})",
        seen.answer,
        seen.during_stop,
        seen.reader_waited
    );
    if seen.during_stop {
        assert_eq!(
            seen.answer, old,
            "MGET answered new before the MSET finished"
        );
    }
    assert_eq!(store.execute(mget(&keys)), new);
}

/// Test A, borrowed: the server answers MGET through `execute_ref_into`, from
/// the read buffer; it must be just as atomic.
#[test]
#[ignore = "6O-D0 reproduction of the pre-existing MSET reader-atomicity defect; fixed in 6O-D1"]
fn a_borrowed_mget_never_sees_part_of_an_mset() {
    let store = KeyValueStore::new();
    let keys = ["atom:ref:a", "atom:ref:b"];
    store.execute(mset(&[(keys[0], "old"), (keys[1], "old")]));
    let seen = read_during_stopped_mset(
        &store,
        mset(&[(keys[0], "new"), (keys[1], "new")]),
        keys[0],
        &keys,
        || {
            let mut out = Vec::new();
            let cmd = CommandRef::MGet(keys.iter().copied().collect());
            store
                .execute_ref_into(&cmd, &mut out)
                .expect("a read runs borrowed");
            out
        },
    );
    let old = b"*2\r\n$3\r\nold\r\n$3\r\nold\r\n".to_vec();
    let new = b"*2\r\n$3\r\nnew\r\n$3\r\nnew\r\n".to_vec();
    assert!(
        seen.answer == old || seen.answer == new,
        "borrowed MGET saw part of an MSET: {:?}",
        String::from_utf8_lossy(&seen.answer)
    );
}

/// How a single-key read command reports one key of the model. Values:
/// `a` is "old" then "new"; `b` is a hash ("hash") then the string "new";
/// `c` is absent (`None`) then "new".
struct SingleRead {
    name: &'static str,
    key: usize,
    cmd: fn(&str) -> Command,
    model: fn(Value) -> Option<String>,
}

fn single_reads() -> Vec<SingleRead> {
    fn string_or_hash(value: Value) -> Option<String> {
        match value {
            Value::Error(error) if error.starts_with("WRONGTYPE") => Some("hash".into()),
            Value::BulkString(Some(v)) if v == b"new" => Some("new".into()),
            Value::Integer(3) => Some("new".into()),
            other => panic!("unexpected answer {other:?}"),
        }
    }
    fn absent_or_new(value: Value) -> Option<String> {
        match value {
            Value::BulkString(None) | Value::Integer(0) | Value::Integer(-2) => None,
            Value::BulkString(Some(v)) if v == b"new" => Some("new".into()),
            Value::Integer(1) | Value::Integer(-1) => Some("new".into()),
            Value::SimpleString(s) if s == "none" => None,
            Value::SimpleString(s) if s == "string" => Some("new".into()),
            other => panic!("unexpected answer {other:?}"),
        }
    }
    vec![
        SingleRead {
            name: "GET b",
            key: 1,
            cmd: |k| Command::Get(k.into()),
            model: string_or_hash,
        },
        SingleRead {
            name: "MGET b",
            key: 1,
            cmd: |k| Command::MGet(vec![k.into()]),
            model: |v| match v {
                // MGET answers nil for a key of another type.
                Value::Array(Some(mut items)) => match items.pop() {
                    Some(Value::BulkString(None)) => Some("hash".into()),
                    Some(Value::BulkString(Some(v))) if v == b"new" => Some("new".into()),
                    other => panic!("unexpected item {other:?}"),
                },
                other => panic!("unexpected answer {other:?}"),
            },
        },
        SingleRead {
            name: "TYPE b",
            key: 1,
            cmd: |k| Command::Type(k.into()),
            model: |v| match v {
                Value::SimpleString(s) if s == "hash" => Some("hash".into()),
                Value::SimpleString(s) if s == "string" => Some("new".into()),
                other => panic!("unexpected answer {other:?}"),
            },
        },
        SingleRead {
            name: "STRLEN b",
            key: 1,
            cmd: |k| Command::Strlen(k.into()),
            model: string_or_hash,
        },
        SingleRead {
            name: "GETRANGE b",
            key: 1,
            cmd: |k| Command::GetRange(k.into(), 0, -1),
            model: string_or_hash,
        },
        SingleRead {
            name: "HLEN b",
            key: 1,
            cmd: |k| Command::HLen(k.into()),
            model: |v| match v {
                Value::Integer(1) => Some("hash".into()),
                Value::Error(error) if error.starts_with("WRONGTYPE") => Some("new".into()),
                other => panic!("unexpected answer {other:?}"),
            },
        },
        SingleRead {
            name: "GET c",
            key: 2,
            cmd: |k| Command::Get(k.into()),
            model: absent_or_new,
        },
        SingleRead {
            name: "EXISTS c",
            key: 2,
            cmd: |k| Command::Exists(vec![k.into()]),
            model: absent_or_new,
        },
        SingleRead {
            name: "TYPE c",
            key: 2,
            cmd: |k| Command::Type(k.into()),
            model: absent_or_new,
        },
        SingleRead {
            name: "TTL c",
            key: 2,
            cmd: |k| Command::Ttl(k.into()),
            model: absent_or_new,
        },
        SingleRead {
            name: "PTTL c",
            key: 2,
            cmd: |k| Command::PTtl(k.into()),
            model: absent_or_new,
        },
    ]
}

/// Test B: `GET a`, then — after it has answered — a read of another of the
/// MSET's keys, with the MSET stopped after writing `a`. Each completed pair
/// must be linearizable with the MSET: once a read has seen `a` new, a read
/// that starts afterwards cannot see a key the same MSET wrote as it was.
///
/// This is not a demand that separate reads share one snapshot: two reads
/// that overlap may take effect in either order, and both may see the old
/// state, or both the new one. Only "new, then old" is refused, and only
/// when the second read began after the first had answered.
#[test]
#[ignore = "6O-D0 reproduction of the pre-existing MSET reader-atomicity defect; fixed in 6O-D1"]
fn sequential_single_key_reads_are_linearizable_with_an_mset() {
    let mut refused = Vec::new();
    for (round, read) in single_reads().into_iter().enumerate() {
        let store = KeyValueStore::new();
        let names = [
            format!("atom:seq{round}:a"),
            format!("atom:seq{round}:b"),
            format!("atom:seq{round}:c"),
        ];
        let keys = [names[0].as_str(), names[1].as_str(), names[2].as_str()];
        store.execute(Command::Set(
            keys[0].into(),
            b"old".to_vec(),
            SetOptions::default(),
        ));
        store.execute(Command::HSet(
            keys[1].into(),
            vec![("f".into(), b"v".to_vec())],
        ));
        let recorder = Recorder::default();
        let write = [
            (keys[0], Some("new")),
            (keys[1], Some("new")),
            (keys[2], Some("new")),
        ];
        let model = |key: usize| ["a", "b", "c"][key].to_string();
        let pause = write_pause::after_mutation_of(keys[0], &keys);
        let reader_done = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let (store, recorder) = (&store, &recorder);
            let writer = scope.spawn(move || {
                recorder.run(
                    "MSET a b c",
                    &[("a", Some("new")), ("b", Some("new")), ("c", Some("new"))],
                    || {
                        store.execute(mset(&write.map(|(k, v)| (k, v.unwrap()))));
                        Vec::new()
                    },
                );
            });
            assert!(
                pause.wait_reached(WITHIN),
                "the MSET never reached its stop"
            );
            let reader_done = &reader_done;
            let read = &read;
            let reader = scope.spawn(move || {
                recorder.run("GET a", &[], || {
                    vec![(
                        model(0),
                        match store.execute(Command::Get(keys[0].into())) {
                            Value::BulkString(Some(v)) => Some(String::from_utf8(v).unwrap()),
                            other => panic!("GET a answered {other:?}"),
                        },
                    )]
                });
                recorder.run(read.name, &[], || {
                    vec![(
                        model(read.key),
                        (read.model)(store.execute((read.cmd)(keys[read.key]))),
                    )]
                });
                reader_done.store(true, Ordering::SeqCst);
            });
            pause.wait_reader_blocked_or(WITHIN, || reader_done.load(Ordering::SeqCst));
            drop(pause);
            writer.join().unwrap();
            reader.join().unwrap();
        });
        let history = recorder.history();
        let initial = state(&[("a", Some("old")), ("b", Some("hash")), ("c", None)]);
        if linearization(&initial, &history).is_none() {
            refused.push(format!(
                "GET a, then {}:\n{}",
                read.name,
                describe(&history)
            ));
        }
    }
    assert!(
        refused.is_empty(),
        "{} of {} histories are not linearizable:\n{}",
        refused.len(),
        single_reads().len(),
        refused.join("\n")
    );
}

/// The reproduction's control: with no MSET in progress, the same reads see
/// the same state, so the refusal above is the stop, not the reads.
#[test]
fn the_reads_agree_with_the_model_without_a_concurrent_write() {
    for read in single_reads() {
        let store = KeyValueStore::new();
        store.execute(Command::Set(
            "ctl:a".into(),
            b"old".to_vec(),
            SetOptions::default(),
        ));
        store.execute(Command::HSet(
            "ctl:b".into(),
            vec![("f".into(), b"v".to_vec())],
        ));
        let key = ["ctl:a", "ctl:b", "ctl:c"][read.key];
        let before = (read.model)(store.execute((read.cmd)(key)));
        store.execute(mset(&[
            ("ctl:a", "new"),
            ("ctl:b", "new"),
            ("ctl:c", "new"),
        ]));
        let after = (read.model)(store.execute((read.cmd)(key)));
        let expected_before = [Some("old"), Some("hash"), None][read.key].map(str::to_string);
        assert_eq!(before, expected_before, "{} before", read.name);
        assert_eq!(after.as_deref(), Some("new"), "{} after", read.name);
    }
}
