//! In-process timing of the store's hot paths, for comparing two builds of
//! `core-engine` on a host too noisy for network benchmarks.
//!
//! No sockets, no load generator: each operation is `KeyValueStore::execute`
//! on one thread. Every scenario runs `REPS` times and the **minimum** ns/op is
//! reported alongside the median — the minimum is the run least disturbed by
//! whatever else the machine was doing, which is what makes two builds
//! comparable on a busy laptop. It measures engine CPU cost only, not
//! networking, parsing or the server's propagation path.
//!
//! ```text
//! cargo run --release -p core-engine --example hotpath
//! ```
//!
//! Uses only API that exists in v0.5 and v0.6, so the same file runs against
//! either tree.

use core_engine::cmd::{Command, SetOptions};
use core_engine::store::KeyValueStore;
use std::time::Instant;

const KEYS: usize = 100_000;
const OPS: usize = 1_000_000;
const REPS: usize = 9;

fn key(i: usize) -> String {
    format!("key:{:06}", i % KEYS)
}

fn value() -> Vec<u8> {
    vec![b'x'; 32]
}

/// Deterministic pseudo-random key order, the same for every build.
fn order() -> Vec<usize> {
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    (0..OPS)
        .map(|_| {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            (state.wrapping_mul(0x2545_F491_4F6C_DD1D) % KEYS as u64) as usize
        })
        .collect()
}

fn loaded() -> KeyValueStore {
    let store = KeyValueStore::new();
    for i in 0..KEYS {
        store.execute(Command::Set(key(i), value(), SetOptions::default()));
    }
    store
}

fn time(
    name: &str,
    mut setup: impl FnMut() -> KeyValueStore,
    run: impl Fn(&KeyValueStore, &[usize]),
) {
    let idx = order();
    let mut samples: Vec<f64> = (0..REPS)
        .map(|_| {
            let store = setup();
            let start = Instant::now();
            run(&store, &idx);
            start.elapsed().as_nanos() as f64 / OPS as f64
        })
        .collect();
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "{name:<28} min {:>7.1} ns/op   median {:>7.1} ns/op",
        samples[0],
        samples[REPS / 2]
    );
}

fn main() {
    time("SET overwrite (existing)", loaded, |s, idx| {
        for &i in idx {
            s.execute(Command::Set(key(i), value(), SetOptions::default()));
        }
    });
    time("SET new keys", KeyValueStore::new, |s, idx| {
        for (n, _) in idx.iter().enumerate() {
            s.execute(Command::Set(
                format!("new:{n:07}"),
                value(),
                SetOptions::default(),
            ));
        }
    });
    time("GET hit", loaded, |s, idx| {
        for &i in idx {
            s.execute(Command::Get(key(i)));
        }
    });
    time("INCR", KeyValueStore::new, |s, idx| {
        for &i in idx {
            s.execute(Command::Incr(key(i)));
        }
    });
    time("SET+PEXPIRE (volatile)", loaded, |s, idx| {
        for &i in idx {
            s.execute(Command::PExpire(key(i), 3_600_000));
        }
    });
    time("DEL then SET (churn)", loaded, |s, idx| {
        for &i in idx {
            s.execute(Command::Del(vec![key(i)]));
            s.execute(Command::Set(key(i), value(), SetOptions::default()));
        }
    });
    time("HSET field overwrite", KeyValueStore::new, |s, idx| {
        for &i in idx {
            s.execute(Command::HSet(key(i), vec![("f".into(), value())]));
        }
    });
}
