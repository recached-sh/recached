//! 6O-G: [`KeyValueStore::apply_transaction`], the store half of two-phase
//! `EXEC` (design `V0.6O-F-TRANSACTION-ATOMICITY-DESIGN.md`, Option E rev 5).
//!
//! A transaction stopped inside its write span ([`write_pause::
//! after_transaction_write_of`]) holds and marks every stripe its writes
//! take. These tests read and write the keyspace from other threads while it
//! is stopped, and check that:
//!
//! - outside readers see none of its writes until all are visible, waiting
//!   on the marks (counted by the hook) rather than answering part-way;
//! - an unrelated writer that shares a stripe waits for the span;
//! - the transaction's own reads see its earlier writes and never deadlock
//!   on its own stripes or capacity gate, capped or not, reserved writes on
//!   or off;
//! - a panic inside the span is contained: nothing after it runs, the
//!   metadata is rebuilt, and no stripe, mark, gate or scope is left behind.
//!
//! Every blocking step runs on its own thread with a deadline, so a deadlock
//! fails the test instead of hanging it.

use super::*;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::Duration;

const WITHIN: Duration = Duration::from_secs(20);

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

fn ok() -> Value {
    Value::SimpleString("OK".into())
}

fn set(key: &str, value: &str) -> Command {
    Command::Set(key.into(), value.as_bytes().to_vec(), SetOptions::default())
}

fn get(key: &str) -> Command {
    Command::Get(key.into())
}

fn mget(keys: &[&str]) -> Command {
    Command::MGet(keys.iter().map(|key| key.to_string()).collect())
}

/// Run `f` on its own thread and return its result, failing the test if it
/// has not returned within [`WITHIN`] (a deadlock).
fn finishes<T: Send + 'static>(what: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(WITHIN)
        .unwrap_or_else(|_| panic!("{what} did not finish: deadlock"))
}

/// Apply `queue` as a transaction the way the server does: each command's
/// store call, at the transaction's instant.
fn apply(store: &KeyValueStore, queue: &[Command]) -> (Vec<Value>, TransactionApplied) {
    let mut replies = Vec::new();
    let applied = store.apply_transaction(queue, clock_ms(), |_, cmd| {
        replies.push(store.execute_reporting_ref(cmd).0);
    });
    (replies, applied)
}

/// A key other than `key` on the same write-order stripe.
fn sharing_stripe_with(key: &str, prefix: &str) -> String {
    let stripe = write_order_stripe(key);
    (0..)
        .map(|i| format!("{prefix}{i}"))
        .find(|other| other != key && write_order_stripe(other) == stripe)
        .unwrap()
}

/// A key on none of `keys`' stripes.
fn apart_from(keys: &[&str], prefix: &str) -> String {
    (0..)
        .map(|i| format!("{prefix}{i}"))
        .find(|other| {
            keys.iter()
                .all(|key| write_order_stripe(key) != write_order_stripe(other))
        })
        .unwrap()
}

fn multi_active(store: &KeyValueStore, key: &str) -> bool {
    store.write_order[write_order_stripe(key)]
        .multi
        .load(Ordering::Acquire)
        & MULTI_ACTIVE
        != 0
}

fn no_stripe_marked(store: &KeyValueStore) -> bool {
    store.write_order.iter().all(|stripe| {
        stripe.multi.load(Ordering::Acquire) & MULTI_ACTIVE == 0
            && stripe.seq.load(Ordering::Acquire) & SEQ_ACTIVE == 0
    })
}

fn no_stripe_held(store: &KeyValueStore) -> bool {
    store
        .write_order
        .iter()
        .all(|stripe| stripe.lock.try_lock().is_ok())
}

/// The stores the scope must work on: uncapped, capped with eviction, and
/// capped with refusal — the last two with reserved writes off and on.
fn stores() -> Vec<(&'static str, KeyValueStore)> {
    let capped_evict =
        || KeyValueStore::with_config(Some(1_000), Some(1 << 24), EvictionPolicy::AllKeysLru);
    let capped_refuse =
        || KeyValueStore::with_config(Some(1_000), Some(1 << 24), EvictionPolicy::NoEviction);
    let reserved = |store: KeyValueStore| {
        store.set_reserved_writes(true);
        store
    };
    vec![
        ("uncapped", KeyValueStore::new()),
        ("capped-evict", capped_evict()),
        ("capped-evict+reserved", reserved(capped_evict())),
        ("capped-refuse", capped_refuse()),
        ("capped-refuse+reserved", reserved(capped_refuse())),
    ]
}

// ── Read-your-writes, no self-deadlock ────────────────────────────────────

#[test]
fn queued_reads_see_earlier_queued_writes_without_deadlock() {
    for (label, store) in stores() {
        let shared = sharing_stripe_with("ryw:a", "ryw:s");
        store.execute(set(&shared, "outside"));
        let queue = vec![
            set("ryw:a", "1"),
            get("ryw:a"),
            mget(&["ryw:a", "ryw:b"]),
            Command::Exists(vec!["ryw:a".into(), "ryw:b".into()]),
            Command::Type("ryw:a".into()),
            // A read of another key on a written key's stripe.
            get(&shared),
            set("ryw:b", "2"),
            Command::Append("ryw:b".into(), b"x".to_vec()),
            mget(&["ryw:a", "ryw:b"]),
            Command::PTtl("ryw:a".into()),
            Command::Set(
                "ryw:c".into(),
                b"v".to_vec(),
                SetOptions {
                    expiry: Some(SetExpiry::Px(60_000)),
                    ..SetOptions::default()
                },
            ),
            // The server's deadline read-back of the write just made.
            Command::PExpireTime("ryw:c".into()),
            Command::SAdd("ryw:set".into(), vec!["m".into()]),
            // The server's claimable-members check.
            Command::SIsMember("ryw:set".into(), "m".into()),
        ];
        let probe = store.clone();
        let (replies, applied) = finishes(label, move || apply(&probe, &queue));
        assert_eq!(applied, TransactionApplied::default(), "{label}");
        assert_eq!(replies[1], bulk("1"), "{label}: GET after SET");
        assert_eq!(
            replies[2],
            Value::Array(Some(vec![bulk("1"), Value::BulkString(None)])),
            "{label}"
        );
        assert_eq!(replies[3], Value::Integer(1), "{label}: EXISTS");
        assert_eq!(replies[4], Value::SimpleString("string".into()), "{label}");
        assert_eq!(replies[5], bulk("outside"), "{label}: shared stripe");
        assert_eq!(
            replies[8],
            Value::Array(Some(vec![bulk("1"), bulk("2x")])),
            "{label}"
        );
        assert_eq!(replies[9], Value::Integer(-1), "{label}: PTTL");
        assert!(
            matches!(replies[11], Value::Integer(at) if at > 0),
            "{label}: deadline read back: {:?}",
            replies[11]
        );
        assert_eq!(replies[13], Value::Integer(1), "{label}: SISMEMBER");
        assert!(
            no_stripe_marked(&store) && no_stripe_held(&store),
            "{label}"
        );
        assert_eq!(TX_SCOPE.with(std::cell::Cell::get).store, 0, "{label}");
    }
}

#[test]
fn whole_keyspace_and_capped_writes_inside_the_span_take_nothing_again() {
    for (label, store) in stores() {
        for i in 0..20 {
            store.execute(set(&format!("ks:{i}"), "v"));
        }
        let queue = vec![
            set("ks:a", "1"),
            Command::FlushDb,
            get("ks:a"),
            set("ks:b", "2"),
            Command::MSet(vec![
                ("ks:c".into(), b"3".to_vec()),
                ("ks:d".into(), b"4".to_vec()),
            ]),
            Command::DbSize,
            Command::Rename("ks:c".into(), "ks:e".into()),
            Command::SUnionStore("ks:f".into(), vec!["ks:none".into()]),
            Command::Del(vec!["ks:d".into(), "ks:b".into()]),
            Command::Keys("ks:*".into()),
        ];
        let probe = store.clone();
        let (replies, applied) = finishes(label, move || apply(&probe, &queue));
        assert_eq!(applied, TransactionApplied::default(), "{label}");
        assert_eq!(replies[2], Value::BulkString(None), "{label}: flushed");
        assert_eq!(replies[5], Value::Integer(3), "{label}: DBSIZE");
        assert_eq!(
            replies[9],
            Value::Array(Some(vec![bulk("ks:e")])),
            "{label}: KEYS"
        );
        assert_eq!(store.key_count(), 1, "{label}");
        assert!(
            no_stripe_marked(&store) && no_stripe_held(&store),
            "{label}"
        );
    }
}

#[test]
fn capped_writes_in_a_transaction_still_evict_and_refuse() {
    // Evicting: the key cap is enforced under the scope's gate.
    let store = KeyValueStore::with_config(Some(3), None, EvictionPolicy::AllKeysLru);
    let probe = store.clone();
    let queue: Vec<Command> = (0..6).map(|i| set(&format!("cap:{i}"), "v")).collect();
    let (replies, applied) = finishes("evicting", move || apply(&probe, &queue));
    assert_eq!(applied, TransactionApplied::default());
    assert!(replies.iter().all(|reply| *reply == ok()));
    assert_eq!(store.key_count(), 3);
    // Refusing.
    let store = KeyValueStore::with_config(Some(2), None, EvictionPolicy::NoEviction);
    let probe = store.clone();
    let queue: Vec<Command> = (0..3).map(|i| set(&format!("cap:{i}"), "v")).collect();
    let (replies, _) = finishes("refusing", move || apply(&probe, &queue));
    assert_eq!(&replies[..2], &[ok(), ok()]);
    assert!(matches!(&replies[2], Value::Error(e) if e.contains("max keys")));
    // And the within-capacity entry point, inside the scope, on a full store.
    let store = KeyValueStore::with_config(Some(1), None, EvictionPolicy::AllKeysLru);
    store.execute(set("cap:x", "v"));
    let probe = store.clone();
    let declined = finishes("within capacity", move || {
        let mut out = None;
        probe.apply_transaction(&[set("cap:y", "v")], clock_ms(), |_, cmd| {
            out = Some(probe.execute_reporting_ref_within_capacity(cmd));
        });
        out.unwrap()
    });
    assert_eq!(declined, None, "a write that must evict is declined");
}

#[test]
fn a_read_only_queue_takes_and_marks_nothing() {
    let store = KeyValueStore::new();
    store.execute(set("ro:a", "1"));
    let words: Vec<u64> = store
        .write_order
        .iter()
        .map(|stripe| stripe.seq.load(Ordering::Acquire))
        .collect();
    let mut in_scope = Vec::new();
    store.apply_transaction(
        &[get("ro:a"), mget(&["ro:a", "ro:b"])],
        clock_ms(),
        |_, _| {
            in_scope.push(store.scope().is_some());
        },
    );
    assert_eq!(in_scope, vec![false, false]);
    let after: Vec<u64> = store
        .write_order
        .iter()
        .map(|stripe| stripe.seq.load(Ordering::Acquire))
        .collect();
    assert_eq!(words, after, "no stripe version moved");
}

#[test]
fn the_scope_covers_exactly_the_write_span() {
    let store = KeyValueStore::new();
    let queue = vec![
        get("span:a"),
        set("span:a", "1"),
        get("span:a"),
        set("span:b", "2"),
        get("span:b"),
    ];
    let mut in_scope = Vec::new();
    store.apply_transaction(&queue, clock_ms(), |_, _| {
        in_scope.push(store.scope().is_some());
    });
    assert_eq!(in_scope, vec![false, true, true, true, false]);
    assert!(no_stripe_marked(&store) && no_stripe_held(&store));
}

#[test]
fn the_scope_is_per_store_and_per_thread() {
    let store = KeyValueStore::new();
    let other = KeyValueStore::new();
    let mut seen = None;
    store.apply_transaction(&[set("own:a", "1")], clock_ms(), |_, cmd| {
        store.execute_reporting_ref(cmd);
        // Another store on this thread takes and marks its own stripes.
        other.execute(set("own:a", "x"));
        assert_eq!(other.execute(get("own:a")), bulk("x"));
        // Another thread is not in the scope.
        let probe = store.clone();
        seen = Some(
            std::thread::spawn(move || probe.scope().is_none())
                .join()
                .unwrap(),
        );
    });
    assert_eq!(seen, Some(true));
    assert!(no_stripe_marked(&other) && no_stripe_held(&other));
}

// ── Outside readers and writers during the span ───────────────────────────

/// Stop `queue` after its write of `stop_at`, run `outside` on another
/// thread, and report whether it waited for the stop (a reader counted by
/// the hook, or simply not done while stopped), then what it returned once
/// the transaction finished.
fn during_the_span<T: Send + 'static>(
    store: &KeyValueStore,
    queue: Vec<Command>,
    stop_at: &str,
    readers_of: &[&str],
    outside: impl FnOnce(&KeyValueStore) -> T + Send + 'static,
) -> (bool, T) {
    let pause = write_pause::after_transaction_write_of(store, stop_at, readers_of);
    let tx_store = store.clone();
    let tx = std::thread::spawn(move || apply(&tx_store, &queue));
    assert!(pause.wait_reached(WITHIN), "the transaction never stopped");
    let done = Arc::new(AtomicBool::new(false));
    let (out_tx, out_rx) = mpsc::channel();
    {
        let (store, done) = (store.clone(), Arc::clone(&done));
        std::thread::spawn(move || {
            let out = outside(&store);
            done.store(true, Ordering::SeqCst);
            let _ = out_tx.send(out);
        });
    }
    let blocked =
        pause.wait_reader_blocked_or(Duration::from_millis(300), || done.load(Ordering::SeqCst));
    // Not counted (a writer, or a reader the hook does not watch): it
    // waited if it is still not done.
    let waited = blocked || !done.load(Ordering::SeqCst);
    drop(pause);
    let out = out_rx
        .recv_timeout(WITHIN)
        .expect("the outside op finished");
    tx.join().unwrap();
    assert!(no_stripe_marked(store) && no_stripe_held(store));
    (waited, out)
}

#[test]
fn an_outside_mget_sees_all_or_none_of_the_transaction() {
    let store = KeyValueStore::new();
    store.execute(Command::MSet(vec![
        ("ext:a".into(), b"old".to_vec()),
        ("ext:b".into(), b"old".to_vec()),
    ]));
    let (waited, seen) = during_the_span(
        &store,
        vec![set("ext:a", "new"), set("ext:b", "new")],
        "ext:a",
        &["ext:a", "ext:b"],
        |store| store.execute(mget(&["ext:a", "ext:b"])),
    );
    assert!(waited, "the MGET answered inside the span: {seen:?}");
    assert_eq!(seen, Value::Array(Some(vec![bulk("new"), bulk("new")])));
}

#[test]
fn outside_gets_of_the_first_and_the_later_key_wait_for_the_span() {
    for (read, label) in [("ext1:a", "first"), ("ext1:b", "later")] {
        let store = KeyValueStore::new();
        store.execute(set("ext1:a", "old"));
        store.execute(set("ext1:b", "old"));
        let key = read.to_string();
        let (waited, seen) = during_the_span(
            &store,
            vec![set("ext1:a", "new"), set("ext1:b", "new")],
            "ext1:a",
            &[read],
            move |store| store.execute(get(&key)),
        );
        assert!(waited, "a GET of the {label} key answered inside the span");
        assert_eq!(seen, bulk("new"), "{label}");
    }
}

#[test]
fn outside_borrowed_reads_wait_for_the_span() {
    let store = KeyValueStore::new();
    store.execute(set("bor:a", "old"));
    store.execute(set("bor:b", "old"));
    let (waited, seen) = during_the_span(
        &store,
        vec![set("bor:a", "new"), set("bor:b", "new")],
        "bor:a",
        &["bor:b"],
        |store| {
            let mut out = Vec::new();
            store.execute_ref_into(&CommandRef::Get("bor:b"), &mut out);
            out
        },
    );
    assert!(waited);
    assert_eq!(seen, b"$3\r\nnew\r\n");
}

#[test]
fn the_marks_cover_every_written_stripe_from_the_first_write() {
    let store = KeyValueStore::new();
    let queue = vec![
        set("mk:a", "1"),
        Command::MSet(vec![
            ("mk:b".into(), b"2".to_vec()),
            ("mk:c".into(), b"3".to_vec()),
        ]),
        Command::Rename("mk:a".into(), "mk:d".into()),
    ];
    let probe = store.clone();
    let (_, marked) = during_the_span(&store, queue, "mk:a", &[], move |_| {
        ["mk:a", "mk:b", "mk:c", "mk:d"]
            .iter()
            .all(|key| multi_active(&probe, key))
    });
    assert!(marked, "a later write's stripe was not marked at the first");
}

#[test]
fn an_unrelated_writer_sharing_a_stripe_waits_and_one_apart_does_not() {
    let store = KeyValueStore::new();
    let shared = sharing_stripe_with("uw:a", "uw:s");
    let (waited, _) = during_the_span(
        &store,
        vec![set("uw:a", "1"), set("uw:b", "2")],
        "uw:a",
        &[],
        move |store| store.execute(set(&shared, "x")),
    );
    assert!(waited, "a writer on a held stripe ran inside the span");
    let apart = apart_from(&["uw2:a", "uw2:b"], "uw2:o");
    let (waited, reply) = during_the_span(
        &store,
        vec![set("uw2:a", "1"), set("uw2:b", "2")],
        "uw2:a",
        &[],
        move |store| store.execute(set(&apart, "x")),
    );
    assert!(!waited, "a writer of an unheld stripe waited");
    assert_eq!(reply, ok());
}

#[test]
fn a_flushdb_in_the_queue_makes_every_reader_wait() {
    let store = KeyValueStore::new();
    store.execute(set("fl:z", "old"));
    let (waited, seen) = during_the_span(
        &store,
        vec![set("fl:a", "1"), Command::FlushDb, set("fl:b", "2")],
        "fl:a",
        &["fl:z"],
        |store| store.execute(get("fl:z")),
    );
    assert!(
        waited,
        "a reader of an unrelated key saw the FLUSHDB part-way"
    );
    assert_eq!(seen, Value::BulkString(None));
}

#[test]
fn concurrent_capped_transactions_sweeps_and_eviction_do_not_deadlock() {
    for reserved in [false, true] {
        let store =
            KeyValueStore::with_config(Some(200), Some(1 << 16), EvictionPolicy::AllKeysLru);
        store.set_reserved_writes(reserved);
        let stop = Arc::new(AtomicBool::new(false));
        let mut threads = Vec::new();
        for t in 0..3 {
            let (store, stop) = (store.clone(), Arc::clone(&stop));
            threads.push(std::thread::spawn(move || {
                let mut n = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    let k = |i: u64| format!("cc:{}", (n * 7 + i * 13 + t) % 400);
                    let queue = vec![
                        set(&k(0), "v"),
                        get(&k(1)),
                        Command::Set(
                            k(2),
                            b"w".to_vec(),
                            SetOptions {
                                expiry: Some(SetExpiry::Px(1)),
                                ..SetOptions::default()
                            },
                        ),
                        Command::MSet(vec![(k(3), b"x".to_vec()), (k(4), b"y".to_vec())]),
                        Command::Incr(k(5)),
                    ];
                    apply(&store, &queue);
                    n += 1;
                }
                n
            }));
        }
        {
            let (store, stop) = (store.clone(), Arc::clone(&stop));
            threads.push(std::thread::spawn(move || {
                let mut n = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    store.sweep_expired_reporting_budget(32);
                    store.try_evict_for_memory_reporting();
                    store.execute(set(&format!("cc:o{}", n % 50), "o"));
                    store.execute(mget(&["cc:1", "cc:2", "cc:3"]));
                    n += 1;
                }
                n
            }));
        }
        std::thread::sleep(Duration::from_millis(500));
        stop.store(true, Ordering::Relaxed);
        let counts = finishes("capped transactions", move || {
            threads
                .into_iter()
                .map(|t| t.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert!(
            counts.iter().all(|&n| n > 0),
            "reserved={reserved}: {counts:?}"
        );
        assert!(store.key_count() <= 200);
        assert!(no_stripe_marked(&store) && no_stripe_held(&store));
    }
}

// ── Panics inside the span ─────────────────────────────────────────────────

/// The metadata a rebuild from the keyspace produces, against what the store
/// holds: equal after a repair.
fn metadata_is_consistent(store: &KeyValueStore) -> bool {
    let (keys, memory) = (store.key_count(), store.approximate_memory_bytes());
    let fresh = store.clone();
    {
        let _all = fresh.order_all();
        fresh.rebuild_metadata();
    }
    keys == fresh.key_count()
        && memory == fresh.approximate_memory_bytes()
        && keys == store.data.len()
}

#[derive(Clone, Copy, Debug)]
enum Fault {
    /// In the caller's work, before the store call.
    Before,
    /// Inside the store call, after the mutation, before its accounting.
    During,
    /// In the caller's work, after the store call (before its frame).
    After,
}

fn panics_at(
    fault: Fault,
    store: &KeyValueStore,
    queue: &[Command],
    at: usize,
    key: &str,
) -> (Vec<Value>, TransactionApplied) {
    let _armed =
        matches!(fault, Fault::During).then(|| transaction_fault::panic_during_mutation_of(key));
    let mut replies = Vec::new();
    let applied = store.apply_transaction(queue, clock_ms(), |i, cmd| {
        if i == at && matches!(fault, Fault::Before) {
            panic!("test-injected panic before the mutation");
        }
        replies.push(store.execute_reporting_ref(cmd).0);
        if i == at && matches!(fault, Fault::After) {
            panic!("test-injected panic after the mutation");
        }
    });
    (replies, applied)
}

#[test]
fn a_panic_in_the_span_stops_the_queue_repairs_and_releases_everything() {
    for (label, store) in stores() {
        for fault in [Fault::Before, Fault::During, Fault::After] {
            let p = format!("pn:{label}:{fault:?}:");
            let key = |s: &str| format!("{p}{s}");
            store.execute(Command::RPush(key("list"), vec![b"only".to_vec()]));
            store.execute(set(&key("c"), "old"));
            let queue = vec![
                set(&key("a"), "new"),
                // Pops the last element: the fault lands before the emptied
                // list's removal.
                Command::LPop(key("list"), None),
                set(&key("c"), "new"),
            ];
            let probe = store.clone();
            let (key_list, queue2) = (key("list"), queue.clone());
            let (replies, applied) =
                finishes(&p, move || panics_at(fault, &probe, &queue2, 1, &key_list));
            assert_eq!(
                applied,
                TransactionApplied {
                    panicked: Some(1),
                    repaired: true
                },
                "{p}"
            );
            let expected_replies = match fault {
                Fault::Before | Fault::During => 1,
                Fault::After => 2,
            };
            assert_eq!(
                replies.len(),
                expected_replies,
                "{p}: nothing ran after the panic"
            );
            assert_eq!(
                store.execute(get(&key("a"))),
                bulk("new"),
                "{p}: prefix kept"
            );
            assert_eq!(
                store.execute(get(&key("c"))),
                bulk("old"),
                "{p}: rest not run"
            );
            let list_exists = store.execute(Command::Exists(vec![key("list")]));
            let expected = match fault {
                Fault::Before => Value::Integer(1),
                _ => Value::Integer(0),
            };
            assert_eq!(
                list_exists, expected,
                "{p}: emptied list removed by the repair"
            );
            assert!(metadata_is_consistent(&store), "{p}: metadata rebuilt");
            assert!(no_stripe_marked(&store) && no_stripe_held(&store), "{p}");
            assert_eq!(TX_SCOPE.with(std::cell::Cell::get).store, 0, "{p}");
            assert_eq!(FROZEN_NOW.with(std::cell::Cell::get), None, "{p}");
            // The gate too: a capped write from another thread proceeds.
            let probe = store.clone();
            let after = key("after");
            assert_eq!(finishes(&p, move || probe.execute(set(&after, "v"))), ok());
        }
    }
}

#[test]
fn a_panic_in_a_read_stops_the_queue_without_a_repair() {
    let store = KeyValueStore::new();
    let mut ran = Vec::new();
    let applied = store.apply_transaction(
        &[set("pr:a", "1"), get("pr:a"), set("pr:b", "2")],
        clock_ms(),
        |i, cmd| {
            if i == 1 {
                panic!("test-injected panic in a read");
            }
            ran.push(i);
            store.execute_reporting_ref(cmd);
        },
    );
    assert_eq!(
        applied,
        TransactionApplied {
            panicked: Some(1),
            repaired: false
        }
    );
    assert_eq!(ran, vec![0]);
    assert!(no_stripe_marked(&store) && no_stripe_held(&store));
}

// ── Review A follow-ups ────────────────────────────────────────────────────

/// A capped transaction marks every stripe: an eviction victim on a stripe
/// apart from its keys is not seen gone before the transaction is (M9).
#[test]
fn a_capped_eviction_victim_is_not_seen_gone_inside_the_span() {
    let store = KeyValueStore::with_config(Some(2), None, EvictionPolicy::AllKeysLru);
    let v1 = apart_from(&["ev:a", "ev:b"], "ev:v1-");
    let v2 = apart_from(&["ev:a", "ev:b", &v1], "ev:v2-");
    store.execute(set(&v1, "old"));
    store.execute(set(&v2, "old"));
    let (v1c, v2c) = (v1.clone(), v2.clone());
    let (waited, seen) = during_the_span(
        &store,
        vec![set("ev:a", "new"), set("ev:b", "new")],
        "ev:a",
        &[&v1, &v2],
        move |store| store.execute(Command::Exists(vec![v1c, v2c])),
    );
    assert!(waited, "a reader saw an eviction inside the span: {seen:?}");
    assert_eq!(seen, Value::Integer(0));
}

/// A capped transaction holds the capacity gate: the eviction tick and a
/// capped expiry sweep wait for its span (M8).
#[test]
fn the_eviction_tick_and_a_capped_sweep_wait_for_a_capped_span() {
    let store = KeyValueStore::with_config(Some(1_000), Some(1 << 20), EvictionPolicy::AllKeysLru);
    let (waited, _) = during_the_span(
        &store,
        vec![set("et:a", "new"), set("et:b", "new")],
        "et:a",
        &[],
        |store| store.try_evict_for_memory_reporting(),
    );
    assert!(waited, "the eviction tick ran inside a capped span");
    store.execute(Command::Set(
        "et:ttl".into(),
        b"x".to_vec(),
        SetOptions {
            expiry: Some(SetExpiry::Px(1)),
            ..SetOptions::default()
        },
    ));
    std::thread::sleep(Duration::from_millis(5));
    let (waited, _) = during_the_span(
        &store,
        vec![set("et:c", "new"), set("et:d", "new")],
        "et:c",
        &[],
        |store| store.sweep_expired_reporting_budget(16),
    );
    assert!(waited, "a capped sweep ran inside a capped span");
}

/// Stopped at its **last** write, the transaction still holds and marks
/// everything: readers of its first key still wait (M2).
#[test]
fn readers_wait_until_after_the_last_write() {
    let store = KeyValueStore::new();
    store.execute(set("lw:a", "old"));
    let (waited, seen) = during_the_span(
        &store,
        vec![set("lw:a", "new"), get("lw:a"), set("lw:b", "new")],
        "lw:b",
        &["lw:a"],
        |store| store.execute(get("lw:a")),
    );
    assert!(
        waited,
        "a reader of the first key answered before the last write"
    );
    assert_eq!(seen, bulk("new"));
}

/// The repair after a panic runs with every stripe held and the queue's
/// still marked — in place — whenever the rest are free; and still repairs,
/// after releasing, when another holder has one (A1, A3).
#[test]
fn a_repair_runs_under_the_held_marks_or_after_a_contended_release() {
    for (label, store) in [
        ("uncapped", KeyValueStore::new()),
        (
            "capped",
            KeyValueStore::with_config(Some(1_000), None, EvictionPolicy::AllKeysLru),
        ),
    ] {
        let seen = Arc::new(std::sync::Mutex::new(None));
        let observer = {
            let (seen, probe) = (Arc::clone(&seen), store.clone());
            transaction_fault::observe_repair(&store, move |in_place| {
                let held = probe
                    .write_order
                    .iter()
                    .all(|stripe| stripe.lock.try_lock().is_err());
                *seen.lock().unwrap() = Some((in_place, held, multi_active(&probe, "rp:a")));
            })
        };
        store.execute(Command::RPush("rp:l".into(), vec![b"x".to_vec()]));
        let (_, applied) = panics_at(
            Fault::During,
            &store,
            &[set("rp:a", "1"), Command::LPop("rp:l".into(), None)],
            1,
            "rp:l",
        );
        drop(observer);
        assert!(applied.repaired, "{label}");
        assert_eq!(
            *seen.lock().unwrap(),
            Some((true, true, true)),
            "{label}: repaired in place, every stripe held, marks kept"
        );
        assert!(metadata_is_consistent(&store), "{label}");
        assert_eq!(
            store.execute(Command::Exists(vec!["rp:l".into()])),
            Value::Integer(0)
        );
    }
    // Contended: an outside holder of a stripe outside the queue's set.
    let store = KeyValueStore::new();
    let outside = apart_from(&["rc:a", "rc:l"], "rc:o");
    let stripe = write_pause::stripe_of(&outside);
    let held = store.write_order[stripe].lock.lock().unwrap();
    let seen = Arc::new(std::sync::Mutex::new(None));
    let observer = {
        let seen = Arc::clone(&seen);
        transaction_fault::observe_repair(&store, move |in_place| {
            *seen.lock().unwrap() = Some(in_place);
        })
    };
    store.execute(Command::RPush("rc:l".into(), vec![b"x".to_vec()]));
    let probe = store.clone();
    let tx = std::thread::spawn(move || {
        panics_at(
            Fault::During,
            &probe,
            &[set("rc:a", "1"), Command::LPop("rc:l".into(), None)],
            1,
            "rc:l",
        )
    });
    // The repair needs the held stripe: it waits for it, having released.
    std::thread::sleep(Duration::from_millis(100));
    assert!(!tx.is_finished());
    drop(held);
    let (_, applied) = tx.join().unwrap();
    drop(observer);
    assert!(applied.repaired);
    assert_eq!(*seen.lock().unwrap(), Some(false));
    assert!(metadata_is_consistent(&store));
    assert!(no_stripe_marked(&store) && no_stripe_held(&store));
}

/// The fault in a probed write (`SET` on an uncapped store, which does not
/// take the general path) is contained and repaired the same way (A7).
#[test]
fn a_panic_in_a_probed_write_is_contained_too() {
    let store = KeyValueStore::new();
    store.execute(set("pw:b", "old"));
    let probe = store.clone();
    let (_, applied) = finishes("probed", move || {
        panics_at(
            Fault::During,
            &probe,
            &[
                set("pw:a", "1"),
                set("pw:b", "a-longer-value"),
                set("pw:c", "1"),
            ],
            1,
            "pw:b",
        )
    });
    assert_eq!(
        applied,
        TransactionApplied {
            panicked: Some(1),
            repaired: true
        }
    );
    assert_eq!(store.execute(get("pw:b")), bulk("a-longer-value"));
    assert_eq!(store.execute(get("pw:c")), Value::BulkString(None));
    assert!(metadata_is_consistent(&store));
    assert!(no_stripe_marked(&store) && no_stripe_held(&store));
}
