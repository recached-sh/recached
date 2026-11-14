//! In-process cost of small-hash commands by field count, for choosing where
//! a packed hash should become a hash table (Phase 6G-C).
//!
//! For each field count N it builds 20,000 hashes of N fields (32-byte
//! values) and times, per operation: HSET of a new field while building,
//! HGET of a random existing field, HSET overwriting a random field, and
//! HGETALL. Every scenario is the minimum of `REPS` runs, as in `hotpath.rs`.
//! Engine cost only, through `KeyValueStore::execute`.
//!
//! ```text
//! cargo run --release -p core-engine --example hashpath
//! ```

use core_engine::cmd::Command;
use core_engine::store::KeyValueStore;
use std::time::Instant;

const HASHES: usize = 20_000;
const REPS: usize = 5;
const FIELD_COUNTS: [usize; 11] = [1, 2, 3, 4, 8, 16, 24, 32, 48, 64, 128];

fn field(i: usize) -> String {
    format!("field:{i:04}")
}

fn rng(seed: u64) -> impl FnMut() -> u64 {
    let mut state = seed | 1;
    move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    }
}

fn min_ns(mut run: impl FnMut() -> (u128, usize)) -> f64 {
    (0..REPS)
        .map(|_| {
            let (ns, ops) = run();
            ns as f64 / ops as f64
        })
        .fold(f64::INFINITY, f64::min)
}

fn build(n: usize) -> KeyValueStore {
    let s = KeyValueStore::new();
    for h in 0..HASHES {
        for f in 0..n {
            s.execute(Command::HSet(
                format!("h:{h}"),
                vec![(field(f), vec![b'v'; 32])],
            ));
        }
    }
    s
}

fn main() {
    println!("fields,hset_build_ns,hget_ns,hset_overwrite_ns,hgetall_ns_per_field");
    for n in FIELD_COUNTS {
        let build_ns = min_ns(|| {
            let t = Instant::now();
            let s = build(n);
            std::hint::black_box(&s);
            (t.elapsed().as_nanos(), HASHES * n)
        });
        let s = build(n);
        let ops = 400_000;
        let hget = min_ns(|| {
            let mut r = rng(7);
            let t = Instant::now();
            for _ in 0..ops {
                let x = r();
                let h = x as usize % HASHES;
                let f = (x >> 32) as usize % n;
                std::hint::black_box(s.execute(Command::HGet(format!("h:{h}"), field(f))));
            }
            (t.elapsed().as_nanos(), ops)
        });
        let hset = min_ns(|| {
            let mut r = rng(11);
            let t = Instant::now();
            for _ in 0..ops {
                let x = r();
                let h = x as usize % HASHES;
                let f = (x >> 32) as usize % n;
                s.execute(Command::HSet(
                    format!("h:{h}"),
                    vec![(field(f), vec![b'w'; 32])],
                ));
            }
            (t.elapsed().as_nanos(), ops)
        });
        let getall_ops = 50_000;
        let hgetall = min_ns(|| {
            let mut r = rng(13);
            let t = Instant::now();
            for _ in 0..getall_ops {
                let h = r() as usize % HASHES;
                std::hint::black_box(s.execute(Command::HGetAll(format!("h:{h}"))));
            }
            (t.elapsed().as_nanos(), getall_ops * n)
        });
        println!("{n},{build_ns:.1},{hget:.1},{hset:.1},{hgetall:.1}");
    }
}
