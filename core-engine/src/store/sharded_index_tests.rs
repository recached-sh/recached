//! The sharded key index (6L-C) against the contracts the single index kept.
//!
//! `SCAN` merges 16 shards through a per-cursor frontier. These tests hold
//! it to:
//! - exactly once, in byte order, for keys present for the whole iteration,
//!   under concurrent creation and deletion
//! - the same order as `KEYS`, including keys whose first 16 bytes tie
//! - bounded cursor state
//!
//! And they hold the other readers to what they were before sharding:
//! - eviction sampling draws every key when the sample covers the keyspace
//! - the budgeted expiry sweep reaches every TTL key

use super::*;
use crate::cmd::{SetExpiry, SetOptions};
use std::sync::atomic::AtomicBool;

fn set(store: &KeyValueStore, key: &str) {
    store.execute(Command::Set(key.into(), "v".into(), SetOptions::default()));
}

/// Every key a full `SCAN` returns, in the order returned.
fn scan_all(store: &KeyValueStore, count: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cursor = 0u64;
    loop {
        let Value::Array(Some(parts)) = store.execute(Command::Scan(cursor, None, Some(count)))
        else {
            panic!("SCAN reply is an array");
        };
        let [Value::BulkString(Some(next)), Value::Array(Some(keys))] = parts.as_slice() else {
            panic!("SCAN reply is [cursor, keys]");
        };
        for key in keys {
            let Value::BulkString(Some(key)) = key else {
                panic!("a key is a bulk string")
            };
            out.push(String::from_utf8(key.clone()).expect("utf-8 key"));
        }
        cursor = std::str::from_utf8(next)
            .expect("cursor")
            .parse()
            .expect("numeric cursor");
        if cursor == 0 {
            return out;
        }
    }
}

fn keys_sorted(store: &KeyValueStore) -> Vec<String> {
    let Value::Array(Some(keys)) = store.execute(Command::Keys("*".into())) else {
        panic!("KEYS reply is an array");
    };
    keys.into_iter()
        .map(|k| match k {
            Value::BulkString(Some(k)) => String::from_utf8(k).expect("utf-8 key"),
            other => panic!("{other:?}"),
        })
        .collect()
}

#[test]
fn scan_order_is_keys_order_including_long_shared_prefixes() {
    let store = KeyValueStore::new();
    // Namespaced keys whose first 16 bytes tie, so ordering falls back to
    // full comparison; short keys, a key that is another's prefix, and an
    // empty-suffix key.
    for i in 0..3_000 {
        set(
            &store,
            &format!("tenant:acme-corp:session:{:05}", (i * 7919) % 3_000),
        );
        set(&store, &format!("t{i}"));
    }
    for key in [
        "tenant:acme-corp:session:",
        "tenant:acme-corp:sessio",
        "a",
        "aa",
        "tenant:acme-corp",
    ] {
        set(&store, key);
    }
    let expected = keys_sorted(&store);
    for count in [1, 7, 10, 100, 1_000, 100_000] {
        assert_eq!(scan_all(&store, count), expected, "COUNT {count}");
    }
}

#[test]
fn scan_returns_every_stable_key_once_in_order_under_churn() {
    let store = Arc::new(KeyValueStore::new());
    let stable: Vec<String> = (0..20_000u64)
        .map(|i| format!("s:{:06}", (i * 7919) % 20_000))
        .collect();
    for key in &stable {
        set(&store, key);
    }
    let mut expected = stable.clone();
    expected.sort();
    let stop = Arc::new(AtomicBool::new(false));
    let churn: Vec<_> = (0..3u64)
        .map(|t| {
            let (store, stop) = (Arc::clone(&store), Arc::clone(&stop));
            std::thread::spawn(move || {
                let mut i = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    // Before, between and after the stable keys.
                    let key = match i % 3 {
                        0 => format!("a:{t}:{i}"),
                        1 => format!("s:{:06}x{t}", i % 20_000),
                        _ => format!("z:{t}:{i}"),
                    };
                    set(&store, &key);
                    if i.is_multiple_of(2) {
                        store.execute(Command::Del(vec![key]));
                    }
                    i += 1;
                }
            })
        })
        .collect();
    for count in [1usize, 7, 10, 100, 1_000] {
        let got = scan_all(&store, count);
        assert!(
            got.windows(2).all(|w| w[0] < w[1]),
            "COUNT {count}: out of order or repeated"
        );
        let stable_got: Vec<&String> = got
            .iter()
            .filter(|k| k.starts_with("s:") && k.len() == 8)
            .collect();
        assert_eq!(
            stable_got.len(),
            expected.len(),
            "COUNT {count}: stable keys missed or repeated"
        );
        assert!(
            stable_got.iter().zip(&expected).all(|(a, b)| *a == b),
            "COUNT {count}"
        );
    }
    stop.store(true, Ordering::Relaxed);
    for t in churn {
        t.join().expect("churn thread");
    }
}

#[test]
fn a_scan_cursor_holds_bounded_state() {
    let store = KeyValueStore::new();
    let big = "k".repeat(600);
    for i in 0..2_000 {
        set(&store, &format!("{big}{i:05}"));
    }
    let Value::Array(Some(parts)) = store.execute(Command::Scan(0, None, Some(1))) else {
        panic!("SCAN reply");
    };
    let Some(Value::BulkString(Some(cursor))) = parts.first() else {
        panic!("cursor")
    };
    let cursor: u64 = std::str::from_utf8(cursor)
        .expect("utf-8")
        .parse()
        .expect("number");
    let cursors = store.scan_cursors.lock().unwrap_or_else(|p| p.into_inner());
    let session = cursors.get(&cursor).expect("a live session");
    let (keys, bytes) = session
        .frontier
        .bufs
        .iter()
        .flat_map(|buf| buf.iter())
        .fold((0usize, 0usize), |(n, b), key| (n + 1, b + key.len()));
    let longest = big.len() + 5;
    assert!(
        keys <= INDEX_SHARDS * SCAN_REFILL_KEYS,
        "{keys} keys buffered"
    );
    assert!(
        bytes <= INDEX_SHARDS * (SCAN_REFILL_BYTES + longest),
        "{bytes} key bytes buffered by one cursor"
    );
}

#[test]
fn keys_spread_evenly_over_the_shards() {
    let store = KeyValueStore::new();
    for i in 0..64_000 {
        set(&store, &format!("user:{i}"));
    }
    let per_shard: Vec<usize> = store.index_all().iter().map(|s| s.ordered.len()).collect();
    let mean = 64_000 / INDEX_SHARDS;
    for (i, n) in per_shard.iter().enumerate() {
        assert!(
            n.abs_diff(mean) * 5 < mean,
            "shard {i} holds {n} keys, mean {mean}: {per_shard:?}"
        );
    }
}

#[test]
fn eviction_sampling_draws_every_key_when_the_sample_covers_the_keyspace() {
    let store = KeyValueStore::with_config(None, None, EvictionPolicy::AllKeysRandom);
    for i in 0..40 {
        set(&store, &format!("k{i}"));
    }
    let mut drawn = store.indexed_candidates(false, 100);
    drawn.sort();
    drawn.dedup();
    assert_eq!(
        drawn.len(),
        40,
        "every key across the shards is a candidate"
    );
}

#[test]
fn the_budgeted_expiry_sweep_reaches_every_ttl_key_across_shards() {
    let store = KeyValueStore::new();
    for i in 0..1_000 {
        store.execute(Command::Set(
            format!("e{i}"),
            "v".into(),
            SetOptions {
                expiry: Some(SetExpiry::Px(1)),
                ..Default::default()
            },
        ));
    }
    std::thread::sleep(std::time::Duration::from_millis(10));
    let mut swept = 0;
    for _ in 0..40 {
        swept += store.sweep_expired_reporting_budget(50).len();
    }
    assert_eq!(swept, 1_000);
    assert_eq!(store.volatile_key_count(), 0);
    assert_eq!(store.key_count(), 0);
}
