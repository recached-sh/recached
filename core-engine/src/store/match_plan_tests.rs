//! Live-query snapshot enumeration (`for_each_live_match`) against an
//! independent reference.
//!
//! The ordered-index plan is an optimisation of "walk every key and glob it".
//! These tests hold it to exactly that: for every pattern shape the glob
//! supports, the set of keys and values returned must equal what a full walk
//! of the map returns, and the work done for a pattern with a literal prefix
//! must be bounded by the keys sharing that prefix — so reverting to a full
//! walk, or treating the prefix as the match, fails here.

use super::*;
use crate::cmd::SetOptions;
use rand::{Rng, SeedableRng};

/// What a full walk of the map returns: the pre-6K implementation.
fn reference(store: &KeyValueStore, pattern: &str) -> BTreeMap<String, (Value, bool)> {
    let now = now_ms();
    store
        .data
        .iter()
        .filter(|e| !e.is_expired(now) && glob_match(pattern, e.key()))
        .map(|e| {
            (
                e.key().to_string(),
                (
                    KeyValueStore::encode_current_value(&e.value),
                    e.deadline.get().is_some(),
                ),
            )
        })
        .collect()
}

fn snapshot(store: &KeyValueStore, pattern: &str) -> BTreeMap<String, (Value, bool)> {
    let values = store.matching_key_values_with_ttl(pattern, usize::MAX);
    let n = values.len();
    let out: BTreeMap<_, _> = values
        .into_iter()
        .map(|(k, v, ttl)| (k, (v, ttl.is_some())))
        .collect();
    assert_eq!(out.len(), n, "a key was returned twice for {pattern:?}");
    out
}

fn set(store: &KeyValueStore, key: &str, value: &str) {
    store.execute(Command::Set(
        key.into(),
        value.into(),
        SetOptions::default(),
    ));
}

#[test]
fn plan_follows_the_first_metacharacter() {
    assert_eq!(MatchPlan::of("tenant:a:book:1"), MatchPlan::Exact);
    assert_eq!(MatchPlan::of(""), MatchPlan::Exact);
    assert_eq!(MatchPlan::of("tenant:a:*"), MatchPlan::Prefix(9));
    assert_eq!(MatchPlan::of("tenant:a:book:?"), MatchPlan::Prefix(14));
    assert_eq!(MatchPlan::of("t?*"), MatchPlan::Prefix(1));
    assert_eq!(MatchPlan::of("*:book"), MatchPlan::Scan);
    assert_eq!(MatchPlan::of("*"), MatchPlan::Scan);
    assert_eq!(MatchPlan::of("?x"), MatchPlan::Scan);
    // Brackets are literal in this glob, so they do not end the prefix.
    assert_eq!(MatchPlan::of("[ab]*"), MatchPlan::Prefix(4));
    // Multi-byte characters before the metacharacter stay whole.
    assert_eq!(MatchPlan::of("é:*"), MatchPlan::Prefix("é:".len()));
}

#[test]
fn every_pattern_shape_matches_the_full_walk() {
    let store = KeyValueStore::new();
    let keys = [
        "",
        "tenant:a",
        "tenant:a:",
        "tenant:a:1",
        "tenant:a:book:1",
        "tenant:a:book:12",
        "tenant:a:book:x",
        "tenant:ab:1",
        "tenant:ab:book:1",
        "tenant:b:1",
        "tenant:b:book:1",
        "tenant:",
        "tenant",
        "tenanu:a:1",
        "x:book",
        "y:book",
        "[ab]",
        "[ab]x",
        "a",
        "*",
        "?",
        "*literal",
        "é:1",
        "é:2",
        "e:1",
        "\u{10FFFF}",
    ];
    for (i, key) in keys.iter().enumerate() {
        set(&store, key, &format!("v{i}"));
    }
    store.execute(Command::HSet(
        "tenant:a:hash".into(),
        vec![("f".into(), "1".into())],
    ));
    store.execute(Command::RPush(
        "tenant:a:list".into(),
        vec!["x".into(), "y".into()],
    ));
    store.execute(Command::SetEx("tenant:a:ttl".into(), 3600, "t".into()));

    let patterns = [
        "",
        "*",
        "?",
        "??",
        "tenant:a:*",
        "tenant:a*",
        "tenant:a",
        "tenant:a:",
        "tenant:a:book:?",
        "tenant:a:book:*",
        "tenant:a:*:1",
        "tenant:*:book:*",
        "tenant:ab:*",
        "tenant:b:*",
        "tenant:c:*",
        "tenant:*",
        "tenant*",
        "*:book",
        "*:1",
        "[ab]*",
        "[ab]",
        "*literal",
        "é:*",
        "e*",
        "\u{10FFFF}*",
        "zzz*",
        "a",
        "a*",
        "*a*",
        "t?nant:a:*",
    ];
    for pattern in patterns {
        assert_eq!(
            snapshot(&store, pattern),
            reference(&store, pattern),
            "pattern {pattern:?} ({:?})",
            MatchPlan::of(pattern)
        );
    }
}

#[test]
fn random_keyspaces_and_patterns_match_the_full_walk() {
    // Small alphabet so prefixes collide, patterns overlap and `?`/`*` land
    // everywhere: the interesting cases are the near-misses.
    let mut rng = rand::rngs::StdRng::seed_from_u64(0x6b_5153_5542);
    let alphabet = ['a', 'b', ':', 'é'];
    let word = |rng: &mut rand::rngs::StdRng, max: usize| -> String {
        let len = rng.random_range(0..=max);
        (0..len)
            .map(|_| alphabet[rng.random_range(0..alphabet.len())])
            .collect()
    };
    for round in 0..40 {
        let store = KeyValueStore::new();
        for _ in 0..rng.random_range(0..400) {
            let key = word(&mut rng, 7);
            match rng.random_range(0..4) {
                0 => {
                    store.execute(Command::SetEx(key, 3600, "t".into()));
                }
                1 => {
                    store.execute(Command::Del(vec![key]));
                }
                _ => set(&store, &key, "v"),
            }
        }
        for _ in 0..60 {
            let mut pattern = word(&mut rng, 4);
            for _ in 0..rng.random_range(0..3) {
                let at = pattern
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain(std::iter::once(pattern.len()))
                    .nth(rng.random_range(0..=pattern.chars().count()))
                    .unwrap_or(pattern.len());
                pattern.insert(at, if rng.random_bool(0.5) { '*' } else { '?' });
            }
            assert_eq!(
                snapshot(&store, &pattern),
                reference(&store, &pattern),
                "round {round}, pattern {pattern:?}"
            );
        }
    }
}

#[test]
fn a_prefix_wider_than_one_index_step_is_walked_exactly_once() {
    // Matches interleaved with non-matches across several lock releases, and
    // a step boundary landing exactly on the prefix's last key.
    let store = KeyValueStore::new();
    let total = MATCH_INDEX_STEP * 3;
    for i in 0..total {
        set(&store, &format!("t:a:{i:06}"), "v");
        set(&store, &format!("t:a:{i:06}:x"), "x");
    }
    set(&store, "t:b:000000", "other");
    set(&store, "t:9", "before");
    for pattern in ["t:a:*", "t:a:*:x", "t:a:??????", "t:*", "t:a:00*"] {
        assert_eq!(
            snapshot(&store, pattern),
            reference(&store, pattern),
            "{pattern}"
        );
    }
    // Exactly the prefix's own keys were examined: twice `total`, nothing
    // past the prefix and nothing twice across the lock releases.
    let (values, examined) = store.matching_key_values_with_ttl_counted("t:a:*", usize::MAX);
    assert_eq!(values.len(), total * 2);
    assert_eq!(examined, total * 2);

    let exact_boundary = KeyValueStore::new();
    for i in 0..MATCH_INDEX_STEP {
        set(&exact_boundary, &format!("p:{i:06}"), "v");
    }
    set(&exact_boundary, "q", "v");
    assert_eq!(
        exact_boundary
            .matching_key_values_with_ttl("p:*", usize::MAX)
            .len(),
        MATCH_INDEX_STEP
    );
}

#[test]
fn a_narrow_prefix_examines_only_its_own_keys() {
    // The point of the plan. If the snapshot reverted to walking the whole
    // keyspace, `examined` would be ~100k here, not 100.
    let store = KeyValueStore::new();
    for tenant in 0..1_000 {
        for item in 0..100 {
            set(&store, &format!("tenant:{tenant}:item:{item}"), "v");
        }
    }
    let (values, examined) = store.matching_key_values_with_ttl_counted("tenant:7:*", usize::MAX);
    assert_eq!(values.len(), 100);
    assert_eq!(examined, 100, "tenant:7: has exactly 100 keys");
    // `tenant:70:*` etc. share the bytes `tenant:7` but not `tenant:7:`.
    assert!(values.iter().all(|(k, _, _)| k.starts_with("tenant:7:")));

    let (none, examined) = store.matching_key_values_with_ttl_counted("tenant:x:*", usize::MAX);
    assert!(none.is_empty());
    assert_eq!(examined, 0);

    let (one, examined) = store.matching_key_values_with_ttl_counted("tenant:7:item:7", usize::MAX);
    assert_eq!(one.len(), 1);
    assert_eq!(examined, 1, "an exact pattern is one lookup");

    let (_, examined) = store.matching_key_values_with_ttl_counted("*:item:7", usize::MAX);
    assert_eq!(examined, 100_000, "no literal prefix: the whole keyspace");
}

#[test]
fn expired_keys_are_skipped_on_every_plan() {
    let store = KeyValueStore::new();
    set(&store, "q:live", "v");
    store.execute(Command::PSetEx("q:dead".into(), 1, "d".into()));
    store.execute(Command::PSetEx("dead".into(), 1, "d".into()));
    std::thread::sleep(std::time::Duration::from_millis(10));
    for pattern in ["q:*", "q:dead", "*", "dead", "?:*"] {
        let got = snapshot(&store, pattern);
        assert!(
            got.keys().all(|k| k != "q:dead" && k != "dead"),
            "{pattern}: {got:?}"
        );
        assert_eq!(got, reference(&store, pattern), "{pattern}");
    }
    // Still physically present (awaiting the sweeper), so still indexed —
    // which is why liveness is checked after the index, not trusted from it.
    assert!(store.data.contains_key("q:dead"));
}

#[test]
fn the_limit_stops_the_walk_without_skipping_live_keys() {
    let store = KeyValueStore::new();
    for i in 0..50 {
        set(&store, &format!("l:{i:02}"), "v");
    }
    // An expired key early in the range must not count towards the limit.
    store.execute(Command::PSetEx("l:00".into(), 1, "d".into()));
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert_eq!(store.matching_key_values_with_ttl("l:*", 49).len(), 49);
    assert_eq!(store.matching_key_values_with_ttl("l:*", 50).len(), 49);
    assert_eq!(store.matching_key_values("l:*", 10).len(), 10);
    assert!(store.matching_key_values("l:*", 0).is_empty());
    assert_eq!(store.matching_keys("l:*").len(), 49);
}

#[test]
fn snapshots_agree_with_the_full_walk_while_writers_run() {
    // Without the server's barrier a snapshot is not atomic (neither was the
    // map walk), but it must never return a key that does not match, a key
    // twice, or a key that was never written.
    let store = KeyValueStore::new();
    for i in 0..2_000 {
        set(&store, &format!("c:{i:05}"), "v");
    }
    /// Stops the writer however the reader side ends: a failed assertion
    /// unwinds through this, and the scope then joins a writer that exits,
    /// instead of waiting forever on one that was never told to stop.
    struct StopOnDrop<'a>(&'a std::sync::atomic::AtomicBool);
    impl Drop for StopOnDrop<'_> {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
        }
    }
    let stop = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        let stop = &stop;
        let _stop_writer = StopOnDrop(stop);
        let writer_store = store.clone();
        scope.spawn(move || {
            let mut i = 0u64;
            while !stop.load(Ordering::Relaxed) {
                let key = format!("c:{:05}", i % 4_000);
                if i.is_multiple_of(3) {
                    writer_store.execute(Command::Del(vec![key]));
                } else {
                    set(&writer_store, &key, "w");
                }
                i += 1;
            }
        });
        for _ in 0..200 {
            let got = store.matching_key_values_with_ttl("c:0*", usize::MAX);
            let mut seen = HashSet::new();
            for (key, _, _) in &got {
                assert!(glob_match("c:0*", key), "{key}");
                assert!(seen.insert(key.clone()), "{key} returned twice");
            }
        }
    });
    // Quiescent again: exact agreement.
    assert_eq!(snapshot(&store, "c:0*"), reference(&store, "c:0*"));
}

/// The pre-6K implementation, verbatim: walk the whole map, glob each key.
fn pre_6k_matching_key_values_with_ttl(
    store: &KeyValueStore,
    pattern: &str,
    limit: usize,
) -> Vec<(String, Value, Option<u64>)> {
    let now = now_ms();
    store
        .data
        .iter()
        .filter(|e| !e.is_expired(now) && glob_match(pattern, e.key()))
        .take(limit)
        .map(|e| {
            (
                e.key().to_string(),
                KeyValueStore::encode_current_value(&e.value),
                e.deadline.get().map(|exp| exp - now),
            )
        })
        .collect()
}

/// QSUB snapshot cost, pre-6K walk vs the 6K prefix plan, same binary, same
/// data. Run with:
///
/// ```text
/// cargo test --release -p core-engine --lib -- --ignored \
///     bench_live_query_snapshot --nocapture
/// ```
///
/// Prints one CSV row per (keyspace, match share): median microseconds of
/// each, keys examined by the new plan, and matches. Keys `k:m:<i>` match
/// `k:m:*`, `k:o:<i>` do not; the 0% row queries an absent prefix.
#[test]
#[ignore = "benchmark; run explicitly in release"]
fn bench_live_query_snapshot() {
    let sizes: Vec<usize> = std::env::var("BENCH_SIZES")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.parse().ok()).collect())
        .unwrap_or_else(|| vec![1_000, 10_000, 100_000, 1_000_000]);
    let shares = [0.0, 0.001, 0.01, 0.1, 1.0];
    println!("keys,share,pre6k_us,k6_us,speedup,examined,matched");
    for &n in &sizes {
        for &share in &shares {
            let store = KeyValueStore::new();
            let matching = ((n as f64) * share).round() as usize;
            for i in 0..n {
                let key = if i < matching {
                    format!("k:m:{i:08}")
                } else {
                    format!("k:o:{i:08}")
                };
                store.execute(Command::Set(
                    key,
                    "value-16-bytes!!".into(),
                    SetOptions::default(),
                ));
            }
            let pattern = if share == 0.0 { "k:x:*" } else { "k:m:*" };
            let reps = if n >= 1_000_000 {
                7
            } else if n >= 100_000 {
                15
            } else {
                41
            };
            let mut old_us = Vec::with_capacity(reps);
            let mut new_us = Vec::with_capacity(reps);
            let mut examined = 0;
            let mut matched = 0;
            for _ in 0..reps {
                let t = std::time::Instant::now();
                let old = pre_6k_matching_key_values_with_ttl(&store, pattern, usize::MAX);
                old_us.push(t.elapsed().as_secs_f64() * 1e6);
                let t = std::time::Instant::now();
                let (new, ex) = store.matching_key_values_with_ttl_counted(pattern, usize::MAX);
                new_us.push(t.elapsed().as_secs_f64() * 1e6);
                assert_eq!(old.len(), new.len());
                examined = ex;
                matched = new.len();
            }
            let median = |v: &mut Vec<f64>| {
                v.sort_by(f64::total_cmp);
                v[v.len() / 2]
            };
            let (o, k) = (median(&mut old_us), median(&mut new_us));
            println!(
                "{n},{share},{o:.1},{k:.1},{:.1},{examined},{matched}",
                o / k.max(0.001)
            );
        }
    }
}
