//! Phase 6J-B: what the ordered SCAN index costs key creation, and what its
//! contract-preserving alternatives would.
//!
//! The keyspace is modelled as the server holds it — `DashMap<Arc<str>,
//! Box<entry>>` with the key's `Arc` shared by the index — and every created
//! key is inserted into the map and then into the index, as
//! `KeyValueStore::execute_reporting*` does under its write-order stripe.
//!
//! | variant      | ordered index                                   | SCAN contract |
//! |--------------|-------------------------------------------------|---------------|
//! | `global`     | one `Mutex<BTreeSet<Arc<str>>>` (production)     | ordered, exactly once, bounded |
//! | `sharded16`  | 16 `Mutex<BTreeSet>` by key hash; SCAN k-way merges | the same |
//! | `sharded64`  | 64 shards                                       | the same |
//! | `none`       | no index: the lower bound, not a SCAN           | — |
//!
//! ```text
//! scan-index-prototype create <variant> <keys> <threads>   # then deletes them
//! scan-index-prototype scan   <variant> <keys> <count>
//! scan-index-prototype check  <variant>
//! ```
//! Each prints one JSON object.

use dashmap::DashMap;
use std::alloc::{GlobalAlloc, Layout};
use std::collections::{BTreeSet, BinaryHeap};
use std::hash::BuildHasher;
use std::ops::Bound;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;
use tikv_jemallocator::Jemalloc;

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every method forwards to jemalloc with the caller's own arguments;
// the counter is a side effect that never touches the memory itself.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded unchanged.
        let p = unsafe { Jemalloc.alloc(layout) };
        if !p.is_null() {
            LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged.
        unsafe { Jemalloc.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarded unchanged.
        let p = unsafe { Jemalloc.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            LIVE.fetch_add(new_size, Ordering::Relaxed);
        }
        p
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

type Key = Arc<str>;
/// The server's entry is 64 bytes with a small value inline.
type Entry = Box<[u8; 64]>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// One page of a SCAN: up to `count` keys after `after`, whether more follow,
/// and how many index entries the call touched.
struct Page {
    keys: Vec<Key>,
    more: bool,
    touched: usize,
}

trait OrderedIndex: Sync + Send {
    fn insert(&self, key: &Key);
    fn remove(&self, key: &str);
    fn page(&self, after: Option<&str>, count: usize) -> Page;
}

struct Global(Mutex<BTreeSet<Key>>);

impl OrderedIndex for Global {
    fn insert(&self, key: &Key) {
        lock(&self.0).insert(Arc::clone(key));
    }
    fn remove(&self, key: &str) {
        lock(&self.0).remove(key);
    }
    fn page(&self, after: Option<&str>, count: usize) -> Page {
        let set = lock(&self.0);
        let lower = after.map_or(Bound::Unbounded, Bound::Excluded);
        let mut keys: Vec<Key> = set
            .range::<str, _>((lower, Bound::Unbounded))
            .take(count + 1)
            .cloned()
            .collect();
        let touched = keys.len();
        let more = keys.len() > count;
        keys.truncate(count);
        Page { keys, more, touched }
    }
}

/// The candidate: the same `BTreeSet`, split by key hash so key creation
/// takes one of N locks instead of one store-wide lock. SCAN keeps global
/// byte order by merging the shards' ranges after the cursor's last key.
struct Sharded {
    shards: Box<[Mutex<BTreeSet<Key>>]>,
    hasher: std::hash::RandomState,
}

impl Sharded {
    fn new(n: usize) -> Self {
        Sharded {
            shards: (0..n).map(|_| Mutex::new(BTreeSet::new())).collect(),
            hasher: std::hash::RandomState::new(),
        }
    }
    fn shard(&self, key: &str) -> &Mutex<BTreeSet<Key>> {
        let h = self.hasher.hash_one(key);
        let n = self.shards.len() as u64;
        &self.shards[(h % n) as usize]
    }
}

impl OrderedIndex for Sharded {
    fn insert(&self, key: &Key) {
        lock(self.shard(key)).insert(Arc::clone(key));
    }
    fn remove(&self, key: &str) {
        lock(self.shard(key)).remove(key);
    }
    fn page(&self, after: Option<&str>, count: usize) -> Page {
        // All shard locks, in index order: a SCAN page is one consistent cut,
        // and writers only ever hold one shard lock, so this cannot deadlock.
        let guards: Vec<_> = self.shards.iter().map(lock).collect();
        let lower = after.map_or(Bound::Unbounded, Bound::Excluded);
        let mut iters: Vec<_> = guards
            .iter()
            .map(|g| g.range::<str, _>((lower, Bound::Unbounded)))
            .collect();
        // Min-heap of each shard's next key.
        let mut heap = BinaryHeap::new();
        let mut touched = 0usize;
        for (i, it) in iters.iter_mut().enumerate() {
            if let Some(k) = it.next() {
                touched += 1;
                heap.push(std::cmp::Reverse((k, i)));
            }
        }
        let mut keys = Vec::with_capacity(count + 1);
        while keys.len() <= count {
            let Some(std::cmp::Reverse((k, i))) = heap.pop() else {
                break;
            };
            keys.push(Arc::clone(k));
            if let Some(next) = iters[i].next() {
                touched += 1;
                heap.push(std::cmp::Reverse((next, i)));
            }
        }
        let more = keys.len() > count;
        keys.truncate(count);
        Page { keys, more, touched }
    }
}

struct NoIndex;

impl OrderedIndex for NoIndex {
    fn insert(&self, _: &Key) {}
    fn remove(&self, _: &str) {}
    fn page(&self, _: Option<&str>, _: usize) -> Page {
        Page {
            keys: Vec::new(),
            more: false,
            touched: 0,
        }
    }
}

fn make(variant: &str) -> Box<dyn OrderedIndex> {
    match variant {
        "global" => Box::new(Global(Mutex::new(BTreeSet::new()))),
        "sharded16" => Box::new(Sharded::new(16)),
        "sharded64" => Box::new(Sharded::new(64)),
        "none" => Box::new(NoIndex),
        other => {
            eprintln!("unknown variant {other}");
            std::process::exit(64)
        }
    }
}

/// Key `i` of a run: a bijection of `i`, so creation order is scattered over
/// the key space the way memtier's random keys are.
fn key_of(i: u64) -> String {
    format!("memtier-{}", (i as u32).wrapping_mul(2_654_435_761))
}

fn cpu_secs() -> f64 {
    // SAFETY: getrusage writes into the zeroed struct we pass; RUSAGE_SELF
    // is a valid `who`.
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: as above.
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    let t = |tv: libc::timeval| tv.tv_sec as f64 + tv.tv_usec as f64 / 1e6;
    t(ru.ru_utime) + t(ru.ru_stime)
}

fn run_threads(threads: u64, keys: u64, f: impl Fn(u64) + Sync) -> (f64, f64) {
    let start = Instant::now();
    let cpu0 = cpu_secs();
    std::thread::scope(|s| {
        for t in 0..threads {
            let f = &f;
            s.spawn(move || {
                let mut i = t;
                while i < keys {
                    f(i);
                    i += threads;
                }
            });
        }
    });
    (start.elapsed().as_secs_f64(), cpu_secs() - cpu0)
}

fn create(variant: &str, keys: u64, threads: u64) {
    let names: Vec<String> = (0..keys).map(key_of).collect();
    let live0 = LIVE.load(Ordering::Relaxed);
    let map: DashMap<Key, Entry> = DashMap::new();
    let index = make(variant);
    let live_empty = LIVE.load(Ordering::Relaxed);
    let (wall, cpu) = run_threads(threads, keys, |i| {
        let key: Key = Arc::from(names[i as usize].as_str());
        map.insert(Arc::clone(&key), Box::new([0u8; 64]));
        index.insert(&key);
    });
    let live_full = LIVE.load(Ordering::Relaxed);
    let (dwall, dcpu) = run_threads(threads, keys, |i| {
        let name = names[i as usize].as_str();
        map.remove(name);
        index.remove(name);
    });
    let k = keys as f64;
    println!(
        "{{\"mode\":\"create\",\"variant\":\"{variant}\",\"keys\":{keys},\"threads\":{threads},\
         \"create_ops_per_sec\":{:.0},\"create_cpu_s_per_mop\":{:.3},\
         \"delete_ops_per_sec\":{:.0},\"delete_cpu_s_per_mop\":{:.3},\
         \"live_bytes_per_key\":{:.1},\"structures_bytes\":{}}}",
        k / wall,
        cpu / (k / 1e6),
        k / dwall,
        dcpu / (k / 1e6),
        (live_full - live_empty) as f64 / k,
        live_empty - live0,
    );
}

fn scan(variant: &str, keys: u64, count: usize) {
    let index = make(variant);
    let mut expect: Vec<Key> = (0..keys).map(|i| Arc::from(key_of(i).as_str())).collect();
    for k in &expect {
        index.insert(k);
    }
    expect.sort();
    let start = Instant::now();
    let mut after: Option<Key> = None;
    let (mut calls, mut max_touched, mut total_touched, mut seen) = (0u64, 0usize, 0usize, 0usize);
    let mut ordered = true;
    let mut last: Option<Key> = None;
    let mut slowest_call = 0f64;
    loop {
        let t = Instant::now();
        let page = index.page(after.as_deref(), count);
        slowest_call = slowest_call.max(t.elapsed().as_secs_f64());
        calls += 1;
        max_touched = max_touched.max(page.touched);
        total_touched += page.touched;
        for k in &page.keys {
            if last.as_ref().is_some_and(|l| l >= k) {
                ordered = false;
            }
            last = Some(Arc::clone(k));
            seen += 1;
        }
        after = page.keys.last().cloned().or(after);
        if !page.more {
            break;
        }
    }
    let wall = start.elapsed().as_secs_f64();
    println!(
        "{{\"mode\":\"scan\",\"variant\":\"{variant}\",\"keys\":{keys},\"count\":{count},\
         \"full_scan_s\":{wall:.4},\"calls\":{calls},\"max_touched_per_call\":{max_touched},\
         \"mean_touched_per_call\":{:.1},\"slowest_call_us\":{:.1},\"returned\":{seen},\
         \"exactly_once_in_order\":{}}}",
        total_touched as f64 / calls as f64,
        slowest_call * 1e6,
        ordered && seen == expect.len(),
    );
}

/// Correctness under concurrent change: keys present for the whole
/// iteration come back exactly once, in order, while other keys are created
/// and deleted around the cursor.
fn check(variant: &str) {
    let index: Arc<dyn OrderedIndex> = Arc::from(make(variant));
    let stable: Vec<Key> = (0..20_000u64)
        .map(|i| Arc::from(format!("s:{:06}", (i * 7919) % 20_000).as_str()))
        .collect();
    for k in &stable {
        index.insert(k);
    }
    let stop = Arc::new(AtomicBool::new(false));
    let churn: Vec<_> = (0..3u64)
        .map(|t| {
            let index = Arc::clone(&index);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                let mut i = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    // Keys sorting before, between and after the stable ones.
                    let k: Key = Arc::from(
                        match i % 3 {
                            0 => format!("a:{t}:{i}"),
                            1 => format!("s:{:06}x{t}", i % 20_000),
                            _ => format!("z:{t}:{i}"),
                        }
                        .as_str(),
                    );
                    index.insert(&k);
                    if i % 2 == 0 {
                        index.remove(&k);
                    }
                    i += 1;
                }
            })
        })
        .collect();
    let mut failures = 0;
    for count in [1usize, 7, 10, 100, 1000] {
        let mut after: Option<Key> = None;
        let mut last: Option<Key> = None;
        let mut got: Vec<Key> = Vec::new();
        let mut ordered = true;
        loop {
            let page = index.page(after.as_deref(), count);
            for k in &page.keys {
                if last.as_ref().is_some_and(|l| l >= k) {
                    ordered = false;
                }
                last = Some(Arc::clone(k));
                if k.starts_with("s:") && k.len() == 8 {
                    got.push(Arc::clone(k));
                }
            }
            after = page.keys.last().cloned().or(after);
            if !page.more {
                break;
            }
        }
        let mut sorted = stable.clone();
        sorted.sort();
        if !(ordered && got == sorted) {
            failures += 1;
            eprintln!(
                "{variant} count {count}: ordered={ordered} got {} of {}",
                got.len(),
                sorted.len()
            );
        }
    }
    stop.store(true, Ordering::Relaxed);
    for t in churn {
        let _ = t.join();
    }
    println!("{{\"mode\":\"check\",\"variant\":\"{variant}\",\"failures\":{failures}}}");
    if failures > 0 {
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    let num = |i: usize, d: u64| arg(i).parse().unwrap_or(d);
    match arg(1) {
        "create" => create(arg(2), num(3, 1_000_000), num(4, 1)),
        "scan" => scan(arg(2), num(3, 1_000_000), num(4, 10) as usize),
        "check" => check(arg(2)),
        _ => {
            eprintln!("usage: create|scan|check <variant> …");
            std::process::exit(64)
        }
    }
}
