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
use std::collections::{BTreeSet, BinaryHeap, VecDeque};
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
    /// One page for a cursor that keeps state between calls. Stateless
    /// indexes resume from the cursor's last key.
    fn page_cursor(&self, cursor: &mut Cursor, count: usize) -> Page {
        let page = self.page(cursor.after.as_deref(), count);
        if let Some(last) = page.keys.last() {
            cursor.after = Some(Arc::clone(last));
        }
        page
    }
}

/// What a SCAN cursor keeps between calls: its last key, and for the cached
/// sharded index, the merge frontier.
#[derive(Default)]
struct Cursor {
    after: Option<Key>,
    frontier: Option<Frontier>,
}

/// A cursor's merge frontier over the shards (6L-C).
///
/// For each shard: a buffer holding a contiguous run of that shard's keys as
/// they stood when fetched, and the last key fetched, from which the next
/// refill resumes. `heap` holds each non-empty buffer's first key.
///
/// Why a stable key — present for the whole iteration — is returned exactly
/// once and in order:
/// - A refill takes every key of the shard after `pos`, up to `batch`, so a
///   stable key between two fetched keys was fetched; keys inserted later
///   into a run already fetched are not stable for that part of the
///   iteration and may be missed, as the contract allows.
/// - No shard that may still hold keys is ever empty while the merge pops:
///   it is refilled first (from its own `pos`, under its own lock). So every
///   pop is the minimum over every shard's next key, and output is in byte
///   order across calls.
/// - A shard is marked exhausted only when a refill came back short: every
///   key it held after `pos` at that moment was taken.
struct Frontier {
    bufs: Vec<VecDeque<Key>>,
    pos: Vec<Option<Key>>,
    exhausted: Vec<bool>,
    /// Shards with a non-empty buffer, ordered by their buffer's first key.
    /// Kept by insertion instead of a heap of cloned keys: a returned key is
    /// moved out of its buffer, with no reference-count traffic, and
    /// re-placing one shard costs ~log2(16) comparisons.
    order: VecDeque<(u128, usize)>,
}

/// A key's first 16 bytes as a big-endian integer, zero-padded. For two
/// keys, `prefix(a) < prefix(b)` implies `a < b` and `prefix(a) > prefix(b)`
/// implies `a > b` in byte order; equal prefixes say nothing (one key may be
/// a zero-padded prefix of the other), so ties fall back to comparing keys.
fn prefix(key: &str) -> u128 {
    let mut bytes = [0u8; 16];
    let head = &key.as_bytes()[..key.len().min(16)];
    bytes[..head.len()].copy_from_slice(head);
    u128::from_be_bytes(bytes)
}

impl Frontier {
    /// Place shard `i` (non-empty buffer) into `order` by its first key.
    fn place(&mut self, i: usize) {
        let Some(head) = self.bufs[i].front() else { return };
        let p = prefix(head);
        let bufs = &self.bufs;
        let at = self.order.partition_point(|&(q, j)| {
            q < p || (q == p && bufs[j].front().is_some_and(|h| h < head))
        });
        self.order.insert(at, (p, i));
    }
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

/// [`Sharded`] whose SCAN keeps a per-cursor merge frontier instead of
/// seeking every shard on every call (6L-C).
struct ShardedCached {
    inner: Sharded,
    batch: usize,
}

impl ShardedCached {
    /// Refill shard `i`'s buffer from its own resume point. Returns the
    /// number of index entries read.
    fn refill(&self, f: &mut Frontier, i: usize) -> usize {
        let set = lock(&self.inner.shards[i]);
        let lower = f.pos[i].as_deref().map_or(Bound::Unbounded, Bound::Excluded);
        let buf = &mut f.bufs[i];
        buf.extend(set.range::<str, _>((lower, Bound::Unbounded)).take(self.batch).cloned());
        drop(set);
        let got = buf.len();
        if got < self.batch {
            f.exhausted[i] = true;
        }
        if let Some(last) = buf.back() {
            f.pos[i] = Some(Arc::clone(last));
        }
        f.place(i);
        got
    }
}

impl OrderedIndex for ShardedCached {
    fn insert(&self, key: &Key) {
        self.inner.insert(key);
    }
    fn remove(&self, key: &str) {
        self.inner.remove(key);
    }
    fn page(&self, after: Option<&str>, count: usize) -> Page {
        self.inner.page(after, count)
    }
    fn page_cursor(&self, cursor: &mut Cursor, count: usize) -> Page {
        let n = self.inner.shards.len();
        let mut touched = 0usize;
        if cursor.frontier.is_none() {
            let mut f = Frontier {
                bufs: (0..n).map(|_| VecDeque::with_capacity(self.batch)).collect(),
                pos: vec![cursor.after.clone(); n],
                exhausted: vec![false; n],
                order: VecDeque::with_capacity(n),
            };
            for i in 0..n {
                touched += self.refill(&mut f, i);
            }
            cursor.frontier = Some(f);
        }
        let f = cursor.frontier.as_mut().expect("frontier set above");
        let mut keys = Vec::with_capacity(count);
        while keys.len() < count {
            let Some((_, i)) = f.order.pop_front() else {
                break;
            };
            let key = f.bufs[i].pop_front().expect("an ordered shard has a key");
            if !f.bufs[i].is_empty() {
                f.place(i);
            } else if !f.exhausted[i] {
                touched += self.refill(f, i);
            }
            keys.push(key);
        }
        if let Some(last) = keys.last() {
            cursor.after = Some(Arc::clone(last));
        }
        Page {
            more: !f.order.is_empty(),
            keys,
            touched,
        }
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
        v if v.starts_with("sharded16c") => Box::new(ShardedCached {
            inner: Sharded::new(16),
            // `sharded16c<batch>`, e.g. sharded16c32; default 16.
            batch: v["sharded16c".len()..].parse().unwrap_or(16).max(1),
        }),
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
    // SCAN_REPEAT=n: n untimed traversals first, so a profiler attached to
    // the process sees the traversal rather than the setup.
    let repeat: usize = std::env::var("SCAN_REPEAT").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    for _ in 0..repeat {
        let mut c = Cursor::default();
        while index.page_cursor(&mut c, count).more {}
    }
    let start = Instant::now();
    let mut cursor = Cursor::default();
    let (mut calls, mut max_touched, mut total_touched, mut seen) = (0u64, 0usize, 0usize, 0usize);
    let mut ordered = true;
    let mut last: Option<Key> = None;
    let mut slowest_call = 0f64;
    loop {
        let t = Instant::now();
        let page = index.page_cursor(&mut cursor, count);
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
        let mut cursor = Cursor::default();
        let mut last: Option<Key> = None;
        let mut got: Vec<Key> = Vec::new();
        let mut ordered = true;
        loop {
            let page = index.page_cursor(&mut cursor, count);
            for k in &page.keys {
                if last.as_ref().is_some_and(|l| l >= k) {
                    ordered = false;
                }
                last = Some(Arc::clone(k));
                if k.starts_with("s:") && k.len() == 8 {
                    got.push(Arc::clone(k));
                }
            }
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
