//! Live heap bytes of the store across a small-hash load / overwrite / delete
//! cycle — the scenario where server RSS roughly doubled on overwrite in the
//! v0.6 memory baseline.
//!
//! RSS mixes what the program holds with what the allocator keeps after a
//! free. This example counts the bytes the program actually holds, exactly:
//! a counting wrapper around the system allocator records every allocation
//! and free. If live bytes stay flat across an overwrite while RSS grows, the
//! growth is the allocator retaining freed pages, not the store holding
//! memory.
//!
//! Commands go through the same path as the server's: RESP bytes →
//! `Value::parse` → `Command::from_value` → `execute`, so buffer capacity left
//! over from parsing would be counted too.
//!
//! ```text
//! CARGO_TARGET_DIR=target/hash-live-bytes cargo run --release \
//!   --manifest-path benchmarks/v0.6.0/hash-memory-20261114/live-bytes/Cargo.toml -- [keys] [value-bytes] [overwrites]
//!
//! (A standalone crate: the
//! workspace forbids `unsafe`, which a counting allocator needs)
//! ```
//!
//! Defaults: 1,000,000 keys, 32-byte values, 3 overwrites. Uses only API that
//! exists in v0.5 and v0.6, so the same file runs against either tree.

use core_engine::cmd::Command;
use core_engine::resp::Value;
use core_engine::store::KeyValueStore;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// The system allocator, counting. Relaxed is enough: every count is read on
/// the thread that did the work, after it finished.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOCS: AtomicU64 = AtomicU64::new(0);

// SAFETY: every method forwards to `System` with the caller's own arguments,
// so `System`'s guarantees carry over unchanged; the counters are side
// effects that never touch the memory itself.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded unchanged; the caller upholds `alloc`'s contract.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let now = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged; `ptr` came from this allocator with
        // this layout, as the caller guarantees.
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarded unchanged; the caller upholds `realloc`'s contract.
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            let now = LIVE.fetch_add(new_size, Ordering::Relaxed) + new_size;
            PEAK.fetch_max(now, Ordering::Relaxed);
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        p
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn arg(n: usize, default: usize) -> usize {
    std::env::args()
        .nth(n)
        .and_then(|a| a.parse().ok())
        .unwrap_or(default)
}

fn encode(parts: &[&[u8]], out: &mut Vec<u8>) {
    out.clear();
    out.extend_from_slice(format!("*{}\r\n", parts.len()).as_bytes());
    for p in parts {
        out.extend_from_slice(format!("${}\r\n", p.len()).as_bytes());
        out.extend_from_slice(p);
        out.extend_from_slice(b"\r\n");
    }
}

/// Parse and execute one command exactly as the server's read loop does.
fn run(store: &KeyValueStore, wire: &[u8]) {
    let (value, _) = Value::parse(wire).expect("well-formed frame");
    let cmd = Command::from_value(value).expect("known command");
    let reply = store.execute(cmd);
    if let Value::Error(e) = reply {
        panic!("command failed: {e}");
    }
}

fn hset_all(store: &KeyValueStore, keys: usize, val: &[u8]) {
    let kind = std::env::var("KIND").unwrap_or_else(|_| "hash".into());
    let mut wire = Vec::new();
    for i in 0..keys {
        let k = format!("mem:{i:010}");
        let k = k.as_bytes();
        match kind.as_str() {
            // Three members, re-added unchanged: the overwrite case for sets.
            "set" => encode(&[b"SADD", k, b"member-one", b"member-two", b"member-three"], &mut wire),
            "string" => encode(&[b"SET", k, val], &mut wire),
            "string-ttl" => encode(&[b"SET", k, val, b"EX", b"3600"], &mut wire),
            "zset" => encode(&[b"ZADD", k, b"1", b"member-one", b"2", b"member-two", b"3", b"member-three"], &mut wire),
            _ => encode(&[b"HSET", k, b"f1", val, b"f2", val, b"f3", val], &mut wire),
        }
        run(store, &wire);
    }
}

fn main() {
    let keys = arg(1, 1_000_000);
    let size = arg(2, 32);
    let overwrites = arg(3, 3);
    let val = vec![b'x'; size];

    let base = LIVE.load(Ordering::Relaxed);
    let store = KeyValueStore::new();
    let mut last = LIVE.load(Ordering::Relaxed);
    let report = |phase: &str, last: &mut usize| {
        let live = LIVE.load(Ordering::Relaxed);
        let peak = PEAK.swap(live, Ordering::Relaxed);
        let delta = live as i64 - *last as i64;
        println!(
            "{phase:<14} live {:>12} B  ({:>8.1} B/key)  Δ {:>+12} B  peak-in-phase {:>12} B  allocs {:>11}",
            live.saturating_sub(base),
            live.saturating_sub(base) as f64 / keys.max(1) as f64,
            delta,
            peak.saturating_sub(base),
            ALLOCS.load(Ordering::Relaxed),
        );
        *last = live;
    };

    println!(
        "kind={} keys={keys} value={size} B fields=3 overwrites={overwrites}",
        std::env::var("KIND").unwrap_or_else(|_| "hash".into())
    );
    report("empty", &mut last);
    hset_all(&store, keys, &val);
    report("loaded", &mut last);
    for n in 1..=overwrites {
        hset_all(&store, keys, &val);
        report(&format!("overwrite {n}"), &mut last);
    }
    let mut wire = Vec::new();
    for i in 0..keys {
        let k = format!("mem:{i:010}");
        encode(&[b"DEL", k.as_bytes()], &mut wire);
        run(&store, &wire);
    }
    report("deleted", &mut last);
    hset_all(&store, keys, &val);
    report("reloaded", &mut last);
    encode(&[b"FLUSHDB"], &mut wire);
    run(&store, &wire);
    report("flushed", &mut last);
}
