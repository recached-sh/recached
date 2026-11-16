//! `growth_bound(cmd)` against what commands actually add (6K-E proof
//! obligation).
//!
//! A capped store already trusts the bound: `fits_without_eviction` lets a
//! write skip eviction when the memory estimate plus the command's bound stays
//! under the eviction target, and atomic capacity reservations (6K-E) would
//! reserve exactly the bound. Either is only safe if, for every command with
//! a bound, from every state a key can be in:
//!
//! ```text
//! memory after - memory before <= growth_bound(cmd)
//! ```
//!
//! measured with the store's own accounting (`approximate_memory_bytes`, which
//! `probe_tests` holds equal to a recomputation from the keyspace). This test
//! checks it over random commands applied to random prior states.

use super::*;
use crate::cmd::{SetExpiry, SetOptions, ZAddOptions};
use rand::{Rng, SeedableRng};

type Rng64 = rand::rngs::StdRng;

fn bytes(rng: &mut Rng64) -> Vec<u8> {
    // Inline-sized, heap-sized and empty values, digits included so numeric
    // commands sometimes succeed on them.
    let len = match rng.random_range(0..4) {
        0 => 0,
        1 => rng.random_range(1..=8),
        2 => rng.random_range(9..=40),
        _ => rng.random_range(41..=300),
    };
    let numeric = rng.random_bool(0.3);
    (0..len)
        .map(|i| {
            if numeric {
                b'0' + ((i * 7 + len) % 10) as u8
            } else {
                b'a' + rng.random_range(0..26)
            }
        })
        .collect()
}

fn text(rng: &mut Rng64) -> String {
    String::from_utf8(bytes(rng)).expect("ASCII")
}

/// One of a few keys, so commands meet each other's state.
fn key(rng: &mut Rng64) -> String {
    ["k", "d", "s", "a-much-longer-key-name-for-renames"][rng.random_range(0..4)].to_string()
}

/// Put `k` into a random state: absent, a string (short, long, numeric), a
/// hash, list, set, sorted set or rate limiter — each with or without a TTL.
fn seed(store: &KeyValueStore, k: &str, rng: &mut Rng64) {
    let cmd = match rng.random_range(0..9) {
        0 => return,
        1 => Command::Set(k.into(), b"7".to_vec(), SetOptions::default()),
        2 => Command::Set(k.into(), bytes(rng), SetOptions::default()),
        3 => Command::HSet(
            k.into(),
            (0..rng.random_range(1..12))
                .map(|i| (format!("f{i}"), bytes(rng)))
                .collect(),
        ),
        4 => Command::RPush(
            k.into(),
            (0..rng.random_range(1..6)).map(|_| bytes(rng)).collect(),
        ),
        5 => Command::SAdd(
            k.into(),
            (0..rng.random_range(1..12))
                .map(|i| format!("m{i}"))
                .collect(),
        ),
        6 => Command::ZAdd(
            k.into(),
            ZAddOptions::default(),
            (0..rng.random_range(1..6))
                .map(|i| (i as f64, format!("z{i}")))
                .collect(),
        ),
        7 => Command::RlSet(k.into(), 5, 1_000),
        _ => Command::Set(
            k.into(),
            b"12345678901234567890".to_vec(),
            SetOptions::default(),
        ),
    };
    store.execute(cmd);
    if rng.random_bool(0.3) {
        store.execute(Command::Expire(k.into(), 3_600));
    }
}

/// A random command that `growth_bound` gives a bound for.
fn bounded_command(rng: &mut Rng64) -> Command {
    let k = key(rng);
    match rng.random_range(0..38) {
        0 => Command::Set(k, bytes(rng), SetOptions::default()),
        1 => Command::Set(
            k,
            bytes(rng),
            SetOptions {
                expiry: Some(SetExpiry::Ex(60)),
                ..SetOptions::default()
            },
        ),
        2 => Command::ESet(k, bytes(rng)),
        3 => Command::GetSet(k, bytes(rng)),
        4 => Command::SetNx(k, bytes(rng)),
        5 => Command::Append(k, bytes(rng)),
        6 => Command::SetEx(k, 60, bytes(rng)),
        7 => Command::PSetEx(k, 60_000, bytes(rng)),
        8 => Command::LSet(k, rng.random_range(-3..3), bytes(rng)),
        9 => Command::MSet(
            (0..rng.random_range(1..4))
                .map(|_| (key(rng), bytes(rng)))
                .collect(),
        ),
        10 => Command::Incr(k),
        11 => Command::Decr(k),
        12 => Command::IncrBy(k, rng.random_range(i64::MIN / 2..i64::MAX / 2)),
        13 => Command::DecrBy(k, rng.random_range(-1_000_000..1_000_000)),
        14 => Command::HSet(
            k,
            (0..rng.random_range(1..5))
                .map(|i| (format!("f{}", i + rng.random_range(0..10)), bytes(rng)))
                .collect(),
        ),
        15 => Command::HSetNx(k, format!("f{}", rng.random_range(0..12)), bytes(rng)),
        16 => Command::HIncrBy(
            k,
            format!("f{}", rng.random_range(0..12)),
            rng.random_range(-99..99),
        ),
        17 => Command::HIncrByFloat(
            k,
            format!("f{}", rng.random_range(0..12)),
            [1e-300, -2.5, 1e300, 0.1][rng.random_range(0..4)],
        ),
        18 => Command::LPush(k, (0..rng.random_range(1..4)).map(|_| bytes(rng)).collect()),
        19 => Command::RPush(k, (0..rng.random_range(1..4)).map(|_| bytes(rng)).collect()),
        20 => Command::LPushX(k, vec![bytes(rng)]),
        21 => Command::RPushX(k, vec![bytes(rng)]),
        22 => Command::SAdd(k, (0..rng.random_range(1..5)).map(|_| text(rng)).collect()),
        23 => Command::EAdd(k, vec![text(rng)]),
        24 => Command::ZAdd(
            k,
            ZAddOptions {
                incr: rng.random_bool(0.2),
                ..ZAddOptions::default()
            },
            vec![(
                rng.random_range(-5.0..5.0),
                format!("z{}", rng.random_range(0..8)),
            )],
        ),
        25 => Command::ZIncrBy(k, 1e300, format!("z{}", rng.random_range(0..8))),
        26 => Command::SMove(k, key(rng), format!("m{}", rng.random_range(0..12))),
        27 => Command::RlSet(k, rng.random_range(1..10), rng.random_range(1..5_000)),
        28 => Command::RlCheck(k, None),
        29 => Command::Rename(k, key(rng)),
        30 => Command::RenameNx(k, key(rng)),
        31 => Command::HDel(k, vec![format!("f{}", rng.random_range(0..12))]),
        32 => Command::LPop(k, Some(rng.random_range(1..3))),
        33 => Command::LRem(k, 0, bytes(rng)),
        34 => Command::LTrim(k, 1, -2),
        35 => Command::SRem(k, vec![format!("m{}", rng.random_range(0..12))]),
        36 => Command::SPop(k, Some(1)),
        _ => Command::ZPopMin(k, Some(1)),
    }
}

#[test]
fn no_command_grows_memory_past_its_bound() {
    let mut rng = Rng64::seed_from_u64(0x6b_e0_b0);
    let mut checked = 0usize;
    let mut worst: Option<(usize, usize, String)> = None;
    for trial in 0..40_000 {
        let store = KeyValueStore::new();
        for k in ["k", "d", "s", "a-much-longer-key-name-for-renames"] {
            seed(&store, k, &mut rng);
        }
        let cmd = bounded_command(&mut rng);
        let Some(bound) = growth_bound(&cmd) else {
            panic!("trial {trial}: {cmd:?} was meant to have a bound");
        };
        let before = store.approximate_memory_bytes();
        let reply = store.execute(cmd.clone());
        let after = store.approximate_memory_bytes();
        let grew = after.saturating_sub(before);
        assert!(
            grew <= bound,
            "trial {trial}: {cmd:?} grew memory by {grew} > bound {bound} (reply {reply:?})"
        );
        checked += 1;
        // Tightest margin seen, for the report.
        let margin = bound - grew;
        if grew > 0 && worst.as_ref().is_none_or(|(m, _, _)| margin < *m) {
            worst = Some((margin, grew, format!("{cmd:?}")));
        }
    }
    assert_eq!(checked, 40_000);
    if let Some((margin, grew, cmd)) = worst {
        println!("tightest: grew {grew}, margin {margin}: {cmd:.120}");
    }
}

#[test]
fn every_bounded_command_kind_was_generated() {
    // Guards the test above against a generator that silently stops
    // producing some command (and so stops checking its bound).
    let mut rng = Rng64::seed_from_u64(1);
    let mut kinds = HashSet::new();
    for _ in 0..20_000 {
        let cmd = bounded_command(&mut rng);
        kinds.insert(std::mem::discriminant(&cmd));
    }
    // 38 arms; two of them are `Set`.
    assert_eq!(
        kinds.len(),
        37,
        "every command kind bounded_command has was produced"
    );
}
