//! Review A probes for 6O-G (`apply_transaction`). Reviewer-owned; not part
//! of the reviewed commit.

use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const WITHIN: Duration = Duration::from_secs(20);

fn set(key: &str, value: &str) -> Command {
    Command::Set(key.into(), value.as_bytes().to_vec(), SetOptions::default())
}

fn get(key: &str) -> Command {
    Command::Get(key.into())
}

fn finishes<T: Send + 'static>(what: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(WITHIN)
        .unwrap_or_else(|_| panic!("{what} did not finish: deadlock"))
}

fn apply(store: &KeyValueStore, queue: &[Command]) -> (Vec<Value>, TransactionApplied) {
    let mut replies = Vec::new();
    let applied = store.apply_transaction(queue, clock_ms(), |_, cmd| {
        replies.push(store.execute_reporting_ref(cmd).0);
    });
    (replies, applied)
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

fn sharing_stripe_with(key: &str, prefix: &str) -> String {
    let stripe = write_order_stripe(key);
    (0..)
        .map(|i| format!("{prefix}{i}"))
        .find(|other| other != key && write_order_stripe(other) == stripe)
        .unwrap()
}

/// Stop `queue` after its write of `stop_at`, run `outside` on another
/// thread; report whether it was still not done after `grace` (it waited)
/// and what it returned once the transaction finished.
fn waits_for_span<T: Send + 'static>(
    store: &KeyValueStore,
    queue: Vec<Command>,
    stop_at: &str,
    outside: impl FnOnce(&KeyValueStore) -> T + Send + 'static,
) -> (bool, T) {
    let pause = write_pause::after_transaction_write_of(store, stop_at, &[]);
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
    let deadline = Instant::now() + Duration::from_millis(300);
    while Instant::now() < deadline && !done.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let waited = !done.load(Ordering::SeqCst);
    drop(pause);
    let out = out_rx.recv_timeout(WITHIN).expect("outside op finished");
    tx.join().unwrap();
    assert!(no_stripe_marked(store) && no_stripe_held(store));
    (waited, out)
}

// ── R-A1: stripe-set completeness over every keyed write kind ───────────────

/// Every write kind `mutation_scope` names, interleaved with in-scope reads,
/// on every store kind. The `debug_assert!` in `order_keys`/`order_all`
/// fails on a write outside the transaction's stripe set; `finishes` fails on
/// a self-deadlock (a stripe or the gate taken again).
#[test]
fn review_a_every_write_kind_stays_inside_the_stripe_set() {
    let stores: Vec<(&str, KeyValueStore)> = vec![
        ("uncapped", KeyValueStore::new()),
        (
            "capped-evict",
            KeyValueStore::with_config(Some(1_000), Some(1 << 24), EvictionPolicy::AllKeysLru),
        ),
        ("capped-refuse+reserved", {
            let s =
                KeyValueStore::with_config(Some(1_000), Some(1 << 24), EvictionPolicy::NoEviction);
            s.set_reserved_writes(true);
            s
        }),
        ("follower-with-caps", {
            let s = KeyValueStore::with_config(Some(3), Some(1 << 24), EvictionPolicy::AllKeysLru);
            s.set_follower(true);
            s
        }),
    ];
    for (label, store) in stores {
        let k = |s: &str| format!("wk:{label}:{s}");
        store.execute(Command::SAdd(k("s1"), vec!["a".into(), "b".into()]));
        store.execute(Command::SAdd(k("s2"), vec!["b".into(), "c".into()]));
        store.execute(Command::RPush(k("l"), vec![b"1".to_vec(), b"2".to_vec()]));
        store.execute(Command::HSet(k("h"), vec![("f".into(), b"v".to_vec())]));
        let queue = vec![
            set(&k("a"), "1"),
            Command::GetSet(k("a"), b"2".to_vec()),
            Command::SetNx(k("n"), b"x".to_vec()),
            Command::SetEx(k("e"), 100, b"x".to_vec()),
            Command::PSetEx(k("pe"), 100_000, b"x".to_vec()),
            Command::Incr(k("i")),
            Command::DecrBy(k("i"), 3),
            Command::Expire(k("a"), 100),
            Command::Persist(k("a")),
            Command::PExpireAt(k("n"), clock_ms() + 100_000),
            Command::HSet(k("h"), vec![("g".into(), b"w".to_vec())]),
            Command::HIncrBy(k("h"), "c".into(), 2),
            Command::HSetNx(k("h"), "z".into(), b"z".to_vec()),
            Command::HDel(k("h"), vec!["f".into(), "g".into(), "c".into(), "z".into()]),
            Command::LPush(k("l"), vec![b"0".to_vec()]),
            Command::LSet(k("l"), 0, b"zero".to_vec()),
            Command::LTrim(k("l"), 0, 1),
            Command::LRem(k("l"), 0, b"zero".to_vec()),
            Command::RPop(k("l"), Some(5)),
            Command::SAdd(k("s3"), vec!["q".into()]),
            Command::SMove(k("s1"), k("s3"), "a".into()),
            Command::SInterStore(k("si"), vec![k("s1"), k("s2")]),
            Command::SDiffStore(k("sd"), vec![k("s2"), k("s1")]),
            Command::SUnionStore(k("su"), vec![k("s1"), k("s2"), k("s3")]),
            Command::SRem(k("s3"), vec!["q".into(), "a".into()]),
            Command::SPop(k("su"), Some(1)),
            Command::ZAdd(k("z"), ZAddOptions::default(), vec![(1.0, "m".into())]),
            Command::ZIncrBy(k("z"), 2.0, "m".into()),
            Command::ZPopMin(k("z"), None),
            Command::Copy(k("a"), k("acopy"), false),
            Command::RenameNx(k("acopy"), k("arn")),
            Command::Rename(k("arn"), k("arn")),
            Command::GetDel(k("arn")),
            Command::GetEx(k("a"), Some(GetExOption::Px(50_000))),
            Command::ESet(k("eph"), b"v".to_vec()),
            Command::EAdd(k("eset"), vec!["m".into()]),
            Command::RlCheck(k("rl"), Some((10, 1_000))),
            Command::JSet(k("j"), "$".into(), "{\"a\":1}".into()),
            Command::Append(k("a"), b"x".to_vec()),
            Command::MSet(vec![(k("m1"), b"1".to_vec()), (k("m2"), b"2".to_vec())]),
            Command::MGet(vec![k("m1"), k("m2"), k("a")]),
            Command::Exists(vec![k("m1"), k("eph"), k("nope")]),
            Command::Del(vec![k("m1"), k("m2"), k("e")]),
            Command::Unlink(vec![k("pe")]),
            get(&k("a")),
        ];
        let probe = store.clone();
        let (replies, applied) = finishes(label, move || apply(&probe, &queue));
        assert_eq!(applied, TransactionApplied::default(), "{label}");
        assert!(
            !replies.iter().any(|r| matches!(r, Value::Error(e) if !e.contains("WRONGTYPE") && !e.contains("max keys"))),
            "{label}: {replies:?}"
        );
        assert!(no_stripe_marked(&store) && no_stripe_held(&store), "{label}");
    }
}

/// The borrowed write path inside the scope (the server's hot path).
#[test]
fn review_a_borrowed_writes_inside_the_scope() {
    let store = KeyValueStore::new();
    let probe = store.clone();
    let queue = vec![set("bw:a", "1"), get("bw:a"), set("bw:b", "2")];
    let out = finishes("borrowed", move || {
        let mut outs = Vec::new();
        probe.apply_transaction(&queue, clock_ms(), |_, cmd| {
            let mut out = Vec::new();
            let r = match cmd {
                Command::Set(k, v, _) => probe.execute_ref_into(&CommandRef::Set(k, v), &mut out),
                Command::Get(k) => probe.execute_ref_into(&CommandRef::Get(k), &mut out),
                _ => None,
            };
            assert!(r.is_some());
            outs.push(out);
        });
        outs
    });
    assert_eq!(out[1], b"$1\r\n1\r\n");
    assert!(no_stripe_marked(&store) && no_stripe_held(&store));
}

// ── R-A2: capped stores hold every stripe and the gate (test gaps) ─────────

/// A capped transaction's eviction victim is outside the queue's keys: its
/// removal must not be visible before the span ends. Kills mutant M9
/// (`if all || capped` → `if all`), which the shipped suite does not.
#[test]
fn review_a_capped_eviction_victim_is_not_seen_gone_inside_the_span() {
    let store = KeyValueStore::with_config(Some(2), None, EvictionPolicy::AllKeysLru);
    // Victims on stripes apart from the queue's keys.
    let v1 = apart_from(&["ev:a", "ev:b"], "ev:v1-");
    let v2 = apart_from(&["ev:a", "ev:b", &v1], "ev:v2-");
    store.execute(set(&v1, "old"));
    store.execute(set(&v2, "old"));
    let (v1c, v2c) = (v1.clone(), v2.clone());
    let (waited, seen) = waits_for_span(
        &store,
        vec![set("ev:a", "new"), set("ev:b", "new")],
        "ev:a",
        move |store| store.execute(Command::Exists(vec![v1c, v2c])),
    );
    assert!(
        waited,
        "an outside reader saw the eviction inside the span: {seen:?}"
    );
    assert_eq!(seen, Value::Integer(0), "both victims evicted by the end");
}

/// The eviction tick (gate, no stripes) must not run inside a capped span.
/// Kills mutant M8 (no gate taken by `hold_transaction`), which the shipped
/// suite does not.
#[test]
fn review_a_eviction_tick_waits_for_a_capped_span() {
    let store = KeyValueStore::with_config(Some(1_000), Some(1 << 20), EvictionPolicy::AllKeysLru);
    let (waited, _) = waits_for_span(
        &store,
        vec![set("et:a", "new"), set("et:b", "new")],
        "et:a",
        |store| store.try_evict_for_memory_reporting(),
    );
    assert!(waited, "the eviction tick ran inside a capped span");
    // The sweep returns before its gate when nothing is volatile.
    store.execute(Command::Set(
        "et:ttl".into(),
        b"x".to_vec(),
        SetOptions {
            expiry: Some(SetExpiry::Px(1)),
            ..SetOptions::default()
        },
    ));
    std::thread::sleep(Duration::from_millis(5));
    let (waited, _) = waits_for_span(
        &store,
        vec![set("et:c", "new"), set("et:d", "new")],
        "et:c",
        |store| store.sweep_expired_reporting_budget(16),
    );
    assert!(waited, "a capped sweep ran inside a capped span");
}

// ── R-A3: repair before release on a capped store ───────────────────────────

/// On a capped store the repair runs before anything is released: a reader
/// waiting on the span must see the repaired state (the emptied list
/// removed). Probabilistic against mutant M5 (release, then repair).
#[test]
fn review_a_capped_reader_never_sees_an_unrepaired_panic() {
    let store = KeyValueStore::with_config(Some(1_000_000), None, EvictionPolicy::AllKeysLru);
    for i in 0..50_000 {
        store.execute(set(&format!("rp:fill{i}"), "v"));
    }
    store.execute(Command::RPush("rp:list".into(), vec![b"only".to_vec()]));
    let pause = write_pause::after_transaction_write_of(&store, "rp:a", &["rp:list"]);
    let tx_store = store.clone();
    let tx = std::thread::spawn(move || {
        let _armed = transaction_fault::panic_during_mutation_of("rp:list");
        apply(
            &tx_store,
            &[
                set("rp:a", "new"),
                Command::LPop("rp:list".into(), None),
                set("rp:c", "new"),
            ],
        )
        .1
    });
    assert!(pause.wait_reached(WITHIN));
    let readers: Vec<_> = (0..4)
        .map(|_| {
            let store = store.clone();
            std::thread::spawn(move || {
                (
                    store.execute(Command::Exists(vec!["rp:list".into()])),
                    store.execute(Command::Type("rp:list".into())),
                )
            })
        })
        .collect();
    std::thread::sleep(Duration::from_millis(200));
    drop(pause);
    let applied = tx.join().unwrap();
    assert_eq!(applied.panicked, Some(1));
    for r in readers {
        let (exists, _ty) = r.join().unwrap();
        assert_eq!(
            exists,
            Value::Integer(0),
            "a reader saw the part-applied, unrepaired LPOP (empty list present)"
        );
    }
}

// ── R-A4: all-or-none under load ────────────────────────────────────────────

/// Transactions write `a` then `b` (different stripes) to the same counter;
/// readers on other threads must never see new-then-old: GET a then GET b
/// (b >= a), GET b then GET a (a >= b), MGET (equal), borrowed GETs.
/// Unrelated writers share the stripes. Uncapped and capped.
#[test]
fn review_a_sequential_reads_never_see_new_then_old_under_load() {
    for capped in [false, true] {
        let store = if capped {
            KeyValueStore::with_config(Some(10_000), Some(1 << 26), EvictionPolicy::AllKeysLru)
        } else {
            KeyValueStore::new()
        };
        let a = "ld:a".to_string();
        let b = apart_from(&["ld:a"], "ld:b");
        let mid = apart_from(&["ld:a", &b], "ld:m");
        store.execute(set(&a, "0"));
        store.execute(set(&b, "0"));
        let stop = Arc::new(AtomicBool::new(false));
        let violations = Arc::new(AtomicU64::new(0));
        let reads = Arc::new(AtomicU64::new(0));
        let mut threads = Vec::new();
        {
            let (store, stop, a, b, mid) =
                (store.clone(), Arc::clone(&stop), a.clone(), b.clone(), mid.clone());
            threads.push(std::thread::spawn(move || {
                let mut i = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    i += 1;
                    let v = i.to_string();
                    apply(
                        &store,
                        &[
                            set(&a, &v),
                            get(&a),
                            set(&mid, &v),
                            Command::MGet(vec![a.clone(), mid.clone()]),
                            set(&b, &v),
                        ],
                    );
                }
            }));
        }
        let num = |v: Value| -> u64 {
            match v {
                Value::BulkString(Some(b)) => String::from_utf8(b).unwrap().parse().unwrap(),
                other => panic!("{other:?}"),
            }
        };
        for mode in 0..4u8 {
            let (store, stop, a, b, violations, reads) = (
                store.clone(),
                Arc::clone(&stop),
                a.clone(),
                b.clone(),
                Arc::clone(&violations),
                Arc::clone(&reads),
            );
            threads.push(std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let bad = match mode {
                        0 => {
                            let x = num(store.execute(get(&a)));
                            let y = num(store.execute(get(&b)));
                            y < x
                        }
                        1 => {
                            let y = num(store.execute(get(&b)));
                            let x = num(store.execute(get(&a)));
                            x < y
                        }
                        2 => match store.execute(Command::MGet(vec![a.clone(), b.clone()])) {
                            Value::Array(Some(v)) => {
                                let mut it = v.into_iter();
                                num(it.next().unwrap()) != num(it.next().unwrap())
                            }
                            other => panic!("{other:?}"),
                        },
                        _ => {
                            let read = |k: &str| {
                                let mut out = Vec::new();
                                store.execute_ref_into(&CommandRef::Get(k), &mut out);
                                let s = String::from_utf8(out).unwrap();
                                s.lines().nth(1).unwrap().parse::<u64>().unwrap()
                            };
                            let x = read(&a);
                            let y = read(&b);
                            y < x
                        }
                    };
                    if bad {
                        violations.fetch_add(1, Ordering::Relaxed);
                    }
                    reads.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }
        // Unrelated writers sharing the transaction's stripes.
        for key in [sharing_stripe_with(&a, "ld:sa"), sharing_stripe_with(&b, "ld:sb")] {
            let (store, stop) = (store.clone(), Arc::clone(&stop));
            threads.push(std::thread::spawn(move || {
                let mut i = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    i += 1;
                    store.execute(set(&key, &i.to_string()));
                    store.execute(Command::MSet(vec![
                        (key.clone(), b"m".to_vec()),
                        (format!("{key}-x"), b"m".to_vec()),
                    ]));
                }
            }));
        }
        std::thread::sleep(Duration::from_millis(1_500));
        stop.store(true, Ordering::Relaxed);
        finishes("load", move || {
            for t in threads {
                t.join().unwrap();
            }
        });
        assert_eq!(
            violations.load(Ordering::Relaxed),
            0,
            "capped={capped}: new-then-old over {} reads",
            reads.load(Ordering::Relaxed)
        );
        assert!(no_stripe_marked(&store) && no_stripe_held(&store));
    }
}

// ── R-A5: deadlock freedom with restore/replace, large reads, sweeps, panics

#[test]
fn review_a_no_deadlock_with_replace_large_reads_sweeps_and_repairs() {
    let store = KeyValueStore::new();
    let stop = Arc::new(AtomicBool::new(false));
    let mut threads = Vec::new();
    for t in 0..3u64 {
        let (store, stop) = (store.clone(), Arc::clone(&stop));
        threads.push(std::thread::spawn(move || {
            let mut n = 0u64;
            while !stop.load(Ordering::Relaxed) {
                n += 1;
                let k = |i: u64| format!("dl:{}", (n * 31 + i * 17 + t) % 300);
                let queue = vec![
                    set(&k(0), "v"),
                    Command::MGet((0..40).map(k).collect()),
                    Command::RPush(k(1), vec![b"x".to_vec()]),
                    Command::Set(
                        k(2),
                        b"w".to_vec(),
                        SetOptions {
                            expiry: Some(SetExpiry::Px(1)),
                            ..SetOptions::default()
                        },
                    ),
                    Command::LPop(k(1), None),
                    if n.is_multiple_of(50) {
                        Command::FlushDb
                    } else {
                        Command::MSet(vec![(k(3), b"x".to_vec()), (k(4), b"y".to_vec())])
                    },
                ];
                let panic_at = n.is_multiple_of(7).then_some((n % 5) as usize);
                store.apply_transaction(&queue, clock_ms(), |i, cmd| {
                    store.execute_reporting_ref(cmd);
                    if Some(i) == panic_at {
                        panic!("review-a injected panic");
                    }
                });
            }
            n
        }));
    }
    {
        let (store, stop) = (store.clone(), Arc::clone(&stop));
        threads.push(std::thread::spawn(move || {
            let mut n = 0u64;
            while !stop.load(Ordering::Relaxed) {
                n += 1;
                store.sweep_expired_reporting_budget(64);
                store.execute(Command::MGet((0..300).map(|i| format!("dl:{i}")).collect()));
                store.execute(Command::Exists((0..30).map(|i| format!("dl:{i}")).collect()));
                if n.is_multiple_of(20) {
                    let snap = store.snapshot();
                    store.replace(snap);
                }
            }
            n
        }));
    }
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    std::thread::sleep(Duration::from_millis(1_500));
    stop.store(true, Ordering::Relaxed);
    let counts = finishes("deadlock stress", move || {
        threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect::<Vec<_>>()
    });
    std::panic::set_hook(prev);
    assert!(counts.iter().all(|&n| n > 0), "{counts:?}");
    assert!(no_stripe_marked(&store) && no_stripe_held(&store));
    // Metadata exact once quiet.
    let (keys, memory) = (store.key_count(), store.approximate_memory_bytes());
    {
        let _all = store.order_all();
        store.rebuild_metadata();
    }
    assert_eq!((keys, memory), (store.key_count(), store.approximate_memory_bytes()));
}

// ── R-A6: uncapped repair runs after release (doc says before) ─────────────

/// `apply_transaction`'s doc and the commit message say the metadata is
/// rebuilt "before anything is released". On an uncapped store (subset
/// path) the stripes and marks are released first, then `order_all` waits
/// for every stripe: while any other party holds one, outside readers see
/// the part-applied command with no repair — here an empty list that
/// "does not exist" by the store's invariant. Deterministic: the test holds
/// one stripe outside the transaction's set.
#[test]
fn review_a_uncapped_repair_runs_after_release() {
    let store = KeyValueStore::new();
    store.execute(Command::RPush("ur:list".into(), vec![b"only".to_vec()]));
    let other = apart_from(&["ur:a", "ur:list", "ur:c"], "ur:o");
    let blocker = write_order_stripe(&other);
    let guard = store.write_order[blocker].lock.lock().unwrap();
    let tx_store = store.clone();
    let tx = std::thread::spawn(move || {
        let _armed = transaction_fault::panic_during_mutation_of("ur:list");
        apply(
            &tx_store,
            &[
                set("ur:a", "new"),
                Command::LPop("ur:list".into(), None),
                set("ur:c", "new"),
            ],
        )
        .1
    });
    // Wait until the transaction has released its own stripes (the list's
    // stripe is free again) while its repair waits for `blocker`.
    let list_stripe = write_order_stripe("ur:list");
    let deadline = Instant::now() + WITHIN;
    loop {
        let freed = store.write_order[list_stripe].multi.load(Ordering::Acquire) & MULTI_ACTIVE == 0
            && store.write_order[list_stripe].lock.try_lock().is_ok();
        if freed && !tx.is_finished() {
            break;
        }
        assert!(Instant::now() < deadline, "never reached the window");
        std::thread::sleep(Duration::from_millis(1));
    }
    std::thread::sleep(Duration::from_millis(50));
    // Outside readers now (a reader holds nothing; this one only reads).
    let exists = store.execute(Command::Exists(vec!["ur:list".into()]));
    let ty = store.execute(Command::Type("ur:list".into()));
    let llen = store.execute(Command::LLen("ur:list".into()));
    drop(guard);
    let applied = tx.join().unwrap();
    assert_eq!(applied.panicked, Some(1));
    // After the repair the empty list is gone.
    assert_eq!(store.execute(Command::Exists(vec!["ur:list".into()])), Value::Integer(0));
    // Report what was visible in the window.
    eprintln!("R-A6 window: EXISTS={exists:?} TYPE={ty:?} LLEN={llen:?}");
    assert_eq!(
        exists,
        Value::Integer(1),
        "expected the unrepaired empty list to be visible after release"
    );
}
