//! Capped-store reservations (6L-B) against hard-cap invariants.
//!
//! A reserved write runs without holding the capacity gate exclusively, so
//! each test races writers and checks what the gate used to guarantee by
//! construction:
//! - the key cap and the memory cap are never exceeded in a settled state
//! - the memory counter equals an independent recomputation
//! - the key counter equals the map
//! - no reservation outlives its write
//!
//! "Settled" means sampled under the gate held exclusively: no reserved or
//! gated write is in flight then. Each test also proves the fast path ran.

use super::*;
use crate::cmd::{SetExpiry, SetOptions};
use rand::{Rng, SeedableRng};
use std::sync::Barrier;
use std::sync::atomic::AtomicBool;

const THREADS: usize = 8;

fn reserved_store(
    max_keys: Option<usize>,
    max_memory: Option<usize>,
    policy: EvictionPolicy,
) -> Arc<KeyValueStore> {
    let store = KeyValueStore::with_config(max_keys, max_memory, policy);
    store.set_reserved_writes(true);
    Arc::new(store)
}

fn set(key: &str, value: &str) -> Command {
    Command::Set(key.into(), value.into(), SetOptions::default())
}

fn recomputed_memory(store: &KeyValueStore) -> usize {
    store
        .data
        .iter()
        .map(|e| entry_size(e.key(), e.value()))
        .sum()
}

/// Every counter agrees with the map, and nothing is still reserved.
fn assert_settled(store: &KeyValueStore, what: &str) {
    let _gate = store
        .capacity_gate
        .write()
        .unwrap_or_else(|p| p.into_inner());
    assert_eq!(
        store.reserved_bytes.load(Ordering::SeqCst),
        0,
        "{what}: bytes still reserved"
    );
    assert_eq!(
        store.reserved_keys.load(Ordering::SeqCst),
        0,
        "{what}: keys still reserved"
    );
    assert_eq!(
        store.key_count(),
        store.data.len(),
        "{what}: key count vs map"
    );
    assert_eq!(
        store.approximate_memory_bytes(),
        recomputed_memory(store),
        "{what}: memory counter vs recomputation"
    );
    if let Some(max) = store.max_keys {
        assert!(
            store.data.len() <= max,
            "{what}: {} keys over a cap of {max}",
            store.data.len()
        );
    }
    if let Some(limit) = store.max_memory_bytes {
        let memory = store.approximate_memory_bytes();
        assert!(
            memory <= limit,
            "{what}: {memory} bytes over a cap of {limit}"
        );
    }
}

/// Run `work(thread, store)` on `THREADS` threads released together, while
/// a watcher samples settled states. Returns the summed fast-path outcomes
/// (hits, fallbacks) and how many settled samples were taken.
fn race(
    store: &Arc<KeyValueStore>,
    work: impl Fn(usize, &KeyValueStore) + Send + Sync + 'static,
) -> ((u64, u64), usize) {
    let work = Arc::new(work);
    let start = Arc::new(Barrier::new(THREADS + 1));
    let done = Arc::new(AtomicBool::new(false));
    let watcher = {
        let (store, done) = (Arc::clone(store), Arc::clone(&done));
        std::thread::spawn(move || {
            let mut samples = 0usize;
            while !done.load(Ordering::Acquire) {
                {
                    let _gate = store
                        .capacity_gate
                        .write()
                        .unwrap_or_else(|p| p.into_inner());
                    // Exclusive: no reservation may be outstanding.
                    assert_eq!(store.reserved_bytes.load(Ordering::SeqCst), 0);
                    assert_eq!(store.reserved_keys.load(Ordering::SeqCst), 0);
                    if let Some(max) = store.max_keys {
                        assert!(
                            store.key_count() <= max,
                            "{} keys over {max}",
                            store.key_count()
                        );
                    }
                    if let Some(limit) = store.max_memory_bytes {
                        let memory = store.approximate_memory_bytes();
                        assert!(memory <= limit, "{memory} bytes over {limit}");
                    }
                }
                samples += 1;
                std::thread::yield_now();
            }
            samples
        })
    };
    let workers: Vec<_> = (0..THREADS)
        .map(|t| {
            let (store, start, work) = (Arc::clone(store), Arc::clone(&start), Arc::clone(&work));
            std::thread::spawn(move || {
                RESERVED_OUTCOMES.with(|o| o.set((0, 0)));
                start.wait();
                work(t, &store);
                RESERVED_OUTCOMES.with(|o| o.get())
            })
        })
        .collect();
    start.wait();
    let outcomes = workers
        .into_iter()
        .map(|w| w.join().expect("worker"))
        .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    done.store(true, Ordering::Release);
    (outcomes, watcher.join().expect("watcher"))
}

#[test]
fn eight_threads_racing_to_max_keys_admit_exactly_the_cap() {
    const MAX: usize = 1_000;
    let store = reserved_store(Some(MAX), None, EvictionPolicy::NoEviction);
    let admitted = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&admitted);
    let ((hits, fallbacks), samples) = race(&store, move |t, store| {
        for i in 0..250 {
            if store.execute(set(&format!("t{t}:{i}"), "v")) == Value::SimpleString("OK".into()) {
                counted.fetch_add(1, Ordering::Relaxed);
            }
        }
    });
    assert_eq!(
        admitted.load(Ordering::Relaxed),
        MAX,
        "exactly the cap is admitted"
    );
    assert!(
        hits > 0 && fallbacks > 0,
        "both paths ran: {hits} fast, {fallbacks} fell back"
    );
    assert!(samples > 0);
    assert_settled(&store, "max_keys race");
    assert_eq!(store.data.len(), MAX);
}

#[test]
fn eight_threads_racing_to_max_memory_stay_under_it() {
    for policy in [EvictionPolicy::NoEviction, EvictionPolicy::AllKeysLru] {
        let limit = 64 * 1024;
        let store = reserved_store(None, Some(limit), policy);
        let ((hits, fallbacks), _) = race(&store, move |t, store| {
            for i in 0..400 {
                store.execute(set(&format!("t{t}:{i}"), &"x".repeat(64)));
            }
        });
        assert!(
            hits > 0 && fallbacks > 0,
            "{policy:?}: {hits} fast, {fallbacks} fell back"
        );
        assert_settled(&store, &format!("max_memory race, {policy:?}"));
        if policy == EvictionPolicy::AllKeysLru {
            assert!(store.evicted_count() > 0, "the race reached the cap");
        }
    }
}

#[test]
fn concurrent_large_value_replacements_keep_memory_exact() {
    let limit = 256 * 1024;
    let store = reserved_store(Some(400), Some(limit), EvictionPolicy::AllKeysLru);
    let ((hits, _), _) = race(&store, move |t, store| {
        let mut rng = rand::rngs::StdRng::seed_from_u64(t as u64);
        for _ in 0..2_000 {
            let key = format!("k{}", rng.random_range(0..64));
            let len = rng.random_range(0..4_096);
            store.execute(set(&key, &"y".repeat(len)));
            if rng.random_ratio(1, 8) {
                store.execute(Command::Append(
                    key,
                    "z".repeat(rng.random_range(0..512)).into_bytes(),
                ));
            }
        }
    });
    assert!(hits > 0);
    assert_settled(&store, "large replacements");
}

#[test]
fn near_cap_eviction_with_reservations_in_flight() {
    // Start just under the eviction threshold: reserved writes and evicting
    // gated writes interleave on every round.
    let store = reserved_store(Some(500), None, EvictionPolicy::AllKeysRandom);
    for i in 0..495 {
        store.execute(set(&format!("seed{i}"), "v"));
    }
    let ((hits, fallbacks), _) = race(&store, move |t, store| {
        for i in 0..300 {
            // Overwrites fit without a new key; creations need a slot.
            store.execute(set(&format!("seed{}", (t * 37 + i) % 495), "w"));
            store.execute(set(&format!("new{t}:{i}"), "v"));
            store.execute(Command::Incr(format!("ctr{t}")));
        }
    });
    assert!(
        hits > 0 && fallbacks > 0,
        "{hits} fast, {fallbacks} fell back"
    );
    assert!(store.evicted_count() > 0);
    assert_settled(&store, "near-cap eviction");
    assert_eq!(store.data.len(), 500);
}

#[test]
fn noeviction_refuses_at_the_cap_and_still_admits_what_fits() {
    let store = reserved_store(Some(10), None, EvictionPolicy::NoEviction);
    for i in 0..10 {
        assert_eq!(
            store.execute(set(&format!("k{i}"), "v")),
            Value::SimpleString("OK".into())
        );
    }
    // A new key is refused (fast path falls back; the gated path refuses).
    assert!(matches!(store.execute(set("extra", "v")), Value::Error(_)));
    // An overwrite and an INCR of an existing key need no slot: fast path.
    RESERVED_OUTCOMES.with(|o| o.set((0, 0)));
    assert_eq!(
        store.execute(set("k0", "w")),
        Value::SimpleString("OK".into())
    );
    store.execute(set("k1", "1"));
    assert_eq!(store.execute(Command::Incr("k1".into())), Value::Integer(2));
    assert_eq!(
        RESERVED_OUTCOMES.with(|o| o.get()).0,
        3,
        "existing keys use the fast path"
    );
    assert_settled(&store, "noeviction");
}

#[test]
fn expiry_sweeps_during_reserved_writes_keep_the_counts() {
    let store = reserved_store(Some(2_000), Some(512 * 1024), EvictionPolicy::VolatileLru);
    let sweeping = Arc::new(AtomicBool::new(true));
    let sweeper = {
        let (store, sweeping) = (Arc::clone(&store), Arc::clone(&sweeping));
        std::thread::spawn(move || {
            let mut swept = 0;
            while sweeping.load(Ordering::Acquire) {
                swept += store.sweep_expired_reporting().len();
            }
            swept
        })
    };
    let ((hits, _), _) = race(&store, move |t, store| {
        for i in 0..1_500 {
            let opts = SetOptions {
                expiry: Some(SetExpiry::Px(1 + (i % 3) as u64)),
                ..Default::default()
            };
            store.execute(Command::Set(format!("e{t}:{}", i % 200), "v".into(), opts));
            store.execute(set(&format!("p{t}:{}", i % 50), "v"));
        }
    });
    sweeping.store(false, Ordering::Release);
    let swept = sweeper.join().expect("sweeper");
    assert!(hits > 0 && swept > 0, "{hits} fast writes, {swept} swept");
    assert_settled(&store, "expiry during reservations");
}

#[test]
fn snapshots_during_reserved_writes_are_within_the_caps() {
    const MAX: usize = 800;
    let store = reserved_store(Some(MAX), None, EvictionPolicy::AllKeysLru);
    let snapping = Arc::new(AtomicBool::new(true));
    let snapshotter = {
        let (store, snapping) = (Arc::clone(&store), Arc::clone(&snapping));
        std::thread::spawn(move || {
            let mut taken = 0;
            while snapping.load(Ordering::Acquire) {
                // As the server takes them, behind a barrier over every
                // write: `lock_all_writes` there, every write-order stripe
                // here, which reserved and gated writes alike hold. An
                // unbarriered `snapshot()` walks the shards while writes
                // land, and held 801 keys under a cap of 800 with
                // reservations off too: it was never atomic.
                let (snap, count) = {
                    let _all = store.order_all();
                    (store.snapshot(), store.key_count())
                };
                assert!(snap.len() <= MAX, "a snapshot held {} keys", snap.len());
                assert_eq!(snap.len(), count, "a barriered snapshot is the keyspace");
                taken += 1;
            }
            taken
        })
    };
    let ((hits, _), _) = race(&store, move |t, store| {
        for i in 0..400 {
            store.execute(set(&format!("s{t}:{i}"), "v"));
        }
    });
    snapping.store(false, Ordering::Release);
    assert!(snapshotter.join().expect("snapshots") > 0 && hits > 0);
    assert_settled(&store, "snapshots");
    let snap = store.snapshot();
    assert_eq!(snap.len(), store.data.len());
}

#[test]
fn a_failed_reserved_write_releases_its_reservation_and_changes_nothing() {
    let store = reserved_store(Some(10), Some(64 * 1024), EvictionPolicy::NoEviction);
    store.execute(Command::HSet("h".into(), vec![("f".into(), b"v".to_vec())]));
    let memory = store.approximate_memory_bytes();
    RESERVED_OUTCOMES.with(|o| o.set((0, 0)));
    // INCR of a hash: qualifies for the fast path, fails WRONGTYPE.
    assert!(
        matches!(store.execute(Command::Incr("h".into())), Value::Error(e) if e.contains("WRONGTYPE"))
    );
    assert!(matches!(
        store.execute(Command::Append("h".into(), "x".into())),
        Value::Error(_)
    ));
    assert_eq!(
        RESERVED_OUTCOMES.with(|o| o.get()).0,
        2,
        "both took the fast path"
    );
    assert_eq!(store.approximate_memory_bytes(), memory);
    assert_settled(&store, "failed command");
}

#[test]
fn a_panicking_reserved_write_releases_its_reservation() {
    let store = reserved_store(Some(10), Some(64 * 1024), EvictionPolicy::NoEviction);
    let cmd = set("p", &"v".repeat(100));
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = store.try_reserved_write(
            &cmd,
            |_store: &KeyValueStore, _cmd: &Command, _probe| -> Value {
                panic!("command panicked mid-write")
            },
        );
    }));
    assert!(panicked.is_err());
    assert_eq!(
        store.reserved_bytes.load(Ordering::SeqCst),
        0,
        "bytes released on unwind"
    );
    assert_eq!(
        store.reserved_keys.load(Ordering::SeqCst),
        0,
        "key slot released on unwind"
    );
    // The key's stripe recovers from the poison; the store keeps working.
    assert_eq!(
        store.execute(set("p", "after")),
        Value::SimpleString("OK".into())
    );
    assert_settled(&store, "after a panic");
}

#[test]
fn a_follower_never_reserves() {
    // A replica applies whatever its primary sends: caps are not enforced,
    // so there is nothing to reserve against.
    let store = reserved_store(Some(2), None, EvictionPolicy::NoEviction);
    store.set_follower(true);
    RESERVED_OUTCOMES.with(|o| o.set((0, 0)));
    for i in 0..5 {
        assert_eq!(
            store.execute(set(&format!("k{i}"), "v")),
            Value::SimpleString("OK".into())
        );
    }
    assert_eq!(RESERVED_OUTCOMES.with(|o| o.get()), (0, 0));
    assert_eq!(store.data.len(), 5);
}

#[test]
fn reserved_and_gated_stores_end_in_the_same_state() {
    // Same single-threaded workload, fast path on and off: identical
    // contents and accounting. Reservations change only who waits.
    let run = |reserved: bool| {
        let store =
            KeyValueStore::with_config(Some(300), Some(48 * 1024), EvictionPolicy::NoEviction);
        store.set_reserved_writes(reserved);
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let mut replies = Vec::new();
        for _ in 0..5_000 {
            let key = format!("k{}", rng.random_range(0..400));
            let cmd = match rng.random_range(0..5) {
                0 => set(&key, &"v".repeat(rng.random_range(0..300))),
                1 => Command::Incr(key),
                2 => Command::Append(key, "a".repeat(rng.random_range(0..64)).into_bytes()),
                3 => Command::HSet(
                    key,
                    vec![("f".into(), "v".repeat(rng.random_range(0..64)).into_bytes())],
                ),
                _ => Command::Del(vec![key]),
            };
            replies.push(store.execute(cmd));
        }
        let mut snap: Vec<(String, Value)> = store
            .data
            .iter()
            .map(|e| e.key().to_string())
            .collect::<Vec<_>>()
            .into_iter()
            .map(|k| {
                let v = store.get_current(&k);
                (k, v)
            })
            .collect();
        snap.sort_by(|a, b| a.0.cmp(&b.0));
        (
            replies,
            snap,
            store.approximate_memory_bytes(),
            store.key_count(),
        )
    };
    assert_eq!(run(true), run(false));
}

/// Gated vs reserved capped writes, in process (6L-B). Run with
/// `cargo test --release -p core-engine --lib bench_reserved_writes -- --ignored --nocapture`.
///
/// Prints CSV: workload, threads, gated and reserved ops/s and sampled p99
/// latency (every 32nd op), and the reserved run's fallback rate. Median of
/// `REPS` interleaved repetitions. Indicative only on a shared host.
#[test]
#[ignore = "benchmark; run explicitly with --ignored --nocapture"]
fn bench_reserved_writes() {
    const REPS: usize = 5;
    const OPS: usize = 400_000;
    println!(
        "workload,threads,gated_ops_s,reserved_ops_s,speedup,gated_p99_ns,reserved_p99_ns,fallback_pct"
    );
    let only = std::env::var("BENCH_WORKLOADS").ok();
    for workload in ["new_keys", "overwrite", "incr", "new_keys_uncapped"] {
        if only
            .as_deref()
            .is_some_and(|w| !w.split(',').any(|x| x == workload))
        {
            continue;
        }
        for &threads in &[1usize, 2, 4, 8] {
            let mut rows: [Vec<(f64, u64, f64)>; 2] = [Vec::new(), Vec::new()];
            for rep in 0..REPS {
                for (slot, reserved) in [(0usize, false), (1, true)] {
                    // Caps far above the workload: every write fits, so this
                    // measures the admission path, not eviction.
                    // `new_keys_uncapped`: the same creations on a store with
                    // no caps at all, the ceiling reservations can reach.
                    let (keys_cap, memory_cap) = if workload == "new_keys_uncapped" {
                        (None, None)
                    } else {
                        (Some(50_000_000), Some(1 << 34))
                    };
                    let store = KeyValueStore::with_config(
                        keys_cap,
                        memory_cap,
                        EvictionPolicy::AllKeysLru,
                    );
                    store.set_reserved_writes(reserved);
                    for i in 0..200_000 {
                        store.execute(set(&format!("bg:{i}"), "v"));
                    }
                    for t in 0..threads {
                        for i in 0..1_024 {
                            store.execute(set(&format!("hot:{t}:{i}"), "1"));
                        }
                    }
                    let per_thread = OPS / threads;
                    let start = Barrier::new(threads);
                    let t0 = std::time::Instant::now();
                    let results: Vec<(Vec<u64>, (u64, u64))> = std::thread::scope(|scope| {
                        let handles: Vec<_> = (0..threads)
                            .map(|t| {
                                let (store, start) = (&store, &start);
                                scope.spawn(move || {
                                    RESERVED_OUTCOMES.with(|o| o.set((0, 0)));
                                    let mut lat = Vec::with_capacity(per_thread / 32 + 1);
                                    start.wait();
                                    for i in 0..per_thread {
                                        let cmd = match workload {
                                            "new_keys" | "new_keys_uncapped" => set(
                                                &format!("r{rep}:{reserved}:t{t}:{i}"),
                                                "value-16-bytes!!",
                                            ),
                                            "overwrite" => set(
                                                &format!("hot:{t}:{}", i % 1_024),
                                                "value-16-bytes!!",
                                            ),
                                            _ => Command::Incr(format!("hot:{t}:{}", i % 1_024)),
                                        };
                                        if i % 32 == 0 {
                                            let at = std::time::Instant::now();
                                            store.execute(cmd);
                                            lat.push(at.elapsed().as_nanos() as u64);
                                        } else {
                                            store.execute(cmd);
                                        }
                                    }
                                    (lat, RESERVED_OUTCOMES.with(|o| o.get()))
                                })
                            })
                            .collect();
                        handles
                            .into_iter()
                            .map(|h| h.join().expect("bench worker"))
                            .collect()
                    });
                    let rate = (per_thread * threads) as f64 / t0.elapsed().as_secs_f64();
                    let mut lat: Vec<u64> =
                        results.iter().flat_map(|r| r.0.iter().copied()).collect();
                    lat.sort_unstable();
                    let p99 = lat[lat.len() * 99 / 100];
                    let (hits, falls) = results
                        .iter()
                        .fold((0u64, 0u64), |a, r| (a.0 + (r.1).0, a.1 + (r.1).1));
                    let fallback = if hits + falls == 0 {
                        0.0
                    } else {
                        falls as f64 * 100.0 / (hits + falls) as f64
                    };
                    rows[slot].push((rate, p99, fallback));
                }
            }
            let median = |v: &mut Vec<(f64, u64, f64)>| {
                v.sort_by(|a, b| a.0.total_cmp(&b.0));
                v[v.len() / 2]
            };
            let (g, r) = (median(&mut rows[0].clone()), median(&mut rows[1].clone()));
            println!(
                "{workload},{threads},{:.0},{:.0},{:.2},{},{},{:.2}",
                g.0,
                r.0,
                r.0 / g.0,
                g.1,
                r.1,
                r.2
            );
        }
    }
}
