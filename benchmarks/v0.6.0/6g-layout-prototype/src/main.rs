//! Phase 6G-B: what each candidate keyspace layout costs per key.
//!
//! The types below mirror `core-engine`'s real ones byte for byte (sizes are
//! asserted), but with the collection payloads stubbed: only the inline
//! footprint matters to a string-keyspace measurement.
//!
//! Layouts (one per process, so RSS is not polluted by an earlier run):
//!
//! | name      | map                                   | ordered index              |
//! |-----------|---------------------------------------|----------------------------|
//! | `current` | `DashMap<String, Entry56>`            | `BTreeSet<Arc<str>>`, own copy |
//! | `boxed`   | `DashMap<Arc<str>, Box<Entry56>>`     | shares the map's `Arc`     |
//! | `inline32`| `DashMap<Arc<str>, Box<Entry64>>`, values ≤ 32 B inline | shares the map's `Arc` |
//! | `thin`    | `DashMap<ThinKey, Box<Entry64>>`, 8-byte keys | shares the map's `ThinKey` |
//! | `noindex` | `DashMap<Box<str>, Box<Entry64>>`     | none (lower bound)         |
//!
//! ```text
//! cargo run --release -- <layout> [keys] [value-bytes]
//! ```
//!
//! Reports, per key: requested live bytes (what the program asked for),
//! jemalloc `stats.allocated` (what it was given, size classes included),
//! jemalloc `stats.resident`, and on Linux the process `VmRSS`. Plus
//! single-threaded ns/op for insert, random-order get, overwrite and delete.

use dashmap::DashMap;
use smallvec::SmallVec;
use std::alloc::{GlobalAlloc, Layout};
use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
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

// ── Mirrors of the engine's types ────────────────────────────────────────────

type Blob24 = SmallVec<[u8; 24]>;
type Blob32 = SmallVec<[u8; 32]>;

#[allow(dead_code)]
enum Value56 {
    Str(Blob24),
    Hash(Box<u64>),
    List(Box<u64>),
    Set(Box<u64>),
    ZSet(Box<u64>),
    RateLimiter(Box<u64>),
    Json(serde_json::Value),
}

#[allow(dead_code)]
enum Value64 {
    Str(Blob32),
    Hash(Box<u64>),
    List(Box<u64>),
    Set(Box<u64>),
    ZSet(Box<u64>),
    RateLimiter(Box<u64>),
    Json(serde_json::Value),
}

#[allow(dead_code)]
struct Entry56 {
    value: Value56,
    deadline: Option<NonZeroU64>,
    last_access_ms: AtomicU64,
}

#[allow(dead_code)]
struct Entry64 {
    value: Value64,
    deadline: Option<NonZeroU64>,
    last_access_ms: AtomicU64,
}

const _: () = assert!(size_of::<Blob24>() == 32);
const _: () = assert!(size_of::<Entry56>() == 56);
const _: () = assert!(size_of::<Entry64>() == 64);

fn e56(v: &[u8]) -> Entry56 {
    Entry56 {
        value: Value56::Str(SmallVec::from_slice(v)),
        deadline: None,
        last_access_ms: AtomicU64::new(1),
    }
}

fn e64(v: &[u8]) -> Entry64 {
    Entry64 {
        value: Value64::Str(SmallVec::from_slice(v)),
        deadline: None,
        last_access_ms: AtomicU64::new(1),
    }
}

fn len56(e: &Entry56) -> usize {
    match &e.value {
        Value56::Str(b) => b.len(),
        _ => 0,
    }
}

fn len64(e: &Entry64) -> usize {
    match &e.value {
        Value64::Str(b) => b.len(),
        _ => 0,
    }
}


/// An 8-byte key: one allocation holding the length and the bytes
/// (`triomphe::ThinArc`), borrowed as `[u8]` for lookups.
#[derive(Clone)]
struct ThinKey(triomphe::ThinArc<(), u8>);

impl ThinKey {
    fn new(key: &str) -> Self {
        ThinKey(triomphe::ThinArc::from_header_and_slice((), key.as_bytes()))
    }
    fn bytes(&self) -> &[u8] {
        &self.0.slice
    }
}
impl PartialEq for ThinKey {
    fn eq(&self, o: &Self) -> bool {
        self.bytes() == o.bytes()
    }
}
impl Eq for ThinKey {}
impl std::hash::Hash for ThinKey {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.bytes().hash(h)
    }
}
impl PartialOrd for ThinKey {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for ThinKey {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.bytes().cmp(o.bytes())
    }
}
impl std::borrow::Borrow<[u8]> for ThinKey {
    fn borrow(&self) -> &[u8] {
        self.bytes()
    }
}
const _: () = assert!(size_of::<ThinKey>() == 8);

// ── Measurement ──────────────────────────────────────────────────────────────

fn key(i: usize) -> String {
    format!("key:{i:012}")
}

fn vm_rss() -> Option<usize> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kb: usize = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

struct Snap {
    live: usize,
    allocated: usize,
    resident: usize,
    rss: Option<usize>,
}

fn snap() -> Snap {
    tikv_jemalloc_ctl::epoch::advance().expect("epoch");
    Snap {
        live: LIVE.load(Ordering::Relaxed),
        allocated: tikv_jemalloc_ctl::stats::allocated::read().expect("allocated"),
        resident: tikv_jemalloc_ctl::stats::resident::read().expect("resident"),
        rss: vm_rss(),
    }
}

fn report(layout: &str, phase: &str, base: &Snap, now: &Snap, keys: usize) {
    let per = |a: usize, b: usize| (a as f64 - b as f64) / keys as f64;
    let rss = match (now.rss, base.rss) {
        (Some(a), Some(b)) => format!("{:.1}", per(a, b)),
        _ => "n/a".into(),
    };
    println!(
        "{layout},{phase},{keys},{:.1},{:.1},{:.1},{rss}",
        per(now.live, base.live),
        per(now.allocated, base.allocated),
        per(now.resident, base.resident),
    );
}

/// A full-period permutation of 0..n (n coprime to the stride), so gets touch
/// keys in an order the load did not.
fn shuffled(n: usize) -> impl Iterator<Item = usize> {
    let mut stride = 2_654_435_761usize % n.max(2);
    while gcd(stride, n) != 1 {
        stride += 1;
    }
    (0..n).map(move |i| (i * stride) % n)
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn timed(label: &str, layout: &str, n: usize, f: impl FnOnce() -> usize) {
    let t = Instant::now();
    let sink = f();
    let ns = t.elapsed().as_nanos() as f64 / n as f64;
    println!("{layout},ns/op {label},{n},{ns:.1},,,,  # sink {sink}");
}

macro_rules! run_layout {
    ($layout:expr, $n:expr, $val:expr, $map:expr, $mk_key:expr, $mk_entry:expr, $len:expr, $index:expr) => {{
        let layout = $layout;
        let n = $n;
        let keys: Vec<String> = (0..n).map(key).collect();
        let base = snap();
        let map = $map;
        let mut index: Option<BTreeSet<Arc<str>>> = $index;
        timed("insert", layout, n, || {
            for k in &keys {
                let (mk, shared) = $mk_key(k.as_str());
                if let (Some(index), Some(shared)) = (index.as_mut(), shared) {
                    index.insert(shared);
                }
                map.insert(mk, $mk_entry($val));
            }
            map.len()
        });
        // keys Vec is measured as part of the base? No: it was built before
        // `base`, so it is excluded.
        report(layout, "loaded", &base, &snap(), n);
        timed("get", layout, n, || {
            let mut total = 0;
            for i in shuffled(n) {
                if let Some(e) = map.get(keys[i].as_str()) {
                    total += $len(&*e);
                }
            }
            total
        });
        timed("overwrite", layout, n, || {
            for i in shuffled(n) {
                if let Some(mut e) = map.get_mut(keys[i].as_str()) {
                    *e = $mk_entry($val).into();
                }
            }
            map.len()
        });
        report(layout, "overwritten", &base, &snap(), n);
        timed("delete", layout, n, || {
            for k in &keys {
                map.remove(k.as_str());
                if let Some(index) = index.as_mut() {
                    index.remove(k.as_str());
                }
            }
            map.len()
        });
        report(layout, "deleted", &base, &snap(), n);
        map.shrink_to_fit();
        report(layout, "shrunk", &base, &snap(), n);
        drop(index);
    }};
}

fn main() {
    let mut args = std::env::args().skip(1);
    let layout = args.next().unwrap_or_else(|| "current".into());
    let n: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(1_000_000);
    let size: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(32);
    let val = vec![b'x'; size];
    let val = val.as_slice();

    match layout.as_str() {
        "current" => run_layout!(
            "current",
            n,
            val,
            DashMap::<String, Entry56>::new(),
            |k: &str| (k.to_string(), Some(Arc::<str>::from(k))),
            e56,
            len56,
            Some(BTreeSet::new())
        ),
        "boxed" => run_layout!(
            "boxed",
            n,
            val,
            DashMap::<Arc<str>, Box<Entry56>>::new(),
            |k: &str| {
                let a: Arc<str> = Arc::from(k);
                (Arc::clone(&a), Some(a))
            },
            |v| Box::new(e56(v)),
            len56,
            Some(BTreeSet::new())
        ),
        "inline32" => run_layout!(
            "inline32",
            n,
            val,
            DashMap::<Arc<str>, Box<Entry64>>::new(),
            |k: &str| {
                let a: Arc<str> = Arc::from(k);
                (Arc::clone(&a), Some(a))
            },
            |v| Box::new(e64(v)),
            len64,
            Some(BTreeSet::new())
        ),
        "noindex" => run_layout!(
            "noindex",
            n,
            val,
            DashMap::<Box<str>, Box<Entry64>>::new(),
            |k: &str| (Box::<str>::from(k), None::<Arc<str>>),
            |v| Box::new(e64(v)),
            len64,
            None
        ),
        "thin" => {
            let layout = "thin";
            let keys: Vec<String> = (0..n).map(key).collect();
            let base = snap();
            let map = DashMap::<ThinKey, Box<Entry64>>::new();
            let mut index: BTreeSet<ThinKey> = BTreeSet::new();
            timed("insert", layout, n, || {
                for k in &keys {
                    let key = ThinKey::new(k);
                    index.insert(key.clone());
                    map.insert(key, Box::new(e64(val)));
                }
                map.len()
            });
            report(layout, "loaded", &base, &snap(), n);
            timed("get", layout, n, || {
                let mut total = 0;
                for i in shuffled(n) {
                    if let Some(e) = map.get(keys[i].as_bytes()) {
                        total += len64(&e);
                    }
                }
                total
            });
            timed("overwrite", layout, n, || {
                for i in shuffled(n) {
                    if let Some(mut e) = map.get_mut(keys[i].as_bytes()) {
                        **e = e64(val);
                    }
                }
                map.len()
            });
            report(layout, "overwritten", &base, &snap(), n);
            timed("delete", layout, n, || {
                for k in &keys {
                    map.remove(k.as_bytes());
                    index.remove(k.as_bytes());
                }
                map.len()
            });
            report(layout, "deleted", &base, &snap(), n);
        }
        other => panic!("unknown layout {other}"),
    }
}
