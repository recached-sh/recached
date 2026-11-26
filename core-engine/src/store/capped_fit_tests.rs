//! 6O-G: a capped store's transaction that certainly fits holds only its own
//! stripes (`queue_fits_without_eviction`). Independent review D's probes,
//! kept as tests: random queues on capped stores near their caps never
//! evict, refuse for capacity or touch a key outside the queue when they are
//! judged to fit; the queue's growth is summed, not checked write by write;
//! and a repair after a panic keeps the capacity gate.
use super::*;
use crate::cmd::{SetExpiry, SetOptions, ZAddOptions};
use rand::{Rng, SeedableRng};

type Rng64 = rand::rngs::StdRng;

fn val(rng: &mut Rng64) -> Vec<u8> {
    let n = match rng.random_range(0..4) {
        0 => 0,
        1 => rng.random_range(1..8),
        2 => rng.random_range(8..64),
        _ => rng.random_range(64..400),
    };
    (0..n).map(|i| b'0' + (i % 10) as u8).collect()
}

fn qkey(rng: &mut Rng64) -> String {
    // Queue keys: a small pool so commands meet each other, plus seeded ones.
    if rng.random_bool(0.7) {
        format!("q{}", rng.random_range(0..6))
    } else {
        format!("seed{}", rng.random_range(0..40))
    }
}

fn command(rng: &mut Rng64) -> Command {
    let k = qkey(rng);
    match rng.random_range(0..24) {
        0 | 1 => Command::Set(k, val(rng), SetOptions::default()),
        2 => Command::Set(
            k,
            val(rng),
            SetOptions {
                expiry: Some(SetExpiry::Px(rng.random_range(1..3))),
                ..SetOptions::default()
            },
        ),
        3 => Command::Append(k, val(rng)),
        4 => Command::MSet(
            (0..rng.random_range(1..5))
                .map(|_| (qkey(rng), val(rng)))
                .collect(),
        ),
        5 => Command::Incr(k),
        6 => Command::HSet(
            k,
            (0..rng.random_range(1..4))
                .map(|i| (format!("f{i}"), val(rng)))
                .collect(),
        ),
        7 => Command::HDel(k, vec!["f0".into(), "f1".into()]),
        8 => Command::RPush(k, (0..rng.random_range(1..4)).map(|_| val(rng)).collect()),
        9 => Command::LPop(k, None),
        10 => Command::SAdd(
            k,
            (0..rng.random_range(1..4))
                .map(|i| format!("m{i}"))
                .collect(),
        ),
        11 => Command::SRem(k, vec!["m0".into()]),
        12 => Command::SMove(k, qkey(rng), "m1".into()),
        13 => Command::ZAdd(
            k,
            ZAddOptions::default(),
            vec![(1.0, "z1".into()), (2.0, "z2".into())],
        ),
        14 => Command::ZPopMax(k, None),
        15 => Command::Del(vec![k, qkey(rng)]),
        16 => Command::Rename(k, qkey(rng)),
        17 => Command::RenameNx(k, qkey(rng)),
        18 => Command::Expire(k, rng.random_range(0..3)),
        19 => Command::Persist(k),
        20 => Command::GetDel(k),
        21 => Command::Get(k),
        22 => Command::LSet(k, 0, val(rng)),
        _ => Command::HIncrByFloat(k, "f2".into(), 1.5),
    }
}

fn queue_keys(queue: &[Command]) -> std::collections::BTreeSet<String> {
    queue
        .iter()
        .flat_map(|cmd| match mutation_scope(cmd) {
            MutationScope::Keys(keys) => keys.into_iter().map(str::to_string).collect(),
            _ => Vec::new(),
        })
        .collect()
}

/// Every key/entry-size pair outside `named`, expired or not.
fn outside(
    store: &KeyValueStore,
    named: &std::collections::BTreeSet<String>,
) -> Vec<(String, usize)> {
    let mut v: Vec<(String, usize)> = store
        .data
        .iter()
        .filter(|e| !named.contains(e.key().as_ref() as &str))
        .map(|e| (e.key().to_string(), entry_size(e.key(), e.value())))
        .collect();
    v.sort();
    v
}

#[test]
fn fitting_queues_never_evict_refuse_or_touch_other_keys() {
    let mut fitted = 0usize;
    let mut declined = 0usize;
    for seed in 0..20_000u64 {
        let mut rng = Rng64::seed_from_u64(seed);
        let policy = [
            EvictionPolicy::AllKeysLru,
            EvictionPolicy::NoEviction,
            EvictionPolicy::VolatileTtl,
            EvictionPolicy::AllKeysRandom,
        ][rng.random_range(0..4)];
        let seeded = rng.random_range(5..40usize);
        // Caps near what the seed fills, so queues sit on either side.
        let max_keys = rng
            .random_bool(0.7)
            .then(|| seeded + rng.random_range(0..12));
        let max_mem = rng
            .random_bool(0.6)
            .then(|| seeded * 120 + rng.random_range(0..3_000));
        if max_keys.is_none() && max_mem.is_none() {
            continue;
        }
        let store = KeyValueStore::with_config(None, None, policy);
        for i in 0..seeded {
            let opts = if rng.random_bool(0.3) {
                SetOptions {
                    expiry: Some(SetExpiry::Px(1)),
                    ..SetOptions::default()
                }
            } else if rng.random_bool(0.3) {
                SetOptions {
                    expiry: Some(SetExpiry::Ex(3_600)),
                    ..SetOptions::default()
                }
            } else {
                SetOptions::default()
            };
            store.execute(Command::Set(format!("seed{i}"), val(&mut rng), opts));
        }
        // Re-open the same keyspace under caps: a fresh capped store, restored.
        std::thread::sleep(std::time::Duration::from_millis(2));
        let capped = KeyValueStore::with_config(max_keys, max_mem, policy);
        {
            let _all = capped.order_all();
            for e in store.data.iter() {
                capped.data.insert(Arc::clone(e.key()), e.value().clone());
            }
            capped.rebuild_metadata();
        }
        let queue: Vec<Command> = (0..rng.random_range(1..7))
            .map(|_| command(&mut rng))
            .collect();
        let named = queue_keys(&queue);
        let before = outside(&capped, &named);
        let evicted_before = capped.evicted_count();
        let fits = {
            let _gate = capped.capacity_gate.write().unwrap();
            capped.queue_fits_without_eviction(&queue)
        };
        let mut replies = Vec::new();
        let applied = capped.apply_transaction(&queue, clock_ms(), |_, cmd| {
            let (reply, evicted) = capped.execute_reporting_ref(cmd);
            replies.push((reply, evicted));
        });
        if !fits {
            declined += 1;
            continue;
        }
        fitted += 1;
        assert!(
            applied.panicked.is_none(),
            "seed {seed}: panicked {queue:?}"
        );
        assert_eq!(
            capped.evicted_count(),
            evicted_before,
            "seed {seed}: evicted {queue:?}"
        );
        for (reply, evicted) in &replies {
            assert!(
                evicted.is_empty(),
                "seed {seed}: victims {evicted:?} for {queue:?}"
            );
            if let Value::Error(e) = reply {
                assert!(
                    !e.contains("OOM") && !e.contains("max keys"),
                    "seed {seed}: refused for capacity: {e} in {queue:?}"
                );
            }
        }
        let after = outside(&capped, &named);
        assert_eq!(
            before, after,
            "seed {seed}: a key outside the queue changed: {queue:?}"
        );
        if let Some(limit) = max_mem {
            assert!(
                capped.approximate_memory_bytes() <= limit - limit / EVICTION_HEADROOM_DIVISOR,
                "seed {seed}: memory past the eviction target"
            );
        }
        if let Some(max) = max_keys {
            assert!(capped.key_count() <= max, "seed {seed}: key cap exceeded");
        }
    }
    eprintln!("review-d probe: fitted={fitted} declined={declined}");
    assert!(
        fitted > 300 && declined > 300,
        "fitted={fitted} declined={declined}"
    );
}

/// Each write fits on its own; the queue together does not: it must decline.
#[test]
fn the_sum_is_checked_not_each_write() {
    let store = KeyValueStore::with_config(Some(4), None, EvictionPolicy::AllKeysLru);
    store.execute(Command::Set(
        "a".into(),
        b"1".to_vec(),
        SetOptions::default(),
    ));
    let set = |k: &str| Command::Set(k.into(), b"1".to_vec(), SetOptions::default());
    let _gate = store.capacity_gate.write().unwrap();
    assert!(store.queue_fits_without_eviction(&[set("b"), set("c"), set("d")]));
    assert!(!store.queue_fits_without_eviction(&[set("b"), set("c"), set("d"), set("e")]));
    // Memory: exact boundary.
    drop(_gate);
    let limit = 16 * 1_000;
    let target = limit - limit / EVICTION_HEADROOM_DIVISOR;
    let mem = KeyValueStore::with_config(None, Some(limit), EvictionPolicy::AllKeysLru);
    let used = mem.approximate_memory_bytes();
    let k = "m";
    let room = target - used - (k.len() + 64);
    let _gate = mem.capacity_gate.write().unwrap();
    assert!(mem.queue_fits_without_eviction(&[Command::Set(
        k.into(),
        vec![b'x'; room],
        SetOptions::default()
    )]));
    assert!(!mem.queue_fits_without_eviction(&[Command::Set(
        k.into(),
        vec![b'x'; room + 1],
        SetOptions::default()
    )]));
}

/// A capped transaction holding only its stripes, whose write panics while
/// another holder has a stripe outside its set, must keep the capacity gate
/// through the repair: an eviction or capped write running between the
/// part-applied write and the metadata rebuild would skew the accounting
/// (review D-1).
#[test]
fn a_contended_repair_on_a_capped_store_keeps_the_gate() {
    use std::time::Duration;
    let store = KeyValueStore::with_config(Some(1_000), Some(1 << 24), EvictionPolicy::AllKeysLru);
    store.execute(Command::RPush("rd:l".into(), vec![b"x".to_vec()]));
    // A key on a stripe apart from the queue's.
    let named = [write_order_stripe("rd:a"), write_order_stripe("rd:l")];
    let outside = (0..)
        .map(|i| format!("rd:o{i}"))
        .find(|k| !named.contains(&write_order_stripe(k)))
        .unwrap();
    let held = store.write_order[write_order_stripe(&outside)]
        .lock
        .lock()
        .unwrap();
    let probe = store.clone();
    let tx = std::thread::spawn(move || {
        let _armed = transaction_fault::panic_during_mutation_of("rd:l");
        probe.apply_transaction(
            &[
                Command::Set("rd:a".into(), b"1".to_vec(), SetOptions::default()),
                Command::LPop("rd:l".into(), None),
            ],
            clock_ms(),
            |_, cmd| {
                probe.execute_reporting_ref(cmd);
            },
        )
    });
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        !tx.is_finished(),
        "the repair should be waiting for the held stripe"
    );
    // Is the gate free while the repair waits for the rest of the stripes?
    let gate_free = store.capacity_gate.try_write().is_ok();
    drop(held);
    let applied = tx.join().unwrap();
    assert!(applied.repaired);
    assert!(
        !gate_free,
        "the capacity gate was released before the repair on a capped store"
    );
}
