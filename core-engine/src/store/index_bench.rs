//! A/B benchmark for the key index (6L-C): one file, compiled into the
//! single-index tree and the sharded one, so both are measured through the
//! same production paths.
//!
//! ```text
//! cargo test --release -p core-engine --lib --no-run   # note the binary
//! BENCH_MODE=scan   BENCH_KEYS=1000000 BENCH_COUNT=10  <bin> --ignored --exact store::index_bench::bench_index --nocapture
//! BENCH_MODE=create BENCH_KEYS=400000  BENCH_THREADS=8 BENCH_CAPPED=0|1 ...
//! BENCH_MODE=qsub   BENCH_KEYS=1000000 BENCH_PATTERN='t:42:*' ...
//! ```
//! Each run prints one JSON line.

use super::*;
use crate::cmd::SetOptions;
use std::time::Instant;

fn env<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Key `i`: a bijection of `i`, scattered over the key space as memtier's
/// random keys are (the 6J/6L prototypes' keys).
fn key_of(i: u64) -> String {
    format!("memtier-{}", (i as u32).wrapping_mul(2_654_435_761))
}

fn set(store: &KeyValueStore, key: String) {
    store.execute(Command::Set(
        key,
        "value-16-bytes!!".into(),
        SetOptions::default(),
    ));
}

#[test]
#[ignore = "benchmark; run explicitly, see the module docs"]
fn bench_index() {
    let mode: String = env("BENCH_MODE", "scan".to_string());
    let keys: u64 = env("BENCH_KEYS", 1_000_000);
    match mode.as_str() {
        "scan" => {
            let count: usize = env("BENCH_COUNT", 10);
            let store = KeyValueStore::new();
            for i in 0..keys {
                set(&store, key_of(i));
            }
            let start = Instant::now();
            let (mut cursor, mut calls, mut returned, mut slowest) = (0u64, 0u64, 0u64, 0f64);
            loop {
                let t = Instant::now();
                let reply = store.execute(Command::Scan(cursor, None, Some(count)));
                slowest = slowest.max(t.elapsed().as_secs_f64());
                calls += 1;
                let Value::Array(Some(parts)) = reply else {
                    panic!("SCAN reply")
                };
                let [Value::BulkString(Some(next)), Value::Array(Some(page))] = parts.as_slice()
                else {
                    panic!("SCAN reply shape")
                };
                returned += page.len() as u64;
                cursor = std::str::from_utf8(next)
                    .expect("utf-8")
                    .parse()
                    .expect("cursor");
                if cursor == 0 {
                    break;
                }
            }
            let wall = start.elapsed().as_secs_f64();
            assert_eq!(returned, keys, "a full SCAN returns every key once");
            println!(
                "{{\"mode\":\"scan\",\"keys\":{keys},\"count\":{count},\"full_scan_s\":{wall:.4},\
                 \"calls\":{calls},\"slowest_call_us\":{:.1}}}",
                slowest * 1e6
            );
        }
        "create" => {
            let threads: u64 = env("BENCH_THREADS", 1);
            let capped: u8 = env("BENCH_CAPPED", 0);
            let store = if capped == 1 {
                let s = KeyValueStore::with_config(
                    Some(50_000_000),
                    Some(1 << 34),
                    EvictionPolicy::AllKeysLru,
                );
                s.set_reserved_writes(true);
                s
            } else {
                KeyValueStore::new()
            };
            let names: Vec<String> = (0..keys).map(key_of).collect();
            let start = Instant::now();
            std::thread::scope(|scope| {
                for t in 0..threads {
                    let (store, names) = (&store, &names);
                    scope.spawn(move || {
                        let mut i = t;
                        while i < keys {
                            set(store, names[i as usize].clone());
                            i += threads;
                        }
                    });
                }
            });
            let wall = start.elapsed().as_secs_f64();
            assert_eq!(store.key_count() as u64, keys);
            println!(
                "{{\"mode\":\"create\",\"keys\":{keys},\"threads\":{threads},\"capped\":{capped},\
                 \"creates_per_s\":{:.0}}}",
                keys as f64 / wall
            );
        }
        "qsub" => {
            // 1,000 tenants x (keys / 1,000) keys; a live query's snapshot
            // for one tenant (0.1%) or a wider pattern.
            let pattern: String = env("BENCH_PATTERN", "t:42:*".to_string());
            let reps: u32 = env("BENCH_REPS", 200);
            let store = KeyValueStore::new();
            let per = (keys / 1_000).max(1);
            // BENCH_SCATTER=1 creates the keys in a scattered order, as
            // keys arriving over time are, instead of tenant by tenant:
            // tenant-by-tenant creation puts a prefix's keys next to each
            // other in memory, which favours walking one tree in order.
            let scatter: u8 = env("BENCH_SCATTER", 0);
            let total = 1_000 * per;
            for n in 0..total {
                let j = if scatter == 1 {
                    // A bijection of 0..total when total and the multiplier
                    // are coprime: 1,000,003 is prime.
                    (n.wrapping_mul(1_000_003)) % total
                } else {
                    n
                };
                set(&store, format!("t:{}:{}", j / per, j % per));
            }
            let now = now_ms();
            let mut matched = 0usize;
            let mut examined = 0usize;
            let start = Instant::now();
            for _ in 0..reps {
                matched = 0;
                examined = store.for_each_live_match(&pattern, now, |_, _| {
                    matched += 1;
                    true
                });
            }
            let per_snapshot = start.elapsed().as_secs_f64() / f64::from(reps);
            println!(
                "{{\"mode\":\"qsub\",\"keys\":{keys},\"scatter\":{scatter},\"pattern\":\"{pattern}\",\"matched\":{matched},\"examined\":{examined},\
                 \"snapshot_us\":{:.1}}}",
                per_snapshot * 1e6
            );
        }
        other => panic!("unknown BENCH_MODE {other}"),
    }
}
