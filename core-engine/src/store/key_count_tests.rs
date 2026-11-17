//! The exact key counter (6K-C) against independent recomputation.
//!
//! `key_count()` is the key index's length, published on every change.
//! Capacity admission reads it under the capacity gate instead of counting
//! the map's 64 shards, so a hard `max_keys` cap now rests on it. These tests
//! recompute the count from the map after randomized single-threaded and
//! concurrent workloads covering every way keys appear and disappear, and
//! race creators to the cap watching that it is never exceeded.

use super::*;
use crate::cmd::SetOptions;
use rand::{Rng, SeedableRng};
use std::sync::Barrier;
use std::sync::atomic::AtomicBool;

/// The map's physical key count, and the store's three views of it.
fn assert_counts_agree(store: &KeyValueStore, what: &str) {
    let physical = store.data.len();
    let shards = store.index_all();
    for (i, shard) in shards.iter().enumerate() {
        for key in &shard.ordered {
            assert_eq!(
                index_shard(key),
                i,
                "{what}: {key} indexed in the wrong shard"
            );
        }
    }
    let indexed: usize = shards.iter().map(|shard| shard.ordered.len()).sum();
    drop(shards);
    assert_eq!(indexed, physical, "{what}: index vs map");
    assert_eq!(
        store.key_count(),
        physical,
        "{what}: published count vs map"
    );
    assert_eq!(
        store.execute(Command::DbSize),
        Value::Integer(physical as i64),
        "{what}: DBSIZE"
    );
    assert_eq!(
        store.keyspace_sample().keys,
        physical,
        "{what}: INFO keyspace"
    );
}

fn s(v: &str) -> Vec<u8> {
    v.as_bytes().to_vec()
}

/// A random command over a small key space, covering creation, overwrite,
/// deletion, expiry, multi-key, rename, copy and collection emptying.
fn random_command(rng: &mut rand::rngs::StdRng) -> Command {
    let k = |rng: &mut rand::rngs::StdRng| format!("k{}", rng.random_range(0..40));
    match rng.random_range(0..17) {
        0 => Command::Set(k(rng), s("v"), SetOptions::default()),
        1 => Command::Del(vec![k(rng), k(rng)]),
        2 => Command::PSetEx(k(rng), rng.random_range(1..4), s("t")),
        3 => Command::Incr(k(rng)),
        4 => Command::MSet(vec![(k(rng), s("a")), (k(rng), s("b"))]),
        5 => Command::Rename(k(rng), k(rng)),
        6 => Command::Copy(k(rng), k(rng), rng.random_bool(0.5)),
        7 => Command::HSet(k(rng), vec![("f".into(), s("1"))]),
        8 => Command::HDel(k(rng), vec!["f".into()]),
        9 => Command::RPush(k(rng), vec![s("x")]),
        10 => Command::LPop(k(rng), None),
        11 => Command::SAdd(k(rng), vec!["m".into()]),
        12 => Command::SRem(k(rng), vec!["m".into()]),
        13 => Command::Expire(k(rng), 0),
        14 => Command::Persist(k(rng)),
        15 => Command::GetDel(k(rng)),
        _ => Command::Append(k(rng), s("+")),
    }
}

#[test]
fn the_count_matches_the_map_after_random_workloads() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(0x6b_c0_57);
    for (round, policy) in [
        EvictionPolicy::NoEviction,
        EvictionPolicy::AllKeysRandom,
        EvictionPolicy::AllKeysLru,
        EvictionPolicy::VolatileLru,
    ]
    .into_iter()
    .enumerate()
    {
        for capped in [None, Some(25usize)] {
            let store = KeyValueStore::with_config(capped, None, policy);
            for step in 0..3_000 {
                store.execute(random_command(&mut rng));
                if step % 97 == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                    store.sweep_expired();
                }
                if step % 500 == 499 {
                    assert_counts_agree(&store, &format!("{policy:?} cap {capped:?} step {step}"));
                }
            }
            if let Some(max) = capped {
                assert!(store.data.len() <= max, "{policy:?}: cap exceeded");
            }
            assert_counts_agree(&store, &format!("round {round} {policy:?} {capped:?}"));
            store.execute(Command::FlushDb);
            assert_counts_agree(&store, "after FLUSHDB");
            assert_eq!(store.key_count(), 0);
        }
    }
}

#[test]
fn snapshot_restore_and_replace_publish_the_new_count() {
    let source = KeyValueStore::new();
    for i in 0..500 {
        source.execute(Command::Set(format!("k{i}"), s("v"), SetOptions::default()));
    }
    source.execute(Command::HSet("h".into(), vec![("f".into(), s("1"))]));
    let entries = source.snapshot();
    let target = KeyValueStore::new();
    for i in 0..50 {
        target.execute(Command::Set(
            format!("stale{i}"),
            s("v"),
            SetOptions::default(),
        ));
    }
    target.replace(entries);
    assert_counts_agree(&target, "after replace");
    assert_eq!(target.key_count(), 501);
    let restored = KeyValueStore::new();
    restored.restore(source.snapshot());
    assert_counts_agree(&restored, "after restore");
}

#[test]
fn a_key_expiring_during_a_rebuild_stays_indexed_and_counted() {
    // Regression: `rebuild_metadata` skipped entries already expired, so a
    // key that expired between a snapshot's load (which filters expired
    // entries) and the rebuild stayed in the map but not in the index: the
    // sweeper never found it and the counts disagreed with the map.
    let store = KeyValueStore::new();
    store.execute(Command::PSetEx("soon".into(), 1, s("v")));
    store.execute(Command::Set("kept".into(), s("v"), SetOptions::default()));
    std::thread::sleep(std::time::Duration::from_millis(5));
    // FLUSHDB-free rebuild of what is there now, with "soon" expired.
    store.rebuild_metadata();
    assert_counts_agree(&store, "rebuild with an expired entry");
    assert_eq!(
        store.volatile_key_count(),
        1,
        "the sweeper can still find it"
    );
    assert_eq!(store.sweep_expired_reporting(), vec!["soon".to_string()]);
    assert_counts_agree(&store, "after the sweep");
    assert_eq!(store.key_count(), 1);
}

#[test]
fn the_count_matches_the_map_after_concurrent_workloads() {
    for capped in [None, Some(30usize)] {
        let store = KeyValueStore::with_config(capped, None, EvictionPolicy::AllKeysRandom);
        let start = Barrier::new(8);
        std::thread::scope(|scope| {
            for t in 0..8u64 {
                let (store, start) = (store.clone(), &start);
                scope.spawn(move || {
                    let mut rng = rand::rngs::StdRng::seed_from_u64(0x6bc + t);
                    start.wait();
                    for step in 0..4_000 {
                        store.execute(random_command(&mut rng));
                        if t == 0 && step % 200 == 0 {
                            store.sweep_expired();
                        }
                    }
                });
            }
        });
        if let Some(max) = capped {
            assert!(
                store.data.len() <= max,
                "cap exceeded: {}",
                store.data.len()
            );
        }
        assert_counts_agree(&store, &format!("concurrent, cap {capped:?}"));
    }
}

#[test]
fn eight_creators_racing_to_max_keys_never_exceed_it() {
    // NoEviction: the cap refuses. Every successful SET is a key that exists
    // afterwards, and the map never holds more than the cap, observed
    // continuously by a ninth thread.
    const MAX: usize = 1_000;
    let store = KeyValueStore::with_max_keys(MAX);
    let start = Barrier::new(9);
    let done = AtomicBool::new(false);
    let accepted = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let observer = {
            let (store, start, done) = (store.clone(), &start, &done);
            scope.spawn(move || {
                start.wait();
                while !done.load(Ordering::Relaxed) {
                    let now = store.data.len();
                    assert!(now <= MAX, "the map held {now} keys, cap {MAX}");
                    assert!(store.key_count() <= MAX);
                }
            })
        };
        let creators: Vec<_> = (0..8usize)
            .map(|t| {
                let (store, start, accepted) = (store.clone(), &start, &accepted);
                scope.spawn(move || {
                    start.wait();
                    for i in 0..400 {
                        let reply = store.execute(Command::Set(
                            format!("t{t}:{i}"),
                            s("v"),
                            SetOptions::default(),
                        ));
                        match reply {
                            Value::SimpleString(_) => {
                                accepted.fetch_add(1, Ordering::Relaxed);
                            }
                            Value::Error(e) => assert!(e.contains("max keys"), "{e}"),
                            other => panic!("{other:?}"),
                        }
                    }
                })
            })
            .collect();
        // Join before re-raising anything, so the observer is always stopped
        // and a failing creator fails the test instead of hanging it.
        let results: Vec<_> = creators.into_iter().map(|h| h.join()).collect();
        done.store(true, Ordering::Relaxed);
        let observed = observer.join();
        for result in results.into_iter().chain(std::iter::once(observed)) {
            if let Err(panic) = result {
                std::panic::resume_unwind(panic);
            }
        }
    });
    assert_eq!(
        accepted.load(Ordering::Relaxed),
        MAX,
        "exactly the cap was admitted"
    );
    assert_counts_agree(&store, "after the race");
    assert_eq!(store.key_count(), MAX);
}

#[test]
fn eight_creators_racing_with_eviction_stay_at_the_cap() {
    const MAX: usize = 500;
    let store = KeyValueStore::with_config(Some(MAX), None, EvictionPolicy::AllKeysLru);
    let start = Barrier::new(8);
    std::thread::scope(|scope| {
        for t in 0..8usize {
            let (store, start) = (store.clone(), &start);
            scope.spawn(move || {
                start.wait();
                for i in 0..1_000 {
                    let key = format!("e{t}:{i}");
                    let reply = store.execute(Command::Set(key, s("v"), SetOptions::default()));
                    assert_eq!(reply, Value::SimpleString("OK".into()));
                    // `DashMap::len()` sums its shards one at a time, so read
                    // while another write evicts (removing the victim from
                    // one shard, inserting into another) it can count both
                    // and report one over. Read it where no capped write is
                    // in flight — under the capacity gate they all hold —
                    // and check the exact published count as well.
                    {
                        let _gate = store
                            .capacity_gate
                            .write()
                            .unwrap_or_else(|p| p.into_inner());
                        assert!(store.data.len() <= MAX, "map over the cap");
                    }
                    assert!(store.key_count() <= MAX, "count over the cap");
                }
            });
        }
    });
    assert_counts_agree(&store, "eviction race");
    assert_eq!(store.key_count(), MAX);
}

/// Capped-store write throughput with admission counting the map's shards
/// (pre-6K) vs the published count, same binary, same workload, interleaved.
/// Run with:
///
/// ```text
/// cargo test --release -p core-engine --lib -- --ignored \
///     bench_capped_admission --nocapture
/// ```
///
/// Threads SET fresh keys into a store capped far above what they write, so
/// every write is admitted on the fast path and the difference is the count.
#[test]
#[ignore = "benchmark; run explicitly in release"]
fn bench_capped_admission() {
    println!("threads,map_len_ops_per_s,counter_ops_per_s,speedup");
    for &threads in &[1usize, 2, 4, 8] {
        let mut map_rates = Vec::new();
        let mut counter_rates = Vec::new();
        for rep in 0..5 {
            for from_map in [true, false] {
                ADMISSION_COUNTS_THE_MAP.store(from_map, Ordering::Relaxed);
                let store =
                    KeyValueStore::with_config(Some(50_000_000), None, EvictionPolicy::NoEviction);
                // A realistic resident keyspace for the shards to hold.
                for i in 0..200_000 {
                    store.execute(Command::Set(
                        format!("bg:{i}"),
                        s("v"),
                        SetOptions::default(),
                    ));
                }
                let per_thread = 200_000 / threads;
                let start = Barrier::new(threads);
                let t0 = std::time::Instant::now();
                std::thread::scope(|scope| {
                    for t in 0..threads {
                        let (store, start) = (store.clone(), &start);
                        scope.spawn(move || {
                            start.wait();
                            for i in 0..per_thread {
                                store.execute(Command::Set(
                                    format!("r{rep}:t{t}:{i}"),
                                    s("value-16-bytes!!"),
                                    SetOptions::default(),
                                ));
                            }
                        });
                    }
                });
                let rate = (per_thread * threads) as f64 / t0.elapsed().as_secs_f64();
                if from_map {
                    map_rates.push(rate);
                } else {
                    counter_rates.push(rate);
                }
            }
        }
        ADMISSION_COUNTS_THE_MAP.store(false, Ordering::Relaxed);
        let median = |v: &mut Vec<f64>| {
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        };
        let (m, c) = (median(&mut map_rates), median(&mut counter_rates));
        println!("{threads},{m:.0},{c:.0},{:.2}", c / m);
    }
}
