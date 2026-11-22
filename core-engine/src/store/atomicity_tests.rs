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
                // Odd rounds read through the by-reference entry point.
                let cmd = (read.cmd)(keys[read.key]);
                recorder.run(read.name, &[], || {
                    let reply = if round % 2 == 1 {
                        store.execute_reporting_ref(&cmd).0
                    } else {
                        store.execute(cmd.clone())
                    };
                    vec![(model(read.key), (read.model)(reply))]
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

/// The stripes of `keys` are distinct, so a test about several keys is not
/// accidentally about one stripe (a mutant that marks only the first stripe
/// of a multi-key write would then pass).
fn distinct_stripes(keys: &[&str]) {
    let mut stripes: Vec<usize> = keys.iter().map(|key| write_order_stripe(key)).collect();
    stripes.sort_unstable();
    stripes.dedup();
    assert_eq!(
        stripes.len(),
        keys.len(),
        "test keys {keys:?} share a stripe"
    );
}

#[test]
fn the_deterministic_tests_use_keys_on_distinct_stripes() {
    distinct_stripes(&["atom:mget:a", "atom:mget:b", "atom:mget:c"]);
    distinct_stripes(&["atom:ref:a", "atom:ref:b"]);
    distinct_stripes(&["atom:val:a", "atom:val:b"]);
    for round in 0..single_reads().len() {
        let names = [
            format!("atom:seq{round}:a"),
            format!("atom:seq{round}:b"),
            format!("atom:seq{round}:c"),
        ];
        distinct_stripes(&[&names[0], &names[1], &names[2]]);
    }
}

/// A multi-key read that loaded its stripes' words *before* an MSET began,
/// and reads while the MSET is part-done, must notice and not answer what it
/// read: the after-load sees the MSET's marks. (Kills a read that only checks
/// for writes in progress before it reads.)
#[test]
fn an_mget_that_began_before_an_mset_validates_what_it_read() {
    let store = KeyValueStore::new();
    let keys = ["atom:val:a", "atom:val:b"];
    store.execute(mset(&[(keys[0], "old"), (keys[1], "old")]));
    let pause = write_pause::after_mutation_of(keys[0], &keys);
    pause.stop_next_reader_after_load();
    let answer = OnceLock::new();
    std::thread::scope(|scope| {
        let reader = scope.spawn(|| {
            let _ = answer.set(store.execute(mget(&keys)));
        });
        assert!(pause.wait_reader_stopped(WITHIN), "the MGET never loaded");
        let writer = scope.spawn(|| store.execute(mset(&[(keys[0], "new"), (keys[1], "new")])));
        assert!(
            pause.wait_reached(WITHIN),
            "the MSET never reached its stop"
        );
        // The MGET now reads `a` new and `b` old.
        pause.release_reader();
        pause.wait_reader_blocked_or(WITHIN, || answer.get().is_some());
        drop(pause);
        writer.join().unwrap();
        reader.join().unwrap();
    });
    let answer = answer.into_inner().unwrap();
    assert!(
        answer == Value::Array(Some(vec![bulk("old"), bulk("old")]))
            || answer == Value::Array(Some(vec![bulk("new"), bulk("new")])),
        "MGET answered what it read across an MSET: {answer:?}"
    );
}

// ── Concurrency ───────────────────────────────────────────────────────────

/// Run `work(thread, round)` on `threads` threads, released together, and
/// fail rather than hang if they do not all finish within `WITHIN` — a
/// deadlock between readers and writers.
fn race_or_deadlock(
    store: &Arc<KeyValueStore>,
    threads: usize,
    rounds: usize,
    work: impl Fn(&KeyValueStore, usize, usize) + Send + Sync + 'static,
) {
    let work = Arc::new(work);
    let barrier = Arc::new(std::sync::Barrier::new(threads));
    let (done, finished) = std::sync::mpsc::channel();
    for t in 0..threads {
        let (store, work, barrier, done) = (
            Arc::clone(store),
            Arc::clone(&work),
            Arc::clone(&barrier),
            done.clone(),
        );
        std::thread::spawn(move || {
            barrier.wait();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                for r in 0..rounds {
                    work(&store, t, r);
                }
            }));
            let _ = done.send(outcome.map_err(|panic| {
                panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default()
            }));
        });
    }
    for _ in 0..threads {
        match finished.recv_timeout(WITHIN) {
            Ok(Ok(())) => {}
            Ok(Err(panic)) => panic!("a worker failed: {panic}"),
            Err(_) => panic!("workers did not finish within {WITHIN:?}: deadlock"),
        }
    }
}

fn values(reply: &Value) -> Vec<Option<Vec<u8>>> {
    match reply {
        Value::Array(Some(items)) => items
            .iter()
            .map(|item| match item {
                Value::BulkString(v) => v.clone(),
                other => panic!("unexpected item {other:?}"),
            })
            .collect(),
        other => panic!("unexpected reply {other:?}"),
    }
}

const LOAD_KEYS: [&str; 8] = [
    "load:k0", "load:k1", "load:k2", "load:k3", "load:k4", "load:k5", "load:k6", "load:k7",
];

/// Tests C and D: MSETs of one generation tag across eight keys, in
/// different key orders, racing MGETs (owned and borrowed) of all eight.
/// Every MGET is one generation; at the end every key holds the same one;
/// nothing deadlocks.
#[test]
fn mgets_racing_msets_in_any_key_order_see_one_generation() {
    let store = Arc::new(KeyValueStore::new());
    store.execute(mset(&LOAD_KEYS.map(|k| (k, "g0"))));
    let locked_before = write_pause::LOCKED_READS.load(Ordering::Relaxed);
    race_or_deadlock(&store, 6, 3_000, |store, t, r| {
        if t < 3 {
            let tag = format!("t{t}r{r}");
            let mut keys = LOAD_KEYS.to_vec();
            // Writers name the keys in different orders.
            keys.rotate_left((t * 3 + r) % LOAD_KEYS.len());
            if t == 1 {
                keys.reverse();
            }
            let pairs: Vec<(&str, &str)> = keys.iter().map(|k| (*k, tag.as_str())).collect();
            store.execute(mset(&pairs));
        } else if t == 3 {
            let mut out = Vec::new();
            let cmd = CommandRef::MGet(LOAD_KEYS.iter().copied().collect());
            store.execute_ref_into(&cmd, &mut out).unwrap();
            let (reply, used) = Value::parse(&out).unwrap();
            assert_eq!(used, out.len(), "a retried MGET left a partial reply behind");
            let seen = values(&reply);
            assert!(
                seen.windows(2).all(|w| w[0] == w[1]),
                "borrowed MGET mixed: {seen:?}"
            );
        } else {
            let seen = values(&store.execute(mget(&LOAD_KEYS)));
            assert!(
                seen.windows(2).all(|w| w[0] == w[1]),
                "MGET mixed: {seen:?}"
            );
        }
    });
    let last = values(&store.execute(mget(&LOAD_KEYS)));
    assert!(
        last.windows(2).all(|w| w[0] == w[1]),
        "final state mixed: {last:?}"
    );
    // The contention made some reads take the locks: that path ran too.
    assert!(
        write_pause::LOCKED_READS.load(Ordering::Relaxed) > locked_before,
        "no read fell back to the locks; the test did not exercise them"
    );
}

/// Test D, single-key reads: one writer MSETs increasing generations over
/// two keys; readers read one key and then the other. Once a read has seen
/// generation g of an MSET, every read that starts afterwards sees g or
/// later — on either key.
#[test]
fn reads_after_seeing_an_mset_generation_never_see_an_earlier_one() {
    let store = Arc::new(KeyValueStore::new());
    let (a, b) = ("mono:a", "mono:b");
    distinct_stripes(&[a, b]);
    store.execute(mset(&[(a, "0"), (b, "0")]));
    let gen_of = |reply: Value| -> u64 {
        match reply {
            Value::BulkString(Some(v)) => String::from_utf8(v).unwrap().parse().unwrap(),
            other => panic!("unexpected {other:?}"),
        }
    };
    race_or_deadlock(&store, 4, 20_000, move |store, t, r| {
        if t == 0 {
            let g = (r + 1).to_string();
            store.execute(mset(&[(a, g.as_str()), (b, g.as_str())]));
        } else {
            let (first, second) = if (t + r) % 2 == 0 { (a, b) } else { (b, a) };
            let seen = gen_of(store.execute(Command::Get(first.into())));
            let then = gen_of(store.execute(Command::Get(second.into())));
            assert!(
                then >= seen,
                "GET {first} saw generation {seen}, then GET {second} saw {then}"
            );
        }
    });
}

/// The other multi-key writes take the same path, and the same protection:
/// RENAME back and forth always leaves exactly one of the two keys, and a
/// reader always sees exactly one; DEL of both racing MSET of both shows both
/// or neither.
#[test]
fn rename_and_del_are_atomic_to_readers_too() {
    let store = Arc::new(KeyValueStore::new());
    let (a, b) = ("ren:a", "ren:b");
    distinct_stripes(&[a, b]);
    store.execute(Command::Set(a.into(), b"v".to_vec(), SetOptions::default()));
    race_or_deadlock(&store, 4, 5_000, move |store, t, _| match t {
        0 => {
            store.execute(Command::Rename(a.into(), b.into()));
            store.execute(Command::Rename(b.into(), a.into()));
        }
        1 => {
            let seen = values(&store.execute(mget(&[a, b])));
            assert_eq!(
                seen.iter().filter(|v| v.is_some()).count(),
                1,
                "MGET {seen:?}"
            );
        }
        2 => assert_eq!(
            store.execute(Command::Exists(vec![a.into(), b.into()])),
            Value::Integer(1)
        ),
        _ => {
            // RENAME of a key that is momentarily the other one fails with
            // "no such key"; ignore that, it changes nothing.
            let _ = store.execute(Command::Rename(a.into(), b.into()));
        }
    });

    let store = Arc::new(KeyValueStore::new());
    let (c, d) = ("del:c", "del:d");
    distinct_stripes(&[c, d]);
    race_or_deadlock(&store, 3, 5_000, move |store, t, r| match t {
        0 => {
            store.execute(mset(&[(c, "x"), (d, "x")]));
        }
        1 => {
            if r % 2 == 0 {
                store.execute(Command::Del(vec![c.into(), d.into()]));
            } else {
                store.execute(Command::Del(vec![d.into(), c.into()]));
            }
        }
        _ => {
            let present = values(&store.execute(mget(&[c, d])))
                .iter()
                .filter(|v| v.is_some())
                .count();
            assert!(
                present != 1,
                "MGET saw one of two keys a DEL or MSET wrote together"
            );
        }
    });
}

// ── Expiry and eviction ───────────────────────────────────────────────────

/// Test F: one MGET evaluates every key's expiry at one time, as Redis does,
/// so keys sharing a deadline are all live or all expired in one answer —
/// even when the clock ticks while the MGET runs.
#[test]
fn one_mget_sees_keys_sharing_a_deadline_together() {
    let store = KeyValueStore::new();
    let keys: Vec<String> = (0..4_000).map(|i| format!("dl:{i}")).collect();
    for key in &keys {
        store.execute(Command::Set(
            key.clone(),
            b"v".to_vec(),
            SetOptions::default(),
        ));
    }
    // Far enough ahead that setting it on every key finishes first.
    let deadline = now_ms() + 250;
    for key in &keys {
        store.execute(Command::PExpireAt(key.clone(), deadline));
    }
    assert!(now_ms() < deadline, "setup outran the deadline");
    let cmd = Command::MGet(keys.clone());
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    let borrowed = CommandRef::MGet(refs.iter().copied().collect());
    let mut answers = 0;
    while now_ms() <= deadline + 5 {
        let live = values(&store.execute(cmd.clone()))
            .iter()
            .filter(|v| v.is_some())
            .count();
        assert!(
            live == 0 || live == keys.len(),
            "one MGET saw {live} of {} live",
            keys.len()
        );
        let mut out = Vec::new();
        store.execute_ref_into(&borrowed, &mut out).unwrap();
        let (reply, used) = Value::parse(&out).unwrap();
        assert_eq!(used, out.len(), "a retried MGET left a partial reply behind");
        let live = values(&reply).iter().filter(|v| v.is_some()).count();
        assert!(
            live == 0 || live == keys.len(),
            "one borrowed MGET saw {live} live"
        );
        answers += 1;
    }
    // MGETs ran back to back across the deadline, so one of them was
    // running when the clock passed it.
    assert!(answers > 1, "only {answers} MGETs ran");
}

/// Test F: the expiry sweeper (which takes each key's stripe) racing MSETs
/// that clear TTLs and MGETs of the same keys: every MGET is one generation
/// or, for keys a sweep removed after their TTL, absent — never a mix of two
/// generations — and nothing deadlocks.
#[test]
fn sweeps_racing_msets_and_mgets_never_mix_generations() {
    let store = Arc::new(KeyValueStore::new());
    let keys = ["sw:a", "sw:b", "sw:c"];
    race_or_deadlock(&store, 4, 3_000, move |store, t, r| match t {
        0 => {
            let tag = format!("g{r}");
            store.execute(mset(&keys.map(|k| (k, tag.as_str()))));
        }
        1 => {
            // Give the keys a TTL that has already passed, then sweep.
            for key in keys {
                store.execute(Command::PExpireAt(key.into(), 1));
            }
            store.sweep_expired();
        }
        _ => {
            let seen = values(&store.execute(mget(&keys)));
            let present: Vec<_> = seen.iter().flatten().collect();
            assert!(
                present.windows(2).all(|w| w[0] == w[1]),
                "MGET mixed generations: {seen:?}"
            );
        }
    });
}

/// Test F, capped: on a store at its memory cap, MSETs evict keys while
/// MGETs read. Eviction removes keys one by one (pre-existing, recorded), so
/// an MGET may see some keys gone — but every value it sees is from one
/// MSET, and nothing deadlocks between readers, writers and eviction.
#[test]
fn eviction_racing_msets_and_mgets_never_mixes_generations() {
    let store = Arc::new(KeyValueStore::with_config(
        None,
        Some(64 * 1024),
        EvictionPolicy::AllKeysLru,
    ));
    let keys = ["ev:a", "ev:b", "ev:c", "ev:d"];
    race_or_deadlock(&store, 4, 2_000, move |store, t, r| match t {
        0 | 1 => {
            let tag = format!("t{t}r{r}{}", "x".repeat(400));
            store.execute(mset(&keys.map(|k| (k, tag.as_str()))));
            // Filler that pushes the store over its cap.
            let filler = format!("fill:{t}:{r}");
            store.execute(Command::Set(
                filler,
                vec![b'f'; 2_000],
                SetOptions::default(),
            ));
        }
        _ => {
            let seen = values(&store.execute(mget(&keys)));
            let present: Vec<_> = seen.iter().flatten().collect();
            assert!(
                present.windows(2).all(|w| w[0] == w[1]),
                "MGET mixed generations on a capped store"
            );
        }
    });
}
