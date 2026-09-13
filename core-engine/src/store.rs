use crate::cmd::{Command, SetCondition, SetExpiry, ZAddOptions};
use crate::resp::Value;
use dashmap::DashMap;
// Aliased: this module has its own `Entry` (the stored value + TTL + recency).
use dashmap::mapref::entry::Entry as DashEntry;
use indexmap::IndexSet;
use rand::Rng;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

const WRONGTYPE: &str = "WRONGTYPE Operation against a key holding the wrong kind of value";

// ── time ──────────────────────────────────────────────────────────────────────

/// Milliseconds since the Unix epoch.
///
/// `std::time::SystemTime::now()` panics outright on wasm32-unknown-unknown —
/// std has no clock for that target. Because this is called on essentially
/// every store operation (TTL checks, LRU recency), an unguarded call makes the
/// whole engine unusable in the browser, so the wasm build reads `Date.now()`.
#[cfg(target_arch = "wasm32")]
fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ── ZSet inner ────────────────────────────────────────────────────────────────

/// A sorted-set score, ordered totally so it can be a `BTreeSet` key.
///
/// `f64` is only `PartialOrd`: `NaN` compares false against everything, which
/// is why the old comparator had to fall back to `Ordering::Equal`. `total_cmp`
/// is a total order over every bit pattern, so the index stays well-formed even
/// if a `NaN` ever reached it (`ZADD`/`ZINCRBY` reject them at the door).
#[derive(Clone, Copy, PartialEq, Debug)]
struct Score(f64);

impl Eq for Score {}

impl Ord for Score {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A sorted set: a member→score map for point lookups, plus a score-ordered
/// index for range queries.
///
/// The index is the whole point. Every range command used to call a `rank_asc`
/// that collected *every* member into a `Vec` and sorted it — O(n log n) per
/// query, with an allocation proportional to the set, all while holding the
/// shard guard. `ZRANGE board 0 9` on a million-member leaderboard sorted a
/// million entries to return ten, and blocked every other key on that shard
/// while it did.
///
/// Members are shared between the two structures as `Arc<str>`, so the index
/// costs a pointer per member rather than a second copy of the string.
pub(crate) struct ZSetInner {
    scores: HashMap<Arc<str>, f64>,
    /// `(score, member)` ascending — the ordering every range command wants —
    /// built on first use and thrown away by the next write.
    ///
    /// Maintaining it on every write instead cost ~48% of `ZADD` throughput
    /// against a large set (measured: 394k → 204k ops/s pipelined), because a
    /// write that keeps a `BTreeSet` in step pays an O(log n) descent, string
    /// comparisons and a node allocation on top of the hash insert. Almost all
    /// of that is wasted when nobody asks for a range.
    ///
    /// So writes only invalidate, which is O(1) plus dropping whatever was
    /// built, and reads rebuild if they find it empty. The three workloads:
    ///
    ///   * write-only — never built, so writes cost exactly what they did
    ///     before the index existed;
    ///   * load-then-read — built once by the first range query, free after;
    ///   * write/read alternating — rebuilt per read, which is the O(n log n)
    ///     sort this replaced, so the floor is the old behaviour and never worse.
    ///
    /// `OnceLock` rather than a `RefCell`/`Mutex` because the entry sits behind
    /// a `DashMap` shard guard shared between readers: it initialises through
    /// `&self`, so a range query does not need an exclusive guard, and once
    /// built it is read with no synchronisation at all.
    index: std::sync::OnceLock<BTreeSet<(Score, Arc<str>)>>,
    /// Writes applied since a range query last used the ordering.
    ///
    /// Lets a write-dominated set shed an ordering nobody is reading any more,
    /// instead of paying to keep it in step forever because of one range query
    /// long ago. `AtomicU32` so a range query can reset it through `&self`.
    writes_since_range: AtomicU32,
    /// Running `member.len() + 8` total. See [`CompactHash`] for why this is
    /// maintained incrementally rather than summed on demand.
    bytes: usize,
}

/// Writes without an intervening range query after which a materialised
/// ordering is abandoned rather than maintained.
///
/// Scaled by the set's own size: rebuilding costs O(n log n), so waiting until
/// at least `n` writes have gone by means the eventual rebuild is amortised
/// against at least as much work as maintaining it would have cost. The floor
/// stops a tiny set from thrashing.
const INDEX_ABANDON_FLOOR: usize = 1024;

impl Clone for ZSetInner {
    fn clone(&self) -> Self {
        Self {
            scores: self.scores.clone(),
            index: self.index.clone(),
            writes_since_range: AtomicU32::new(self.writes_since_range.load(Ordering::Relaxed)),
            bytes: self.bytes,
        }
    }
}

impl ZSetInner {
    fn new() -> Self {
        Self {
            scores: HashMap::new(),
            index: std::sync::OnceLock::new(),
            writes_since_range: AtomicU32::new(0),
            bytes: 0,
        }
    }

    fn len(&self) -> usize {
        self.scores.len()
    }

    /// Sum of `member.len() + 8` (the score) across the set. O(1).
    fn heap_bytes(&self) -> usize {
        self.bytes
    }

    fn score(&self, member: &str) -> Option<f64> {
        self.scores.get(member).copied()
    }

    /// Inserts or updates `member`, returning its previous score.
    ///
    /// Both structures move together; a score change is a remove plus an insert
    /// in the index, because the score is part of the key.
    fn insert(&mut self, member: &str, score: f64) -> Option<f64> {
        match self.scores.get_key_value(member) {
            Some((key, &old)) => {
                if old.to_bits() == score.to_bits() {
                    // Same score: the ordering cannot have moved.
                    return Some(old);
                }
                let key = Arc::clone(key);
                self.reindex(Some(old), score, &key);
                self.scores.insert(key, score);
                Some(old)
            }
            None => {
                let key: Arc<str> = Arc::from(member);
                self.reindex(None, score, &key);
                self.scores.insert(Arc::clone(&key), score);
                // Only a genuinely new member changes the total; a score
                // update replaces eight bytes with eight bytes.
                self.bytes = self.bytes.saturating_add(member.len() + 8);
                None
            }
        }
    }

    /// Keeps a materialised ordering in step with a write — or abandons it.
    ///
    /// The rule is: maintain what exists, build nothing that doesn't. A set
    /// nobody has run a range query against has no ordering, so its writes cost
    /// exactly what they did before the index existed; a set being read keeps
    /// its ordering current so reads stay O(log n + k) instead of rebuilding.
    /// The counter catches the leftover case — one range query long ago,
    /// millions of writes since — by dropping an ordering that has stopped
    /// paying for itself.
    fn reindex(&mut self, old: Option<f64>, score: f64, key: &Arc<str>) {
        if self.index.get().is_none() {
            return;
        }
        let writes = self.writes_since_range.load(Ordering::Relaxed) as usize;
        if writes > INDEX_ABANDON_FLOOR.max(self.scores.len()) {
            self.index.take();
            self.writes_since_range.store(0, Ordering::Relaxed);
            return;
        }
        if let Some(index) = self.index.get_mut() {
            if let Some(old_score) = old {
                index.remove(&(Score(old_score), Arc::clone(key)));
            }
            index.insert((Score(score), Arc::clone(key)));
        }
        self.writes_since_range.fetch_add(1, Ordering::Relaxed);
    }

    /// The score-ordered view, built on first use.
    ///
    /// Initialising through `&self` is what lets range queries run under a
    /// shared shard guard rather than an exclusive one.
    fn index(&self) -> &BTreeSet<(Score, Arc<str>)> {
        self.writes_since_range.store(0, Ordering::Relaxed);
        self.index.get_or_init(|| {
            self.scores
                .iter()
                .map(|(m, &s)| (Score(s), Arc::clone(m)))
                .collect()
        })
    }

    /// Removes `member`, returning its previous score.
    fn remove(&mut self, member: &str) -> Option<f64> {
        let (key, old) = self.scores.remove_entry(member)?;
        self.bytes = self.bytes.saturating_sub(key.len() + 8);
        if let Some(index) = self.index.get_mut() {
            index.remove(&(Score(old), key));
        }
        Some(old)
    }

    /// Members in `(score ASC, member ASC)` order, one step at a time — no
    /// collection, no sort, and reversible for the `ZREV*` commands.
    fn iter_asc(&self) -> impl DoubleEndedIterator<Item = (&str, f64)> {
        self.index().iter().map(|(s, m)| (m.as_ref(), s.0))
    }

    /// Members whose score falls within `min..max`.
    ///
    /// Seeks to the first candidate through the index and stops at the first
    /// score past `max`, so this is O(log n + k) in the number returned rather
    /// than O(n log n) in the size of the set. The `filter` only ever discards
    /// the run of members sitting exactly on an exclusive lower bound.
    fn range_by_score<'a>(
        &'a self,
        min: &'a ScoreBound,
        max: &'a ScoreBound,
    ) -> impl Iterator<Item = (&'a str, f64)> {
        let start = match min {
            ScoreBound::NegInf => f64::NEG_INFINITY,
            ScoreBound::PosInf => f64::INFINITY,
            ScoreBound::Inclusive(v) | ScoreBound::Exclusive(v) => *v,
        };
        let empty: Arc<str> = Arc::from("");
        self.index()
            .range((Score(start), empty)..)
            .take_while(move |(s, _)| below_max(s.0, max))
            .filter(move |(s, _)| above_min(s.0, min))
            .map(|(s, m)| (m.as_ref(), s.0))
    }

    /// 0-based position of `member` in ascending order.
    ///
    /// O(rank) — a `BTreeSet` cannot answer a positional query in log time. That
    /// is still strictly better than sorting the whole set, which is what this
    /// used to do.
    fn rank(&self, member: &str) -> Option<usize> {
        let score = self.score(member)?;
        Some(
            self.index()
                .range(..(Score(score), Arc::from(member)))
                .count(),
        )
    }

    /// Members in no particular order, for callers that sort by something else.
    fn members(&self) -> impl Iterator<Item = (&str, f64)> {
        self.scores.iter().map(|(m, &s)| (m.as_ref(), s))
    }

    fn from_pairs(pairs: impl IntoIterator<Item = (String, f64)>) -> Self {
        let mut z = Self::new();
        for (member, score) in pairs {
            z.insert(&member, score);
        }
        z
    }
}

// ── score bounds ──────────────────────────────────────────────────────────────

enum ScoreBound {
    NegInf,
    PosInf,
    Inclusive(f64),
    Exclusive(f64),
}

impl ScoreBound {
    fn parse(s: &str) -> Result<Self, Value> {
        if s == "-inf" {
            Ok(Self::NegInf)
        } else if s == "+inf" || s == "inf" {
            Ok(Self::PosInf)
        } else if let Some(rest) = s.strip_prefix('(') {
            rest.parse::<f64>()
                .map(Self::Exclusive)
                .map_err(|_| Value::Error("ERR min or max is not a float".to_string()))
        } else {
            s.parse::<f64>()
                .map(Self::Inclusive)
                .map_err(|_| Value::Error("ERR min or max is not a float".to_string()))
        }
    }
}

/// Whether `score` clears the lower bound.
fn above_min(score: f64, min: &ScoreBound) -> bool {
    match min {
        ScoreBound::NegInf => true,
        ScoreBound::PosInf => false,
        ScoreBound::Inclusive(v) => score >= *v,
        ScoreBound::Exclusive(v) => score > *v,
    }
}

/// Whether `score` is still under the upper bound. Split out from
/// [`in_score_range`] so an ordered walk can use it as a stopping condition:
/// once it goes false, every later score is out of range too.
fn below_max(score: f64, max: &ScoreBound) -> bool {
    match max {
        ScoreBound::PosInf => true,
        ScoreBound::NegInf => false,
        ScoreBound::Inclusive(v) => score <= *v,
        ScoreBound::Exclusive(v) => score < *v,
    }
}

/// The straightforward definition of "in range", kept as the reference the
/// index walk in [`ZSetInner::range_by_score`] is checked against — see
/// `range_by_score_agrees_with_a_linear_scan`. Nothing on the hot path calls
/// it: seeking through the index and stopping early is the whole point.
#[cfg(test)]
fn in_score_range(score: f64, min: &ScoreBound, max: &ScoreBound) -> bool {
    above_min(score, min) && below_max(score, max)
}

// ── Byte payloads ─────────────────────────────────────────────────────────────

/// A stored value's bytes.
///
/// Values are payloads and may be arbitrary bytes — compressed blobs, protobuf,
/// images. *Identifiers* (keys, hash fields, set and sorted-set members) stay
/// `String`: they are looked up, pattern-matched and scope-checked as text, and
/// making them bytes would spread through the glob matcher, sync scopes and
/// pub/sub routing for no practical gain.
///
/// The serde impls are hand-written for two reasons. `Vec<u8>` serializes as an
/// array of integers under rmp-serde, which would roughly double snapshot size;
/// `serialize_bytes` emits a compact msgpack `bin`. And deserialization accepts
/// *either* a string or bytes, so snapshots written by 0.2.1 and earlier — where
/// values were `String` — still load.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct Blob(pub SmallVec<[u8; 24]>);

impl Blob {
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn into_vec(self) -> Vec<u8> {
        self.0.into_vec()
    }
    /// Alias of `into_vec`, mirroring `String::into_bytes` at the many call
    /// sites that build a RESP `BulkString` straight from a stored value.
    pub fn into_bytes(self) -> Vec<u8> {
        self.0.into_vec()
    }
    /// Append bytes, for `APPEND`.
    pub fn extend(&mut self, other: &[u8]) {
        self.0.extend_from_slice(other);
    }
    /// Parse the payload as text, for the commands that require a number.
    /// Non-UTF-8 fails the same way non-numeric text does.
    pub fn parse_as<T: std::str::FromStr>(&self) -> Option<T> {
        self.as_str()?.parse().ok()
    }
    /// Interpret the payload as text. Commands that need a number or a JSON
    /// document (`INCR`, `JSET`, …) go through this and error when it fails,
    /// rather than the storage layer refusing the write in the first place.
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }
}

impl From<Vec<u8>> for Blob {
    fn from(v: Vec<u8>) -> Self {
        Blob(SmallVec::from_vec(v))
    }
}
impl From<&[u8]> for Blob {
    fn from(v: &[u8]) -> Self {
        Blob(SmallVec::from_slice(v))
    }
}
impl From<String> for Blob {
    fn from(v: String) -> Self {
        Blob(SmallVec::from_vec(v.into_bytes()))
    }
}
impl From<&str> for Blob {
    fn from(v: &str) -> Self {
        Blob(SmallVec::from_slice(v.as_bytes()))
    }
}

impl std::fmt::Debug for Blob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Text payloads dominate, so print them readably and fall back to hex.
        match self.as_str() {
            Some(t) => write!(f, "{t:?}"),
            None => write!(f, "<{} bytes>", self.0.len()),
        }
    }
}

impl Serialize for Blob {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for Blob {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Blob;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bytes or a string")
            }
            // Pre-0.2.2 snapshots stored values as msgpack strings.
            fn visit_str<E>(self, v: &str) -> Result<Blob, E> {
                Ok(Blob(SmallVec::from_slice(v.as_bytes())))
            }
            fn visit_string<E>(self, v: String) -> Result<Blob, E> {
                Ok(Blob(SmallVec::from_vec(v.into_bytes())))
            }
            fn visit_bytes<E>(self, v: &[u8]) -> Result<Blob, E> {
                Ok(Blob(SmallVec::from_slice(v)))
            }
            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Blob, E> {
                Ok(Blob(SmallVec::from_vec(v)))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Blob, A::Error> {
                let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
                while let Some(b) = seq.next_element::<u8>()? {
                    out.push(b);
                }
                Ok(Blob(SmallVec::from_vec(out)))
            }
        }
        de.deserialize_any(V)
    }
}

// ── Compact hash encoding ─────────────────────────────────────────────────────

const INLINE_HASH_FIELDS: usize = 2;

/// A hash plus a running byte total.
///
/// The total is maintained incrementally rather than computed on demand
/// because [`entry_size`] is called before *and* after every write. Walking
/// the fields there made building an N-field hash O(N²): measured at 2,838
/// `HSET`/s into a 100k-field hash against 349,650 `SET`/s, and halving with
/// every doubling. The same reasoning applies to [`CompactSet`],
/// [`CompactList`] and [`ZSetInner`].
///
/// `bytes` counts exactly what `entry_size` used to sum — `key.len() +
/// value.len()` per field — so `INFO used_memory` is unchanged.
#[derive(Clone)]
struct CompactHash {
    repr: CompactHashRepr,
    bytes: usize,
}

#[derive(Clone)]
enum CompactHashRepr {
    Inline(SmallVec<[(String, Blob); INLINE_HASH_FIELDS]>),
    Table(HashMap<String, Blob>),
}

impl CompactHash {
    fn new() -> Self {
        Self {
            repr: CompactHashRepr::Inline(SmallVec::new()),
            bytes: 0,
        }
    }

    fn from_map(map: HashMap<String, Blob>) -> Self {
        let bytes = map.iter().map(|(key, value)| key.len() + value.len()).sum();
        let repr = if map.len() <= INLINE_HASH_FIELDS {
            CompactHashRepr::Inline(map.into_iter().collect())
        } else {
            CompactHashRepr::Table(map)
        };
        Self { repr, bytes }
    }

    /// Sum of `key.len() + value.len()` across every field. O(1).
    fn heap_bytes(&self) -> usize {
        self.bytes
    }

    fn to_map(&self) -> HashMap<String, Blob> {
        self.iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    fn len(&self) -> usize {
        match &self.repr {
            CompactHashRepr::Inline(fields) => fields.len(),
            CompactHashRepr::Table(fields) => fields.len(),
        }
    }

    fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    fn get(&self, key: &str) -> Option<&Blob> {
        match &self.repr {
            CompactHashRepr::Inline(fields) => fields
                .iter()
                .find_map(|(field, value)| (field == key).then_some(value)),
            CompactHashRepr::Table(fields) => fields.get(key),
        }
    }

    fn insert(&mut self, key: String, value: Blob) -> Option<Blob> {
        let (key_len, value_len) = (key.len(), value.len());
        let old = match &mut self.repr {
            CompactHashRepr::Inline(fields) => {
                if let Some((_, slot)) = fields.iter_mut().find(|(field, _)| field == &key) {
                    Some(std::mem::replace(slot, value))
                } else if fields.len() < INLINE_HASH_FIELDS {
                    fields.push((key, value));
                    None
                } else {
                    let mut table = HashMap::with_capacity(fields.len() + 1);
                    table.extend(fields.drain(..));
                    let old = table.insert(key, value);
                    self.repr = CompactHashRepr::Table(table);
                    old
                }
            }
            CompactHashRepr::Table(fields) => fields.insert(key, value),
        };
        match &old {
            // Overwrite: the field name is already counted, only the value moved.
            Some(previous) => {
                self.bytes = self
                    .bytes
                    .saturating_add(value_len)
                    .saturating_sub(previous.len());
            }
            None => self.bytes = self.bytes.saturating_add(key_len + value_len),
        }
        old
    }

    fn insert_if_absent(&mut self, key: String, value: Blob) -> bool {
        if self.contains_key(&key) {
            false
        } else {
            self.insert(key, value);
            true
        }
    }

    fn remove(&mut self, key: &str) -> Option<Blob> {
        let removed = match &mut self.repr {
            CompactHashRepr::Inline(fields) => fields
                .iter()
                .position(|(field, _)| field == key)
                .map(|position| fields.swap_remove(position).1),
            CompactHashRepr::Table(fields) => fields.remove(key),
        };
        if let Some(ref value) = removed {
            self.bytes = self.bytes.saturating_sub(key.len() + value.len());
        }
        removed
    }

    fn iter(&self) -> CompactHashIter<'_> {
        match &self.repr {
            CompactHashRepr::Inline(fields) => CompactHashIter::Inline(fields.iter()),
            CompactHashRepr::Table(fields) => CompactHashIter::Table(fields.iter()),
        }
    }
}

enum CompactHashIter<'a> {
    Inline(std::slice::Iter<'a, (String, Blob)>),
    Table(std::collections::hash_map::Iter<'a, String, Blob>),
}

impl<'a> Iterator for CompactHashIter<'a> {
    type Item = (&'a String, &'a Blob);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Inline(iter) => iter.next().map(|(key, value)| (key, value)),
            Self::Table(iter) => iter.next(),
        }
    }
}

// ── Compact set encoding ──────────────────────────────────────────────────────

const INLINE_SET_MEMBERS: usize = 4;

/// A set plus a running byte total. See [`CompactHash`] for why the total is
/// incremental. `bytes` sums `member.len()`, matching what `entry_size` used
/// to walk.
#[derive(Clone)]
struct CompactSet {
    repr: CompactSetRepr,
    bytes: usize,
}

#[derive(Clone)]
enum CompactSetRepr {
    Inline(SmallVec<[String; INLINE_SET_MEMBERS]>),
    Table(IndexSet<String>),
}

impl CompactSet {
    fn new() -> Self {
        Self {
            repr: CompactSetRepr::Inline(SmallVec::new()),
            bytes: 0,
        }
    }

    fn from_index_set(set: IndexSet<String>) -> Self {
        let bytes = set.iter().map(String::len).sum();
        let repr = if set.len() <= INLINE_SET_MEMBERS {
            CompactSetRepr::Inline(set.into_iter().collect())
        } else {
            CompactSetRepr::Table(set)
        };
        Self { repr, bytes }
    }

    /// Sum of `member.len()` across the set. O(1).
    fn heap_bytes(&self) -> usize {
        self.bytes
    }

    fn len(&self) -> usize {
        match &self.repr {
            CompactSetRepr::Inline(members) => members.len(),
            CompactSetRepr::Table(members) => members.len(),
        }
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn contains(&self, member: &str) -> bool {
        match &self.repr {
            CompactSetRepr::Inline(members) => members.iter().any(|candidate| candidate == member),
            CompactSetRepr::Table(members) => members.contains(member),
        }
    }

    fn insert(&mut self, member: String) -> bool {
        let member_len = member.len();
        let inserted = match &mut self.repr {
            CompactSetRepr::Inline(members) => {
                if members.iter().any(|candidate| candidate == &member) {
                    false
                } else if members.len() < INLINE_SET_MEMBERS {
                    members.push(member);
                    true
                } else {
                    let mut table = IndexSet::with_capacity(members.len() + 1);
                    table.extend(members.drain(..));
                    let inserted = table.insert(member);
                    self.repr = CompactSetRepr::Table(table);
                    inserted
                }
            }
            CompactSetRepr::Table(members) => members.insert(member),
        };
        if inserted {
            self.bytes = self.bytes.saturating_add(member_len);
        }
        inserted
    }

    fn swap_remove(&mut self, member: &str) -> bool {
        let removed = match &mut self.repr {
            CompactSetRepr::Inline(members) => members
                .iter()
                .position(|candidate| candidate == member)
                .map(|position| members.swap_remove(position))
                .is_some(),
            CompactSetRepr::Table(members) => members.swap_remove(member),
        };
        if removed {
            self.bytes = self.bytes.saturating_sub(member.len());
        }
        removed
    }

    fn swap_remove_index(&mut self, index: usize) -> Option<String> {
        let removed = match &mut self.repr {
            CompactSetRepr::Inline(members) => {
                (index < members.len()).then(|| members.swap_remove(index))
            }
            CompactSetRepr::Table(members) => members.swap_remove_index(index),
        };
        if let Some(ref member) = removed {
            self.bytes = self.bytes.saturating_sub(member.len());
        }
        removed
    }

    fn get_index(&self, index: usize) -> Option<&String> {
        match &self.repr {
            CompactSetRepr::Inline(members) => members.get(index),
            CompactSetRepr::Table(members) => members.get_index(index),
        }
    }

    fn drain_all(&mut self) -> Vec<String> {
        self.bytes = 0;
        match &mut self.repr {
            CompactSetRepr::Inline(members) => members.drain(..).collect(),
            CompactSetRepr::Table(members) => members.drain(..).collect(),
        }
    }

    fn iter(&self) -> CompactSetIter<'_> {
        match &self.repr {
            CompactSetRepr::Inline(members) => CompactSetIter::Inline(members.iter()),
            CompactSetRepr::Table(members) => CompactSetIter::Table(members.iter()),
        }
    }
}

impl FromIterator<String> for CompactSet {
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Self {
        Self::from_index_set(iter.into_iter().collect())
    }
}

enum CompactSetIter<'a> {
    Inline(std::slice::Iter<'a, String>),
    Table(indexmap::set::Iter<'a, String>),
}

impl<'a> Iterator for CompactSetIter<'a> {
    type Item = &'a String;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Inline(iter) => iter.next(),
            Self::Table(iter) => iter.next(),
        }
    }
}

// ── Compact list encoding ─────────────────────────────────────────────────────

/// A list plus a running byte total. See [`CompactHash`] for why the total is
/// incremental. `bytes` sums `element.len()`.
///
/// The inner `VecDeque` is deliberately private and there is no `DerefMut`:
/// every mutation has to go through a method here, because one that bypassed
/// the counter would desynchronise `INFO used_memory` and, with a memory cap
/// configured, eviction along with it.
#[derive(Clone, Default)]
struct CompactList {
    items: VecDeque<Blob>,
    bytes: usize,
}

impl CompactList {
    fn new() -> Self {
        Self::default()
    }

    /// Sum of `element.len()` across the list. O(1).
    fn heap_bytes(&self) -> usize {
        self.bytes
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn iter(&self) -> std::collections::vec_deque::Iter<'_, Blob> {
        self.items.iter()
    }

    fn get(&self, index: usize) -> Option<&Blob> {
        self.items.get(index)
    }

    fn push_front(&mut self, value: Blob) {
        self.bytes = self.bytes.saturating_add(value.len());
        self.items.push_front(value);
    }

    fn push_back(&mut self, value: Blob) {
        self.bytes = self.bytes.saturating_add(value.len());
        self.items.push_back(value);
    }

    fn pop_front(&mut self) -> Option<Blob> {
        let value = self.items.pop_front()?;
        self.bytes = self.bytes.saturating_sub(value.len());
        Some(value)
    }

    fn pop_back(&mut self) -> Option<Blob> {
        let value = self.items.pop_back()?;
        self.bytes = self.bytes.saturating_sub(value.len());
        Some(value)
    }

    /// Replace the element at `index`, returning false when out of range.
    fn set(&mut self, index: usize, value: Blob) -> bool {
        let Some(slot) = self.items.get_mut(index) else {
            return false;
        };
        self.bytes = self
            .bytes
            .saturating_add(value.len())
            .saturating_sub(slot.len());
        *slot = value;
        true
    }

    fn remove(&mut self, index: usize) -> Option<Blob> {
        let value = self.items.remove(index)?;
        self.bytes = self.bytes.saturating_sub(value.len());
        Some(value)
    }

    /// Remove and return the first `count` elements.
    fn drain_front(&mut self, count: usize) -> Vec<Blob> {
        let taken: Vec<Blob> = self.items.drain(..count.min(self.items.len())).collect();
        let freed: usize = taken.iter().map(Blob::len).sum();
        self.bytes = self.bytes.saturating_sub(freed);
        taken
    }

    /// Remove and return the last `count` elements, last-first — the order
    /// `RPOP key <count>` reports them in.
    fn drain_back(&mut self, count: usize) -> Vec<Blob> {
        let mut taken = Vec::with_capacity(count.min(self.items.len()));
        for _ in 0..count.min(self.items.len()) {
            if let Some(value) = self.pop_back() {
                taken.push(value);
            }
        }
        taken
    }

    /// Keep only `start..=end`, discarding everything outside it.
    fn retain_range(&mut self, start: usize, end: usize) {
        let kept: VecDeque<Blob> = self.items.drain(start..=end).collect();
        self.bytes = kept.iter().map(Blob::len).sum();
        self.items = kept;
    }

    fn clear(&mut self) {
        self.items.clear();
        self.bytes = 0;
    }
}

impl FromIterator<Blob> for CompactList {
    fn from_iter<T: IntoIterator<Item = Blob>>(iter: T) -> Self {
        let items: VecDeque<Blob> = iter.into_iter().collect();
        let bytes = items.iter().map(Blob::len).sum();
        Self { items, bytes }
    }
}

// ── Entry value type ──────────────────────────────────────────────────────────

#[derive(Clone)]
enum EntryValue {
    Str(Blob),
    Hash(Box<CompactHash>),
    List(CompactList),
    // IndexSet rather than HashSet: SPOP / SRANDMEMBER need O(1) access to a
    // random member by index, which a hash table cannot provide.
    Set(Box<CompactSet>),
    // Boxed for the same reason as `Hash` and `Set`: `ZSetInner` is 96 bytes
    // and `EntryValue` is stored inline in every entry, so leaving it unboxed
    // makes every string key pay for the largest collection variant.
    ZSet(Box<ZSetInner>),
    RateLimiter(RateLimiterInner),
    Json(serde_json::Value),
}

impl EntryValue {
    fn type_name(&self) -> &'static str {
        match self {
            EntryValue::Str(_) => "string",
            EntryValue::Hash(_) => "hash",
            EntryValue::List(_) => "list",
            EntryValue::Set(_) => "set",
            EntryValue::ZSet(_) => "zset",
            EntryValue::RateLimiter(_) => "ratelimit",
            EntryValue::Json(_) => "json",
        }
    }
}

// ── JSON paths & merge (JSET / JGET / JMERGE) ─────────────────────────────────

enum JsonPathSeg {
    Field(String),
    Index(usize),
}

const MAX_JSON_PATH_DEPTH: usize = 128;

/// Parse a deterministic JSON path: `$` (whole document), `$.user.name`,
/// `$.items[2].qty`. The leading `$` is optional. Wildcards, slices, and
/// filters are not supported — every path addresses exactly one location.
fn parse_json_path(path: &str) -> Result<Vec<JsonPathSeg>, String> {
    let mut rest = path.strip_prefix('$').unwrap_or(path);
    let mut segs = Vec::new();
    while !rest.is_empty() {
        if segs.len() >= MAX_JSON_PATH_DEPTH {
            return Err("ERR JSON path too deep".to_string());
        }
        if let Some(r) = rest.strip_prefix('.') {
            let end = r.find(['.', '[']).unwrap_or(r.len());
            let field = &r[..end];
            if field.is_empty() {
                return Err("ERR invalid JSON path: empty field".to_string());
            }
            segs.push(JsonPathSeg::Field(field.to_string()));
            rest = &r[end..];
        } else if let Some(r) = rest.strip_prefix('[') {
            let end = r
                .find(']')
                .ok_or_else(|| "ERR invalid JSON path: unterminated index".to_string())?;
            let idx: usize = r[..end]
                .parse()
                .map_err(|_| "ERR invalid JSON path: bad index".to_string())?;
            segs.push(JsonPathSeg::Index(idx));
            rest = &r[end + 1..];
        } else {
            // Bare leading field, e.g. `user.name` without `$.`.
            let end = rest.find(['.', '[']).unwrap_or(rest.len());
            segs.push(JsonPathSeg::Field(rest[..end].to_string()));
            rest = &rest[end..];
        }
    }
    Ok(segs)
}

/// Set `value` at the path. Intermediate objects are auto-created when a
/// field segment hits `null`; array indices must already exist.
fn json_set_at(
    cur: &mut serde_json::Value,
    segs: &[JsonPathSeg],
    value: serde_json::Value,
) -> Result<(), String> {
    let Some((seg, rest)) = segs.split_first() else {
        *cur = value;
        return Ok(());
    };
    match seg {
        JsonPathSeg::Field(f) => {
            if cur.is_null() {
                *cur = serde_json::Value::Object(serde_json::Map::new());
            }
            let obj = cur
                .as_object_mut()
                .ok_or_else(|| format!("ERR path segment '.{}' is not an object", f))?;
            let slot = obj.entry(f.clone()).or_insert(serde_json::Value::Null);
            json_set_at(slot, rest, value)
        }
        JsonPathSeg::Index(i) => {
            let arr = cur
                .as_array_mut()
                .ok_or_else(|| format!("ERR path segment '[{}]' is not an array", i))?;
            let len = arr.len();
            let slot = arr
                .get_mut(*i)
                .ok_or_else(|| format!("ERR index {} out of bounds (len {})", i, len))?;
            json_set_at(slot, rest, value)
        }
    }
}

fn json_get_at<'a>(
    cur: &'a serde_json::Value,
    segs: &[JsonPathSeg],
) -> Option<&'a serde_json::Value> {
    let mut c = cur;
    for seg in segs {
        c = match seg {
            JsonPathSeg::Field(f) => c.get(f.as_str())?,
            JsonPathSeg::Index(i) => c.get(*i)?,
        };
    }
    Some(c)
}

/// RFC 7386 JSON Merge Patch: objects merge recursively, `null` removes the
/// field, and any non-object patch replaces the target wholesale.
fn json_merge_patch(target: &mut serde_json::Value, patch: serde_json::Value) {
    match patch {
        serde_json::Value::Object(pobj) => {
            if !target.is_object() {
                *target = serde_json::Value::Object(serde_json::Map::new());
            }
            let tobj = target.as_object_mut().expect("just ensured object");
            for (k, v) in pobj {
                if v.is_null() {
                    tobj.remove(&k);
                } else {
                    let slot = tobj.entry(k).or_insert(serde_json::Value::Null);
                    json_merge_patch(slot, v);
                }
            }
        }
        other => *target = other,
    }
}

fn json_approx_size(v: &serde_json::Value) -> usize {
    use serde_json::Value as J;
    match v {
        J::Null | J::Bool(_) => 8,
        J::Number(_) => 16,
        J::String(s) => s.len() + 8,
        J::Array(a) => 8 + a.iter().map(json_approx_size).sum::<usize>(),
        J::Object(o) => {
            8 + o
                .iter()
                .map(|(k, val)| k.len() + json_approx_size(val))
                .sum::<usize>()
        }
    }
}

// ── Sliding-window rate limiter (RLSET / RLCHECK) ─────────────────────────────

/// Buckets the sliding window is divided into. Memory per limiter is fixed at
/// this many `(start, count)` pairs regardless of the configured limit.
const RL_BUCKETS: u64 = 64;

#[derive(Clone)]
struct RateLimiterInner {
    limit: u64,
    window_ms: u64,
    /// `(bucket_start_ms, attempts)` for buckets inside the window, oldest
    /// first.
    ///
    /// Storing one timestamp per attempt was exact but unbounded in practice:
    /// a `RLSET key 100000 3600` limiter held 100 000 `u64`s — roughly 800 KB
    /// for a single key, and token-cost limiting (roadmap #9) makes six-figure
    /// limits ordinary. Counting into a fixed number of buckets caps a limiter
    /// at ~1 KB whatever the limit.
    ///
    /// The cost is granularity: the window advances one bucket at a time, so a
    /// limiter is exact to within `window_ms / RL_BUCKETS`. Attempts are never
    /// under-counted — a bucket only leaves the window once it is entirely
    /// outside it — so the limiter errs toward rejecting slightly early rather
    /// than admitting over the limit.
    buckets: VecDeque<(u64, u64)>,
}

impl RateLimiterInner {
    fn new(limit: u64, window_ms: u64) -> Self {
        Self {
            limit,
            window_ms,
            buckets: VecDeque::new(),
        }
    }

    /// Width of one bucket, at least 1 ms.
    fn bucket_ms(&self) -> u64 {
        (self.window_ms / RL_BUCKETS).max(1)
    }

    /// Record an attempt at `now`, returning `(allowed, remaining, retry_after_ms)`.
    /// Denied attempts are not recorded — a client hammering a full limiter
    /// does not push its own recovery further away.
    fn check(&mut self, now: u64) -> (i64, u64, u64) {
        let width = self.bucket_ms();
        let cutoff = now.saturating_sub(self.window_ms);
        // A bucket leaves the window only once its whole span is behind the
        // cutoff, so attempts are never dropped early.
        while self
            .buckets
            .front()
            .is_some_and(|&(start, _)| start + width <= cutoff)
        {
            self.buckets.pop_front();
        }

        let used: u64 = self.buckets.iter().map(|&(_, c)| c).sum();
        if used < self.limit {
            let current = now - (now % width);
            match self.buckets.back_mut() {
                Some((start, count)) if *start == current => *count += 1,
                _ => self.buckets.push_back((current, 1)),
            }
            (1, self.limit - used - 1, 0)
        } else {
            // Recovery arrives when the oldest bucket falls out of the window.
            let retry_after = self
                .buckets
                .front()
                .map(|&(start, _)| (start + width + self.window_ms).saturating_sub(now))
                .unwrap_or(0)
                // A bucket spans forward from its start, so the raw figure can
                // land a fraction past the window. The wait never legitimately
                // exceeds one window.
                .min(self.window_ms);
            (0, 0, retry_after)
        }
    }
}

// ── Entry ─────────────────────────────────────────────────────────────────────

struct Entry {
    value: EntryValue,
    expires_at_ms: Option<u64>,
    /// Last time this entry was read or written, in ms. Drives LRU eviction.
    /// Atomic so reads can refresh recency while holding only a shared
    /// (DashMap read-lock) reference — no writer lock on the GET path.
    last_access_ms: AtomicU64,
}

impl Clone for Entry {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            expires_at_ms: self.expires_at_ms,
            last_access_ms: AtomicU64::new(self.last_access_ms.load(Ordering::Relaxed)),
        }
    }
}

impl Entry {
    fn new_str(value: impl Into<Blob>) -> Self {
        Self {
            value: EntryValue::Str(value.into()),
            expires_at_ms: None,
            last_access_ms: AtomicU64::new(now_ms()),
        }
    }

    fn new_str_ex(value: impl Into<Blob>, expires_at_ms: u64) -> Self {
        Self {
            value: EntryValue::Str(value.into()),
            expires_at_ms: Some(expires_at_ms),
            last_access_ms: AtomicU64::new(now_ms()),
        }
    }

    fn is_expired(&self, now: u64) -> bool {
        self.expires_at_ms.is_some_and(|exp| now >= exp)
    }

    /// Mark this entry as just-used so LRU eviction treats it as recent.
    fn touch(&self, now: u64) {
        self.last_access_ms.store(now, Ordering::Relaxed);
    }
}

// ── resolve list range helpers ────────────────────────────────────────────────

/// Convert a possibly-negative index into an absolute index in `[0, len)`.
fn resolve_idx(idx: i64, len: usize) -> Option<usize> {
    let resolved = if idx >= 0 {
        idx as usize
    } else {
        (len as i64 + idx) as usize
    };
    if resolved < len { Some(resolved) } else { None }
}

/// Clamp `start..=stop` (both possibly negative) to valid slice bounds.
/// Returns `(start_inclusive, end_inclusive)` with `start <= end`, or `None` for empty.
fn resolve_range(start: i64, stop: i64, len: usize) -> Option<(usize, usize)> {
    if len == 0 {
        return None;
    }
    let len_i = len as i64;
    let s = (if start < 0 { len_i + start } else { start }).max(0) as usize;
    let e = (if stop < 0 { len_i + stop } else { stop }).min(len_i - 1);
    if e < 0 || s >= len || s > e as usize {
        None
    } else {
        Some((s, e as usize))
    }
}

// ── zset range helpers ────────────────────────────────────────────────────────

/// Collects the `start..=stop` window (Redis index semantics, negatives count
/// from the end) out of an already-ordered iterator.
///
/// Takes an iterator rather than a slice so the caller never materialises the
/// whole set: `ZRANGE board 0 9` walks ten entries out of the index and stops,
/// where the previous shape collected and sorted every member first.
///
/// Reaching the window is O(start) — a `BTreeSet` has no positional index — so
/// a deep offset still walks, but nothing is allocated or sorted along the way.
fn index_slice<'a>(
    ordered: impl Iterator<Item = (&'a str, f64)>,
    len: usize,
    start: i64,
    stop: i64,
) -> Vec<(&'a str, f64)> {
    match resolve_range(start, stop, len) {
        None => Vec::new(),
        Some((s, e)) => ordered.skip(s).take(e - s + 1).collect(),
    }
}

fn apply_limit<T: Clone>(items: Vec<T>, limit: Option<(i64, i64)>) -> Vec<T> {
    match limit {
        None => items,
        Some((offset, count)) => {
            let start = offset.max(0) as usize;
            if start >= items.len() {
                return vec![];
            }
            let slice = &items[start..];
            if count < 0 {
                slice.to_vec()
            } else {
                slice[..count.min(slice.len() as i64) as usize].to_vec()
            }
        }
    }
}

fn encode_zrange(items: &[(&str, f64)], withscores: bool) -> Value {
    let mut out: Vec<Value> = Vec::with_capacity(if withscores {
        items.len() * 2
    } else {
        items.len()
    });
    for (m, s) in items {
        out.push(Value::BulkString(Some(m.as_bytes().to_vec())));
        if withscores {
            out.push(Value::BulkString(Some(format_score(*s).into_bytes())));
        }
    }
    Value::Array(Some(out))
}

pub fn format_score(s: f64) -> String {
    if s == f64::INFINITY {
        "inf".to_string()
    } else if s == f64::NEG_INFINITY {
        "-inf".to_string()
    } else if s.fract() == 0.0 && s.abs() < 1e15 {
        format!("{}", s as i64)
    } else {
        format!("{}", s)
    }
}

// ── macro: check entry type and prepare for mutation ─────────────────────────

/// Binds `$guard` to the entry for `$key` and `$inner` to the payload inside
/// its `$variant`, creating the entry from `$default` when the key is absent or
/// expired. Returns `WRONGTYPE` from the enclosing function if the key holds a
/// different type.
///
/// The type check, the expiry reset and the insertion all happen under the one
/// write guard that hands out `$inner`. The previous shape — a `get()` that
/// dropped its read guard, then a separate `entry()` — left a window in which
/// another connection could change the key's type or resurrect an expired key:
///
///   * `HSET k f v` on a missing key racing `SET k v` found a `Str` behind the
///     `entry()` and hit an `unreachable!()`, panicking the connection task.
///   * `was_expired` computed from the earlier read clobbered a value that a
///     concurrent writer had stored in the meantime.
///
/// Both are gone: nothing observes the key between the check and the write.
/// Splitting the guard is also why the old shape cost two shard locks per
/// mutating command instead of one.
macro_rules! typed_entry {
    ($guard:ident, $inner:ident, $data:expr, $key:expr, $now:expr, $variant:path, $default:expr) => {
        let mut $guard = $data.entry($key).or_insert_with(|| Entry {
            value: $variant($default),
            expires_at_ms: None,
            last_access_ms: AtomicU64::new($now),
        });
        // A freshly inserted entry carries no TTL, so this only fires for a key
        // that was already present and has aged out.
        if $guard.is_expired($now) {
            $guard.value = $variant($default);
            $guard.expires_at_ms = None;
        }
        let $variant($inner) = &mut $guard.value else {
            return Value::Error(WRONGTYPE.to_string());
        };
    };
}

// ── KeyspaceSample ────────────────────────────────────────────────────────────

/// Incrementally maintained keyspace totals reported to metrics and `INFO`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KeyspaceSample {
    /// Stored keys. Expired physical entries leave during active expiry.
    pub keys: usize,
    /// Of those, how many carry a TTL (`INFO keyspace` reports it as `expires`).
    pub volatile_keys: usize,
    /// Approximate heap usage: key+value sizes plus fixed per-entry overhead.
    pub memory_bytes: usize,
}

// ── EvictionPolicy ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum EvictionPolicy {
    #[default]
    NoEviction,
    AllKeysLru,
    AllKeysRandom,
    VolatileLru,
    VolatileTtl,
}

// ── KeyValueStore ─────────────────────────────────────────────────────────────

// ── Snapshot types ────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub enum SnapshotValue {
    Str(Blob),
    Hash(HashMap<String, Blob>),
    List(Vec<Blob>),
    Set(Vec<String>),
    ZSet(Vec<(String, f64)>),
    // Appended after the original variants: rmp-serde encodes variants by
    // index, so new variants must go last to keep old snapshots readable.
    RateLimiter {
        limit: u64,
        window_ms: u64,
        events: Vec<u64>,
    },
    /// JSON document, stored serialized.
    Json(String),
}

#[derive(Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub key: String,
    pub value: SnapshotValue,
    pub expires_at_ms: Option<u64>,
}

#[derive(Default)]
struct DenseKeys {
    keys: Vec<String>,
    positions: HashMap<String, usize>,
}

impl DenseKeys {
    fn contains(&self, key: &str) -> bool {
        self.positions.contains_key(key)
    }

    fn insert(&mut self, key: &str) {
        if self.positions.contains_key(key) {
            return;
        }
        let pos = self.keys.len();
        self.keys.push(key.to_string());
        self.positions.insert(key.to_string(), pos);
    }

    fn remove(&mut self, key: &str) {
        let Some(pos) = self.positions.remove(key) else {
            return;
        };
        self.keys.swap_remove(pos);
        if let Some(moved) = self.keys.get(pos) {
            self.positions.insert(moved.clone(), pos);
        }
    }
}

#[derive(Default)]
struct KeyIndex {
    ordered: BTreeSet<String>,
    all: DenseKeys,
    volatile: DenseKeys,
}

impl KeyIndex {
    /// Bring the index into line with a key's current state.
    ///
    /// Called on **every** write, so the overwhelmingly common case — a write
    /// to a key that is already indexed and whose TTL-ness has not changed —
    /// has to cost as little as possible. It used to cost the most: the
    /// unconditional `ordered.insert(key.to_string())` allocated a `String`
    /// and walked a `BTreeSet<String>`, comparing strings at every level, only
    /// to discover the key was already there. Together with the rest of this
    /// function that was ~25-30% of a single-threaded `SET`, and because it
    /// all runs under one store-wide lock it cost proportionally more as
    /// worker threads were added.
    ///
    /// `ordered` and `all` are maintained in lockstep and always hold exactly
    /// the same key set, so one hash lookup in `all` decides whether either
    /// needs touching at all. That is the invariant this relies on; keep the
    /// two in step in any future edit.
    fn sync(&mut self, key: &str, live_ttl: Option<bool>) {
        match live_ttl {
            None => {
                if self.all.contains(key) {
                    self.ordered.remove(key);
                    self.all.remove(key);
                }
                self.volatile.remove(key);
            }
            Some(has_ttl) => {
                if !self.all.contains(key) {
                    self.ordered.insert(key.to_string());
                    self.all.insert(key);
                }
                if has_ttl {
                    self.volatile.insert(key);
                } else {
                    self.volatile.remove(key);
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct KeyValueStore {
    data: Arc<DashMap<String, Entry>>,
    max_keys: Option<usize>,
    max_memory_bytes: Option<usize>,
    eviction_policy: EvictionPolicy,
    dirty: Arc<AtomicU64>,
    /// Keys sampled per eviction pass. Approximate-LRU quality rises with the
    /// sample and so does the cost, so the right value is workload-dependent —
    /// Redis exposes the same knob as `maxmemory-samples`. Configured rather
    /// than read from the environment because this crate also runs in the
    /// browser, where there is no environment to read.
    eviction_sample: usize,
    /// Total keys evicted since start. Exported as a metric: without it an
    /// operator cannot tell a healthy cache from one thrashing at its cap.
    evicted: Arc<AtomicU64>,
    /// Incrementally maintained logical footprint. It is intentionally a
    /// payload/accounting estimate, not process RSS; reads are O(1) and writes
    /// update only the keys they touched.
    memory_bytes: Arc<AtomicUsize>,
    index: Arc<std::sync::Mutex<KeyIndex>>,
    scan_cursors: Arc<std::sync::Mutex<HashMap<u64, String>>>,
    next_scan_cursor: Arc<AtomicU64>,
    expiry_cursor: Arc<AtomicUsize>,
    /// Serializes capacity checks only when a key or memory cap is configured.
    capacity_gate: Arc<std::sync::Mutex<()>>,
}

/// Fraction of `max_memory_bytes` eviction drops to once the cap is hit,
/// expressed as the divisor of the overshoot to shed (`limit - limit/16` ≈ 94%).
///
/// Without this headroom the store would sit exactly on the limit and the very
/// next write would trip the gate again. Evicting a little below it means the
/// next capacity pass is only due after roughly `limit/16` more bytes are
/// written, which amortises eviction work against the writes that made it
/// necessary.
const EVICTION_HEADROOM_DIVISOR: usize = 16;

impl Default for KeyValueStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyValueStore {
    pub fn new() -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            max_keys: None,
            max_memory_bytes: None,
            eviction_policy: EvictionPolicy::NoEviction,
            dirty: Arc::new(AtomicU64::new(0)),
            eviction_sample: 10,
            evicted: Arc::new(AtomicU64::new(0)),
            memory_bytes: Arc::new(AtomicUsize::new(0)),
            index: Arc::new(std::sync::Mutex::new(KeyIndex::default())),
            scan_cursors: Arc::new(std::sync::Mutex::new(HashMap::new())),
            next_scan_cursor: Arc::new(AtomicU64::new(1)),
            expiry_cursor: Arc::new(AtomicUsize::new(0)),
            capacity_gate: Arc::new(std::sync::Mutex::new(())),
        }
    }

    pub fn with_max_keys(max: usize) -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            max_keys: Some(max),
            max_memory_bytes: None,
            eviction_policy: EvictionPolicy::NoEviction,
            dirty: Arc::new(AtomicU64::new(0)),
            eviction_sample: 10,
            evicted: Arc::new(AtomicU64::new(0)),
            memory_bytes: Arc::new(AtomicUsize::new(0)),
            index: Arc::new(std::sync::Mutex::new(KeyIndex::default())),
            scan_cursors: Arc::new(std::sync::Mutex::new(HashMap::new())),
            next_scan_cursor: Arc::new(AtomicU64::new(1)),
            expiry_cursor: Arc::new(AtomicUsize::new(0)),
            capacity_gate: Arc::new(std::sync::Mutex::new(())),
        }
    }

    pub fn with_config(
        max_keys: Option<usize>,
        max_memory_bytes: Option<usize>,
        eviction_policy: EvictionPolicy,
    ) -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            max_keys,
            max_memory_bytes,
            eviction_policy,
            dirty: Arc::new(AtomicU64::new(0)),
            eviction_sample: 10,
            evicted: Arc::new(AtomicU64::new(0)),
            memory_bytes: Arc::new(AtomicUsize::new(0)),
            index: Arc::new(std::sync::Mutex::new(KeyIndex::default())),
            scan_cursors: Arc::new(std::sync::Mutex::new(HashMap::new())),
            next_scan_cursor: Arc::new(AtomicU64::new(1)),
            expiry_cursor: Arc::new(AtomicUsize::new(0)),
            capacity_gate: Arc::new(std::sync::Mutex::new(())),
        }
    }

    /// Number of write commands applied since the last `reset_dirty()`.
    pub fn dirty_count(&self) -> u64 {
        self.dirty.load(Ordering::Relaxed)
    }

    /// Reset the dirty counter to zero (call after a successful snapshot save).
    pub fn reset_dirty(&self) {
        self.dirty.store(0, Ordering::Relaxed);
    }

    /// Increment the dirty counter by one. Called by the server after every
    /// successful write command so the autosave loop can skip saves when
    /// nothing has changed.
    pub fn mark_dirty(&self) {
        self.dirty.fetch_add(1, Ordering::Relaxed);
    }

    /// Incrementally maintained logical heap usage in bytes.
    ///
    /// This is key/value payload plus the fixed overhead used by
    /// [`entry_size`]. It is O(1) to read and deliberately does not claim to be
    /// allocator RSS.
    pub fn approximate_memory_bytes(&self) -> usize {
        self.memory_bytes.load(Ordering::Relaxed)
    }

    fn entry_memory(&self, key: &str) -> usize {
        self.data
            .get(key)
            .map_or(0, |entry| entry_size(key, entry.value()))
    }

    fn adjust_memory(&self, before: usize, after: usize) {
        if after >= before {
            self.memory_bytes
                .fetch_add(after.saturating_sub(before), Ordering::Relaxed);
        } else {
            let removed = before - after;
            let _ =
                self.memory_bytes
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                        Some(current.saturating_sub(removed))
                    });
        }
    }

    fn sync_key_metadata(&self, key: &str) {
        // Keep expired physical entries in the TTL index until the sweeper
        // removes them. A very short TTL can elapse between mutation and this
        // bookkeeping step; dropping it here would make it unreachable to the
        // bounded sweeper and suppress the expiry notification.
        let live_ttl = self
            .data
            .get(key)
            .map(|entry| entry.expires_at_ms.is_some());
        self.index
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .sync(key, live_ttl);
    }

    fn rebuild_metadata(&self) {
        let now = now_ms();
        let mut index = KeyIndex::default();
        let mut memory = 0usize;
        for entry in self.data.iter() {
            if entry.value().is_expired(now) {
                continue;
            }
            index.sync(entry.key(), Some(entry.value().expires_at_ms.is_some()));
            memory = memory.saturating_add(entry_size(entry.key(), entry.value()));
        }
        *self
            .index
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = index;
        self.memory_bytes.store(memory, Ordering::Relaxed);
    }

    /// Evict entries until the logical memory estimate is below the configured
    /// cap, or the selected policy has no eligible victim.
    pub fn try_evict_for_memory(&self) -> bool {
        self.try_evict_for_memory_reporting().0
    }

    /// The maintenance form used by the server. It returns every implicit
    /// deletion so replication, AOF, browser sync, and watchers can observe the
    /// same keyspace transition as the local store.
    pub fn try_evict_for_memory_reporting(&self) -> (bool, Vec<String>) {
        let _capacity_guard = self
            .capacity_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut evicted = Vec::new();
        let within_limit = self.try_evict_for_memory_inner(&mut evicted);
        (within_limit, evicted)
    }

    fn try_evict_for_memory_inner(&self, evicted: &mut Vec<String>) -> bool {
        let Some(limit) = self.max_memory_bytes else {
            return true;
        };
        let target = limit - limit / EVICTION_HEADROOM_DIVISOR;
        while self.approximate_memory_bytes() > target {
            if self.evict_one_reporting(now_ms(), evicted).is_none() {
                break;
            }
        }
        self.approximate_memory_bytes() <= limit
    }

    fn encode_current_value(value: &EntryValue) -> Value {
        fn tagged(tag: &str, mut items: Vec<Value>) -> Value {
            let mut out = Vec::with_capacity(items.len() + 1);
            out.push(Value::BulkString(Some(tag.as_bytes().to_vec())));
            out.append(&mut items);
            Value::Array(Some(out))
        }
        fn bulk(s: &str) -> Value {
            Value::BulkString(Some(s.as_bytes().to_vec()))
        }
        fn blob(b: &Blob) -> Value {
            Value::BulkString(Some(b.as_slice().to_vec()))
        }

        match value {
            EntryValue::Str(s) => Value::BulkString(Some(s.clone().into_bytes())),
            EntryValue::Hash(m) => {
                let mut fields: Vec<(&String, &Blob)> = m.iter().collect();
                fields.sort_by(|a, b| a.0.cmp(b.0));
                let items = fields
                    .into_iter()
                    .flat_map(|(f, v)| [bulk(f), blob(v)])
                    .collect();
                tagged("hash", items)
            }
            EntryValue::List(l) => tagged("list", l.iter().map(blob).collect()),
            EntryValue::Set(st) => tagged("set", st.iter().map(|m| bulk(m)).collect()),
            EntryValue::ZSet(z) => {
                let pairs: Vec<(&str, f64)> = z.iter_asc().collect();
                let items = pairs
                    .into_iter()
                    .flat_map(|(m, sc)| [bulk(m), bulk(&format_score(sc))])
                    .collect();
                tagged("zset", items)
            }
            // Attempt state is transient and server-side; clients have no use
            // for it and it must not leak into a browser replica.
            EntryValue::RateLimiter(_) => Value::SimpleString("ratelimit".to_string()),
            EntryValue::Json(doc) => tagged("json", vec![bulk(&doc.to_string())]),
        }
    }

    /// The current value of `key`, as delivered to live-query subscribers.
    ///
    /// Strings come back as a bulk string and a missing or expired key as nil.
    /// Collections come back type-tagged with their complete contents:
    ///
    /// ```text
    /// hash  →  ["hash", field, value, ...]     (HGETALL order)
    /// list  →  ["list", element, ...]          (head to tail)
    /// set   →  ["set", member, ...]
    /// zset  →  ["zset", member, score, ...]    (ascending score)
    /// json  →  ["json", serialized-document]
    /// ```
    pub fn get_current(&self, key: &str) -> Value {
        let now = now_ms();
        match self.data.get(key) {
            None => Value::BulkString(None),
            Some(e) if e.is_expired(now) => Value::BulkString(None),
            Some(e) => Self::encode_current_value(&e.value),
        }
    }

    /// Every live key matching `pattern`, without materialising its value.
    ///
    /// `matching_key_values` clones each value, which is wasted work for a
    /// caller that only needs to know *which* keys are present — reconciling a
    /// live query's local copy against a fresh snapshot, for instance.
    pub fn matching_keys(&self, pattern: &str) -> Vec<String> {
        let now = now_ms();
        self.data
            .iter()
            .filter(|e| !e.is_expired(now) && glob_match(pattern, e.key()))
            .map(|e| e.key().clone())
            .collect()
    }

    /// Current state of every live key matching the glob pattern, in
    /// `get_current` form (strings in full; collections type-tagged with their
    /// complete contents), capped at `limit` entries. Backs QSUB initial state.
    pub fn matching_key_values(&self, pattern: &str, limit: usize) -> Vec<(String, Value)> {
        let now = now_ms();
        self.data
            .iter()
            .filter(|e| !e.is_expired(now) && glob_match(pattern, e.key()))
            .take(limit)
            .map(|e| (e.key().clone(), Self::encode_current_value(&e.value)))
            .collect()
    }

    /// Number of physical keys carrying a TTL.
    ///
    /// The server uses this cheap index read to avoid taking its global write
    /// barrier on maintenance ticks when active expiry has no possible work.
    pub fn volatile_key_count(&self) -> usize {
        self.index
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .volatile
            .keys
            .len()
    }

    /// Drop every expired volatile entry. This full form is retained for
    /// embedders and explicit maintenance; servers should use the bounded form.
    pub fn sweep_expired(&self) {
        self.sweep_expired_reporting();
    }

    /// Drop every currently expired volatile entry and return its key names.
    pub fn sweep_expired_reporting(&self) -> Vec<String> {
        let keys = self
            .index
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .volatile
            .keys
            .clone();
        self.remove_expired_candidates(keys)
    }

    /// Inspect at most `limit` volatile keys and remove the expired ones.
    /// Cursoring through the dense TTL index bounds each server tick while
    /// still eventually visiting the complete volatile keyspace.
    pub fn sweep_expired_reporting_budget(&self, limit: usize) -> Vec<String> {
        if limit == 0 {
            return Vec::new();
        }
        let keys = {
            let index = self
                .index
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let len = index.volatile.keys.len();
            if len == 0 {
                return Vec::new();
            }
            let start = self.expiry_cursor.fetch_add(limit, Ordering::Relaxed) % len;
            let take = limit.min(len);
            (0..take)
                .map(|offset| index.volatile.keys[(start + offset) % len].clone())
                .collect()
        };
        self.remove_expired_candidates(keys)
    }

    fn remove_expired_candidates(&self, keys: Vec<String>) -> Vec<String> {
        let now = now_ms();
        let mut removed = Vec::new();
        for key in keys {
            if let Some((removed_key, entry)) =
                self.data.remove_if(&key, |_, entry| entry.is_expired(now))
            {
                let bytes = entry_size(&removed_key, &entry);
                self.adjust_memory(bytes, 0);
                removed.push(removed_key);
            }
            // Re-read current state: a concurrent replacement may have landed
            // between removal and bookkeeping.
            self.sync_key_metadata(&key);
        }
        removed
    }

    /// Set how many keys each eviction pass samples (default 10, minimum 1).
    pub fn set_eviction_sample(&mut self, sample: usize) {
        self.eviction_sample = sample.max(1);
    }

    /// Keys evicted since start.
    pub fn evicted_count(&self) -> u64 {
        self.evicted.load(Ordering::Relaxed)
    }

    /// Configured memory cap, if any. Reported by `INFO memory` as `maxmemory`.
    pub fn max_memory_bytes(&self) -> Option<usize> {
        self.max_memory_bytes
    }

    /// Configured key-count cap, if any.
    pub fn max_keys(&self) -> Option<usize> {
        self.max_keys
    }

    /// Active eviction policy. Reported by `INFO memory` as `maxmemory_policy`.
    pub fn eviction_policy(&self) -> EvictionPolicy {
        self.eviction_policy
    }

    /// Whether a write may choose and remove an unrelated capacity victim.
    /// The server uses this to reserve every ordering stripe for capped stores.
    pub fn has_capacity_limits(&self) -> bool {
        self.max_keys.is_some() || self.max_memory_bytes.is_some()
    }

    /// O(1) stored-key count maintained with the key index. Expired physical
    /// entries remain counted until the bounded active-expiry task removes them.
    pub fn key_count(&self) -> usize {
        self.index
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .all
            .keys
            .len()
    }

    /// O(1) keyspace counters and logical-memory estimate for INFO/metrics.
    /// Expired physical entries leave these counters during active expiry.
    pub fn keyspace_sample(&self) -> KeyspaceSample {
        let index = self
            .index
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        KeyspaceSample {
            keys: index.all.keys.len(),
            volatile_keys: index.volatile.keys.len(),
            memory_bytes: self.approximate_memory_bytes(),
        }
    }

    fn indexed_candidates(&self, volatile_only: bool, want: usize) -> Vec<String> {
        let index = self
            .index
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let source = if volatile_only {
            &index.volatile.keys
        } else {
            &index.all.keys
        };
        if source.is_empty() || want == 0 {
            return Vec::new();
        }
        let mut rng = rand::rng();
        let take = want.min(source.len());
        let mut positions = BTreeSet::new();
        while positions.len() < take {
            positions.insert(rng.random_range(0..source.len()));
        }
        positions
            .into_iter()
            .map(|position| source[position].clone())
            .collect()
    }

    /// Choose one victim from a bounded random sample. Candidate acquisition
    /// is O(sample), independent of total key count.
    fn evict_one_reporting(&self, now: u64, evicted: &mut Vec<String>) -> Option<usize> {
        let volatile = matches!(
            self.eviction_policy,
            EvictionPolicy::VolatileLru | EvictionPolicy::VolatileTtl
        );
        let want = if self.eviction_policy == EvictionPolicy::AllKeysRandom {
            1
        } else {
            self.eviction_sample
        };
        if self.eviction_policy == EvictionPolicy::NoEviction {
            return None;
        }
        let candidates = self.indexed_candidates(volatile, want);
        let chosen = candidates
            .into_iter()
            .filter_map(|key| {
                let entry = self.data.get(&key)?;
                if entry.is_expired(now) {
                    return Some((key, 0));
                }
                let weight = match self.eviction_policy {
                    EvictionPolicy::AllKeysRandom => 0,
                    EvictionPolicy::AllKeysLru | EvictionPolicy::VolatileLru => {
                        entry.last_access_ms.load(Ordering::Relaxed)
                    }
                    EvictionPolicy::VolatileTtl => entry.expires_at_ms?,
                    EvictionPolicy::NoEviction => return None,
                };
                Some((key, weight))
            })
            .min_by_key(|(_, weight)| *weight)
            .map(|(key, _)| key)?;
        let (removed_key, entry) = self.data.remove(&chosen)?;
        let freed = entry_size(&removed_key, &entry);
        self.adjust_memory(freed, 0);
        self.sync_key_metadata(&removed_key);
        self.evicted.fetch_add(1, Ordering::Relaxed);
        evicted.push(removed_key);
        Some(freed)
    }

    #[cfg(test)]
    fn evict_one(&self, now: u64) -> Option<usize> {
        self.evict_one_reporting(now, &mut Vec::new())
    }

    pub fn snapshot(&self) -> Vec<SnapshotEntry> {
        let now = now_ms();
        self.data
            .iter()
            .filter(|e| !e.is_expired(now))
            .map(|e| {
                let value = match &e.value {
                    EntryValue::Str(s) => SnapshotValue::Str(s.clone()),
                    EntryValue::Hash(m) => SnapshotValue::Hash(m.to_map()),
                    EntryValue::List(l) => SnapshotValue::List(l.iter().cloned().collect()),
                    EntryValue::Set(s) => SnapshotValue::Set(s.iter().cloned().collect()),
                    EntryValue::ZSet(z) => {
                        SnapshotValue::ZSet(z.iter_asc().map(|(m, s)| (m.to_string(), s)).collect())
                    }
                    // Attempt counts are deliberately not persisted: they age
                    // out within a single window, and a restart has already
                    // interrupted that window. Only the configuration is
                    // restored. The field remains for snapshot compatibility.
                    EntryValue::RateLimiter(rl) => SnapshotValue::RateLimiter {
                        limit: rl.limit,
                        window_ms: rl.window_ms,
                        events: Vec::new(),
                    },
                    EntryValue::Json(doc) => {
                        SnapshotValue::Json(serde_json::to_string(doc).unwrap_or_default())
                    }
                };
                SnapshotEntry {
                    key: e.key().clone(),
                    value,
                    expires_at_ms: e.expires_at_ms,
                }
            })
            .collect()
    }

    pub fn restore(&self, entries: Vec<SnapshotEntry>) {
        let now = now_ms();
        for e in entries {
            if let Some(exp) = e.expires_at_ms
                && now >= exp
            {
                continue;
            }
            let value = match e.value {
                SnapshotValue::Str(s) => EntryValue::Str(s),
                SnapshotValue::Hash(m) => EntryValue::Hash(Box::new(CompactHash::from_map(m))),
                SnapshotValue::List(l) => EntryValue::List(l.into_iter().collect()),
                SnapshotValue::Set(s) => EntryValue::Set(Box::new(s.into_iter().collect())),
                SnapshotValue::ZSet(pairs) => {
                    EntryValue::ZSet(Box::new(ZSetInner::from_pairs(pairs)))
                }
                SnapshotValue::RateLimiter {
                    limit,
                    window_ms,
                    events,
                } => {
                    let _ = events; // older snapshots carried attempt timestamps
                    EntryValue::RateLimiter(RateLimiterInner {
                        limit,
                        window_ms,
                        buckets: VecDeque::new(),
                    })
                }
                SnapshotValue::Json(s) => {
                    EntryValue::Json(serde_json::from_str(&s).unwrap_or(serde_json::Value::Null))
                }
            };
            self.data.insert(
                e.key,
                Entry {
                    value,
                    expires_at_ms: e.expires_at_ms,
                    last_access_ms: AtomicU64::new(now),
                },
            );
        }
        self.rebuild_metadata();
    }

    /// Replace the complete keyspace with a full-sync snapshot.
    ///
    /// Replication reconnects must not overlay a primary snapshot onto stale
    /// local keys: keys deleted while disconnected would otherwise survive.
    pub fn replace(&self, entries: Vec<SnapshotEntry>) {
        self.data.clear();
        self.memory_bytes.store(0, Ordering::Relaxed);
        *self
            .index
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = KeyIndex::default();
        self.restore(entries);
    }

    /// Runs one command and discards any implicit capacity-eviction details.
    pub fn execute(&self, cmd: Command) -> Value {
        self.execute_reporting(cmd).0
    }

    /// Runs one command and returns the keys removed implicitly by capacity
    /// eviction. Server transports propagate these deletions as ordered DELs.
    pub fn execute_reporting(&self, cmd: Command) -> (Value, Vec<String>) {
        let scope = mutation_scope(&cmd);
        let mut evicted = Vec::new();
        if matches!(scope, MutationScope::ReadOnly) {
            return (self.execute_inner(cmd, &mut evicted), evicted);
        }

        // Capacity checks and eviction choose across keys, so capped stores use
        // one short synchronous gate. Uncapped stores retain DashMap's normal
        // per-shard concurrency.
        let _capacity_guard = self.has_capacity_limits().then(|| {
            self.capacity_gate
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
        });

        let out = match scope {
            MutationScope::ReadOnly => unreachable!(),
            MutationScope::All => {
                let out = self.execute_inner(cmd, &mut evicted);
                self.rebuild_metadata();
                if self.max_memory_bytes.is_some() {
                    self.try_evict_for_memory_inner(&mut evicted);
                }
                out
            }
            MutationScope::Keys(mut keys) => {
                keys.sort_unstable();
                keys.dedup();
                let memory_before = self.approximate_memory_bytes();
                // `NoEviction` is a refusal policy, not permission to exceed
                // the configured cap. Keep only the touched entries so an
                // oversized write can be rolled back without cloning the
                // entire keyspace on every command.
                let rollback = (self.max_memory_bytes.is_some()
                    && self.eviction_policy == EvictionPolicy::NoEviction)
                    .then(|| {
                        keys.iter()
                            .map(|key| {
                                (
                                    key.clone(),
                                    self.data.get(key).map(|entry| entry.value().clone()),
                                )
                            })
                            .collect::<Vec<_>>()
                    });
                let before = keys.iter().map(|key| self.entry_memory(key)).sum();
                let out = self.execute_inner(cmd, &mut evicted);
                // One lookup per key, not two. Reading the post-write size and
                // reading the key's TTL state both need the same entry, and
                // hash lookups are the largest single cost on the write path
                // (~21% of a `SET` between the probe and its key comparison),
                // so they share one. The index is then locked once for the
                // whole command rather than once per key.
                //
                // The two phases stay separate on purpose: `sync_key_metadata`
                // takes a `data` guard and then the index lock, so holding the
                // index lock across a `data` lookup here would invert that
                // order and risk a deadlock.
                let mut after = 0usize;
                let mut ttl_states: SmallVec<[Option<bool>; 4]> =
                    SmallVec::with_capacity(keys.len());
                for key in &keys {
                    // Expired-but-unswept entries stay in the TTL index on
                    // purpose: dropping them here would make them unreachable
                    // to the bounded sweeper and suppress the expiry
                    // notification. So this asks whether the entry is present,
                    // not whether it is live — matching `sync_key_metadata`.
                    ttl_states.push(match self.data.get(key) {
                        Some(entry) => {
                            after = after.saturating_add(entry_size(key, entry.value()));
                            Some(entry.value().expires_at_ms.is_some())
                        }
                        None => None,
                    });
                }
                {
                    let mut index = self
                        .index
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    for (key, live_ttl) in keys.iter().zip(ttl_states) {
                        index.sync(key, live_ttl);
                    }
                }
                self.adjust_memory(before, after);
                if self.max_memory_bytes.is_some() {
                    let within_limit = self.try_evict_for_memory_inner(&mut evicted);
                    if !within_limit
                        && self.approximate_memory_bytes() > memory_before
                        && let Some(rollback) = rollback
                    {
                        for (key, previous) in rollback {
                            match previous {
                                Some(entry) => {
                                    self.data.insert(key, entry);
                                }
                                None => {
                                    self.data.remove(&key);
                                }
                            }
                        }
                        self.rebuild_metadata();
                        evicted.clear();
                        return (
                            Value::Error(
                                "OOM command not allowed when used memory > 'maxmemory'."
                                    .to_string(),
                            ),
                            evicted,
                        );
                    }
                }
                out
            }
        };
        (out, evicted)
    }

    fn execute_inner(&self, cmd: Command, evicted: &mut Vec<String>) -> Value {
        match cmd {
            // Ephemeral keys are ordinary strings to the engine — their lifetime
            // is enforced by the server, which is the layer that knows about
            // connections. Keeping the engine unaware keeps it I/O-free.
            Command::ESet(key, value) => self.execute_inner(
                Command::Set(key, value, crate::cmd::SetOptions::default()),
                evicted,
            ),

            // ── Core ─────────────────────────────────────────────────────────
            Command::Ping(msg) => match msg {
                Some(m) => Value::BulkString(Some(m.into_bytes())),
                None => Value::SimpleString("PONG".to_string()),
            },
            Command::Auth(_) => Value::Error(
                "ERR AUTH is handled by the connection layer, not the store".to_string(),
            ),
            Command::Hello(_) => Value::Error(
                "ERR HELLO is handled by the connection layer, not the store".to_string(),
            ),
            Command::Info(_) => Value::Error(
                "ERR INFO is handled by the connection layer, not the store".to_string(),
            ),
            Command::Quit => Value::Error(
                "ERR QUIT is handled by the connection layer, not the store".to_string(),
            ),
            Command::Client(_) => Value::Error(
                "ERR CLIENT is handled by the connection layer, not the store".to_string(),
            ),
            Command::Config(_) => Value::Error(
                "ERR CONFIG is handled by the connection layer, not the store".to_string(),
            ),
            Command::CommandQuery(_) => Value::Error(
                "ERR COMMAND is handled by the connection layer, not the store".to_string(),
            ),
            Command::Cluster(_) => Value::Error(
                "ERR CLUSTER is handled by the connection layer, not the store".to_string(),
            ),
            Command::Module(_) => Value::Error(
                "ERR MODULE is handled by the connection layer, not the store".to_string(),
            ),
            Command::PubSub(_) => Value::Error(
                "ERR PUBSUB is handled by the connection layer, not the store".to_string(),
            ),
            Command::Memory(_) => Value::Error(
                "ERR MEMORY is handled by the connection layer, not the store".to_string(),
            ),

            // ── Strings ───────────────────────────────────────────────────────
            Command::Set(key, val, opts) => {
                let now = now_ms();
                if let Some(max) = self.max_keys
                    && self.data.len() >= max
                    && !self.data.contains_key(&key)
                    && self.evict_one_reporting(now, evicted).is_none()
                {
                    return Value::Error("ERR max keys limit reached".to_string());
                }

                let expiry = |existing_ttl: Option<u64>| -> Result<Option<u64>, Value> {
                    match &opts.expiry {
                        None => Ok(None),
                        Some(SetExpiry::Ex(seconds)) => {
                            if *seconds > u64::MAX / 1000 {
                                return Err(Value::Error("ERR TTL overflow".to_string()));
                            }
                            Ok(Some(now.saturating_add(seconds * 1000)))
                        }
                        Some(SetExpiry::Px(ms)) => Ok(Some(now.saturating_add(*ms))),
                        Some(SetExpiry::Exat(seconds)) => Ok(Some(seconds.saturating_mul(1000))),
                        Some(SetExpiry::Pxat(ms)) => Ok(Some(*ms)),
                        Some(SetExpiry::KeepTtl) => Ok(existing_ttl),
                    }
                };

                // The condition check, optional old-value read, and replacement
                // all happen under one DashMap entry guard. Two concurrent NX
                // writers can therefore never both succeed.
                match self.data.entry(key) {
                    DashEntry::Occupied(mut occupied) => {
                        let live = !occupied.get().is_expired(now);
                        let existing_ttl = live.then_some(occupied.get().expires_at_ms).flatten();
                        let existing_str = if live {
                            match &occupied.get().value {
                                EntryValue::Str(value) => Some(value.clone()),
                                _ if opts.get => return Value::Error(WRONGTYPE.to_string()),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        let condition_met = match opts.condition {
                            Some(SetCondition::Nx) => !live,
                            Some(SetCondition::Xx) => live,
                            None => true,
                        };
                        if !condition_met {
                            return if opts.get {
                                existing_str
                                    .map(|value| Value::BulkString(Some(value.into_bytes())))
                                    .unwrap_or(Value::BulkString(None))
                            } else {
                                Value::BulkString(None)
                            };
                        }
                        let expires_at_ms = match expiry(existing_ttl) {
                            Ok(expiry) => expiry,
                            Err(error) => return error,
                        };
                        occupied.insert(Entry {
                            value: EntryValue::Str(val.into()),
                            expires_at_ms,
                            last_access_ms: AtomicU64::new(now),
                        });
                        if opts.get {
                            existing_str
                                .map(|value| Value::BulkString(Some(value.into_bytes())))
                                .unwrap_or(Value::BulkString(None))
                        } else {
                            Value::SimpleString("OK".to_string())
                        }
                    }
                    DashEntry::Vacant(vacant) => {
                        if matches!(opts.condition, Some(SetCondition::Xx)) {
                            return Value::BulkString(None);
                        }
                        let expires_at_ms = match expiry(None) {
                            Ok(expiry) => expiry,
                            Err(error) => return error,
                        };
                        vacant.insert(Entry {
                            value: EntryValue::Str(val.into()),
                            expires_at_ms,
                            last_access_ms: AtomicU64::new(now),
                        });
                        if opts.get {
                            Value::BulkString(None)
                        } else {
                            Value::SimpleString("OK".to_string())
                        }
                    }
                }
            }

            Command::Get(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    Some(e) if !e.is_expired(now) => match &e.value {
                        EntryValue::Str(s) => {
                            e.touch(now);
                            Value::BulkString(Some(s.clone().into_bytes()))
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                    _ => Value::BulkString(None),
                }
            }

            Command::Del(keys) | Command::Unlink(keys) => {
                let now = now_ms();
                let count = keys
                    .into_iter()
                    .filter(|k| self.data.remove_if(k, |_, e| !e.is_expired(now)).is_some())
                    .count();
                Value::Integer(count as i64)
            }

            Command::Append(key, suffix) => {
                let now = now_ms();
                typed_entry!(
                    entry,
                    s,
                    self.data,
                    key,
                    now,
                    EntryValue::Str,
                    Blob::default()
                );
                s.extend(&suffix);
                Value::Integer(s.len() as i64)
            }

            Command::Strlen(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    Some(e) if !e.is_expired(now) => match &e.value {
                        EntryValue::Str(s) => Value::Integer(s.len() as i64),
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                    _ => Value::Integer(0),
                }
            }

            Command::GetRange(key, start, end) => {
                let now = now_ms();
                match self.data.get(&key) {
                    Some(e) if !e.is_expired(now) => match &e.value {
                        EntryValue::Str(s) => {
                            e.touch(now);
                            Value::BulkString(Some(byte_range(s.as_slice(), start, end).to_vec()))
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                    // A missing key is an empty string, so every range of it is
                    // empty — an empty bulk, not a nil.
                    _ => Value::BulkString(Some(Vec::new())),
                }
            }

            Command::GetSet(key, new_val) => {
                let now = now_ms();
                // Read the old value and install the new one under a single
                // guard. Taking the old value through a separate `get` first
                // (as this used to) released the shard between the read and
                // the write, so two connections on two worker threads could
                // both observe the same old value and both report it — a lost
                // update in the one command whose entire purpose is an atomic
                // read-and-replace. Redis gets that atomicity for free from
                // executing on one thread; here it has to be asked for.
                match self.data.entry(key) {
                    DashEntry::Occupied(mut occupied) => {
                        let old = if occupied.get().is_expired(now) {
                            Value::BulkString(None)
                        } else {
                            match &occupied.get().value {
                                EntryValue::Str(s) => {
                                    Value::BulkString(Some(s.clone().into_bytes()))
                                }
                                // Leave the existing value in place: a type
                                // error must not destroy the key.
                                _ => return Value::Error(WRONGTYPE.to_string()),
                            }
                        };
                        occupied.insert(Entry::new_str(new_val));
                        old
                    }
                    DashEntry::Vacant(vacant) => {
                        vacant.insert(Entry::new_str(new_val));
                        Value::BulkString(None)
                    }
                }
            }

            Command::MGet(keys) => {
                let now = now_ms();
                let results = keys
                    .iter()
                    .map(|k| match self.data.get(k) {
                        Some(e) if !e.is_expired(now) => match &e.value {
                            EntryValue::Str(s) => {
                                e.touch(now);
                                Value::BulkString(Some(s.clone().into_bytes()))
                            }
                            _ => Value::BulkString(None),
                        },
                        _ => Value::BulkString(None),
                    })
                    .collect();
                Value::Array(Some(results))
            }

            Command::MSet(pairs) => {
                let now = now_ms();
                if let Some(max) = self.max_keys {
                    let new_count = pairs
                        .iter()
                        .filter(|(k, _)| !self.data.contains_key(k))
                        .count();
                    let available = max.saturating_sub(self.data.len());
                    if new_count > available {
                        let needed = new_count - available;
                        for _ in 0..needed {
                            if self.evict_one_reporting(now, evicted).is_none() {
                                return Value::Error("ERR max keys limit reached".to_string());
                            }
                        }
                    }
                }
                for (k, v) in pairs {
                    self.data.insert(k, Entry::new_str(v));
                }
                Value::SimpleString("OK".to_string())
            }

            Command::SetNx(key, val) => {
                let now = now_ms();
                if let Some(max) = self.max_keys
                    && self.data.len() >= max
                    && !self.data.contains_key(&key)
                    && self.evict_one_reporting(now, evicted).is_none()
                {
                    return Value::Error("ERR max keys limit reached".to_string());
                }
                match self.data.entry(key) {
                    DashEntry::Vacant(vacant) => {
                        vacant.insert(Entry::new_str(val));
                        Value::Integer(1)
                    }
                    DashEntry::Occupied(mut occupied) if occupied.get().is_expired(now) => {
                        occupied.insert(Entry::new_str(val));
                        Value::Integer(1)
                    }
                    DashEntry::Occupied(_) => Value::Integer(0),
                }
            }

            Command::SetEx(key, secs, val) => {
                let now = now_ms();
                let exp = now.saturating_add(secs.saturating_mul(1000));
                if let Some(max) = self.max_keys
                    && self.data.len() >= max
                    && !self.data.contains_key(&key)
                    && self.evict_one_reporting(now, evicted).is_none()
                {
                    return Value::Error("ERR max keys limit reached".to_string());
                }
                self.data.insert(key, Entry::new_str_ex(val, exp));
                Value::SimpleString("OK".to_string())
            }

            Command::PSetEx(key, ms, val) => {
                let now = now_ms();
                let exp = now.saturating_add(ms);
                if let Some(max) = self.max_keys
                    && self.data.len() >= max
                    && !self.data.contains_key(&key)
                    && self.evict_one_reporting(now, evicted).is_none()
                {
                    return Value::Error("ERR max keys limit reached".to_string());
                }
                self.data.insert(key, Entry::new_str_ex(val, exp));
                Value::SimpleString("OK".to_string())
            }

            Command::Incr(key) => incr_by(&self.data, key, 1),
            Command::Decr(key) => incr_by(&self.data, key, -1),
            Command::IncrBy(key, delta) => incr_by(&self.data, key, delta),
            Command::DecrBy(key, delta) => match delta.checked_neg() {
                Some(delta) => incr_by(&self.data, key, delta),
                None => Value::Error("ERR increment or decrement would overflow".to_string()),
            },

            // ── Expiry ────────────────────────────────────────────────────────
            Command::Expire(key, secs) => set_expiry(
                &self.data,
                key,
                now_ms().saturating_add(secs.saturating_mul(1000)),
            ),
            Command::PExpire(key, ms) => set_expiry(&self.data, key, now_ms().saturating_add(ms)),
            Command::ExpireAt(key, ts) => set_expiry(&self.data, key, ts.saturating_mul(1000)),
            Command::PExpireAt(key, ts) => set_expiry(&self.data, key, ts),

            Command::Ttl(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Integer(-2),
                    Some(e) if e.is_expired(now) => Value::Integer(-2),
                    Some(e) => match e.expires_at_ms {
                        None => Value::Integer(-1),
                        // Rounded to nearest, not truncated, matching Redis.
                        // Truncating meant `SET k v EX 100` followed
                        // immediately by `TTL k` answered 99: the handful of
                        // microseconds spent between the two commands took the
                        // remainder just below 100_000 ms, and `/ 1000` threw
                        // the rest away. Every TTL read was up to a second
                        // short, which breaks ported test suites asserting the
                        // value they just set and makes any client that renews
                        // at a threshold renew early, forever.
                        Some(exp) => {
                            let remaining_ms = exp.saturating_sub(now);
                            Value::Integer((remaining_ms.saturating_add(500) / 1000) as i64)
                        }
                    },
                }
            }

            Command::PTtl(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Integer(-2),
                    Some(e) if e.is_expired(now) => Value::Integer(-2),
                    Some(e) => match e.expires_at_ms {
                        None => Value::Integer(-1),
                        Some(exp) => Value::Integer(exp.saturating_sub(now) as i64),
                    },
                }
            }

            Command::Persist(key) => {
                let now = now_ms();
                match self.data.get_mut(&key) {
                    Some(mut e) if !e.is_expired(now) && e.expires_at_ms.is_some() => {
                        e.expires_at_ms = None;
                        Value::Integer(1)
                    }
                    Some(e) if !e.is_expired(now) => Value::Integer(0),
                    _ => Value::Integer(0),
                }
            }

            // ── Keys ──────────────────────────────────────────────────────────
            Command::Exists(keys) => {
                let now = now_ms();
                let count = keys
                    .iter()
                    .filter(|k| self.data.get(*k).is_some_and(|e| !e.is_expired(now)))
                    .count();
                Value::Integer(count as i64)
            }

            Command::Keys(pattern) => {
                let now = now_ms();
                let mut keys: Vec<Value> = self
                    .data
                    .iter()
                    .filter(|r| !r.value().is_expired(now) && glob_match(&pattern, r.key()))
                    .map(|r| Value::BulkString(Some(r.key().as_bytes().to_vec())))
                    .collect();
                keys.sort_unstable_by(|a, b| {
                    let ka = if let Value::BulkString(Some(d)) = a {
                        d.as_slice()
                    } else {
                        &[]
                    };
                    let kb = if let Value::BulkString(Some(d)) = b {
                        d.as_slice()
                    } else {
                        &[]
                    };
                    ka.cmp(kb)
                });
                Value::Array(Some(keys))
            }

            Command::Scan(cursor, pattern, count) => {
                let last_key = if cursor == 0 {
                    None
                } else {
                    self.scan_cursors
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .remove(&cursor)
                };
                if cursor != 0 && last_key.is_none() {
                    return empty_scan();
                }

                let batch = count.unwrap_or(10).max(1);
                let mut candidates: Vec<String> = {
                    let index = self
                        .index
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    match last_key {
                        Some(ref key) => index
                            .ordered
                            .range::<String, _>((
                                std::ops::Bound::Excluded(key),
                                std::ops::Bound::Unbounded,
                            ))
                            .take(batch.saturating_add(1))
                            .cloned()
                            .collect(),
                        None => index
                            .ordered
                            .iter()
                            .take(batch.saturating_add(1))
                            .cloned()
                            .collect(),
                    }
                };
                let has_more = candidates.len() > batch;
                if has_more {
                    candidates.truncate(batch);
                }
                let next_cursor = if has_more {
                    let token = self.next_scan_cursor.fetch_add(1, Ordering::Relaxed).max(1);
                    let last = candidates.last().cloned().unwrap_or_default();
                    let mut cursors = self
                        .scan_cursors
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    if cursors.len() >= 4096
                        && let Some(oldest) = cursors.keys().next().copied()
                    {
                        cursors.remove(&oldest);
                    }
                    cursors.insert(token, last);
                    token
                } else {
                    0
                };
                let now = now_ms();
                let pattern = pattern.as_deref().unwrap_or("*");
                let values = candidates
                    .into_iter()
                    .filter(|key| {
                        glob_match(pattern, key)
                            && self
                                .data
                                .get(key)
                                .is_some_and(|entry| !entry.is_expired(now))
                    })
                    .map(|key| Value::BulkString(Some(key.into_bytes())))
                    .collect();
                scan_reply(next_cursor, values)
            }

            Command::DbSize => Value::Integer(self.key_count() as i64),

            Command::FlushDb => {
                self.data.clear();
                Value::SimpleString("OK".to_string())
            }

            Command::Rename(src, dst) => {
                let now = now_ms();
                match self.data.remove(&src) {
                    None => Value::Error("ERR no such key".to_string()),
                    Some((_, e)) if e.is_expired(now) => {
                        Value::Error("ERR no such key".to_string())
                    }
                    Some((_, entry)) => {
                        self.data.insert(dst, entry);
                        Value::SimpleString("OK".to_string())
                    }
                }
            }

            Command::Type(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    Some(e) if !e.is_expired(now) => {
                        Value::SimpleString(e.value.type_name().to_string())
                    }
                    _ => Value::SimpleString("none".to_string()),
                }
            }
            Command::MemoryUsage(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    // The same figure the eviction loop bills the key for, so
                    // "which key is eating my maxmemory" and "which key gets
                    // evicted next" are answered from one measurement rather
                    // than two that can disagree.
                    Some(e) if !e.is_expired(now) => Value::Integer(entry_size(&key, &e) as i64),
                    _ => Value::BulkString(None),
                }
            }

            // ── Hash ──────────────────────────────────────────────────────────
            Command::HSet(key, pairs) => {
                let now = now_ms();
                typed_entry!(
                    entry,
                    h,
                    self.data,
                    key,
                    now,
                    EntryValue::Hash,
                    Box::new(CompactHash::new())
                );
                let new_count = pairs
                    .iter()
                    .filter(|(f, _)| !h.contains_key(f.as_str()))
                    .count();
                for (field, val) in pairs {
                    h.insert(field, val.into());
                }
                Value::Integer(new_count as i64)
            }

            Command::HGet(key, field) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::BulkString(None),
                    Some(e) if e.is_expired(now) => Value::BulkString(None),
                    Some(e) => match &e.value {
                        EntryValue::Hash(h) => {
                            e.touch(now);
                            h.get(&field)
                                .map(|v| Value::BulkString(Some(v.clone().into_bytes())))
                                .unwrap_or(Value::BulkString(None))
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::HGetAll(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Array(Some(vec![])),
                    Some(e) if e.is_expired(now) => Value::Array(Some(vec![])),
                    Some(e) => match &e.value {
                        EntryValue::Hash(h) => {
                            e.touch(now);
                            let mut pairs: Vec<(&str, &Blob)> =
                                h.iter().map(|(f, v)| (f.as_str(), v)).collect();
                            pairs.sort_unstable_by_key(|(f, _)| *f);
                            let out = pairs
                                .into_iter()
                                .flat_map(|(f, v)| {
                                    [
                                        Value::BulkString(Some(f.as_bytes().to_vec())),
                                        Value::BulkString(Some(v.as_slice().to_vec())),
                                    ]
                                })
                                .collect();
                            Value::Array(Some(out))
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::HDel(key, fields) => {
                let now = now_ms();
                match self.data.get_mut(&key) {
                    None => Value::Integer(0),
                    Some(e) if e.is_expired(now) => Value::Integer(0),
                    Some(mut e) => match &mut e.value {
                        EntryValue::Hash(h) => {
                            let count =
                                fields.into_iter().filter(|f| h.remove(f).is_some()).count();
                            Value::Integer(count as i64)
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::HKeys(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Array(Some(vec![])),
                    Some(e) if e.is_expired(now) => Value::Array(Some(vec![])),
                    Some(e) => match &e.value {
                        EntryValue::Hash(h) => {
                            let mut keys: Vec<&str> =
                                h.iter().map(|(field, _)| field.as_str()).collect();
                            keys.sort_unstable();
                            Value::Array(Some(
                                keys.into_iter()
                                    .map(|k| Value::BulkString(Some(k.as_bytes().to_vec())))
                                    .collect(),
                            ))
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::HScan(key, args) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => empty_scan(),
                    Some(e) if e.is_expired(now) => empty_scan(),
                    Some(e) => match &e.value {
                        EntryValue::Hash(h) => {
                            e.touch(now);
                            let pat = args.pattern.as_deref().unwrap_or("*");
                            let mut fields: Vec<(&str, &Blob)> = h
                                .iter()
                                .filter(|(f, _)| glob_match(pat, f))
                                .map(|(f, v)| (f.as_str(), v))
                                .collect();
                            fields.sort_unstable_by_key(|(f, _)| *f);
                            let (page, next) = scan_page(&fields, args.cursor, args.count);
                            let out = page
                                .iter()
                                .flat_map(|(f, v)| {
                                    let field = Value::BulkString(Some(f.as_bytes().to_vec()));
                                    if args.novalues {
                                        vec![field]
                                    } else {
                                        vec![field, Value::BulkString(Some(v.as_slice().to_vec()))]
                                    }
                                })
                                .collect();
                            scan_reply(next, out)
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::HVals(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Array(Some(vec![])),
                    Some(e) if e.is_expired(now) => Value::Array(Some(vec![])),
                    Some(e) => match &e.value {
                        EntryValue::Hash(h) => {
                            let mut pairs: Vec<(&str, &Blob)> =
                                h.iter().map(|(f, v)| (f.as_str(), v)).collect();
                            pairs.sort_unstable_by_key(|(f, _)| *f);
                            Value::Array(Some(
                                pairs
                                    .into_iter()
                                    .map(|(_, v)| Value::BulkString(Some(v.as_slice().to_vec())))
                                    .collect(),
                            ))
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::HLen(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Integer(0),
                    Some(e) if e.is_expired(now) => Value::Integer(0),
                    Some(e) => match &e.value {
                        EntryValue::Hash(h) => Value::Integer(h.len() as i64),
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::HIncrBy(key, field, delta) => hash_incr_int(&self.data, key, field, delta),

            Command::HIncrByFloat(key, field, delta) => {
                hash_incr_float(&self.data, key, field, delta)
            }

            Command::HExists(key, field) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Integer(0),
                    Some(e) if e.is_expired(now) => Value::Integer(0),
                    Some(e) => match &e.value {
                        EntryValue::Hash(h) => {
                            Value::Integer(if h.contains_key(&field) { 1 } else { 0 })
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::HSetNx(key, field, val) => {
                let now = now_ms();
                typed_entry!(
                    entry,
                    h,
                    self.data,
                    key,
                    now,
                    EntryValue::Hash,
                    Box::new(CompactHash::new())
                );
                Value::Integer(if h.insert_if_absent(field, val.into()) {
                    1
                } else {
                    0
                })
            }

            Command::HMGet(key, fields) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Array(Some(
                        fields.iter().map(|_| Value::BulkString(None)).collect(),
                    )),
                    Some(e) if e.is_expired(now) => Value::Array(Some(
                        fields.iter().map(|_| Value::BulkString(None)).collect(),
                    )),
                    Some(e) => match &e.value {
                        EntryValue::Hash(h) => Value::Array(Some(
                            fields
                                .iter()
                                .map(|f| {
                                    h.get(f)
                                        .map(|v| Value::BulkString(Some(v.clone().into_bytes())))
                                        .unwrap_or(Value::BulkString(None))
                                })
                                .collect(),
                        )),
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            // ── List ──────────────────────────────────────────────────────────
            Command::LPush(key, vals) => {
                let now = now_ms();
                typed_entry!(
                    entry,
                    list,
                    self.data,
                    key,
                    now,
                    EntryValue::List,
                    CompactList::new()
                );
                for v in vals {
                    list.push_front(v.into());
                }
                Value::Integer(list.len() as i64)
            }

            Command::RPush(key, vals) => {
                let now = now_ms();
                typed_entry!(
                    entry,
                    list,
                    self.data,
                    key,
                    now,
                    EntryValue::List,
                    CompactList::new()
                );
                for v in vals {
                    list.push_back(v.into());
                }
                Value::Integer(list.len() as i64)
            }

            Command::LPushX(key, vals) => {
                let now = now_ms();
                match self.data.get_mut(&key) {
                    None => Value::Integer(0),
                    Some(e) if e.is_expired(now) => Value::Integer(0),
                    Some(mut e) => match &mut e.value {
                        EntryValue::List(list) => {
                            for v in vals {
                                list.push_front(v.into());
                            }
                            Value::Integer(list.len() as i64)
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::RPushX(key, vals) => {
                let now = now_ms();
                match self.data.get_mut(&key) {
                    None => Value::Integer(0),
                    Some(e) if e.is_expired(now) => Value::Integer(0),
                    Some(mut e) => match &mut e.value {
                        EntryValue::List(list) => {
                            for v in vals {
                                list.push_back(v.into());
                            }
                            Value::Integer(list.len() as i64)
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::LPop(key, count) => {
                let now = now_ms();
                match self.data.get_mut(&key) {
                    None => no_list_response(count),
                    Some(e) if e.is_expired(now) => no_list_response(count),
                    Some(mut e) => match &mut e.value {
                        EntryValue::List(list) => {
                            if let Some(n) = count {
                                // Bounded by the list, never by `n`. Asking for
                                // more than exists is legitimate, but iterating
                                // the shortfall pins this worker thread *and*
                                // the shard guard for as long as the count says
                                // — `LPOP key 9223372036854775807` measured out
                                // at roughly 240 years on an empty list.
                                let take = n.min(list.len() as u64) as usize;
                                let items: Vec<Value> = list
                                    .drain_front(take)
                                    .into_iter()
                                    .map(|v| Value::BulkString(Some(v.into_bytes())))
                                    .collect();
                                Value::Array(Some(items))
                            } else {
                                list.pop_front()
                                    .map(|v| Value::BulkString(Some(v.into_bytes())))
                                    .unwrap_or(Value::BulkString(None))
                            }
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::RPop(key, count) => {
                let now = now_ms();
                match self.data.get_mut(&key) {
                    None => no_list_response(count),
                    Some(e) if e.is_expired(now) => no_list_response(count),
                    Some(mut e) => match &mut e.value {
                        EntryValue::List(list) => {
                            if let Some(n) = count {
                                // Bounded by the list — see `LPop`. `.rev()`
                                // keeps the reply in pop order (tail first),
                                // which is what the repeated `pop_back` this
                                // replaces produced.
                                let take = n.min(list.len() as u64) as usize;
                                let items: Vec<Value> = list
                                    .drain_back(take)
                                    .into_iter()
                                    .map(|v| Value::BulkString(Some(v.into_bytes())))
                                    .collect();
                                Value::Array(Some(items))
                            } else {
                                list.pop_back()
                                    .map(|v| Value::BulkString(Some(v.into_bytes())))
                                    .unwrap_or(Value::BulkString(None))
                            }
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::LRange(key, start, stop) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Array(Some(vec![])),
                    Some(e) if e.is_expired(now) => Value::Array(Some(vec![])),
                    Some(e) => match &e.value {
                        EntryValue::List(list) => {
                            e.touch(now);
                            let slice: Vec<&Blob> = list.iter().collect();
                            match resolve_range(start, stop, slice.len()) {
                                None => Value::Array(Some(vec![])),
                                Some((s, e)) => Value::Array(Some(
                                    slice[s..=e]
                                        .iter()
                                        .map(|v| Value::BulkString(Some(v.as_slice().to_vec())))
                                        .collect(),
                                )),
                            }
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::LLen(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Integer(0),
                    Some(e) if e.is_expired(now) => Value::Integer(0),
                    Some(e) => match &e.value {
                        EntryValue::List(l) => Value::Integer(l.len() as i64),
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::LIndex(key, idx) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::BulkString(None),
                    Some(e) if e.is_expired(now) => Value::BulkString(None),
                    Some(e) => match &e.value {
                        EntryValue::List(list) => {
                            let slice: Vec<&Blob> = list.iter().collect();
                            resolve_idx(idx, slice.len())
                                .map(|i| Value::BulkString(Some(slice[i].as_slice().to_vec())))
                                .unwrap_or(Value::BulkString(None))
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::LSet(key, idx, val) => {
                let now = now_ms();
                match self.data.get_mut(&key) {
                    None => Value::Error("ERR no such key".to_string()),
                    Some(e) if e.is_expired(now) => Value::Error("ERR no such key".to_string()),
                    Some(mut e) => match &mut e.value {
                        EntryValue::List(list) => {
                            let len = list.len();
                            match resolve_idx(idx, len) {
                                None => Value::Error("ERR index out of range".to_string()),
                                Some(i) => {
                                    list.set(i, val.into());
                                    Value::SimpleString("OK".to_string())
                                }
                            }
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::LRem(key, count, element) => {
                let now = now_ms();
                match self.data.get_mut(&key) {
                    None => Value::Integer(0),
                    Some(e) if e.is_expired(now) => Value::Integer(0),
                    Some(mut e) => match &mut e.value {
                        EntryValue::List(list) => {
                            let mut removed = 0i64;
                            let abs = count.unsigned_abs() as usize;
                            if count >= 0 {
                                let mut i = 0;
                                while i < list.len() && (count == 0 || removed < abs as i64) {
                                    if list
                                        .get(i)
                                        .is_some_and(|v| v.as_slice() == element.as_slice())
                                    {
                                        list.remove(i);
                                        removed += 1;
                                    } else {
                                        i += 1;
                                    }
                                }
                            } else {
                                let mut i = list.len();
                                while i > 0 && removed < abs as i64 {
                                    i -= 1;
                                    if list
                                        .get(i)
                                        .is_some_and(|v| v.as_slice() == element.as_slice())
                                    {
                                        list.remove(i);
                                        removed += 1;
                                    }
                                }
                            }
                            Value::Integer(removed)
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::LTrim(key, start, stop) => {
                let now = now_ms();
                match self.data.get_mut(&key) {
                    None => Value::SimpleString("OK".to_string()),
                    Some(e) if e.is_expired(now) => Value::SimpleString("OK".to_string()),
                    Some(mut e) => match &mut e.value {
                        EntryValue::List(list) => {
                            let len = list.len();
                            match resolve_range(start, stop, len) {
                                None => list.clear(),
                                Some((s, e)) => list.retain_range(s, e),
                            }
                            Value::SimpleString("OK".to_string())
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            // ── Set ───────────────────────────────────────────────────────────
            Command::SAdd(key, members) => {
                let now = now_ms();
                typed_entry!(
                    entry,
                    set,
                    self.data,
                    key,
                    now,
                    EntryValue::Set,
                    Box::new(CompactSet::new())
                );
                let added = members
                    .into_iter()
                    .filter(|m| set.insert(m.clone()))
                    .count();
                Value::Integer(added as i64)
            }

            Command::SMembers(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Array(Some(vec![])),
                    Some(e) if e.is_expired(now) => Value::Array(Some(vec![])),
                    Some(e) => match &e.value {
                        EntryValue::Set(s) => {
                            e.touch(now);
                            let mut members: Vec<&str> = s.iter().map(|m| m.as_str()).collect();
                            members.sort_unstable();
                            Value::Array(Some(
                                members
                                    .into_iter()
                                    .map(|m| Value::BulkString(Some(m.as_bytes().to_vec())))
                                    .collect(),
                            ))
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::SScan(key, args) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => empty_scan(),
                    Some(e) if e.is_expired(now) => empty_scan(),
                    Some(e) => match &e.value {
                        EntryValue::Set(s) => {
                            e.touch(now);
                            let pat = args.pattern.as_deref().unwrap_or("*");
                            let mut members: Vec<&str> = s
                                .iter()
                                .map(|m| m.as_str())
                                .filter(|m| glob_match(pat, m))
                                .collect();
                            members.sort_unstable();
                            let (page, next) = scan_page(&members, args.cursor, args.count);
                            let out = page
                                .iter()
                                .map(|m| Value::BulkString(Some(m.as_bytes().to_vec())))
                                .collect();
                            scan_reply(next, out)
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::SRem(key, members) => {
                let now = now_ms();
                match self.data.get_mut(&key) {
                    None => Value::Integer(0),
                    Some(e) if e.is_expired(now) => Value::Integer(0),
                    Some(mut e) => match &mut e.value {
                        EntryValue::Set(s) => {
                            let removed = members.into_iter().filter(|m| s.swap_remove(m)).count();
                            Value::Integer(removed as i64)
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::SCard(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Integer(0),
                    Some(e) if e.is_expired(now) => Value::Integer(0),
                    Some(e) => match &e.value {
                        EntryValue::Set(s) => Value::Integer(s.len() as i64),
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::SIsMember(key, member) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Integer(0),
                    Some(e) if e.is_expired(now) => Value::Integer(0),
                    Some(e) => match &e.value {
                        EntryValue::Set(s) => {
                            e.touch(now);
                            Value::Integer(if s.contains(&member) { 1 } else { 0 })
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::SMIsMember(key, members) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Array(Some(members.iter().map(|_| Value::Integer(0)).collect())),
                    Some(e) if e.is_expired(now) => {
                        Value::Array(Some(members.iter().map(|_| Value::Integer(0)).collect()))
                    }
                    Some(e) => match &e.value {
                        EntryValue::Set(s) => Value::Array(Some(
                            members
                                .iter()
                                .map(|m| Value::Integer(if s.contains(m) { 1 } else { 0 }))
                                .collect(),
                        )),
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::SInter(keys) => {
                let now = now_ms();
                match set_inter(&self.data, &keys, now) {
                    Err(e) => e,
                    Ok(result) => set_to_value(result),
                }
            }

            Command::SInterStore(dst, keys) => {
                let now = now_ms();
                let result = {
                    match set_inter(&self.data, &keys, now) {
                        Err(e) => return e,
                        Ok(r) => r,
                    }
                };
                let len = result.len();
                self.data.insert(
                    dst,
                    Entry {
                        value: EntryValue::Set(Box::new(CompactSet::from_index_set(result))),
                        expires_at_ms: None,
                        last_access_ms: AtomicU64::new(now_ms()),
                    },
                );
                Value::Integer(len as i64)
            }

            Command::SUnion(keys) => {
                let now = now_ms();
                match set_union(&self.data, &keys, now) {
                    Err(e) => e,
                    Ok(result) => set_to_value(result),
                }
            }

            Command::SUnionStore(dst, keys) => {
                let now = now_ms();
                let result = {
                    match set_union(&self.data, &keys, now) {
                        Err(e) => return e,
                        Ok(r) => r,
                    }
                };
                let len = result.len();
                self.data.insert(
                    dst,
                    Entry {
                        value: EntryValue::Set(Box::new(CompactSet::from_index_set(result))),
                        expires_at_ms: None,
                        last_access_ms: AtomicU64::new(now_ms()),
                    },
                );
                Value::Integer(len as i64)
            }

            Command::SDiff(keys) => {
                let now = now_ms();
                match set_diff(&self.data, &keys, now) {
                    Err(e) => e,
                    Ok(result) => set_to_value(result),
                }
            }

            Command::SDiffStore(dst, keys) => {
                let now = now_ms();
                let result = {
                    match set_diff(&self.data, &keys, now) {
                        Err(e) => return e,
                        Ok(r) => r,
                    }
                };
                let len = result.len();
                self.data.insert(
                    dst,
                    Entry {
                        value: EntryValue::Set(Box::new(CompactSet::from_index_set(result))),
                        expires_at_ms: None,
                        last_access_ms: AtomicU64::new(now_ms()),
                    },
                );
                Value::Integer(len as i64)
            }

            Command::SPop(key, count) => {
                let now = now_ms();
                match self.data.get_mut(&key) {
                    None => no_list_response(count),
                    Some(e) if e.is_expired(now) => no_list_response(count),
                    Some(mut e) => match &mut e.value {
                        EntryValue::Set(s) => {
                            // Compared as `u64` rather than cast to `usize`:
                            // `usize` is 32-bit under wasm32, where this same
                            // engine runs in the browser, so a count above
                            // `u32::MAX` would truncate — potentially to zero.
                            let n = count.unwrap_or(1);
                            let mut rng = rand::rng();
                            // SPOP removes *random* members, not iteration-order ones.
                            // swap_remove_index is O(1), so popping k members costs
                            // O(k) regardless of set size.
                            let popped: Vec<String> = if n >= s.len() as u64 {
                                s.drain_all()
                            } else {
                                // `n < s.len()` here, so the narrowing is exact.
                                (0..n as usize)
                                    .map(|_| {
                                        let idx = rng.random_range(0..s.len());
                                        s.swap_remove_index(idx).expect("index in range")
                                    })
                                    .collect()
                            };
                            if count.is_some() {
                                Value::Array(Some(
                                    popped
                                        .into_iter()
                                        .map(|m| Value::BulkString(Some(m.into_bytes())))
                                        .collect(),
                                ))
                            } else {
                                popped
                                    .into_iter()
                                    .next()
                                    .map(|m| Value::BulkString(Some(m.into_bytes())))
                                    .unwrap_or(Value::BulkString(None))
                            }
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::SRandMember(key, count) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => match count {
                        None => Value::BulkString(None),
                        Some(_) => Value::Array(Some(vec![])),
                    },
                    Some(e) if e.is_expired(now) => match count {
                        None => Value::BulkString(None),
                        Some(_) => Value::Array(Some(vec![])),
                    },
                    Some(e) => match &e.value {
                        EntryValue::Set(s) => match count {
                            None => {
                                if s.is_empty() {
                                    return Value::BulkString(None);
                                }
                                let mut rng = rand::rng();
                                let idx = rng.random_range(0..s.len());
                                Value::BulkString(Some(
                                    s.get_index(idx)
                                        .expect("index in range")
                                        .as_bytes()
                                        .to_vec(),
                                ))
                            }
                            Some(n) if n >= 0 => {
                                // Positive count: up to n *distinct* random members.
                                let mut rng = rand::rng();
                                // `try_from` rather than `as`: `usize` is
                                // 32-bit under wasm32, where a count above
                                // `u32::MAX` would truncate instead of clamp.
                                let amount = usize::try_from(n).unwrap_or(usize::MAX).min(s.len());
                                let idxs = rand::seq::index::sample(&mut rng, s.len(), amount);
                                Value::Array(Some(
                                    idxs.iter()
                                        .map(|i| {
                                            Value::BulkString(Some(
                                                s.get_index(i)
                                                    .expect("index in range")
                                                    .as_bytes()
                                                    .to_vec(),
                                            ))
                                        })
                                        .collect(),
                                ))
                            }
                            Some(n) => {
                                // Negative: allow repetition, return |n| random elements.
                                if s.is_empty() {
                                    return Value::Array(Some(vec![]));
                                }
                                let mut rng = rand::rng();
                                // The parser refuses a larger magnitude, but a
                                // `Command` also arrives from AOF replay, a
                                // replication frame or `sync-client`, none of
                                // which pass through it.
                                let abs = usize::try_from(n.unsigned_abs())
                                    .unwrap_or(usize::MAX)
                                    .min(crate::resp::MAX_ARRAY_ELEMENTS);
                                Value::Array(Some(
                                    (0..abs)
                                        .map(|_| {
                                            let idx = rng.random_range(0..s.len());
                                            Value::BulkString(Some(
                                                s.get_index(idx)
                                                    .expect("index in range")
                                                    .as_bytes()
                                                    .to_vec(),
                                            ))
                                        })
                                        .collect(),
                                ))
                            }
                        },
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::SMove(src, dst, member) => {
                let now = now_ms();
                // Check types
                let src_type_ok = match self.data.get(&src) {
                    None => true,
                    Some(e) if e.is_expired(now) => true,
                    Some(e) => matches!(&e.value, EntryValue::Set(_)),
                };
                let dst_type_ok = match self.data.get(&dst) {
                    None => true,
                    Some(e) if e.is_expired(now) => true,
                    Some(e) => matches!(&e.value, EntryValue::Set(_)),
                };
                if !src_type_ok || !dst_type_ok {
                    return Value::Error(WRONGTYPE.to_string());
                }
                // Remove from source
                let removed = match self.data.get_mut(&src) {
                    Some(mut e) if !e.is_expired(now) => {
                        if let EntryValue::Set(s) = &mut e.value {
                            s.swap_remove(&member)
                        } else {
                            false
                        }
                    }
                    _ => false,
                };
                if !removed {
                    return Value::Integer(0);
                }
                // Add to destination. Expiry and type resolve under the one
                // guard that performs the insert — reading them separately
                // first (as this used to) let the destination be retyped in
                // between, after which the member was silently dropped and the
                // reply still claimed a successful move.
                let mut dst_entry = self.data.entry(dst).or_insert_with(|| Entry {
                    value: EntryValue::Set(Box::new(CompactSet::new())),
                    expires_at_ms: None,
                    last_access_ms: AtomicU64::new(now),
                });
                if dst_entry.is_expired(now) {
                    dst_entry.value = EntryValue::Set(Box::new(CompactSet::new()));
                    dst_entry.expires_at_ms = None;
                }
                if let EntryValue::Set(s) = &mut dst_entry.value {
                    s.insert(member);
                    return Value::Integer(1);
                }
                // Destination was retyped after the type check above. The source
                // removal has already happened, so put the member back rather
                // than losing it: SMOVE either moves or fails.
                drop(dst_entry);
                if let Some(mut e) = self.data.get_mut(&src)
                    && let EntryValue::Set(s) = &mut e.value
                {
                    s.insert(member);
                }
                Value::Error(WRONGTYPE.to_string())
            }

            // ── Sorted Set ────────────────────────────────────────────────────
            Command::ZAdd(key, opts, pairs) => {
                let now = now_ms();
                typed_entry!(
                    entry,
                    zset,
                    self.data,
                    key,
                    now,
                    EntryValue::ZSet,
                    Box::new(ZSetInner::new())
                );
                zadd_exec(zset, opts, pairs)
            }

            Command::ZRange(key, start, stop, withscores) => zset_read(&self.data, &key, |zset| {
                Ok(encode_zrange(
                    &index_slice(zset.iter_asc(), zset.len(), start, stop),
                    withscores,
                ))
            }),

            Command::ZRevRange(key, start, stop, withscores) => {
                zset_read(&self.data, &key, |zset| {
                    Ok(encode_zrange(
                        &index_slice(zset.iter_asc().rev(), zset.len(), start, stop),
                        withscores,
                    ))
                })
            }

            Command::ZRangeByScore(key, min_s, max_s, withscores, limit) => {
                zset_read(&self.data, &key, |zset| {
                    let min = ScoreBound::parse(&min_s)?;
                    let max = ScoreBound::parse(&max_s)?;
                    let filtered: Vec<(&str, f64)> = zset.range_by_score(&min, &max).collect();
                    let limited = apply_limit(filtered, limit);
                    Ok(encode_zrange(&limited, withscores))
                })
            }

            Command::ZRevRangeByScore(key, max_s, min_s, withscores, limit) => {
                zset_read(&self.data, &key, |zset| {
                    let min = ScoreBound::parse(&min_s)?;
                    let max = ScoreBound::parse(&max_s)?;
                    let mut filtered: Vec<(&str, f64)> = zset.range_by_score(&min, &max).collect();
                    filtered.reverse();
                    let limited = apply_limit(filtered, limit);
                    Ok(encode_zrange(&limited, withscores))
                })
            }

            Command::ZScore(key, member) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::BulkString(None),
                    Some(e) if e.is_expired(now) => Value::BulkString(None),
                    Some(e) => match &e.value {
                        EntryValue::ZSet(z) => {
                            e.touch(now);
                            z.score(&member)
                                .map(|s| Value::BulkString(Some(format_score(s).into_bytes())))
                                .unwrap_or(Value::BulkString(None))
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::ZMScore(key, members) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Array(Some(
                        members.iter().map(|_| Value::BulkString(None)).collect(),
                    )),
                    Some(e) if e.is_expired(now) => Value::Array(Some(
                        members.iter().map(|_| Value::BulkString(None)).collect(),
                    )),
                    Some(e) => match &e.value {
                        EntryValue::ZSet(z) => Value::Array(Some(
                            members
                                .iter()
                                .map(|m| {
                                    z.score(m)
                                        .map(|s| {
                                            Value::BulkString(Some(format_score(s).into_bytes()))
                                        })
                                        .unwrap_or(Value::BulkString(None))
                                })
                                .collect(),
                        )),
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::ZRank(key, member) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::BulkString(None),
                    Some(e) if e.is_expired(now) => Value::BulkString(None),
                    Some(e) => match &e.value {
                        EntryValue::ZSet(z) => z
                            .rank(&member)
                            .map(|i| Value::Integer(i as i64))
                            .unwrap_or(Value::BulkString(None)),
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::ZRevRank(key, member) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::BulkString(None),
                    Some(e) if e.is_expired(now) => Value::BulkString(None),
                    Some(e) => match &e.value {
                        EntryValue::ZSet(z) => z
                            .rank(&member)
                            .map(|i| Value::Integer((z.len() - 1 - i) as i64))
                            .unwrap_or(Value::BulkString(None)),
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::ZRem(key, members) => {
                let now = now_ms();
                match self.data.get_mut(&key) {
                    None => Value::Integer(0),
                    Some(e) if e.is_expired(now) => Value::Integer(0),
                    Some(mut e) => match &mut e.value {
                        EntryValue::ZSet(z) => {
                            let removed = members.iter().filter(|m| z.remove(m).is_some()).count();
                            Value::Integer(removed as i64)
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::ZCard(key) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::Integer(0),
                    Some(e) if e.is_expired(now) => Value::Integer(0),
                    Some(e) => match &e.value {
                        EntryValue::ZSet(z) => Value::Integer(z.len() as i64),
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::ZIncrBy(key, delta, member) => {
                let now = now_ms();
                typed_entry!(
                    entry,
                    zset,
                    self.data,
                    key,
                    now,
                    EntryValue::ZSet,
                    Box::new(ZSetInner::new())
                );
                let prev_score = zset.score(&member).unwrap_or(0.0);
                let new_score = prev_score + delta;
                if new_score.is_nan() || new_score.is_infinite() {
                    return Value::Error("ERR increment would produce NaN or Infinity".to_string());
                }
                zset.insert(&member, new_score);
                Value::BulkString(Some(format_score(new_score).into_bytes()))
            }

            Command::ZCount(key, min_s, max_s) => zset_read(&self.data, &key, |zset| {
                let min = ScoreBound::parse(&min_s)?;
                let max = ScoreBound::parse(&max_s)?;
                let count = zset.range_by_score(&min, &max).count();
                Ok(Value::Integer(count as i64))
            }),

            Command::ZScan(key, args) => zset_read(&self.data, &key, |zset| {
                let pat = args.pattern.as_deref().unwrap_or("*");
                let mut members: Vec<(&str, f64)> =
                    zset.members().filter(|(m, _)| glob_match(pat, m)).collect();
                members.sort_unstable_by_key(|(m, _)| *m);
                let (page, next) = scan_page(&members, args.cursor, args.count);
                let out = page
                    .iter()
                    .flat_map(|(m, s)| {
                        [
                            Value::BulkString(Some(m.as_bytes().to_vec())),
                            Value::BulkString(Some(format_score(*s).into_bytes())),
                        ]
                    })
                    .collect();
                Ok(scan_reply(next, out))
            }),

            // ── JSON ──────────────────────────────────────────────────────────
            Command::JSet(key, path, value) => {
                let now = now_ms();
                // Parsed before the map is touched: a malformed path or payload
                // never creates a key, and the guard below is held for as short
                // a time as possible.
                let segs = match parse_json_path(&path) {
                    Ok(s) => s,
                    Err(e) => return Value::Error(e),
                };
                let val: serde_json::Value = match serde_json::from_str(&value) {
                    Ok(v) => v,
                    Err(e) => return Value::Error(format!("ERR invalid JSON value: {}", e)),
                };
                // A fresh document starts as null; a leading index segment can
                // never apply to it — reject before creating the key.
                let leading_index = matches!(segs.first(), Some(JsonPathSeg::Index(_)));
                const NOT_AN_ARRAY: &str =
                    "ERR path segment '[..]' is not an array (key does not exist)";
                // Freshness, type and write all resolve under one guard. Reading
                // `contains_key` separately (as this used to) let a concurrent
                // writer create the key between the test and the insert.
                let mut entry = match self.data.entry(key) {
                    DashEntry::Vacant(v) => {
                        if leading_index {
                            return Value::Error(NOT_AN_ARRAY.to_string());
                        }
                        v.insert(Entry {
                            value: EntryValue::Json(serde_json::Value::Null),
                            expires_at_ms: None,
                            last_access_ms: AtomicU64::new(now),
                        })
                    }
                    DashEntry::Occupied(mut o) => {
                        let e = o.get_mut();
                        if e.is_expired(now) {
                            if leading_index {
                                return Value::Error(NOT_AN_ARRAY.to_string());
                            }
                            e.value = EntryValue::Json(serde_json::Value::Null);
                            e.expires_at_ms = None;
                        }
                        o.into_ref()
                    }
                };
                let EntryValue::Json(doc) = &mut entry.value else {
                    return Value::Error(WRONGTYPE.to_string());
                };
                match json_set_at(doc, &segs, val) {
                    Ok(()) => Value::SimpleString("OK".to_string()),
                    Err(e) => Value::Error(e),
                }
            }

            Command::JGet(key, path) => {
                let now = now_ms();
                match self.data.get(&key) {
                    None => Value::BulkString(None),
                    Some(e) if e.is_expired(now) => Value::BulkString(None),
                    Some(e) => match &e.value {
                        EntryValue::Json(doc) => {
                            e.touch(now);
                            let segs = match parse_json_path(path.as_deref().unwrap_or("$")) {
                                Ok(s) => s,
                                Err(err) => return Value::Error(err),
                            };
                            match json_get_at(doc, &segs) {
                                Some(v) => Value::BulkString(Some(
                                    serde_json::to_string(v).unwrap_or_default().into_bytes(),
                                )),
                                None => Value::BulkString(None),
                            }
                        }
                        _ => Value::Error(WRONGTYPE.to_string()),
                    },
                }
            }

            Command::JMerge(key, patch) => {
                let now = now_ms();
                let patch: serde_json::Value = match serde_json::from_str(&patch) {
                    Ok(v) => v,
                    Err(e) => return Value::Error(format!("ERR invalid JSON patch: {}", e)),
                };
                if patch.is_null() {
                    // RFC 7386: a null patch replaces the target — the key is
                    // deleted rather than left holding a bare null. Removed only
                    // if it really is a JSON document, so a null patch cannot be
                    // used to delete a string or a hash.
                    let mut wrong_type = false;
                    self.data.remove_if(&key, |_, e| {
                        let ok = e.is_expired(now) || matches!(e.value, EntryValue::Json(_));
                        wrong_type = !ok;
                        ok
                    });
                    if wrong_type {
                        return Value::Error(WRONGTYPE.to_string());
                    }
                    return Value::SimpleString("OK".to_string());
                }
                typed_entry!(
                    entry,
                    doc,
                    self.data,
                    key,
                    now,
                    EntryValue::Json,
                    serde_json::Value::Null
                );
                json_merge_patch(doc, patch);
                Value::SimpleString("OK".to_string())
            }

            // ── Rate limiting ─────────────────────────────────────────────────
            Command::RlSet(key, limit, window_secs) => {
                let now = now_ms();
                let window_ms = window_secs.saturating_mul(1000);
                typed_entry!(
                    entry,
                    rl,
                    self.data,
                    key,
                    now,
                    EntryValue::RateLimiter,
                    RateLimiterInner::new(limit, window_ms)
                );
                // Reconfigure in place; recorded attempts are kept so a live
                // limiter is not reset by a config change.
                rl.limit = limit;
                rl.window_ms = window_ms;
                // Explicitly configured limiters persist until DEL/EXPIRE, unlike
                // limiters auto-created by inline RLCHECK config.
                entry.expires_at_ms = None;
                Value::SimpleString("OK".to_string())
            }

            Command::RlCheck(key, config) => {
                let now = now_ms();
                // Auto-created limiters self-clean: they expire one window after
                // the last attempt, so per-IP / per-user keys don't accumulate
                // forever.
                let fresh_limiter = |k: &str| -> Result<Entry, Value> {
                    let Some((limit, window_secs)) = config else {
                        return Err(Value::Error(format!(
                            "ERR no rate limit configured for '{}'; call RLSET first or use RLCHECK key limit window",
                            k
                        )));
                    };
                    let window_ms = window_secs.saturating_mul(1000);
                    Ok(Entry {
                        value: EntryValue::RateLimiter(RateLimiterInner::new(limit, window_ms)),
                        expires_at_ms: Some(now.saturating_add(window_ms)),
                        last_access_ms: AtomicU64::new(now),
                    })
                };
                // One guard for the lot. The old shape took three separate
                // lookups — type check, existence check, then `get_mut` — and
                // needed an "ERR rate limiter vanished mid-check" arm for the
                // case where the key was deleted in between. It cannot happen
                // now, so that error is gone.
                let mut entry = match self.data.entry(key) {
                    DashEntry::Vacant(v) => match fresh_limiter(v.key()) {
                        Ok(e) => v.insert(e),
                        Err(err) => return err,
                    },
                    DashEntry::Occupied(mut o) => {
                        if o.get().is_expired(now) {
                            match fresh_limiter(o.key()) {
                                Ok(e) => *o.get_mut() = e,
                                Err(err) => return err,
                            }
                        }
                        o.into_ref()
                    }
                };
                let EntryValue::RateLimiter(rl) = &mut entry.value else {
                    return Value::Error(WRONGTYPE.to_string());
                };
                if let Some((limit, window_secs)) = config {
                    // Inline config wins: middleware config changes propagate
                    // without a separate RLSET.
                    rl.limit = limit;
                    rl.window_ms = window_secs.saturating_mul(1000);
                }
                let (allowed, remaining, retry_after_ms) = rl.check(now);
                let window_ms = rl.window_ms;
                if entry.expires_at_ms.is_some() {
                    entry.expires_at_ms = Some(now.saturating_add(window_ms));
                }
                Value::Array(Some(vec![
                    Value::Integer(allowed),
                    Value::Integer(remaining as i64),
                    Value::Integer(retry_after_ms as i64),
                ]))
            }

            // ── Transactions ─────────────────────────────────────────────────
            // These are handled at the server layer before reaching the store.
            // The arms below are fallback-only (e.g. store used in tests).
            Command::Multi => Value::SimpleString("OK".to_string()),
            Command::Exec => Value::Error("ERR EXEC without MULTI".to_string()),
            Command::Discard => Value::Error("ERR DISCARD without MULTI".to_string()),

            // ── Pub/Sub ───────────────────────────────────────────────────────
            // Routing is handled entirely in the server layer.
            Command::Subscribe(_)
            | Command::Unsubscribe(_)
            | Command::PSubscribe(_)
            | Command::PUnsubscribe(_) => Value::Error("ERR only in pub/sub context".to_string()),
            // Sync scoping is a WebSocket-connection concern, handled entirely
            // in the server layer.
            Command::Sync(_) => {
                Value::Error("ERR SYNC is only available on the WebSocket port".to_string())
            }
            // Live queries are likewise per-WebSocket-connection state.
            Command::QSub(_) | Command::QUnsub(_) => Value::Error(
                "ERR live queries are only available on the WebSocket port".to_string(),
            ),
            // Deduplication is unwrapped in the server layer before execution.
            Command::Dedup(_, _, _) => {
                Value::Error("ERR DEDUP is only available on the WebSocket port".to_string())
            }
            Command::Publish(_, _) => Value::Integer(0),

            Command::Unknown(name) => Value::Error(format!("ERR unknown command '{}'", name)),
            Command::Watch(_) | Command::Unwatch(_) => {
                Value::Error("ERR WATCH/UNWATCH only supported over WebSocket".to_string())
            }
            Command::Save | Command::BgSave | Command::LastSave => {
                Value::Error("ERR persistence commands must be handled by the server".to_string())
            }
            Command::ReplicaOfNoOne => {
                Value::Error("ERR REPLICAOF NO ONE must be handled by the server".to_string())
            }
        }
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

enum MutationScope {
    ReadOnly,
    Keys(Vec<String>),
    All,
}

fn mutation_scope(cmd: &Command) -> MutationScope {
    if let Command::Dedup(_, _, inner) = cmd {
        return mutation_scope(inner);
    }
    let one = |key: &String| MutationScope::Keys(vec![key.clone()]);
    match cmd {
        Command::Set(key, _, _)
        | Command::ESet(key, _)
        | Command::Append(key, _)
        | Command::GetSet(key, _)
        | Command::SetNx(key, _)
        | Command::SetEx(key, _, _)
        | Command::PSetEx(key, _, _)
        | Command::Incr(key)
        | Command::Decr(key)
        | Command::IncrBy(key, _)
        | Command::DecrBy(key, _)
        | Command::Expire(key, _)
        | Command::PExpire(key, _)
        | Command::ExpireAt(key, _)
        | Command::PExpireAt(key, _)
        | Command::Persist(key)
        | Command::HSet(key, _)
        | Command::HDel(key, _)
        | Command::HIncrBy(key, _, _)
        | Command::HIncrByFloat(key, _, _)
        | Command::HSetNx(key, _, _)
        | Command::LPush(key, _)
        | Command::RPush(key, _)
        | Command::LPushX(key, _)
        | Command::RPushX(key, _)
        | Command::LPop(key, _)
        | Command::RPop(key, _)
        | Command::LSet(key, _, _)
        | Command::LRem(key, _, _)
        | Command::LTrim(key, _, _)
        | Command::SAdd(key, _)
        | Command::SRem(key, _)
        | Command::SPop(key, _)
        | Command::ZAdd(key, _, _)
        | Command::ZRem(key, _)
        | Command::ZIncrBy(key, _, _)
        | Command::RlSet(key, _, _)
        | Command::RlCheck(key, _)
        | Command::JSet(key, _, _)
        | Command::JMerge(key, _) => one(key),
        Command::Del(keys) | Command::Unlink(keys) => MutationScope::Keys(keys.clone()),
        Command::MSet(pairs) => {
            MutationScope::Keys(pairs.iter().map(|(key, _)| key.clone()).collect())
        }
        Command::Rename(source, destination) | Command::SMove(source, destination, _) => {
            MutationScope::Keys(vec![source.clone(), destination.clone()])
        }
        Command::SInterStore(destination, sources)
        | Command::SUnionStore(destination, sources)
        | Command::SDiffStore(destination, sources) => {
            let mut keys = Vec::with_capacity(sources.len() + 1);
            keys.push(destination.clone());
            keys.extend(sources.iter().cloned());
            MutationScope::Keys(keys)
        }
        Command::FlushDb => MutationScope::All,
        _ => MutationScope::ReadOnly,
    }
}

/// Fixed byte charge for a command that grows a value without carrying the new
/// bytes in its arguments — a counter bumped by `INCR`, a rate-limiter bucket.
#[cfg(test)]
const NOMINAL_WRITE_BYTES: usize = 24;

/// Upper bound on the bytes `cmd` could add to the keyspace, used by
/// `note_write` to decide when an exact measurement is worth paying for.
///
/// Deliberately exhaustive rather than `_ => 0`: a new write command that
/// escapes accounting would silently reopen the hole this closes, and the
/// compiler is the only reliable place to catch that.
///
/// Reads are 0. Overestimating costs one extra keyspace walk; underestimating
/// only delays enforcement to the next background sweep, which is where every
/// command sat before.
#[cfg(test)]
fn write_cost(cmd: &Command) -> usize {
    // Per-entry overhead is charged so that many tiny keys still register.
    const OVERHEAD: usize = 64;
    match cmd {
        // ── Writes carrying their payload inline ──────────────────────────
        Command::Set(k, v, _) | Command::ESet(k, v) | Command::GetSet(k, v) => {
            k.len() + v.len() + OVERHEAD
        }
        Command::SetNx(k, v) | Command::Append(k, v) => k.len() + v.len() + OVERHEAD,
        Command::SetEx(k, _, v) | Command::PSetEx(k, _, v) => k.len() + v.len() + OVERHEAD,
        Command::MSet(pairs) => pairs
            .iter()
            .map(|(k, v)| k.len() + v.len() + OVERHEAD)
            .sum(),
        Command::HSet(k, pairs) => {
            k.len() + OVERHEAD + pairs.iter().map(|(f, v)| f.len() + v.len()).sum::<usize>()
        }
        Command::HSetNx(k, f, v) => k.len() + f.len() + v.len() + OVERHEAD,
        Command::LPush(k, vals)
        | Command::RPush(k, vals)
        | Command::LPushX(k, vals)
        | Command::RPushX(k, vals) => {
            k.len() + OVERHEAD + vals.iter().map(Vec::len).sum::<usize>()
        }
        Command::LSet(k, _, v) => k.len() + v.len(),
        Command::SAdd(k, members) => {
            k.len() + OVERHEAD + members.iter().map(String::len).sum::<usize>()
        }
        Command::ZAdd(k, _, pairs) => {
            k.len()
                + OVERHEAD
                + pairs
                    .iter()
                    .map(|(_, m)| m.len() + size_of::<f64>())
                    .sum::<usize>()
        }
        Command::ZIncrBy(k, _, m) => k.len() + m.len() + size_of::<f64>() + OVERHEAD,
        Command::SMove(_, dst, m) => dst.len() + m.len() + OVERHEAD,
        Command::JSet(k, path, value) => k.len() + path.len() + value.len() + OVERHEAD,
        Command::JMerge(k, patch) => k.len() + patch.len() + OVERHEAD,

        // ── Writes that grow a value by a bounded amount ──────────────────
        Command::Incr(k) | Command::Decr(k) | Command::IncrBy(k, _) | Command::DecrBy(k, _) => {
            k.len() + NOMINAL_WRITE_BYTES + OVERHEAD
        }
        Command::HIncrBy(k, f, _) | Command::HIncrByFloat(k, f, _) => {
            k.len() + f.len() + NOMINAL_WRITE_BYTES + OVERHEAD
        }
        // A limiter is created on first use and its buckets grow with traffic.
        Command::RlSet(k, _, _) | Command::RlCheck(k, _) => {
            k.len() + NOMINAL_WRITE_BYTES + OVERHEAD
        }
        // Result size is not knowable without doing the work; charge the key so
        // the destination at least registers, and let the sweep catch the rest.
        Command::SInterStore(dst, _) | Command::SUnionStore(dst, _) | Command::SDiffStore(dst, _) => {
            dst.len() + OVERHEAD
        }
        // Moves the value rather than adding one; the new key is the only growth.
        Command::Rename(_, dst) => dst.len(),

        // ── Everything else cannot grow the keyspace ──────────────────────
        // Deletions, expiry changes, pops and trims only shrink it; reads and
        // connection-level commands do not touch it at all. Listed rather than
        // collapsed into `_` so a new variant has to be classified.
        Command::Ping(_)
        | Command::Auth(_)
        | Command::Hello(_)
        | Command::Info(_)
        | Command::Quit
        | Command::Client(_)
        | Command::Config(_)
        | Command::CommandQuery(_)
        | Command::Get(_)
        | Command::Del(_)
        | Command::Unlink(_)
        | Command::Strlen(_)
        | Command::GetRange(_, _, _)
        | Command::MGet(_)
        | Command::Expire(_, _)
        | Command::PExpire(_, _)
        | Command::ExpireAt(_, _)
        | Command::PExpireAt(_, _)
        | Command::Ttl(_)
        | Command::PTtl(_)
        | Command::Persist(_)
        | Command::Exists(_)
        | Command::Keys(_)
        | Command::Scan(_, _, _)
        | Command::DbSize
        | Command::FlushDb
        | Command::Type(_)
        | Command::HGet(_, _)
        | Command::HGetAll(_)
        | Command::HDel(_, _)
        | Command::HKeys(_)
        | Command::HVals(_)
        | Command::HLen(_)
        | Command::HExists(_, _)
        | Command::HMGet(_, _)
        | Command::HScan(_, _)
        | Command::LPop(_, _)
        | Command::RPop(_, _)
        | Command::LRange(_, _, _)
        | Command::LLen(_)
        | Command::LIndex(_, _)
        | Command::LRem(_, _, _)
        | Command::LTrim(_, _, _)
        | Command::SMembers(_)
        | Command::SRem(_, _)
        | Command::SCard(_)
        | Command::SIsMember(_, _)
        | Command::SMIsMember(_, _)
        | Command::SInter(_)
        | Command::SUnion(_)
        | Command::SDiff(_)
        | Command::SPop(_, _)
        | Command::SRandMember(_, _)
        | Command::SScan(_, _)
        | Command::ZRange(_, _, _, _)
        | Command::ZRevRange(_, _, _, _)
        | Command::ZRangeByScore(_, _, _, _, _)
        | Command::ZRevRangeByScore(_, _, _, _, _)
        | Command::ZScore(_, _)
        | Command::ZMScore(_, _)
        | Command::ZRank(_, _)
        | Command::ZRevRank(_, _)
        | Command::ZRem(_, _)
        | Command::ZCard(_)
        | Command::ZCount(_, _, _)
        | Command::ZScan(_, _)
        | Command::JGet(_, _)
        // Transactions, pub/sub, live queries, sync scoping and replication are
        // resolved in the server layer; the store either never sees them or
        // treats them as no-ops. `Dedup` is unwrapped before it gets here, so
        // charging its inner command would double-count.
        | Command::Multi
        | Command::Exec
        | Command::Discard
        | Command::Subscribe(_)
        | Command::Unsubscribe(_)
        | Command::PSubscribe(_)
        | Command::PUnsubscribe(_)
        | Command::Publish(_, _)
        | Command::Watch(_)
        | Command::Unwatch(_)
        | Command::Sync(_)
        | Command::Dedup(_, _, _)
        | Command::QSub(_)
        | Command::QUnsub(_)
        | Command::Cluster(_)
        | Command::Module(_)
        | Command::PubSub(_)
        | Command::Memory(_)
        | Command::MemoryUsage(_)
        | Command::Save
        | Command::BgSave
        | Command::LastSave
        | Command::ReplicaOfNoOne
        | Command::Unknown(_) => 0,
    }
}

/// Approximate heap footprint of a single entry: key + value bytes plus a fixed
/// per-entry overhead. Shared by `approximate_memory_bytes` and the eviction
/// loop so both agree on what a key "costs".
/// Logical byte cost of one entry.
///
/// Every collection answers in O(1) from a total it maintains as it is
/// mutated. It used to walk the value instead, and because the write path
/// calls this before *and* after each command, that made building an
/// N-element collection O(N^2) — 2,838 `HSET`/s into a 100k-field hash where
/// `SET` managed 349,650/s, halving again with every doubling.
fn entry_size(key: &str, e: &Entry) -> usize {
    let val_size = match &e.value {
        EntryValue::Str(s) => s.len(),
        EntryValue::Hash(m) => m.heap_bytes(),
        EntryValue::List(l) => l.heap_bytes(),
        EntryValue::Set(s) => s.heap_bytes(),
        EntryValue::ZSet(z) => z.heap_bytes(),
        EntryValue::RateLimiter(rl) => rl.buckets.len() * 16 + 16,
        EntryValue::Json(doc) => json_approx_size(doc),
    };
    key.len() + val_size + 64
}

fn incr_by(data: &DashMap<String, Entry>, key: String, delta: i64) -> Value {
    let now = now_ms();
    typed_entry!(entry, s, data, key, now, EntryValue::Str, Blob::from("0"));
    // A non-UTF-8 value fails here exactly as non-numeric text does: the bytes
    // are stored faithfully, they are simply not a number.
    match s.parse_as::<i64>() {
        None => Value::Error("ERR value is not an integer or out of range".to_string()),
        Some(n) => match n.checked_add(delta) {
            None => Value::Error("ERR increment or decrement would overflow".to_string()),
            Some(new) => {
                *s = new.to_string().into();
                Value::Integer(new)
            }
        },
    }
}

fn set_expiry(data: &DashMap<String, Entry>, key: String, ts_ms: u64) -> Value {
    let now = now_ms();
    match data.get_mut(&key) {
        None => Value::Integer(0),
        Some(e) if e.is_expired(now) => Value::Integer(0),
        Some(mut e) => {
            e.expires_at_ms = Some(ts_ms);
            Value::Integer(1)
        }
    }
}

/// Longest glob pattern accepted from a client, in bytes.
///
/// Matching is O(pattern × text) in the worst case and runs once per key for
/// `KEYS`/`SCAN MATCH`, once per key per write for sync-scope filtering, and
/// once per published message per subscriber for `PSUBSCRIBE`. Pattern length
/// was previously bounded only by `proto-max-bulk-len` (64 MB), so a single
/// `KEYS <very long pattern>` against a large keyspace could occupy the process
/// for an unbounded time. No legitimate pattern is anywhere near this cap.
pub const MAX_PATTERN_BYTES: usize = 1024;

/// Glob match with `*` and `?` against raw bytes.
///
/// Two-pointer greedy matching with a single backtrack point: worst case
/// O(pattern × text) time, and **no allocation at all**.
///
/// This is the third implementation. The first was recursive and backtracked
/// exponentially on patterns like `*a*a*a*b`; the second fixed the time bound
/// with an iterative DP but allocated two `Vec<bool>` of `text.len() + 1` on
/// *every call*, so a single 64 MB value made `KEYS *` allocate 128 MB — on a
/// path that runs once per key. The greedy form keeps the DP's time bound and
/// needs no memory, which is why it replaced it.
///
/// Supports `*` (any run of bytes, including empty) and `?` (exactly one byte).
/// **Character classes like `[abc]` are not supported** — brackets match
/// literally, so `[ab]` matches the four-byte string `[ab]` and not `a`. An
/// earlier version of this comment claimed class support; it was never
/// implemented. Matching is byte-wise, so `?` matches one *byte* and a
/// multi-byte UTF-8 character spans several positions.
pub fn glob_match(pattern: &str, s: &str) -> bool {
    let pat = pattern.as_bytes();
    let text = s.as_bytes();
    let (m, n) = (pat.len(), text.len());

    let mut i = 0usize; // position in text
    let mut j = 0usize; // position in pattern
    // The most recent `*` and the text position it was first allowed to stop at.
    // Only the latest `*` needs remembering: any backtracking an earlier one
    // could do, the latest one can do too.
    let mut star: Option<usize> = None;
    let mut resume = 0usize;

    while i < n {
        if j < m && (pat[j] == b'?' || pat[j] == text[i]) {
            i += 1;
            j += 1;
        } else if j < m && pat[j] == b'*' {
            star = Some(j);
            j += 1;
            resume = i;
        } else if let Some(star_at) = star {
            // The last `*` consumed too little — give it one more byte and
            // retry. `resume` only ever advances, which is what bounds this.
            j = star_at + 1;
            resume += 1;
            i = resume;
        } else {
            return false;
        }
    }

    // Any `*` left over matches the empty remainder; anything else does not.
    while j < m && pat[j] == b'*' {
        j += 1;
    }
    j == m
}

fn no_list_response(count: Option<u64>) -> Value {
    if count.is_some() {
        Value::Array(Some(vec![]))
    } else {
        Value::BulkString(None)
    }
}

fn set_to_value(mut result: IndexSet<String>) -> Value {
    let mut members: Vec<String> = result.drain(..).collect();
    members.sort_unstable();
    Value::Array(Some(
        members
            .into_iter()
            .map(|m| Value::BulkString(Some(m.into_bytes())))
            .collect(),
    ))
}

fn set_inter(
    data: &DashMap<String, Entry>,
    keys: &[String],
    now: u64,
) -> Result<IndexSet<String>, Value> {
    if keys.is_empty() {
        return Ok(IndexSet::new());
    }
    let mut sets: Vec<Option<IndexSet<String>>> = Vec::with_capacity(keys.len());
    for k in keys {
        let cloned = {
            let entry = data.get(k);
            match entry {
                None => None,
                Some(e) if e.is_expired(now) => None,
                Some(e) => match &e.value {
                    EntryValue::Set(s) => Some(s.iter().cloned().collect::<IndexSet<String>>()),
                    _ => return Err(Value::Error(WRONGTYPE.to_string())),
                },
            }
        };
        sets.push(cloned);
    }
    if sets.iter().any(|s| s.is_none()) {
        return Ok(IndexSet::new());
    }
    let non_empty: Vec<IndexSet<String>> = sets.into_iter().flatten().collect();
    let mut result: IndexSet<String> = non_empty[0].iter().cloned().collect();
    for s in &non_empty[1..] {
        result.retain(|m| s.contains(m));
    }
    Ok(result)
}

fn set_union(
    data: &DashMap<String, Entry>,
    keys: &[String],
    now: u64,
) -> Result<IndexSet<String>, Value> {
    let mut result: IndexSet<String> = IndexSet::new();
    for k in keys {
        let s_clone = {
            let entry = data.get(k);
            match entry {
                None => None,
                Some(e) if e.is_expired(now) => None,
                Some(e) => match &e.value {
                    EntryValue::Set(s) => Some(s.iter().cloned().collect::<IndexSet<String>>()),
                    _ => return Err(Value::Error(WRONGTYPE.to_string())),
                },
            }
        };
        if let Some(s) = s_clone {
            result.extend(s);
        }
    }
    Ok(result)
}

fn set_diff(
    data: &DashMap<String, Entry>,
    keys: &[String],
    now: u64,
) -> Result<IndexSet<String>, Value> {
    if keys.is_empty() {
        return Ok(IndexSet::new());
    }
    let mut result: IndexSet<String> = match data.get(&keys[0]) {
        None => IndexSet::new(),
        Some(e) if e.is_expired(now) => IndexSet::new(),
        Some(e) => match &e.value {
            EntryValue::Set(s) => s.iter().cloned().collect(),
            _ => return Err(Value::Error(WRONGTYPE.to_string())),
        },
    };
    for k in &keys[1..] {
        let s_clone = {
            let entry = data.get(k);
            match entry {
                None => None,
                Some(e) if e.is_expired(now) => None,
                Some(e) => match &e.value {
                    EntryValue::Set(s) => Some(s.iter().cloned().collect::<IndexSet<String>>()),
                    _ => return Err(Value::Error(WRONGTYPE.to_string())),
                },
            }
        };
        if let Some(s) = s_clone {
            result.retain(|m| !s.contains(m));
        }
    }
    Ok(result)
}

fn hash_incr_int(data: &DashMap<String, Entry>, key: String, field: String, delta: i64) -> Value {
    let now = now_ms();
    typed_entry!(
        entry,
        h,
        data,
        key,
        now,
        EntryValue::Hash,
        Box::new(CompactHash::new())
    );
    let cur: i64 = h.get(&field).and_then(|s| s.parse_as()).unwrap_or(0);
    match cur.checked_add(delta) {
        None => Value::Error("ERR increment or decrement would overflow".to_string()),
        Some(new) => {
            h.insert(field, new.to_string().into());
            Value::Integer(new)
        }
    }
}

fn hash_incr_float(data: &DashMap<String, Entry>, key: String, field: String, delta: f64) -> Value {
    let now = now_ms();
    typed_entry!(
        entry,
        h,
        data,
        key,
        now,
        EntryValue::Hash,
        Box::new(CompactHash::new())
    );
    let cur: f64 = h.get(&field).and_then(|s| s.parse_as()).unwrap_or(0.0);
    let new = cur + delta;
    if new.is_nan() || new.is_infinite() {
        return Value::Error("ERR increment would produce NaN or Infinity".to_string());
    }
    let new_str = format_score(new);
    h.insert(field, new_str.clone().into());
    Value::BulkString(Some(new_str.into_bytes()))
}

/// Resolve `GETRANGE`'s inclusive bounds against a value of `data.len()` bytes.
///
/// Redis clamps the two ends asymmetrically and the difference matters: `end`
/// is pulled back to the last byte, but `start` is left alone, so a `start`
/// past the end of the value inverts the range and yields nothing instead of
/// the final byte.
fn byte_range(data: &[u8], start: i64, end: i64) -> &[u8] {
    let len = data.len() as i64;
    if len == 0 {
        return &[];
    }
    let mut s = if start < 0 {
        start.saturating_add(len)
    } else {
        start
    };
    let mut e = if end < 0 {
        end.saturating_add(len)
    } else {
        end
    };
    if s < 0 {
        s = 0;
    }
    if e < 0 {
        e = 0;
    }
    if e >= len {
        e = len - 1;
    }
    if s > e {
        return &[];
    }
    // `s <= e < len` here, so both casts are in range.
    &data[s as usize..=e as usize]
}

/// Take one page out of an ordered element list for the collection scans.
///
/// The cursor is an offset rather than Redis's reverse-binary bucket index:
/// there is no incremental rehashing to survive here, and keyspace `SCAN`
/// already established the convention. The ordering must be stable across
/// calls for the offset to mean anything, which is why every caller sorts by
/// field or member name. A cursor past the end returns an empty final page
/// rather than erroring, matching what Redis does with a stale cursor.
fn scan_page<T>(items: &[T], cursor: u64, count: Option<usize>) -> (&[T], u64) {
    let batch = count.unwrap_or(10).max(1);
    let start = (cursor as usize).min(items.len());
    let end = start.saturating_add(batch).min(items.len());
    let next = if end >= items.len() { 0 } else { end as u64 };
    (&items[start..end], next)
}

/// `[cursor, [elements…]]` — the two-element reply every SCAN family member
/// returns. The cursor is a bulk string, as in Redis, because it may exceed
/// what a RESP integer promises.
fn scan_reply(next: u64, items: Vec<Value>) -> Value {
    Value::Array(Some(vec![
        Value::BulkString(Some(next.to_string().into_bytes())),
        Value::Array(Some(items)),
    ]))
}

/// The reply for scanning a key that does not exist: a completed iteration
/// over nothing, which is also what Redis answers.
fn empty_scan() -> Value {
    scan_reply(0, vec![])
}

fn zset_read<F>(data: &DashMap<String, Entry>, key: &str, f: F) -> Value
where
    F: FnOnce(&ZSetInner) -> Result<Value, Value>,
{
    let now = now_ms();
    let empty = ZSetInner::new();
    let result = match data.get(key) {
        None => f(&empty),
        Some(e) if e.is_expired(now) => f(&empty),
        Some(e) => match &e.value {
            EntryValue::ZSet(z) => {
                e.touch(now);
                f(z)
            }
            _ => return Value::Error(WRONGTYPE.to_string()),
        },
    };
    match result {
        Ok(v) | Err(v) => v,
    }
}

/// Whether a GT/LT-qualified `ZADD` should overwrite `old_score` with `score`.
/// With neither flag the caller decides; this only answers the ordered cases.
fn score_should_update(score: f64, old_score: f64, opts: &ZAddOptions) -> bool {
    if opts.gt {
        score > old_score
    } else if opts.lt {
        score < old_score
    } else {
        (score - old_score).abs() > f64::EPSILON
    }
}

fn zadd_exec(zset: &mut ZSetInner, opts: ZAddOptions, pairs: Vec<(f64, String)>) -> Value {
    use crate::cmd::ZAddCondition;

    if opts.incr {
        let (delta, member) = match pairs.into_iter().next() {
            Some(p) => p,
            None => return Value::BulkString(None),
        };
        let score = zset.score(&member).unwrap_or(0.0) + delta;
        zset.insert(&member, score);
        return Value::BulkString(Some(format_score(score).into_bytes()));
    }

    let mut added = 0i64;
    let mut changed = 0i64;
    for (score, member) in pairs {
        // Plain `ZADD key score member` — no condition, no GT/LT — is both the
        // overwhelmingly common case and the one the benchmarks hammer, so it
        // gets a path that touches the map once. `insert` already reports the
        // previous score, which is all this needs to classify the write; asking
        // for it separately first, as the general path below does, doubles the
        // hash lookups on the hottest sorted-set command there is.
        if opts.condition.is_none() && !opts.gt && !opts.lt {
            match zset.insert(&member, score) {
                None => {
                    added += 1;
                    changed += 1;
                }
                Some(old_score) if (old_score - score).abs() > f64::EPSILON => changed += 1,
                Some(_) => {}
            }
            continue;
        }

        // `insert` keeps the score index in step, so each branch decides
        // whether to write and then writes through the one entry point.
        match (&opts.condition, zset.score(&member)) {
            // NX: only members that are not there yet.
            (Some(ZAddCondition::Nx), Some(_)) => {}
            (Some(ZAddCondition::Nx), None) => {
                zset.insert(&member, score);
                added += 1;
                changed += 1;
            }
            // XX: only members that already exist.
            (Some(ZAddCondition::Xx), None) => {}
            (Some(ZAddCondition::Xx), Some(old_score)) => {
                if score_should_update(score, old_score, &opts) {
                    zset.insert(&member, score);
                    changed += 1;
                }
            }
            (None, None) => {
                zset.insert(&member, score);
                added += 1;
                changed += 1;
            }
            (None, Some(old_score)) => {
                let update = if opts.gt || opts.lt {
                    score_should_update(score, old_score, &opts)
                } else {
                    (old_score - score).abs() > f64::EPSILON
                };
                if update {
                    zset.insert(&member, score);
                    changed += 1;
                } else if !opts.gt && !opts.lt {
                    // An unconditional ZADD still writes an equal score; only
                    // the `changed` count treats it as a no-op.
                    zset.insert(&member, score);
                }
            }
        }
    }
    Value::Integer(if opts.ch { changed } else { added })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Counts that a caller can ask for but the collection cannot satisfy.
///
/// These are execution-layer guards, deliberately duplicated with the parser's
/// sign check: a `Command` also reaches `execute` from AOF replay, a
/// replication frame and `sync-client`, none of which parse RESP.
#[cfg(test)]
mod oversized_count_tests {
    use super::*;
    use crate::cmd::Command;

    fn list_of(n: usize) -> KeyValueStore {
        let s = KeyValueStore::new();
        s.execute(Command::RPush(
            "l".into(),
            (0..n).map(|i| format!("v{i}").into_bytes()).collect(),
        ));
        s
    }

    #[test]
    fn lpop_asking_for_more_than_exists_returns_the_list_and_stops() {
        let s = list_of(3);
        // The bound that matters is the list's, not the count's: iterating the
        // shortfall would hold the shard guard for ~240 years at i64::MAX.
        let reply = s.execute(Command::LPop("l".into(), Some(i64::MAX as u64)));
        assert_eq!(
            reply,
            Value::Array(Some(vec![
                Value::BulkString(Some(b"v0".to_vec())),
                Value::BulkString(Some(b"v1".to_vec())),
                Value::BulkString(Some(b"v2".to_vec())),
            ]))
        );
        assert_eq!(s.execute(Command::LLen("l".into())), Value::Integer(0));
    }

    #[test]
    fn rpop_asking_for_more_than_exists_returns_the_list_in_pop_order() {
        let s = list_of(3);
        let reply = s.execute(Command::RPop("l".into(), Some(u64::MAX)));
        assert_eq!(
            reply,
            Value::Array(Some(vec![
                Value::BulkString(Some(b"v2".to_vec())),
                Value::BulkString(Some(b"v1".to_vec())),
                Value::BulkString(Some(b"v0".to_vec())),
            ]))
        );
    }

    #[test]
    fn a_partial_rpop_still_pops_from_the_tail() {
        let s = list_of(4);
        assert_eq!(
            s.execute(Command::RPop("l".into(), Some(2))),
            Value::Array(Some(vec![
                Value::BulkString(Some(b"v3".to_vec())),
                Value::BulkString(Some(b"v2".to_vec())),
            ]))
        );
        assert_eq!(s.execute(Command::LLen("l".into())), Value::Integer(2));
    }

    #[test]
    fn a_partial_lpop_still_pops_from_the_head() {
        let s = list_of(4);
        assert_eq!(
            s.execute(Command::LPop("l".into(), Some(2))),
            Value::Array(Some(vec![
                Value::BulkString(Some(b"v0".to_vec())),
                Value::BulkString(Some(b"v1".to_vec())),
            ]))
        );
        assert_eq!(s.execute(Command::LLen("l".into())), Value::Integer(2));
    }

    #[test]
    fn popping_zero_takes_nothing() {
        let s = list_of(2);
        assert_eq!(
            s.execute(Command::LPop("l".into(), Some(0))),
            Value::Array(Some(vec![]))
        );
        assert_eq!(
            s.execute(Command::RPop("l".into(), Some(0))),
            Value::Array(Some(vec![]))
        );
        assert_eq!(s.execute(Command::LLen("l".into())), Value::Integer(2));
    }

    #[test]
    fn spop_beyond_the_set_size_empties_it_exactly_once() {
        let s = KeyValueStore::new();
        s.execute(Command::SAdd(
            "s".into(),
            (0..100).map(|i| format!("m{i}")).collect(),
        ));
        let reply = s.execute(Command::SPop("s".into(), Some(u64::MAX)));
        match reply {
            Value::Array(Some(v)) => assert_eq!(v.len(), 100),
            other => panic!("expected an array, got {other:?}"),
        }
        assert_eq!(s.execute(Command::SCard("s".into())), Value::Integer(0));
    }

    #[test]
    fn srandmember_with_repetition_is_capped_at_what_a_reply_can_carry() {
        let s = KeyValueStore::new();
        s.execute(Command::SAdd("s".into(), vec!["only".into()]));
        // The parser refuses this magnitude; reaching `execute` anyway (replay,
        // replication) must clamp rather than allocate i64::MAX elements.
        let reply = s.execute(Command::SRandMember("s".into(), Some(i64::MIN)));
        match reply {
            Value::Array(Some(v)) => assert_eq!(v.len(), crate::resp::MAX_ARRAY_ELEMENTS),
            other => panic!("expected an array, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::{Command, ScanArgs, SetOptions, ZAddOptions};

    fn store() -> KeyValueStore {
        KeyValueStore::new()
    }

    fn bulk(s: &str) -> Value {
        Value::BulkString(Some(s.as_bytes().to_vec()))
    }

    fn int(n: i64) -> Value {
        Value::Integer(n)
    }

    fn ok() -> Value {
        Value::SimpleString("OK".to_string())
    }

    fn nil() -> Value {
        Value::BulkString(None)
    }

    fn arr(items: &[&str]) -> Value {
        Value::Array(Some(items.iter().map(|s| bulk(s)).collect()))
    }

    // ── Hash ──────────────────────────────────────────────────────────────────

    #[test]
    fn hash_basic() {
        let s = store();
        assert_eq!(
            s.execute(Command::HSet(
                "h".into(),
                vec![("f1".into(), "v1".into()), ("f2".into(), "v2".into())]
            )),
            int(2)
        );
        assert_eq!(
            s.execute(Command::HGet("h".into(), "f1".into())),
            bulk("v1")
        );
        assert_eq!(
            s.execute(Command::HGet("h".into(), "f2".into())),
            bulk("v2")
        );
        assert_eq!(s.execute(Command::HGet("h".into(), "nope".into())), nil());
        assert_eq!(s.execute(Command::HLen("h".into())), int(2));
    }

    #[test]
    fn hash_getall_sorted() {
        let s = store();
        s.execute(Command::HSet(
            "h".into(),
            vec![("b".into(), "2".into()), ("a".into(), "1".into())],
        ));
        // HGETALL returns field-value pairs sorted by field
        let res = s.execute(Command::HGetAll("h".into()));
        assert_eq!(res, arr(&["a", "1", "b", "2"]));
    }

    #[test]
    fn hash_del() {
        let s = store();
        s.execute(Command::HSet("h".into(), vec![("f".into(), "v".into())]));
        assert_eq!(
            s.execute(Command::HDel("h".into(), vec!["f".into()])),
            int(1)
        );
        assert_eq!(
            s.execute(Command::HDel("h".into(), vec!["f".into()])),
            int(0)
        );
        assert_eq!(s.execute(Command::HGet("h".into(), "f".into())), nil());
    }

    #[test]
    fn hash_incr() {
        let s = store();
        assert_eq!(
            s.execute(Command::HIncrBy("h".into(), "n".into(), 5)),
            int(5)
        );
        assert_eq!(
            s.execute(Command::HIncrBy("h".into(), "n".into(), 3)),
            int(8)
        );
        let res = s.execute(Command::HIncrByFloat("h".into(), "f".into(), 1.5));
        assert_eq!(res, bulk("1.5"));
    }

    #[test]
    fn hash_hsetnx() {
        let s = store();
        assert_eq!(
            s.execute(Command::HSetNx("h".into(), "f".into(), "v1".into())),
            int(1)
        );
        assert_eq!(
            s.execute(Command::HSetNx("h".into(), "f".into(), "v2".into())),
            int(0)
        );
        assert_eq!(s.execute(Command::HGet("h".into(), "f".into())), bulk("v1"));
    }

    #[test]
    fn hash_hmget() {
        let s = store();
        s.execute(Command::HSet("h".into(), vec![("a".into(), "1".into())]));
        let res = s.execute(Command::HMGet("h".into(), vec!["a".into(), "b".into()]));
        assert_eq!(res, Value::Array(Some(vec![bulk("1"), nil()])));
    }

    #[test]
    fn hash_wrongtype() {
        let s = store();
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        let res = s.execute(Command::HGet("k".into(), "f".into()));
        assert!(matches!(res, Value::Error(e) if e.contains("WRONGTYPE")));
    }

    // ── List ─────────────────────────────────────────────────────────────────

    #[test]
    fn list_push_pop() {
        let s = store();
        assert_eq!(
            s.execute(Command::RPush("l".into(), vec!["a".into(), "b".into()])),
            int(2)
        );
        assert_eq!(
            s.execute(Command::LPush("l".into(), vec!["z".into()])),
            int(3)
        );
        // list is now: z a b
        assert_eq!(s.execute(Command::LPop("l".into(), None)), bulk("z"));
        assert_eq!(s.execute(Command::RPop("l".into(), None)), bulk("b"));
        assert_eq!(s.execute(Command::LLen("l".into())), int(1));
    }

    #[test]
    fn list_lrange() {
        let s = store();
        s.execute(Command::RPush(
            "l".into(),
            vec!["a".into(), "b".into(), "c".into()],
        ));
        assert_eq!(
            s.execute(Command::LRange("l".into(), 0, -1)),
            arr(&["a", "b", "c"])
        );
        assert_eq!(
            s.execute(Command::LRange("l".into(), 1, 2)),
            arr(&["b", "c"])
        );
        assert_eq!(s.execute(Command::LRange("l".into(), 0, 0)), arr(&["a"]));
    }

    #[test]
    fn list_lindex_lset() {
        let s = store();
        s.execute(Command::RPush("l".into(), vec!["a".into(), "b".into()]));
        assert_eq!(s.execute(Command::LIndex("l".into(), 0)), bulk("a"));
        assert_eq!(s.execute(Command::LIndex("l".into(), -1)), bulk("b"));
        assert_eq!(s.execute(Command::LSet("l".into(), 0, "x".into())), ok());
        assert_eq!(s.execute(Command::LIndex("l".into(), 0)), bulk("x"));
    }

    #[test]
    fn list_lrem() {
        let s = store();
        s.execute(Command::RPush(
            "l".into(),
            vec!["a".into(), "b".into(), "a".into(), "c".into()],
        ));
        assert_eq!(s.execute(Command::LRem("l".into(), 1, "a".into())), int(1));
        assert_eq!(
            s.execute(Command::LRange("l".into(), 0, -1)),
            arr(&["b", "a", "c"])
        );
    }

    #[test]
    fn list_ltrim() {
        let s = store();
        s.execute(Command::RPush(
            "l".into(),
            vec!["a".into(), "b".into(), "c".into()],
        ));
        s.execute(Command::LTrim("l".into(), 1, 2));
        assert_eq!(
            s.execute(Command::LRange("l".into(), 0, -1)),
            arr(&["b", "c"])
        );
    }

    #[test]
    fn list_wrongtype() {
        let s = store();
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        let res = s.execute(Command::LPush("k".into(), vec!["x".into()]));
        assert!(matches!(res, Value::Error(e) if e.contains("WRONGTYPE")));
    }

    // ── Set ───────────────────────────────────────────────────────────────────

    #[test]
    fn set_basic() {
        let s = store();
        assert_eq!(
            s.execute(Command::SAdd("s".into(), vec!["a".into(), "b".into()])),
            int(2)
        );
        assert_eq!(
            s.execute(Command::SAdd("s".into(), vec!["a".into()])),
            int(0)
        );
        assert_eq!(s.execute(Command::SCard("s".into())), int(2));
        assert_eq!(
            s.execute(Command::SIsMember("s".into(), "a".into())),
            int(1)
        );
        assert_eq!(
            s.execute(Command::SIsMember("s".into(), "z".into())),
            int(0)
        );
    }

    #[test]
    fn set_smembers_sorted() {
        let s = store();
        s.execute(Command::SAdd(
            "s".into(),
            vec!["c".into(), "a".into(), "b".into()],
        ));
        assert_eq!(
            s.execute(Command::SMembers("s".into())),
            arr(&["a", "b", "c"])
        );
    }

    #[test]
    fn set_rem() {
        let s = store();
        s.execute(Command::SAdd("s".into(), vec!["a".into(), "b".into()]));
        assert_eq!(
            s.execute(Command::SRem("s".into(), vec!["a".into()])),
            int(1)
        );
        assert_eq!(s.execute(Command::SCard("s".into())), int(1));
    }

    #[test]
    fn set_inter_union_diff() {
        let s = store();
        s.execute(Command::SAdd(
            "a".into(),
            vec!["1".into(), "2".into(), "3".into()],
        ));
        s.execute(Command::SAdd(
            "b".into(),
            vec!["2".into(), "3".into(), "4".into()],
        ));

        let inter = s.execute(Command::SInter(vec!["a".into(), "b".into()]));
        assert_eq!(inter, arr(&["2", "3"]));

        let union = s.execute(Command::SUnion(vec!["a".into(), "b".into()]));
        assert_eq!(union, arr(&["1", "2", "3", "4"]));

        let diff = s.execute(Command::SDiff(vec!["a".into(), "b".into()]));
        assert_eq!(diff, arr(&["1"]));
    }

    #[test]
    fn set_smove() {
        let s = store();
        s.execute(Command::SAdd("src".into(), vec!["m".into()]));
        assert_eq!(
            s.execute(Command::SMove("src".into(), "dst".into(), "m".into())),
            int(1)
        );
        assert_eq!(
            s.execute(Command::SIsMember("src".into(), "m".into())),
            int(0)
        );
        assert_eq!(
            s.execute(Command::SIsMember("dst".into(), "m".into())),
            int(1)
        );
    }

    #[test]
    fn set_wrongtype() {
        let s = store();
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        let res = s.execute(Command::SAdd("k".into(), vec!["x".into()]));
        assert!(matches!(res, Value::Error(e) if e.contains("WRONGTYPE")));
    }

    // ── Sorted Set ────────────────────────────────────────────────────────────

    #[test]
    fn zset_zadd_zrange() {
        let s = store();
        assert_eq!(
            s.execute(Command::ZAdd(
                "z".into(),
                ZAddOptions::default(),
                vec![(1.0, "a".into()), (2.0, "b".into())]
            )),
            int(2)
        );
        assert_eq!(
            s.execute(Command::ZRange("z".into(), 0, -1, false)),
            arr(&["a", "b"])
        );
        assert_eq!(
            s.execute(Command::ZRevRange("z".into(), 0, -1, false)),
            arr(&["b", "a"])
        );
    }

    #[test]
    fn zset_withscores() {
        let s = store();
        s.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions::default(),
            vec![(1.0, "a".into())],
        ));
        let res = s.execute(Command::ZRange("z".into(), 0, -1, true));
        assert_eq!(res, arr(&["a", "1"]));
    }

    #[test]
    fn zset_zscore_zrank() {
        let s = store();
        s.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions::default(),
            vec![(5.0, "a".into()), (3.0, "b".into())],
        ));
        assert_eq!(
            s.execute(Command::ZScore("z".into(), "a".into())),
            bulk("5")
        );
        assert_eq!(s.execute(Command::ZRank("z".into(), "b".into())), int(0));
        assert_eq!(s.execute(Command::ZRevRank("z".into(), "b".into())), int(1));
    }

    #[test]
    fn zset_zincrby() {
        let s = store();
        s.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions::default(),
            vec![(1.0, "m".into())],
        ));
        assert_eq!(
            s.execute(Command::ZIncrBy("z".into(), 2.5, "m".into())),
            bulk("3.5")
        );
        assert_eq!(
            s.execute(Command::ZScore("z".into(), "m".into())),
            bulk("3.5")
        );
        // New member
        assert_eq!(
            s.execute(Command::ZIncrBy("z".into(), 10.0, "new".into())),
            bulk("10")
        );
    }

    #[test]
    fn zset_zrem_zcard() {
        let s = store();
        s.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions::default(),
            vec![(1.0, "a".into()), (2.0, "b".into())],
        ));
        assert_eq!(
            s.execute(Command::ZRem("z".into(), vec!["a".into()])),
            int(1)
        );
        assert_eq!(s.execute(Command::ZCard("z".into())), int(1));
    }

    #[test]
    fn zset_zrangebyscore() {
        let s = store();
        s.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions::default(),
            vec![(1.0, "a".into()), (2.0, "b".into()), (3.0, "c".into())],
        ));
        assert_eq!(
            s.execute(Command::ZRangeByScore(
                "z".into(),
                "1".into(),
                "2".into(),
                false,
                None
            )),
            arr(&["a", "b"])
        );
        assert_eq!(
            s.execute(Command::ZRangeByScore(
                "z".into(),
                "(1".into(),
                "+inf".into(),
                false,
                None
            )),
            arr(&["b", "c"])
        );
        assert_eq!(
            s.execute(Command::ZCount("z".into(), "-inf".into(), "2".into())),
            int(2)
        );
    }

    #[test]
    fn zset_zadd_nx_xx() {
        let s = store();
        s.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions::default(),
            vec![(1.0, "m".into())],
        ));
        // NX: don't update existing
        s.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions {
                condition: Some(crate::cmd::ZAddCondition::Nx),
                ..Default::default()
            },
            vec![(99.0, "m".into())],
        ));
        assert_eq!(
            s.execute(Command::ZScore("z".into(), "m".into())),
            bulk("1")
        );
        // XX: update existing only
        s.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions {
                condition: Some(crate::cmd::ZAddCondition::Xx),
                ..Default::default()
            },
            vec![(5.0, "m".into()), (5.0, "new".into())],
        ));
        assert_eq!(
            s.execute(Command::ZScore("z".into(), "m".into())),
            bulk("5")
        );
        assert_eq!(s.execute(Command::ZScore("z".into(), "new".into())), nil());
    }

    #[test]
    fn zset_wrongtype() {
        let s = store();
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        let res = s.execute(Command::ZAdd(
            "k".into(),
            ZAddOptions::default(),
            vec![(1.0, "m".into())],
        ));
        assert!(matches!(res, Value::Error(e) if e.contains("WRONGTYPE")));
    }

    // ── Cross-type TTL ────────────────────────────────────────────────────────

    #[test]
    fn collection_ttl_expire() {
        let s = store();
        s.execute(Command::HSet("h".into(), vec![("f".into(), "v".into())]));
        assert_eq!(s.execute(Command::Expire("h".into(), 60)), int(1));
        assert_eq!(s.execute(Command::HGet("h".into(), "f".into())), bulk("v"));
        // Force expiry by manipulating via PEXPIRE with 0ms
        s.execute(Command::PExpire("h".into(), 0));
        // Now lazy-expired
        assert_eq!(s.execute(Command::HGet("h".into(), "f".into())), nil());
    }

    // ── Transactions (store-layer stubs) ──────────────────────────────────────

    #[test]
    fn multi_returns_ok_stub() {
        // Store-layer stub: server intercepts MULTI before execute(), but the
        // stub must return OK so the exhaustiveness arm is exercised here.
        let s = store();
        assert_eq!(s.execute(Command::Multi), ok());
    }

    #[test]
    fn getset_does_not_lose_updates_under_concurrency() {
        // GETSET exists to do one thing: hand back the old value and install a
        // new one, indivisibly. Redis gets that from executing on one thread.
        // Here the read and the write have to happen under one shard guard —
        // with two guards, two worker threads could both observe the same old
        // value, and one write would vanish from the returned history.
        //
        // The invariant that catches it: every value ever written is replaced
        // exactly once, so no two callers may be handed the same old value.
        use std::sync::Arc;

        const THREADS: usize = 8;
        const ROUNDS: usize = 250;

        let store = Arc::new(KeyValueStore::new());
        store.execute(Command::Set(
            "k".into(),
            b"seed".to_vec(),
            SetOptions::default(),
        ));

        let handles: Vec<_> = (0..THREADS)
            .map(|t| {
                let store = Arc::clone(&store);
                std::thread::spawn(move || {
                    let mut seen = Vec::with_capacity(ROUNDS);
                    for r in 0..ROUNDS {
                        let mine = format!("t{t}-r{r}").into_bytes();
                        if let Value::BulkString(Some(old)) =
                            store.execute(Command::GetSet("k".into(), mine))
                        {
                            seen.push(old);
                        }
                    }
                    seen
                })
            })
            .collect();

        let mut observed: Vec<Vec<u8>> = handles
            .into_iter()
            .flat_map(|h| h.join().expect("a GETSET worker panicked"))
            .collect();

        let total = observed.len();
        assert_eq!(total, THREADS * ROUNDS, "every GETSET returns a value");
        observed.sort();
        observed.dedup();
        assert_eq!(
            observed.len(),
            total,
            "the same old value was handed to two callers: GETSET lost an update"
        );
    }

    #[test]
    fn setnx_has_exactly_one_winner_under_concurrency() {
        use std::sync::{Arc, Barrier};

        const THREADS: usize = 16;
        let store = Arc::new(KeyValueStore::new());
        let barrier = Arc::new(Barrier::new(THREADS));
        let winners: usize = (0..THREADS)
            .map(|worker| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    matches!(
                        store.execute(Command::SetNx(
                            "single-winner".into(),
                            format!("worker-{worker}").into_bytes(),
                        )),
                        Value::Integer(1)
                    ) as usize
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|worker| worker.join().expect("SETNX worker panicked"))
            .sum();
        assert_eq!(winners, 1);
    }

    #[test]
    fn set_nx_get_has_exactly_one_missing_predecessor() {
        use std::sync::{Arc, Barrier};

        const THREADS: usize = 16;
        let store = Arc::new(KeyValueStore::new());
        let barrier = Arc::new(Barrier::new(THREADS));
        let missing: usize = (0..THREADS)
            .map(|worker| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    matches!(
                        store.execute(Command::Set(
                            "single-winner".into(),
                            format!("worker-{worker}").into_bytes(),
                            SetOptions {
                                condition: Some(crate::cmd::SetCondition::Nx),
                                get: true,
                                ..Default::default()
                            },
                        )),
                        Value::BulkString(None)
                    ) as usize
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|worker| worker.join().expect("SET NX GET worker panicked"))
            .sum();
        assert_eq!(missing, 1);
    }

    #[test]
    fn exec_without_multi_error() {
        let s = store();
        let res = s.execute(Command::Exec);
        assert!(matches!(res, Value::Error(e) if e.contains("EXEC without MULTI")));
    }

    #[test]
    fn discard_without_multi_error() {
        let s = store();
        let res = s.execute(Command::Discard);
        assert!(matches!(res, Value::Error(e) if e.contains("DISCARD without MULTI")));
    }

    #[test]
    fn publish_stub_returns_zero() {
        let s = store();
        assert_eq!(
            s.execute(Command::Publish("ch".into(), "msg".into())),
            int(0)
        );
    }

    // ── Strings ───────────────────────────────────────────────────────────────

    #[test]
    fn string_get_missing_returns_nil() {
        let s = store();
        assert_eq!(s.execute(Command::Get("no_such_key".into())), nil());
    }

    #[test]
    fn string_append_and_strlen() {
        let s = store();
        s.execute(Command::Set(
            "k".into(),
            "hello".into(),
            SetOptions::default(),
        ));
        assert_eq!(
            s.execute(Command::Append("k".into(), " world".into())),
            int(11)
        );
        assert_eq!(s.execute(Command::Strlen("k".into())), int(11));
        assert_eq!(s.execute(Command::Get("k".into())), bulk("hello world"));
        // APPEND on missing key creates it
        assert_eq!(
            s.execute(Command::Append("new".into(), "abc".into())),
            int(3)
        );
        assert_eq!(s.execute(Command::Strlen("new".into())), int(3));
    }

    #[test]
    fn getrange_slices_with_inclusive_bounds() {
        let s = store();
        s.execute(Command::Set(
            "k".into(),
            "This is a string".into(),
            SetOptions::default(),
        ));
        assert_eq!(s.execute(Command::GetRange("k".into(), 0, 3)), bulk("This"));
        assert_eq!(
            s.execute(Command::GetRange("k".into(), -3, -1)),
            bulk("ing")
        );
        assert_eq!(
            s.execute(Command::GetRange("k".into(), 0, -1)),
            bulk("This is a string")
        );
        assert_eq!(
            s.execute(Command::GetRange("k".into(), 10, 100)),
            bulk("string"),
            "an end past the value clamps to the last byte"
        );
    }

    #[test]
    fn getrange_empty_cases() {
        let s = store();
        s.execute(Command::Set(
            "k".into(),
            "hello".into(),
            SetOptions::default(),
        ));
        // A start past the end inverts the range — Redis clamps `end` but not
        // `start`, so this is empty rather than the final byte.
        assert_eq!(s.execute(Command::GetRange("k".into(), 10, 20)), bulk(""));
        assert_eq!(s.execute(Command::GetRange("k".into(), 3, 1)), bulk(""));
        // A missing key is an empty string, and an empty bulk — not a nil.
        assert_eq!(
            s.execute(Command::GetRange("ghost".into(), 0, -1)),
            bulk("")
        );
        // Offsets far below zero clamp to the start rather than wrapping.
        assert_eq!(
            s.execute(Command::GetRange("k".into(), -100, 1)),
            bulk("he")
        );
        assert_eq!(
            s.execute(Command::GetRange("k".into(), i64::MIN, i64::MAX)),
            bulk("hello")
        );
    }

    #[test]
    fn getrange_is_binary_safe_and_type_checked() {
        let s = store();
        s.execute(Command::Set(
            "b".into(),
            vec![0xff, 0x00, 0xfe],
            SetOptions::default(),
        ));
        assert_eq!(
            s.execute(Command::GetRange("b".into(), 0, 1)),
            Value::BulkString(Some(vec![0xff, 0x00]))
        );
        s.execute(Command::HSet("h".into(), vec![("f".into(), "v".into())]));
        assert!(
            matches!(s.execute(Command::GetRange("h".into(), 0, -1)), Value::Error(e) if e.starts_with("WRONGTYPE"))
        );
    }

    #[test]
    fn string_getset() {
        let s = store();
        s.execute(Command::Set(
            "k".into(),
            "old".into(),
            SetOptions::default(),
        ));
        assert_eq!(
            s.execute(Command::GetSet("k".into(), "new".into())),
            bulk("old")
        );
        assert_eq!(s.execute(Command::Get("k".into())), bulk("new"));
        // GETSET on missing key returns nil
        assert_eq!(
            s.execute(Command::GetSet("missing".into(), "v".into())),
            nil()
        );
    }

    #[test]
    fn string_incr_decr() {
        let s = store();
        assert_eq!(s.execute(Command::Incr("n".into())), int(1));
        assert_eq!(s.execute(Command::Incr("n".into())), int(2));
        assert_eq!(s.execute(Command::Decr("n".into())), int(1));
        assert_eq!(s.execute(Command::Decr("n".into())), int(0));
        assert_eq!(s.execute(Command::Decr("n".into())), int(-1));
    }

    #[test]
    fn string_incrby_decrby() {
        let s = store();
        assert_eq!(s.execute(Command::IncrBy("n".into(), 10)), int(10));
        assert_eq!(s.execute(Command::IncrBy("n".into(), 5)), int(15));
        assert_eq!(s.execute(Command::DecrBy("n".into(), 3)), int(12));
        assert_eq!(s.execute(Command::DecrBy("n".into(), 20)), int(-8));
        assert!(matches!(
            s.execute(Command::DecrBy("n".into(), i64::MIN)),
            Value::Error(message) if message.contains("overflow")
        ));
    }

    #[test]
    fn string_mset_mget() {
        let s = store();
        assert_eq!(
            s.execute(Command::MSet(vec![
                ("a".into(), "1".into()),
                ("b".into(), "2".into()),
            ])),
            ok()
        );
        let res = s.execute(Command::MGet(vec![
            "a".into(),
            "b".into(),
            "missing".into(),
        ]));
        assert_eq!(res, Value::Array(Some(vec![bulk("1"), bulk("2"), nil()])));
    }

    #[test]
    fn string_setnx() {
        let s = store();
        assert_eq!(s.execute(Command::SetNx("k".into(), "v1".into())), int(1));
        assert_eq!(s.execute(Command::SetNx("k".into(), "v2".into())), int(0));
        assert_eq!(s.execute(Command::Get("k".into())), bulk("v1"));
    }

    #[test]
    fn string_setex() {
        let s = store();
        assert_eq!(s.execute(Command::SetEx("k".into(), 60, "v".into())), ok());
        assert_eq!(s.execute(Command::Get("k".into())), bulk("v"));
        // TTL should be positive
        let ttl = s.execute(Command::Ttl("k".into()));
        assert!(matches!(ttl, Value::Integer(n) if n > 0 && n <= 60));
    }

    #[test]
    fn string_del_multi_key() {
        let s = store();
        s.execute(Command::MSet(vec![
            ("a".into(), "1".into()),
            ("b".into(), "2".into()),
            ("c".into(), "3".into()),
        ]));
        // DEL returns count of deleted keys
        assert_eq!(
            s.execute(Command::Del(vec!["a".into(), "b".into(), "ghost".into()])),
            int(2)
        );
        assert_eq!(s.execute(Command::Get("a".into())), nil());
        assert_eq!(s.execute(Command::Get("c".into())), bulk("3"));
    }

    #[test]
    fn string_unlink_behaves_like_del() {
        let s = store();
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        assert_eq!(s.execute(Command::Unlink(vec!["k".into()])), int(1));
        assert_eq!(s.execute(Command::Get("k".into())), nil());
    }

    // ── Keys ──────────────────────────────────────────────────────────────────

    #[test]
    fn keys_exists_single_and_multi() {
        let s = store();
        s.execute(Command::Set("a".into(), "1".into(), SetOptions::default()));
        s.execute(Command::Set("b".into(), "2".into(), SetOptions::default()));
        assert_eq!(s.execute(Command::Exists(vec!["a".into()])), int(1));
        assert_eq!(s.execute(Command::Exists(vec!["ghost".into()])), int(0));
        // EXISTS counts duplicates
        assert_eq!(
            s.execute(Command::Exists(vec!["a".into(), "b".into(), "a".into()])),
            int(3)
        );
    }

    #[test]
    fn keys_type_reports_correct_type() {
        let s = store();
        s.execute(Command::Set(
            "str".into(),
            "v".into(),
            SetOptions::default(),
        ));
        s.execute(Command::HSet("hsh".into(), vec![("f".into(), "v".into())]));
        s.execute(Command::LPush("lst".into(), vec!["x".into()]));
        s.execute(Command::SAdd("st".into(), vec!["x".into()]));
        s.execute(Command::ZAdd(
            "zst".into(),
            ZAddOptions::default(),
            vec![(1.0, "m".into())],
        ));

        assert_eq!(
            s.execute(Command::Type("str".into())),
            Value::SimpleString("string".into())
        );
        assert_eq!(
            s.execute(Command::Type("hsh".into())),
            Value::SimpleString("hash".into())
        );
        assert_eq!(
            s.execute(Command::Type("lst".into())),
            Value::SimpleString("list".into())
        );
        assert_eq!(
            s.execute(Command::Type("st".into())),
            Value::SimpleString("set".into())
        );
        assert_eq!(
            s.execute(Command::Type("zst".into())),
            Value::SimpleString("zset".into())
        );
        assert_eq!(
            s.execute(Command::Type("none".into())),
            Value::SimpleString("none".into())
        );
    }

    #[test]
    fn keys_rename() {
        let s = store();
        s.execute(Command::Set(
            "src".into(),
            "v".into(),
            SetOptions::default(),
        ));
        assert_eq!(s.execute(Command::Rename("src".into(), "dst".into())), ok());
        assert_eq!(s.execute(Command::Get("src".into())), nil());
        assert_eq!(s.execute(Command::Get("dst".into())), bulk("v"));
        // Rename missing key is an error
        let res = s.execute(Command::Rename("ghost".into(), "dst".into()));
        assert!(matches!(res, Value::Error(_)));
    }

    #[test]
    fn keys_dbsize_and_flushdb() {
        let s = store();
        assert_eq!(s.execute(Command::DbSize), int(0));
        s.execute(Command::MSet(vec![
            ("a".into(), "1".into()),
            ("b".into(), "2".into()),
        ]));
        assert_eq!(s.execute(Command::DbSize), int(2));
        assert_eq!(s.execute(Command::FlushDb), ok());
        assert_eq!(s.execute(Command::DbSize), int(0));
    }

    #[test]
    fn keys_keys_pattern() {
        let s = store();
        s.execute(Command::MSet(vec![
            ("user:1".into(), "a".into()),
            ("user:2".into(), "b".into()),
            ("post:1".into(), "c".into()),
        ]));
        let res = s.execute(Command::Keys("user:*".into()));
        // Returns sorted array of matching keys
        assert_eq!(res, arr(&["user:1", "user:2"]));
        // Wildcard matches all
        let all = s.execute(Command::Keys("*".into()));
        assert_eq!(all, arr(&["post:1", "user:1", "user:2"]));
    }

    #[test]
    fn keys_scan_cursor_zero_returns_all() {
        let s = store();
        s.execute(Command::MSet(vec![
            ("x".into(), "1".into()),
            ("y".into(), "2".into()),
        ]));
        let res = s.execute(Command::Scan(0, None, None));
        // SCAN returns [cursor_bulk, [keys]]
        match res {
            Value::Array(Some(parts)) => {
                assert_eq!(parts[0], Value::BulkString(Some(b"0".to_vec())));
                assert!(matches!(&parts[1], Value::Array(Some(keys)) if keys.len() == 2));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn keys_scan_nonzero_cursor_returns_empty() {
        let s = store();
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        let res = s.execute(Command::Scan(42, None, None));
        match res {
            Value::Array(Some(parts)) => {
                assert_eq!(parts[0], Value::BulkString(Some(b"0".to_vec())));
                assert_eq!(parts[1], Value::Array(Some(vec![])));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn scan_paginates_with_count() {
        let s = store();
        for i in 0..5 {
            s.execute(Command::Set(
                format!("k{i}"),
                "v".into(),
                SetOptions::default(),
            ));
        }
        // Walk the cursor in pages of 2, collecting every key exactly once.
        let mut seen: Vec<String> = Vec::new();
        let mut cursor = 0u64;
        let mut iterations = 0;
        loop {
            let res = s.execute(Command::Scan(cursor, None, Some(2)));
            let Value::Array(Some(parts)) = res else {
                panic!("expected array")
            };
            let Value::BulkString(Some(c)) = &parts[0] else {
                panic!("expected cursor bulk")
            };
            let next: u64 = String::from_utf8_lossy(c).parse().unwrap();
            let Value::Array(Some(keys)) = &parts[1] else {
                panic!("expected keys array")
            };
            assert!(keys.len() <= 2, "page must honour COUNT");
            for k in keys {
                if let Value::BulkString(Some(d)) = k {
                    seen.push(String::from_utf8_lossy(d).into_owned());
                }
            }
            cursor = next;
            iterations += 1;
            if cursor == 0 {
                break;
            }
            assert!(iterations < 10, "cursor should terminate");
        }
        seen.sort();
        assert_eq!(seen, vec!["k0", "k1", "k2", "k3", "k4"]);
    }

    // ── Collection scans ──────────────────────────────────────────────────────

    /// Unpack a `[cursor, [elements…]]` reply into the pair it represents.
    fn scan_parts(v: &Value) -> (u64, Vec<String>) {
        let Value::Array(Some(parts)) = v else {
            panic!("scan reply must be an array, got {v:?}")
        };
        assert_eq!(parts.len(), 2, "scan reply is [cursor, elements]");
        let Value::BulkString(Some(c)) = &parts[0] else {
            panic!("cursor must be a bulk string")
        };
        let cursor: u64 = String::from_utf8_lossy(c).parse().expect("numeric cursor");
        let Value::Array(Some(items)) = &parts[1] else {
            panic!("elements must be an array")
        };
        let items = items
            .iter()
            .map(|i| match i {
                Value::BulkString(Some(d)) => String::from_utf8_lossy(d).into_owned(),
                other => panic!("element must be a bulk string, got {other:?}"),
            })
            .collect();
        (cursor, items)
    }

    /// Walk a scan to completion in pages of `count`, returning every element
    /// in the order the cursor produced them. Panics rather than looping
    /// forever if the cursor fails to terminate.
    fn drain_scan(
        s: &KeyValueStore,
        cmd: impl Fn(ScanArgs) -> Command,
        count: usize,
        per_element: usize,
    ) -> Vec<String> {
        let mut out = Vec::new();
        let mut cursor = 0u64;
        for _ in 0..100 {
            let (next, items) = scan_parts(&s.execute(cmd(ScanArgs {
                cursor,
                count: Some(count),
                ..Default::default()
            })));
            assert!(
                items.len() <= count * per_element,
                "page of {} exceeds COUNT {count}",
                items.len()
            );
            out.extend(items);
            cursor = next;
            if cursor == 0 {
                return out;
            }
        }
        panic!("cursor did not terminate");
    }

    #[test]
    fn hscan_visits_every_field_exactly_once() {
        let s = store();
        let fields: Vec<(String, Vec<u8>)> = (0..7)
            .map(|i| (format!("f{i}"), format!("v{i}").into_bytes()))
            .collect();
        s.execute(Command::HSet("h".into(), fields));
        let seen = drain_scan(&s, |a| Command::HScan("h".into(), a), 2, 2);
        assert_eq!(seen.len(), 14, "7 fields as field/value pairs");
        let names: Vec<&String> = seen.iter().step_by(2).collect();
        assert_eq!(names, vec!["f0", "f1", "f2", "f3", "f4", "f5", "f6"]);
        assert_eq!(seen[1], "v0");
    }

    #[test]
    fn hscan_novalues_returns_field_names_only() {
        let s = store();
        s.execute(Command::HSet(
            "h".into(),
            vec![("a".into(), "1".into()), ("b".into(), "2".into())],
        ));
        let (cursor, items) = scan_parts(&s.execute(Command::HScan(
            "h".into(),
            ScanArgs {
                novalues: true,
                ..Default::default()
            },
        )));
        assert_eq!(cursor, 0);
        assert_eq!(items, vec!["a", "b"]);
    }

    #[test]
    fn hscan_match_filters_field_names() {
        let s = store();
        s.execute(Command::HSet(
            "h".into(),
            vec![
                ("user:1".into(), "a".into()),
                ("user:2".into(), "b".into()),
                ("other".into(), "c".into()),
            ],
        ));
        let (_, items) = scan_parts(&s.execute(Command::HScan(
            "h".into(),
            ScanArgs {
                pattern: Some("user:*".into()),
                ..Default::default()
            },
        )));
        assert_eq!(items, vec!["user:1", "a", "user:2", "b"]);
    }

    #[test]
    fn collection_scans_on_missing_key_complete_empty() {
        let s = store();
        for cmd in [
            Command::HScan("ghost".into(), ScanArgs::default()),
            Command::SScan("ghost".into(), ScanArgs::default()),
            Command::ZScan("ghost".into(), ScanArgs::default()),
        ] {
            let (cursor, items) = scan_parts(&s.execute(cmd));
            assert_eq!(cursor, 0);
            assert!(items.is_empty());
        }
    }

    #[test]
    fn collection_scans_reject_the_wrong_type() {
        let s = store();
        s.execute(Command::Set(
            "str".into(),
            "v".into(),
            SetOptions::default(),
        ));
        for cmd in [
            Command::HScan("str".into(), ScanArgs::default()),
            Command::SScan("str".into(), ScanArgs::default()),
            Command::ZScan("str".into(), ScanArgs::default()),
        ] {
            assert!(matches!(s.execute(cmd), Value::Error(e) if e.starts_with("WRONGTYPE")));
        }
    }

    #[test]
    fn scan_cursor_past_the_end_completes_without_panicking() {
        // A client resuming a cursor into a collection that shrank underneath
        // it must get an empty final page, not an out-of-bounds slice.
        let s = store();
        s.execute(Command::SAdd("st".into(), vec!["a".into()]));
        let (cursor, items) = scan_parts(&s.execute(Command::SScan(
            "st".into(),
            ScanArgs {
                cursor: 9_999,
                ..Default::default()
            },
        )));
        assert_eq!(cursor, 0);
        assert!(items.is_empty());
    }

    #[test]
    fn sscan_visits_every_member_exactly_once() {
        let s = store();
        s.execute(Command::SAdd(
            "st".into(),
            (0..5).map(|i| format!("m{i}")).collect(),
        ));
        let seen = drain_scan(&s, |a| Command::SScan("st".into(), a), 2, 1);
        assert_eq!(seen, vec!["m0", "m1", "m2", "m3", "m4"]);
    }

    #[test]
    fn zscan_pages_members_with_scores() {
        let s = store();
        s.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions::default(),
            vec![(2.0, "bob".into()), (1.5, "amy".into())],
        ));
        // Ordered by member, not by score: the offset cursor has to survive a
        // score being updated mid-iteration.
        let seen = drain_scan(&s, |a| Command::ZScan("z".into(), a), 1, 2);
        assert_eq!(seen, vec!["amy", "1.5", "bob", "2"]);
    }

    // ── Expiry ────────────────────────────────────────────────────────────────

    #[test]
    fn expiry_ttl_on_no_ttl_key() {
        let s = store();
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        assert_eq!(s.execute(Command::Ttl("k".into())), int(-1));
        assert_eq!(s.execute(Command::PTtl("k".into())), int(-1));
    }

    #[test]
    fn expiry_ttl_missing_key() {
        let s = store();
        assert_eq!(s.execute(Command::Ttl("ghost".into())), int(-2));
        assert_eq!(s.execute(Command::PTtl("ghost".into())), int(-2));
    }

    #[test]
    fn expiry_ttl_after_expire() {
        let s = store();
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        s.execute(Command::Expire("k".into(), 100));
        let ttl = s.execute(Command::Ttl("k".into()));
        assert!(matches!(ttl, Value::Integer(n) if n > 0 && n <= 100));
        let pttl = s.execute(Command::PTtl("k".into()));
        assert!(matches!(pttl, Value::Integer(n) if n > 0 && n <= 100_000));
    }

    #[test]
    fn expiry_persist_removes_ttl() {
        let s = store();
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        s.execute(Command::Expire("k".into(), 60));
        assert_eq!(s.execute(Command::Persist("k".into())), int(1));
        assert_eq!(s.execute(Command::Ttl("k".into())), int(-1));
        // PERSIST on key with no TTL returns 0
        assert_eq!(s.execute(Command::Persist("k".into())), int(0));
    }

    #[test]
    fn expiry_pexpire_zero_ms_immediate() {
        let s = store();
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        s.execute(Command::PExpire("k".into(), 0));
        // Lazy expiry — next access sees it gone
        assert_eq!(s.execute(Command::Get("k".into())), nil());
        assert_eq!(s.execute(Command::Exists(vec!["k".into()])), int(0));
    }

    // ── Hash (additional) ─────────────────────────────────────────────────────

    #[test]
    fn hash_hkeys_hvals() {
        let s = store();
        s.execute(Command::HSet(
            "h".into(),
            vec![("b".into(), "2".into()), ("a".into(), "1".into())],
        ));
        assert_eq!(s.execute(Command::HKeys("h".into())), arr(&["a", "b"]));
        assert_eq!(s.execute(Command::HVals("h".into())), arr(&["1", "2"]));
        // Missing key returns empty array
        assert_eq!(
            s.execute(Command::HKeys("ghost".into())),
            Value::Array(Some(vec![]))
        );
    }

    #[test]
    fn hash_hexists() {
        let s = store();
        s.execute(Command::HSet("h".into(), vec![("f".into(), "v".into())]));
        assert_eq!(s.execute(Command::HExists("h".into(), "f".into())), int(1));
        assert_eq!(s.execute(Command::HExists("h".into(), "no".into())), int(0));
        assert_eq!(
            s.execute(Command::HExists("ghost".into(), "f".into())),
            int(0)
        );
    }

    // ── List (additional) ─────────────────────────────────────────────────────

    #[test]
    fn list_lpushx_rpushx_no_create() {
        let s = store();
        // LPUSHX/RPUSHX on non-existing key return 0 and don't create
        assert_eq!(
            s.execute(Command::LPushX("l".into(), vec!["x".into()])),
            int(0)
        );
        assert_eq!(
            s.execute(Command::RPushX("l".into(), vec!["x".into()])),
            int(0)
        );
        assert_eq!(s.execute(Command::Exists(vec!["l".into()])), int(0));
        // Once the list exists they work normally
        s.execute(Command::LPush("l".into(), vec!["a".into()]));
        assert_eq!(
            s.execute(Command::LPushX("l".into(), vec!["b".into()])),
            int(2)
        );
        assert_eq!(
            s.execute(Command::RPushX("l".into(), vec!["c".into()])),
            int(3)
        );
    }

    #[test]
    fn list_lpop_rpop_with_count() {
        let s = store();
        s.execute(Command::RPush(
            "l".into(),
            vec!["a".into(), "b".into(), "c".into(), "d".into()],
        ));
        assert_eq!(
            s.execute(Command::LPop("l".into(), Some(2))),
            arr(&["a", "b"])
        );
        assert_eq!(
            s.execute(Command::RPop("l".into(), Some(2))),
            arr(&["d", "c"])
        );
    }

    // ── Set (additional) ──────────────────────────────────────────────────────

    #[test]
    fn set_smismember() {
        let s = store();
        s.execute(Command::SAdd("s".into(), vec!["a".into(), "b".into()]));
        let res = s.execute(Command::SMIsMember(
            "s".into(),
            vec!["a".into(), "c".into(), "b".into()],
        ));
        assert_eq!(res, Value::Array(Some(vec![int(1), int(0), int(1)])));
    }

    #[test]
    fn set_sinterstore_sunionstore_sdiffstore() {
        let s = store();
        s.execute(Command::SAdd(
            "a".into(),
            vec!["1".into(), "2".into(), "3".into()],
        ));
        s.execute(Command::SAdd(
            "b".into(),
            vec!["2".into(), "3".into(), "4".into()],
        ));

        assert_eq!(
            s.execute(Command::SInterStore(
                "dst_i".into(),
                vec!["a".into(), "b".into()]
            )),
            int(2)
        );
        assert_eq!(
            s.execute(Command::SMembers("dst_i".into())),
            arr(&["2", "3"])
        );

        assert_eq!(
            s.execute(Command::SUnionStore(
                "dst_u".into(),
                vec!["a".into(), "b".into()]
            )),
            int(4)
        );
        assert_eq!(
            s.execute(Command::SMembers("dst_u".into())),
            arr(&["1", "2", "3", "4"])
        );

        assert_eq!(
            s.execute(Command::SDiffStore(
                "dst_d".into(),
                vec!["a".into(), "b".into()]
            )),
            int(1)
        );
        assert_eq!(s.execute(Command::SMembers("dst_d".into())), arr(&["1"]));
    }

    #[test]
    fn set_spop_removes_member() {
        let s = store();
        s.execute(Command::SAdd(
            "s".into(),
            vec!["a".into(), "b".into(), "c".into()],
        ));
        let popped = s.execute(Command::SPop("s".into(), None));
        // Result must be one of the members
        assert!(matches!(&popped, Value::BulkString(Some(v)) if matches!(
            String::from_utf8_lossy(v).as_ref(), "a" | "b" | "c"
        )));
        // Card decremented
        assert_eq!(s.execute(Command::SCard("s".into())), int(2));
    }

    #[test]
    fn set_spop_with_count() {
        let s = store();
        s.execute(Command::SAdd(
            "s".into(),
            vec!["a".into(), "b".into(), "c".into()],
        ));
        let res = s.execute(Command::SPop("s".into(), Some(2)));
        assert!(matches!(&res, Value::Array(Some(v)) if v.len() == 2));
        assert_eq!(s.execute(Command::SCard("s".into())), int(1));
    }

    #[test]
    fn set_srandmember_no_count() {
        let s = store();
        s.execute(Command::SAdd("s".into(), vec!["x".into(), "y".into()]));
        let res = s.execute(Command::SRandMember("s".into(), None));
        assert!(matches!(&res, Value::BulkString(Some(v))
            if matches!(String::from_utf8_lossy(v).as_ref(), "x" | "y")));
        // SCard unchanged
        assert_eq!(s.execute(Command::SCard("s".into())), int(2));
    }

    #[test]
    fn set_srandmember_with_count() {
        let s = store();
        s.execute(Command::SAdd(
            "s".into(),
            vec!["a".into(), "b".into(), "c".into()],
        ));
        let res = s.execute(Command::SRandMember("s".into(), Some(2)));
        assert!(matches!(&res, Value::Array(Some(v)) if v.len() == 2));
        // Negative count allows duplicates — count by absolute value
        let res_neg = s.execute(Command::SRandMember("s".into(), Some(-5)));
        assert!(matches!(&res_neg, Value::Array(Some(v)) if v.len() == 5));
    }

    // ── Sorted Set (additional) ───────────────────────────────────────────────

    #[test]
    fn zset_zmscore() {
        let s = store();
        s.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions::default(),
            vec![(1.0, "a".into()), (2.5, "b".into())],
        ));
        let res = s.execute(Command::ZMScore(
            "z".into(),
            vec!["a".into(), "ghost".into(), "b".into()],
        ));
        assert_eq!(res, Value::Array(Some(vec![bulk("1"), nil(), bulk("2.5")])));
    }

    #[test]
    fn zset_zrevrangebyscore() {
        let s = store();
        s.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions::default(),
            vec![(1.0, "a".into()), (2.0, "b".into()), (3.0, "c".into())],
        ));
        // ZREVRANGEBYSCORE max min — returns high to low
        assert_eq!(
            s.execute(Command::ZRevRangeByScore(
                "z".into(),
                "3".into(),
                "1".into(),
                false,
                None
            )),
            arr(&["c", "b", "a"])
        );
        // Exclusive bound
        assert_eq!(
            s.execute(Command::ZRevRangeByScore(
                "z".into(),
                "(3".into(),
                "1".into(),
                false,
                None
            )),
            arr(&["b", "a"])
        );
    }

    #[test]
    fn zset_zrevrangebyscore_with_limit() {
        let s = store();
        s.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions::default(),
            vec![
                (1.0, "a".into()),
                (2.0, "b".into()),
                (3.0, "c".into()),
                (4.0, "d".into()),
            ],
        ));
        let res = s.execute(Command::ZRevRangeByScore(
            "z".into(),
            "+inf".into(),
            "-inf".into(),
            false,
            Some((0, 2)),
        ));
        assert_eq!(res, arr(&["d", "c"]));
    }

    #[test]
    fn zset_zrevrange_withscores() {
        let s = store();
        s.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions::default(),
            vec![(1.0, "a".into()), (2.0, "b".into())],
        ));
        let res = s.execute(Command::ZRevRange("z".into(), 0, -1, true));
        assert_eq!(res, arr(&["b", "2", "a", "1"]));
    }

    // ── Snapshot / Restore ────────────────────────────────────────────────────

    #[test]
    fn snapshot_round_trip_all_types() {
        let s = store();
        s.execute(Command::Set(
            "str".into(),
            "hello".into(),
            SetOptions::default(),
        ));
        s.execute(Command::HSet("hash".into(), vec![("f".into(), "v".into())]));
        s.execute(Command::LPush("list".into(), vec!["a".into(), "b".into()]));
        s.execute(Command::SAdd("set".into(), vec!["x".into()]));
        s.execute(Command::ZAdd(
            "zset".into(),
            ZAddOptions::default(),
            vec![(1.5, "m".into())],
        ));

        let entries = s.snapshot();
        assert_eq!(entries.len(), 5);

        let s2 = store();
        s2.restore(entries);

        assert_eq!(s2.execute(Command::Get("str".into())), bulk("hello"));
        assert_eq!(
            s2.execute(Command::HGet("hash".into(), "f".into())),
            bulk("v")
        );
        assert_eq!(
            s2.execute(Command::LRange("list".into(), 0, -1)),
            arr(&["b", "a"])
        );
        assert_eq!(
            s2.execute(Command::SIsMember("set".into(), "x".into())),
            int(1)
        );
        assert_eq!(
            s2.execute(Command::ZScore("zset".into(), "m".into())),
            bulk("1.5")
        );
    }

    #[test]
    fn snapshot_skips_expired_keys() {
        use std::time::Duration;
        let s = store();
        s.execute(Command::Set(
            "live".into(),
            "v".into(),
            SetOptions::default(),
        ));
        s.execute(Command::PSetEx("dead".into(), 1, "v".into()));
        std::thread::sleep(Duration::from_millis(10));

        let entries = s.snapshot();
        assert!(entries.iter().any(|e| e.key == "live"));
        assert!(!entries.iter().any(|e| e.key == "dead"));
    }

    #[test]
    fn restore_skips_already_expired() {
        let s = store();
        let entry = SnapshotEntry {
            key: "ghost".into(),
            value: SnapshotValue::Str("v".into()),
            expires_at_ms: Some(1),
        };
        s.restore(vec![entry]);
        assert_eq!(s.execute(Command::DbSize), int(0));
    }

    #[test]
    fn snapshot_preserves_ttl() {
        let s = store();
        s.execute(Command::SetEx("k".into(), 60, "v".into()));

        let entries = s.snapshot();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].expires_at_ms.is_some());

        let s2 = store();
        s2.restore(entries);

        let ttl = s2.execute(Command::Ttl("k".into()));
        assert!(matches!(ttl, Value::Integer(n) if n > 0 && n <= 60));
    }

    // ── Rate limiting (RLSET / RLCHECK) ───────────────────────────────────────

    /// Unpack an RLCHECK reply into (allowed, remaining, retry_after_ms).
    fn rl(v: Value) -> (i64, i64, i64) {
        match v {
            Value::Array(Some(items)) => match items.as_slice() {
                [Value::Integer(a), Value::Integer(r), Value::Integer(t)] => (*a, *r, *t),
                other => panic!("unexpected RLCHECK reply shape: {:?}", other),
            },
            other => panic!("unexpected RLCHECK reply: {:?}", other),
        }
    }

    #[test]
    fn rlset_and_check_enforce_limit() {
        let s = store();
        assert_eq!(s.execute(Command::RlSet("api".into(), 3, 60)), ok());
        for expected_remaining in [2, 1, 0] {
            let (allowed, remaining, retry) = rl(s.execute(Command::RlCheck("api".into(), None)));
            assert_eq!(allowed, 1);
            assert_eq!(remaining, expected_remaining);
            assert_eq!(retry, 0);
        }
        let (allowed, remaining, retry) = rl(s.execute(Command::RlCheck("api".into(), None)));
        assert_eq!(allowed, 0);
        assert_eq!(remaining, 0);
        assert!(retry > 0 && retry <= 60_000, "retry_after_ms = {}", retry);
    }

    #[test]
    fn rlcheck_inline_config_creates_limiter() {
        let s = store();
        let key = "ip:10.0.0.1".to_string();
        let (allowed, _, _) = rl(s.execute(Command::RlCheck(key.clone(), Some((2, 60)))));
        assert_eq!(allowed, 1);
        let (allowed, _, _) = rl(s.execute(Command::RlCheck(key.clone(), Some((2, 60)))));
        assert_eq!(allowed, 1);
        let (allowed, _, _) = rl(s.execute(Command::RlCheck(key, Some((2, 60)))));
        assert_eq!(allowed, 0);
    }

    #[test]
    fn rlcheck_unconfigured_errors() {
        let s = store();
        let r = s.execute(Command::RlCheck("nope".into(), None));
        assert!(matches!(&r, Value::Error(e) if e.contains("no rate limit configured")));
    }

    #[test]
    fn rl_wrongtype_interactions() {
        let s = store();
        s.execute(Command::Set(
            "str".into(),
            "v".into(),
            SetOptions::default(),
        ));
        let r = s.execute(Command::RlCheck("str".into(), Some((5, 60))));
        assert!(matches!(&r, Value::Error(e) if e.contains("WRONGTYPE")));
        let r = s.execute(Command::RlSet("str".into(), 5, 60));
        assert!(matches!(&r, Value::Error(e) if e.contains("WRONGTYPE")));

        s.execute(Command::RlSet("rl".into(), 5, 60));
        let r = s.execute(Command::Get("rl".into()));
        assert!(matches!(&r, Value::Error(e) if e.contains("WRONGTYPE")));
        assert_eq!(
            s.execute(Command::Type("rl".into())),
            Value::SimpleString("ratelimit".into())
        );
    }

    #[test]
    fn rlset_reconfig_keeps_recorded_attempts() {
        let s = store();
        s.execute(Command::RlSet("api".into(), 2, 60));
        rl(s.execute(Command::RlCheck("api".into(), None)));
        // Raise the limit: the one recorded attempt still counts against it.
        s.execute(Command::RlSet("api".into(), 3, 60));
        let (allowed, remaining, _) = rl(s.execute(Command::RlCheck("api".into(), None)));
        assert_eq!((allowed, remaining), (1, 1));
        let (allowed, _, _) = rl(s.execute(Command::RlCheck("api".into(), None)));
        assert_eq!(allowed, 1);
        let (allowed, _, _) = rl(s.execute(Command::RlCheck("api".into(), None)));
        assert_eq!(allowed, 0);
    }

    #[test]
    fn rl_snapshot_preserves_config_but_not_attempt_state() {
        // Attempt counts are transient: they age out within one window, and a
        // restart has already interrupted that window. Only the configuration
        // is restored, so a limiter comes back enforcing the same policy with a
        // clean slate rather than a stale partial count.
        let s = store();
        s.execute(Command::RlSet("api".into(), 3, 60));
        s.execute(Command::RlCheck("api".into(), None));
        s.execute(Command::RlCheck("api".into(), None));

        let restored = store();
        restored.restore(s.snapshot());

        // Config survived: still a 3-per-60s limiter.
        let (allowed, remaining, _) = rl(restored.execute(Command::RlCheck("api".into(), None)));
        assert_eq!(allowed, 1);
        assert_eq!(
            remaining, 2,
            "attempts reset on restore — the limiter enforces the same policy afresh"
        );
    }

    // ── JSON (JSET / JGET / JMERGE) ───────────────────────────────────────────

    fn jset(s: &KeyValueStore, key: &str, path: &str, value: &str) -> Value {
        s.execute(Command::JSet(key.into(), path.into(), value.into()))
    }
    fn jget(s: &KeyValueStore, key: &str, path: Option<&str>) -> Value {
        s.execute(Command::JGet(key.into(), path.map(String::from)))
    }

    #[test]
    fn jset_jget_root_and_paths() {
        let s = store();
        assert_eq!(
            jset(&s, "doc", "$", r#"{"user":{"name":"amy"},"items":[1,2,3]}"#),
            ok()
        );
        // serde_json serializes object keys in sorted order — deterministic
        // output regardless of insertion order.
        assert_eq!(
            jget(&s, "doc", None),
            bulk(r#"{"items":[1,2,3],"user":{"name":"amy"}}"#)
        );
        assert_eq!(jget(&s, "doc", Some("$.user.name")), bulk(r#""amy""#));
        assert_eq!(jget(&s, "doc", Some("$.items[1]")), bulk("2"));
        // Set a nested field and an array element in place.
        assert_eq!(jset(&s, "doc", "$.user.age", "30"), ok());
        assert_eq!(jget(&s, "doc", Some("$.user.age")), bulk("30"));
        assert_eq!(jset(&s, "doc", "$.items[0]", "9"), ok());
        assert_eq!(jget(&s, "doc", Some("$.items")), bulk("[9,2,3]"));
        // Missing path → nil; missing key → nil.
        assert_eq!(jget(&s, "doc", Some("$.nope.deep")), nil());
        assert_eq!(jget(&s, "ghost", None), nil());
        // TYPE reports json.
        assert_eq!(
            s.execute(Command::Type("doc".into())),
            Value::SimpleString("json".into())
        );
    }

    #[test]
    fn jset_autocreates_intermediate_objects() {
        let s = store();
        // Fresh key, deep field path: intermediate objects are created.
        assert_eq!(jset(&s, "doc", "$.a.b.c", "1"), ok());
        assert_eq!(jget(&s, "doc", None), bulk(r#"{"a":{"b":{"c":1}}}"#));
        // Fresh key with a leading index cannot apply — and must not create
        // the key as a side effect.
        let r = jset(&s, "fresh", "$[0]", "1");
        assert!(matches!(&r, Value::Error(_)));
        assert_eq!(s.execute(Command::Exists(vec!["fresh".into()])), int(0));
        // Index out of bounds errors.
        let r = jset(&s, "doc", "$.a.b.c[5]", "1");
        assert!(matches!(&r, Value::Error(e) if e.contains("not an array")));
    }

    #[test]
    fn jmerge_rfc7386_semantics() {
        let s = store();
        jset(
            &s,
            "doc",
            "$",
            r#"{"title":"old","meta":{"draft":true,"v":1}}"#,
        );
        // Deep merge + null removal.
        assert_eq!(
            s.execute(Command::JMerge(
                "doc".into(),
                r#"{"title":"new","meta":{"draft":null}}"#.into()
            )),
            ok()
        );
        assert_eq!(
            jget(&s, "doc", None),
            bulk(r#"{"meta":{"v":1},"title":"new"}"#)
        );
        // Merge into a missing key creates the document.
        assert_eq!(
            s.execute(Command::JMerge("fresh".into(), r#"{"a":1}"#.into())),
            ok()
        );
        assert_eq!(jget(&s, "fresh", None), bulk(r#"{"a":1}"#));
        // A null patch deletes the key.
        assert_eq!(
            s.execute(Command::JMerge("fresh".into(), "null".into())),
            ok()
        );
        assert_eq!(jget(&s, "fresh", None), nil());
    }

    #[test]
    fn json_wrongtype_and_invalid_input() {
        let s = store();
        s.execute(Command::Set(
            "str".into(),
            "v".into(),
            SetOptions::default(),
        ));
        assert!(matches!(&jset(&s, "str", "$", "1"), Value::Error(e) if e.contains("WRONGTYPE")));
        assert!(matches!(&jget(&s, "str", None), Value::Error(e) if e.contains("WRONGTYPE")));
        jset(&s, "doc", "$", "{}");
        let r = s.execute(Command::Get("doc".into()));
        assert!(matches!(&r, Value::Error(e) if e.contains("WRONGTYPE")));
        // Invalid JSON / invalid path.
        assert!(
            matches!(&jset(&s, "doc", "$", "{oops"), Value::Error(e) if e.contains("invalid JSON"))
        );
        assert!(
            matches!(&jset(&s, "doc", "$.a[x]", "1"), Value::Error(e) if e.contains("bad index"))
        );
    }

    #[test]
    fn json_snapshot_roundtrip() {
        let s = store();
        jset(&s, "doc", "$", r#"{"n":42,"arr":[true,null,"x"]}"#);
        let s2 = store();
        s2.restore(s.snapshot());
        assert_eq!(
            s2.execute(Command::JGet("doc".into(), None)),
            bulk(r#"{"arr":[true,null,"x"],"n":42}"#)
        );
        assert_eq!(
            s2.execute(Command::Type("doc".into())),
            Value::SimpleString("json".into())
        );
    }

    // ── Live-query initial state ──────────────────────────────────────────────

    #[test]
    fn matching_key_values_globs_caps_and_skips_expired() {
        use std::time::Duration;
        let s = store();
        s.execute(Command::Set(
            "cart:1".into(),
            "a".into(),
            SetOptions::default(),
        ));
        s.execute(Command::Set(
            "cart:2".into(),
            "b".into(),
            SetOptions::default(),
        ));
        s.execute(Command::Set(
            "other:1".into(),
            "c".into(),
            SetOptions::default(),
        ));
        s.execute(Command::LPush("cart:list".into(), vec!["x".into()]));
        s.execute(Command::PSetEx("cart:dead".into(), 1, "d".into()));
        std::thread::sleep(Duration::from_millis(10));

        let mut kvs = s.matching_key_values("cart:*", 100);
        kvs.sort_by(|(a, _), (b, _)| a.cmp(b));
        assert_eq!(kvs.len(), 3, "expired key must be skipped: {kvs:?}");
        assert_eq!(kvs[0], ("cart:1".to_string(), bulk("a")));
        assert_eq!(kvs[1], ("cart:2".to_string(), bulk("b")));
        // Collections carry a tag plus their complete current contents.
        assert_eq!(
            kvs[2],
            (
                "cart:list".to_string(),
                Value::Array(Some(vec![bulk("list"), bulk("x")]))
            )
        );
        // Cap respected.
        assert_eq!(s.matching_key_values("cart:*", 2).len(), 2);
        // Non-matching pattern.
        assert!(s.matching_key_values("nope:*", 100).is_empty());
    }

    #[test]
    fn rl_window_slides_and_inline_limiter_expires() {
        use std::time::Duration;
        let s = store();
        // Persistent limiter: 1 attempt per 1-second window.
        s.execute(Command::RlSet("persist".into(), 1, 1));
        let (allowed, _, _) = rl(s.execute(Command::RlCheck("persist".into(), None)));
        assert_eq!(allowed, 1);
        let (allowed, _, retry) = rl(s.execute(Command::RlCheck("persist".into(), None)));
        assert_eq!(allowed, 0);
        assert!(retry > 0 && retry <= 1_000);
        // Auto-created limiter with a 1-second window.
        let (allowed, _, _) = rl(s.execute(Command::RlCheck("perip".into(), Some((1, 1)))));
        assert_eq!(allowed, 1);

        std::thread::sleep(Duration::from_millis(1_050));

        // The window slid past the recorded attempt: allowed again.
        let (allowed, _, _) = rl(s.execute(Command::RlCheck("persist".into(), None)));
        assert_eq!(allowed, 1);
        // The auto-created limiter expired with its window — bare RLCHECK
        // finds no config.
        let r = s.execute(Command::RlCheck("perip".into(), None));
        assert!(matches!(&r, Value::Error(e) if e.contains("no rate limit configured")));
    }
}

#[cfg(test)]
mod capacity_tests {
    use super::*;
    use crate::cmd::{Command, SetOptions};

    fn bulk(s: &str) -> Value {
        Value::BulkString(Some(s.as_bytes().to_vec()))
    }

    fn set(s: &KeyValueStore, k: &str, v: &str) -> Value {
        s.execute(Command::Set(k.into(), v.into(), SetOptions::default()))
    }

    /// Fill `n` keys named `k0..k{n-1}` with a fixed-size value.
    fn fill(s: &KeyValueStore, n: usize) {
        for i in 0..n {
            set(s, &format!("k{i}"), "0123456789");
        }
    }

    // ── Dirty counter ─────────────────────────────────────────────────────────
    // Drives the autosave loop: it skips a snapshot when nothing changed, so an
    // off-by-one here means either wasted saves or a missed one.

    #[test]
    fn dirty_counter_starts_at_zero_and_increments() {
        let s = KeyValueStore::new();
        assert_eq!(s.dirty_count(), 0);
        s.mark_dirty();
        s.mark_dirty();
        assert_eq!(s.dirty_count(), 2);
    }

    #[test]
    fn reset_dirty_clears_the_counter() {
        let s = KeyValueStore::new();
        s.mark_dirty();
        s.reset_dirty();
        assert_eq!(s.dirty_count(), 0);
        // Still usable after a reset — the counter is not consumed.
        s.mark_dirty();
        assert_eq!(s.dirty_count(), 1);
    }

    // ── get_current ───────────────────────────────────────────────────────────
    // The live-query read path. Strings return their value; every collection
    // type returns a type marker instead, which clients use to decide whether a
    // typed re-read is needed.

    #[test]
    fn get_current_returns_value_for_strings() {
        let s = KeyValueStore::new();
        set(&s, "k", "v");
        assert_eq!(s.get_current("k"), bulk("v"));
    }

    #[test]
    fn get_current_returns_nil_for_missing_and_expired() {
        let s = KeyValueStore::new();
        assert_eq!(s.get_current("nope"), Value::BulkString(None));

        s.execute(Command::Set(
            "gone".into(),
            "v".into(),
            SetOptions {
                expiry: Some(crate::cmd::SetExpiry::Px(1)),
                ..Default::default()
            },
        ));
        std::thread::sleep(std::time::Duration::from_millis(15));
        assert_eq!(s.get_current("gone"), Value::BulkString(None));
    }

    #[test]
    fn get_current_returns_type_tagged_collection_values() {
        // Live-query subscribers get the actual contents, tagged with the type
        // so the payload is unambiguous. Previously only the type name was sent,
        // which forced a follow-up HGETALL/LRANGE — a network round-trip in a
        // system whose whole premise is local reads.
        let s = KeyValueStore::new();
        s.execute(Command::HSet("h".into(), vec![("f".into(), "v".into())]));
        s.execute(Command::RPush("l".into(), vec!["a".into(), "b".into()]));
        s.execute(Command::SAdd("st".into(), vec!["m".into()]));
        s.execute(Command::ZAdd(
            "z".into(),
            Default::default(),
            vec![(1.5, "alice".into())],
        ));
        s.execute(Command::JSet("j".into(), "$".into(), "{\"a\":1}".into()));

        fn parts(v: Value) -> Vec<String> {
            match v {
                Value::Array(Some(items)) => items
                    .iter()
                    .map(|i| match i {
                        Value::BulkString(Some(b)) => String::from_utf8_lossy(b).into_owned(),
                        other => format!("{other:?}"),
                    })
                    .collect(),
                other => panic!("expected a tagged array, got {other:?}"),
            }
        }

        assert_eq!(parts(s.get_current("h")), vec!["hash", "f", "v"]);
        assert_eq!(parts(s.get_current("l")), vec!["list", "a", "b"]);
        assert_eq!(parts(s.get_current("st")), vec!["set", "m"]);
        assert_eq!(parts(s.get_current("z")), vec!["zset", "alice", "1.5"]);
        assert_eq!(parts(s.get_current("j")), vec!["json", "{\"a\":1}"]);
    }

    #[test]
    fn get_current_orders_collections_deterministically() {
        // Two clients receiving the same key must build identical local state,
        // so ordering cannot depend on hash iteration order.
        let s = KeyValueStore::new();
        s.execute(Command::HSet(
            "h".into(),
            vec![("b".into(), "2".into()), ("a".into(), "1".into())],
        ));
        s.execute(Command::ZAdd(
            "z".into(),
            Default::default(),
            vec![(9.0, "high".into()), (1.0, "low".into())],
        ));
        for _ in 0..5 {
            match s.get_current("h") {
                Value::Array(Some(items)) => {
                    // Fields sorted: a before b.
                    assert_eq!(items[1], Value::BulkString(Some(b"a".to_vec())));
                }
                other => panic!("{other:?}"),
            }
            match s.get_current("z") {
                Value::Array(Some(items)) => {
                    // Ascending score: low before high.
                    assert_eq!(items[1], Value::BulkString(Some(b"low".to_vec())));
                }
                other => panic!("{other:?}"),
            }
        }
    }

    // ── sweep_expired ─────────────────────────────────────────────────────────

    #[test]
    fn sweep_expired_drops_only_expired_keys() {
        let s = KeyValueStore::new();
        set(&s, "live", "v");
        s.execute(Command::Set(
            "dead".into(),
            "v".into(),
            SetOptions {
                expiry: Some(crate::cmd::SetExpiry::Px(1)),
                ..Default::default()
            },
        ));
        std::thread::sleep(std::time::Duration::from_millis(15));

        // Before the sweep the expired key still occupies memory — expiry is
        // lazy on read, so the sweep is what actually reclaims it.
        s.sweep_expired();

        assert_eq!(s.get_current("live"), bulk("v"));
        assert_eq!(s.get_current("dead"), Value::BulkString(None));
        assert_eq!(s.execute(Command::DbSize), Value::Integer(1));
    }

    #[test]
    fn sweep_expired_reports_the_keys_it_removed() {
        // Expiry is the one removal no client commanded, and reads only mask an
        // expired entry rather than removing it — so this sweep is the sole
        // remover, and its return value is the only way a caller replicating
        // the keyspace can learn the key is gone.
        let s = KeyValueStore::new();
        set(&s, "live", "v");
        for key in ["dead-a", "dead-b"] {
            s.execute(Command::Set(
                key.into(),
                "v".into(),
                SetOptions {
                    expiry: Some(crate::cmd::SetExpiry::Px(1)),
                    ..Default::default()
                },
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(15));

        let mut removed = s.sweep_expired_reporting();
        removed.sort();
        assert_eq!(removed, vec!["dead-a".to_string(), "dead-b".to_string()]);

        // A sweep that expires nothing reports nothing.
        assert!(s.sweep_expired_reporting().is_empty());
    }

    // ── Memory accounting ─────────────────────────────────────────────────────

    #[test]
    fn approximate_memory_grows_with_stored_data() {
        let s = KeyValueStore::new();
        let empty = s.approximate_memory_bytes();
        set(&s, "k", &"x".repeat(1000));
        let filled = s.approximate_memory_bytes();
        assert!(
            filled > empty + 1000,
            "1 KB value should add at least its own size: {empty} -> {filled}"
        );
    }

    #[test]
    fn approximate_memory_counts_every_value_type() {
        let s = KeyValueStore::new();
        let base = s.approximate_memory_bytes();
        s.execute(Command::HSet(
            "h".into(),
            vec![("f".into(), "v".repeat(500).into())],
        ));
        s.execute(Command::LPush("l".into(), vec!["v".repeat(500).into()]));
        s.execute(Command::SAdd("st".into(), vec!["v".repeat(500)]));
        s.execute(Command::ZAdd(
            "z".into(),
            Default::default(),
            vec![(1.0, "v".repeat(500))],
        ));
        s.execute(Command::JSet(
            "j".into(),
            "$".into(),
            format!("{{\"a\":\"{}\"}}", "v".repeat(500)),
        ));
        assert!(
            s.approximate_memory_bytes() > base + 2500,
            "collections must contribute to the memory estimate"
        );
    }

    // ── max_keys ──────────────────────────────────────────────────────────────

    #[test]
    fn max_keys_rejects_new_keys_without_an_eviction_policy() {
        let s = KeyValueStore::with_max_keys(2);
        set(&s, "a", "1");
        set(&s, "b", "2");
        // NoEviction is the default: the write is refused rather than making room.
        assert!(matches!(set(&s, "c", "3"), Value::Error(_)));
        assert_eq!(s.execute(Command::DbSize), Value::Integer(2));
        // Overwriting an existing key is still fine — it adds no key.
        assert_eq!(set(&s, "a", "updated"), Value::SimpleString("OK".into()));
    }

    #[test]
    fn max_keys_evicts_instead_of_failing_when_a_policy_is_set() {
        let s = KeyValueStore::with_config(Some(3), None, EvictionPolicy::AllKeysRandom);
        fill(&s, 3);
        assert_eq!(s.execute(Command::DbSize), Value::Integer(3));

        // The write succeeds; something older is evicted to make room.
        assert_eq!(set(&s, "new", "v"), Value::SimpleString("OK".into()));
        assert_eq!(s.execute(Command::DbSize), Value::Integer(3));
        assert_eq!(s.get_current("new"), bulk("v"));
    }

    #[test]
    fn mset_evicts_enough_room_for_every_new_key() {
        let s = KeyValueStore::with_config(Some(4), None, EvictionPolicy::AllKeysRandom);
        fill(&s, 4);
        // Three new keys against a full store — eviction must run per key, not once.
        s.execute(Command::MSet(vec![
            ("n1".into(), "v".into()),
            ("n2".into(), "v".into()),
            ("n3".into(), "v".into()),
        ]));
        assert_eq!(s.execute(Command::DbSize), Value::Integer(4));
        for k in ["n1", "n2", "n3"] {
            assert_eq!(s.get_current(k), bulk("v"), "{k} should have been written");
        }
    }

    #[test]
    fn mset_is_refused_when_it_cannot_make_room() {
        let s = KeyValueStore::with_max_keys(2); // NoEviction
        fill(&s, 2);
        let r = s.execute(Command::MSet(vec![
            ("n1".into(), "v".into()),
            ("n2".into(), "v".into()),
        ]));
        assert!(matches!(r, Value::Error(_)), "got {r:?}");
        assert_eq!(s.execute(Command::DbSize), Value::Integer(2));
    }

    // ── Memory-driven eviction ────────────────────────────────────────────────

    #[test]
    fn try_evict_is_a_noop_without_a_memory_limit() {
        let s = KeyValueStore::new();
        fill(&s, 5);
        assert!(s.try_evict_for_memory(), "no limit configured -> always ok");
        assert_eq!(s.execute(Command::DbSize), Value::Integer(5));
    }

    #[test]
    fn try_evict_reports_failure_when_policy_cannot_free_memory() {
        // A tiny limit with NoEviction: nothing can be freed, so the call must
        // report failure for an already-oversized restored snapshot. Ordinary
        // writes cannot create this state because the write path rejects them.
        let s = KeyValueStore::with_config(None, Some(64), EvictionPolicy::NoEviction);
        s.restore(vec![SnapshotEntry {
            key: "oversized".into(),
            value: SnapshotValue::Str(vec![b'x'; 1_024].into()),
            expires_at_ms: None,
        }]);
        assert!(s.approximate_memory_bytes() > 64);
        assert!(!s.try_evict_for_memory());
    }

    #[test]
    fn try_evict_frees_until_under_the_limit() {
        let s = KeyValueStore::with_config(None, Some(4096), EvictionPolicy::AllKeysRandom);
        for i in 0..60 {
            set(&s, &format!("k{i}"), &"x".repeat(200));
        }
        assert!(s.try_evict_for_memory());
        assert!(
            s.approximate_memory_bytes() <= 4096,
            "still over the limit: {}",
            s.approximate_memory_bytes()
        );
    }

    #[test]
    fn volatile_policies_only_evict_keys_that_have_a_ttl() {
        for policy in [EvictionPolicy::VolatileLru, EvictionPolicy::VolatileTtl] {
            let s = KeyValueStore::with_config(None, Some(256), policy);
            // Persistent keys only — a volatile policy has nothing it may touch.
            for i in 0..30 {
                set(&s, &format!("p{i}"), &"x".repeat(100));
            }
            assert!(
                !s.try_evict_for_memory(),
                "{policy:?} must not evict keys without a TTL"
            );
            assert_eq!(s.execute(Command::DbSize), Value::Integer(30));
        }
    }

    #[test]
    fn volatile_ttl_evicts_the_soonest_to_expire_first() {
        let s = KeyValueStore::with_config(None, Some(512), EvictionPolicy::VolatileTtl);
        // Long TTL first so it is not simply insertion order deciding.
        for (k, ttl) in [("keep", 600_000u64), ("drop", 1_000)] {
            s.execute(Command::Set(
                k.into(),
                "x".repeat(400).into(),
                SetOptions {
                    expiry: Some(crate::cmd::SetExpiry::Px(ttl)),
                    ..Default::default()
                },
            ));
        }
        s.try_evict_for_memory();
        assert_eq!(
            s.get_current("drop"),
            Value::BulkString(None),
            "soonest-to-expire key should go first"
        );
    }

    #[test]
    fn all_keys_lru_evicts_the_least_recently_used() {
        // Two 400-byte values cost ~936 bytes (entry_size = key + value + 64),
        // so a 600-byte cap forces exactly one eviction.
        let s = KeyValueStore::with_config(None, Some(600), EvictionPolicy::AllKeysLru);
        set(&s, "cold", &"x".repeat(400));
        std::thread::sleep(std::time::Duration::from_millis(5));
        set(&s, "warm", &"x".repeat(400));

        // Read through `execute` — that is the path that refreshes recency.
        // `get_current` deliberately does not touch the entry, so using it here
        // would leave both keys with their write-time timestamps.
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert_ne!(
            s.execute(Command::Get("warm".into())),
            Value::BulkString(None)
        );

        assert!(s.try_evict_for_memory());
        assert_eq!(
            s.get_current("cold"),
            Value::BulkString(None),
            "least-recently-used key should be evicted first"
        );
        assert_ne!(s.get_current("warm"), Value::BulkString(None));
    }
}

#[cfg(test)]
mod semantics_tests {
    use super::*;
    use crate::cmd::{Command, SetExpiry, SetOptions, ZAddOptions};

    fn bulk(s: &str) -> Value {
        Value::BulkString(Some(s.as_bytes().to_vec()))
    }

    fn zadd(s: &KeyValueStore, key: &str, opts: ZAddOptions, members: Vec<(f64, String)>) -> Value {
        s.execute(Command::ZAdd(key.into(), opts, members))
    }

    fn score(s: &KeyValueStore, key: &str, member: &str) -> Option<String> {
        match s.execute(Command::ZScore(key.into(), member.into())) {
            Value::BulkString(Some(b)) => Some(String::from_utf8(b).unwrap()),
            _ => None,
        }
    }

    // ── ZADD GT / LT ──────────────────────────────────────────────────────────
    // Only move a score in one direction. Getting these inverted silently
    // corrupts leaderboards, which is the headline use case for sorted sets.

    #[test]
    fn zadd_gt_only_raises_scores() {
        let s = KeyValueStore::new();
        zadd(&s, "lb", ZAddOptions::default(), vec![(100.0, "p".into())]);

        let gt = ZAddOptions {
            gt: true,
            ..Default::default()
        };
        zadd(&s, "lb", gt.clone(), vec![(50.0, "p".into())]);
        assert_eq!(
            score(&s, "lb", "p").as_deref(),
            Some("100"),
            "GT must not lower"
        );

        zadd(&s, "lb", gt, vec![(150.0, "p".into())]);
        assert_eq!(
            score(&s, "lb", "p").as_deref(),
            Some("150"),
            "GT must raise"
        );
    }

    #[test]
    fn zadd_lt_only_lowers_scores() {
        let s = KeyValueStore::new();
        zadd(&s, "lb", ZAddOptions::default(), vec![(100.0, "p".into())]);

        let lt = ZAddOptions {
            lt: true,
            ..Default::default()
        };
        zadd(&s, "lb", lt.clone(), vec![(150.0, "p".into())]);
        assert_eq!(
            score(&s, "lb", "p").as_deref(),
            Some("100"),
            "LT must not raise"
        );

        zadd(&s, "lb", lt, vec![(50.0, "p".into())]);
        assert_eq!(score(&s, "lb", "p").as_deref(), Some("50"), "LT must lower");
    }

    #[test]
    fn zadd_ch_counts_changed_not_just_added() {
        let s = KeyValueStore::new();
        zadd(&s, "z", ZAddOptions::default(), vec![(1.0, "a".into())]);
        let ch = ZAddOptions {
            ch: true,
            ..Default::default()
        };
        // Without CH this returns 0 (nothing *added*); with CH it counts the update.
        assert_eq!(
            zadd(&s, "z", ch, vec![(2.0, "a".into())]),
            Value::Integer(1)
        );
    }

    // ── ZINCRBY ───────────────────────────────────────────────────────────────

    #[test]
    fn zincrby_accumulates_and_creates() {
        let s = KeyValueStore::new();
        assert_eq!(
            s.execute(Command::ZIncrBy("z".into(), 5.0, "m".into())),
            bulk("5")
        );
        assert_eq!(
            s.execute(Command::ZIncrBy("z".into(), 2.5, "m".into())),
            bulk("7.5")
        );
        // Negative deltas subtract.
        assert_eq!(
            s.execute(Command::ZIncrBy("z".into(), -7.5, "m".into())),
            bulk("0")
        );
    }

    // ── TTL overflow guards ───────────────────────────────────────────────────

    #[test]
    fn set_rejects_ttl_that_would_overflow_milliseconds() {
        let s = KeyValueStore::new();
        let r = s.execute(Command::Set(
            "k".into(),
            "v".into(),
            SetOptions {
                expiry: Some(SetExpiry::Ex(u64::MAX / 1000 + 1)),
                ..Default::default()
            },
        ));
        assert!(matches!(r, Value::Error(_)), "got {r:?}");
    }

    #[test]
    fn keepttl_preserves_the_existing_expiry() {
        let s = KeyValueStore::new();
        s.execute(Command::Set(
            "k".into(),
            "v1".into(),
            SetOptions {
                expiry: Some(SetExpiry::Ex(600)),
                ..Default::default()
            },
        ));
        s.execute(Command::Set(
            "k".into(),
            "v2".into(),
            SetOptions {
                expiry: Some(SetExpiry::KeepTtl),
                ..Default::default()
            },
        ));
        assert_eq!(s.execute(Command::Get("k".into())), bulk("v2"));
        // A TTL still exists — the rewrite did not clear it.
        assert!(
            matches!(s.execute(Command::Ttl("k".into())), Value::Integer(n) if n > 0),
            "KEEPTTL should retain the expiry"
        );
    }

    #[test]
    fn plain_set_clears_an_existing_expiry() {
        let s = KeyValueStore::new();
        s.execute(Command::Set(
            "k".into(),
            "v1".into(),
            SetOptions {
                expiry: Some(SetExpiry::Ex(600)),
                ..Default::default()
            },
        ));
        s.execute(Command::Set("k".into(), "v2".into(), SetOptions::default()));
        // -1 means "exists, no expiry".
        assert_eq!(s.execute(Command::Ttl("k".into())), Value::Integer(-1));
    }

    // ── SMOVE ─────────────────────────────────────────────────────────────────

    #[test]
    fn smove_transfers_a_member_between_sets() {
        let s = KeyValueStore::new();
        s.execute(Command::SAdd("src".into(), vec!["a".into(), "b".into()]));
        s.execute(Command::SAdd("dst".into(), vec!["z".into()]));

        assert_eq!(
            s.execute(Command::SMove("src".into(), "dst".into(), "a".into())),
            Value::Integer(1)
        );
        assert_eq!(
            s.execute(Command::SIsMember("src".into(), "a".into())),
            Value::Integer(0)
        );
        assert_eq!(
            s.execute(Command::SIsMember("dst".into(), "a".into())),
            Value::Integer(1)
        );
    }

    #[test]
    fn smove_is_a_noop_when_the_member_is_absent() {
        let s = KeyValueStore::new();
        s.execute(Command::SAdd("src".into(), vec!["a".into()]));
        assert_eq!(
            s.execute(Command::SMove("src".into(), "dst".into(), "missing".into())),
            Value::Integer(0)
        );
        // Destination must not be created as a side effect of a failed move.
        assert_eq!(
            s.execute(Command::Exists(vec!["dst".into()])),
            Value::Integer(0)
        );
    }

    // ── keyspace_sample ───────────────────────────────────────────────────────

    #[test]
    fn keyspace_sample_counts_keys_ttls_and_bytes_in_one_pass() {
        let s = KeyValueStore::new();
        s.execute(Command::Set(
            "a".into(),
            b"xx".to_vec(),
            SetOptions::default(),
        ));
        s.execute(Command::Set(
            "b".into(),
            b"yy".to_vec(),
            SetOptions::default(),
        ));
        s.execute(Command::Expire("b".into(), 60));

        let sample = s.keyspace_sample();
        assert_eq!(sample.keys, 2);
        assert_eq!(sample.volatile_keys, 1, "only b carries a TTL");
        assert!(sample.memory_bytes > 0);
        // Must agree with the single-purpose accessors it replaces at call sites.
        assert_eq!(sample.keys, s.key_count());
        assert_eq!(sample.memory_bytes, s.approximate_memory_bytes());
    }

    #[test]
    fn keyspace_sample_changes_after_active_expiry_removes_entries() {
        let s = KeyValueStore::new();
        s.execute(Command::Set(
            "gone".into(),
            b"v".to_vec(),
            SetOptions::default(),
        ));
        s.execute(Command::PExpire("gone".into(), 1));
        std::thread::sleep(std::time::Duration::from_millis(20));

        let pending = s.keyspace_sample();
        assert_eq!(pending.keys, 1);
        assert_eq!(pending.volatile_keys, 1);
        assert!(pending.memory_bytes > 0);

        assert_eq!(s.sweep_expired_reporting_budget(1), vec!["gone"]);
        assert_eq!(s.keyspace_sample(), KeyspaceSample::default());
    }

    #[test]
    fn keyspace_sample_is_empty_on_a_fresh_store() {
        assert_eq!(
            KeyValueStore::new().keyspace_sample(),
            KeyspaceSample::default()
        );
    }

    #[test]
    fn store_config_accessors_report_what_was_configured() {
        let s = KeyValueStore::with_config(Some(10), Some(4096), EvictionPolicy::AllKeysLru);
        assert_eq!(s.max_keys(), Some(10));
        assert_eq!(s.max_memory_bytes(), Some(4096));
        assert_eq!(s.eviction_policy(), EvictionPolicy::AllKeysLru);

        let unbounded = KeyValueStore::new();
        assert_eq!(unbounded.max_keys(), None);
        assert_eq!(unbounded.max_memory_bytes(), None);
        assert_eq!(unbounded.eviction_policy(), EvictionPolicy::NoEviction);
    }

    // ── Commands the engine deliberately refuses ──────────────────────────────
    // These are server-layer concerns; the pure engine must reject rather than
    // half-implement them, so a WASM build can never silently "succeed".

    #[test]
    fn engine_refuses_server_layer_commands() {
        let s = KeyValueStore::new();
        for cmd in [
            Command::Save,
            Command::BgSave,
            Command::LastSave,
            Command::ReplicaOfNoOne,
            Command::Watch(vec!["k".into()]),
            Command::Unwatch(vec![]),
            Command::Hello(None),
            Command::Info(vec![]),
        ] {
            assert!(
                matches!(s.execute(cmd.clone()), Value::Error(_)),
                "{cmd:?} should be refused by the engine"
            );
        }
    }

    #[test]
    fn unknown_command_names_itself_in_the_error() {
        let s = KeyValueStore::new();
        match s.execute(Command::Unknown("FLERB".into())) {
            Value::Error(e) => assert!(e.contains("FLERB"), "got {e}"),
            other => panic!("expected error, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod critical_path_tests {
    use super::*;
    use crate::cmd::{Command, SetExpiry, SetOptions};

    fn bulk(s: &str) -> Value {
        Value::BulkString(Some(s.as_bytes().to_vec()))
    }

    // ── glob_match ────────────────────────────────────────────────────────────
    //
    // This is not just the KEYS/SCAN matcher. `server-native` uses it as the
    // sync-scope authorization primitive: a connection may touch a key only if
    // some granted pattern glob_matches it. A false positive here is a
    // cross-tenant data leak, so the security-relevant cases are pinned
    // explicitly rather than left to the callers' tests.

    #[test]
    fn glob_literal_matches_exactly() {
        assert!(glob_match("key", "key"));
        assert!(!glob_match("key", "keys"));
        assert!(!glob_match("keys", "key"));
        assert!(!glob_match("key", "Key"), "matching is case-sensitive");
    }

    #[test]
    fn glob_star_matches_any_run_including_empty() {
        assert!(glob_match("*", ""));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("a*", "a"), "trailing * may match nothing");
        assert!(glob_match("a*c", "ac"), "interior * may match nothing");
        assert!(glob_match("a*c", "abbbc"));
        assert!(glob_match("*c", "abc"));
        assert!(glob_match("*b*", "abc"));
        assert!(glob_match("a*b*c", "axxbyyc"));
    }

    #[test]
    fn glob_question_matches_exactly_one_byte() {
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "ac"), "? must not match empty");
        assert!(!glob_match("a?c", "abbc"), "? must not match two");
        assert!(glob_match("???", "abc"));
        assert!(!glob_match("???", "ab"));
    }

    #[test]
    fn glob_empty_pattern_matches_only_empty_string() {
        assert!(glob_match("", ""));
        assert!(!glob_match("", "x"));
    }

    #[test]
    fn glob_scope_prefix_grants_do_not_leak_across_siblings() {
        // The documented model: `cart:*` covers everything under `cart:`.
        assert!(glob_match("cart:*", "cart:42"));
        assert!(glob_match("cart:*", "cart:42:item:9"));

        // But a narrower grant must not reach a sibling tenant's keys. If any
        // of these flip to true, scoped connections can read other users' data.
        assert!(!glob_match("cart:42:*", "cart:99:item:1"));
        assert!(!glob_match("user:1:*", "user:2:secret"));
        assert!(!glob_match("cart:*", "carts:42"), "':' is a real boundary");
        assert!(
            !glob_match("cart:*", "xcart:42"),
            "no implicit leading wildcard"
        );
    }

    #[test]
    fn glob_prefix_confusion_between_similar_keys() {
        // `user:1*` legitimately covers user:1, user:10, user:19 — a caller
        // granting it is granting all of them. Pinned so the behaviour is a
        // documented decision rather than an accident.
        assert!(glob_match("user:1*", "user:1"));
        assert!(glob_match("user:1*", "user:10"));
        assert!(glob_match("user:1*", "user:1:private"));
        assert!(!glob_match("user:1*", "user:2"));
    }

    #[test]
    fn glob_star_crosses_separators() {
        // Unlike shell globbing, `*` spans ':' — this is what makes a single
        // `tenant:7:*` grant cover the whole subtree.
        assert!(glob_match("tenant:7:*", "tenant:7:orders:2024:11"));
        assert!(glob_match("*:secret", "a:b:c:secret"));
    }

    #[test]
    fn glob_consecutive_stars_behave_as_one() {
        assert!(glob_match("**", "abc"));
        assert!(glob_match("a**c", "abc"));
        assert!(glob_match("a**c", "ac"));
    }

    #[test]
    fn glob_pathological_pattern_terminates_quickly() {
        // The implementation replaced a recursive matcher with exponential
        // backtracking on inputs of exactly this shape. A regression would hang
        // the server, so this asserts on wall-clock, not just correctness.
        let text = "a".repeat(2_000);
        let pattern = "*a*a*a*a*a*a*a*a*a*b";
        let start = std::time::Instant::now();
        assert!(!glob_match(pattern, &text));
        assert!(
            start.elapsed() < std::time::Duration::from_secs(1),
            "glob_match took {:?} — exponential backtracking has returned",
            start.elapsed()
        );
    }

    /// The DP implementation this replaced, kept as a reference oracle.
    ///
    /// `glob_match` is not just a `KEYS` helper — it decides sync-scope access,
    /// so a rewrite that changed semantics anywhere would be a silent
    /// authorisation change. Comparing against the previous implementation is
    /// the only way to make "behaviour is unchanged" an assertion rather than a
    /// claim.
    fn glob_match_dp_reference(pattern: &str, s: &str) -> bool {
        let pat = pattern.as_bytes();
        let text = s.as_bytes();
        let (m, n) = (pat.len(), text.len());
        let mut prev = vec![false; n + 1];
        let mut curr = vec![false; n + 1];
        prev[0] = true;
        for i in 1..=m {
            curr[0] = pat[i - 1] == b'*' && prev[0];
            for j in 1..=n {
                curr[j] = if pat[i - 1] == b'*' {
                    prev[j] || curr[j - 1]
                } else if pat[i - 1] == b'?' || pat[i - 1] == text[j - 1] {
                    prev[j - 1]
                } else {
                    false
                };
            }
            std::mem::swap(&mut prev, &mut curr);
        }
        prev[n]
    }

    #[test]
    fn glob_match_agrees_with_the_dp_reference_on_every_small_input() {
        // Exhaustive rather than random: every pattern over {a, b, *, ?} up to
        // length 5 against every text over {a, b} up to length 4. That is the
        // whole space where `*` interacts with `?` and with literals, which is
        // where a greedy matcher would differ from the DP if it were wrong.
        let pat_alphabet = *b"ab*?";
        let txt_alphabet = *b"ab";

        fn all_strings(alphabet: &[u8], max_len: usize) -> Vec<String> {
            let mut out = vec![String::new()];
            let mut frontier = vec![String::new()];
            for _ in 0..max_len {
                let mut next = Vec::new();
                for s in &frontier {
                    for &c in alphabet {
                        let mut t = s.clone();
                        t.push(c as char);
                        next.push(t);
                    }
                }
                out.extend(next.iter().cloned());
                frontier = next;
            }
            out
        }

        let patterns = all_strings(&pat_alphabet, 5);
        let texts = all_strings(&txt_alphabet, 4);
        let mut compared = 0usize;
        for p in &patterns {
            for t in &texts {
                let got = glob_match(p, t);
                let want = glob_match_dp_reference(p, t);
                assert_eq!(got, want, "glob_match({p:?}, {t:?}) disagrees with the DP");
                compared += 1;
            }
        }
        // Guard against the loops silently collapsing to nothing.
        assert!(compared > 20_000, "only compared {compared} pairs");
    }

    #[test]
    fn glob_match_agrees_with_the_dp_reference_on_realistic_key_shapes() {
        // The exhaustive sweep uses a two-letter alphabet, so it never produces
        // a `:`-delimited key or a repeated-literal run — the shapes real
        // patterns and real keys actually have.
        let cases = [
            ("cart:*", "cart:42:item:9"),
            ("cart:42:*", "cart:99:item:1"),
            ("*:secret", "a:b:c:secret"),
            ("user:1*", "user:1:private"),
            ("tenant:7:*", "tenant:7:orders:2024:11"),
            ("*a*a*a*a*b", "aaaaaaaaaaaaaaaaaaaa"),
            ("a*b*c*d", "axxbyyczzd"),
            ("a*b*c*d", "axxbyyczz"),
            ("?????", "abcde"),
            ("*?", ""),
            ("?*", "x"),
            ("**?**", "xy"),
            ("session:*:token", "session:abc:token"),
            ("session:*:token", "session::token"),
            ("[ab]", "a"),
            ("[ab]", "[ab]"),
        ];
        for (p, t) in cases {
            assert_eq!(
                glob_match(p, t),
                glob_match_dp_reference(p, t),
                "glob_match({p:?}, {t:?}) disagrees with the DP"
            );
        }
    }

    #[test]
    fn glob_match_does_not_allocate_per_call() {
        // The DP allocated two Vec<bool> of text.len() + 1 on every call, so one
        // 64 MB value made `KEYS *` request 128 MB — on a path that runs once
        // per key. A long text must now cost nothing but time.
        let long = "k".repeat(4 * 1024 * 1024);
        assert!(glob_match("*", &long));
        assert!(glob_match("k*k", &long));
        assert!(!glob_match("*z", &long));
    }

    #[test]
    fn glob_pattern_cap_is_generous_but_finite() {
        // Enforced where patterns are parsed, not here — this pins the value so
        // it cannot drift without someone noticing.
        assert_eq!(MAX_PATTERN_BYTES, 1024);
    }

    #[test]
    fn glob_operates_on_bytes_so_multibyte_chars_span_several_positions() {
        // Documented consequence of byte-wise matching: '?' matches one *byte*,
        // and 'é' is two bytes in UTF-8. Callers building scopes from user input
        // need to know this.
        assert!(!glob_match("?", "é"));
        assert!(glob_match("??", "é"));
        assert!(glob_match("*", "héllo"));
        assert!(glob_match("h*o", "héllo"));
    }

    // ── Expiry ────────────────────────────────────────────────────────────────
    // Returning an expired value is a correctness bug with security weight —
    // revoked sessions and flags are exactly what people put in a cache.

    #[test]
    fn expired_keys_are_invisible_to_every_read_path() {
        let s = KeyValueStore::new();
        s.execute(Command::Set(
            "k".into(),
            "v".into(),
            SetOptions {
                expiry: Some(SetExpiry::Px(1)),
                ..Default::default()
            },
        ));
        std::thread::sleep(std::time::Duration::from_millis(15));

        assert_eq!(s.execute(Command::Get("k".into())), Value::BulkString(None));
        assert_eq!(
            s.execute(Command::Exists(vec!["k".into()])),
            Value::Integer(0)
        );
        assert_eq!(s.get_current("k"), Value::BulkString(None));
        assert_eq!(s.execute(Command::Ttl("k".into())), Value::Integer(-2));
        // KEYS and live-query matching must not surface it either.
        assert_eq!(
            s.execute(Command::Keys("*".into())),
            Value::Array(Some(vec![]))
        );
        assert!(s.matching_key_values("*", 100).is_empty());
    }

    #[test]
    fn ttl_distinguishes_missing_from_persistent() {
        let s = KeyValueStore::new();
        // -2 = no such key, -1 = exists but never expires.
        assert_eq!(s.execute(Command::Ttl("nope".into())), Value::Integer(-2));
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        assert_eq!(s.execute(Command::Ttl("k".into())), Value::Integer(-1));
    }

    #[test]
    fn expiry_in_the_past_deletes_immediately() {
        let s = KeyValueStore::new();
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        // EXAT with a timestamp already behind us.
        s.execute(Command::ExpireAt("k".into(), 1_000));
        assert_eq!(s.execute(Command::Get("k".into())), Value::BulkString(None));
    }

    // ── Numeric edges ─────────────────────────────────────────────────────────

    #[test]
    fn incr_refuses_to_overflow() {
        let s = KeyValueStore::new();
        s.execute(Command::Set(
            "n".into(),
            i64::MAX.to_string().into(),
            SetOptions::default(),
        ));
        let r = s.execute(Command::Incr("n".into()));
        assert!(
            matches!(r, Value::Error(_)),
            "i64::MAX + 1 must error, got {r:?}"
        );
        // The stored value must be untouched after a refused increment.
        assert_eq!(
            s.execute(Command::Get("n".into())),
            bulk(&i64::MAX.to_string())
        );
    }

    #[test]
    fn decr_refuses_to_underflow() {
        let s = KeyValueStore::new();
        s.execute(Command::Set(
            "n".into(),
            i64::MIN.to_string().into(),
            SetOptions::default(),
        ));
        assert!(matches!(
            s.execute(Command::Decr("n".into())),
            Value::Error(_)
        ));
    }

    #[test]
    fn incr_rejects_non_numeric_values() {
        let s = KeyValueStore::new();
        s.execute(Command::Set(
            "k".into(),
            "abc".into(),
            SetOptions::default(),
        ));
        match s.execute(Command::Incr("k".into())) {
            Value::Error(e) => assert!(e.contains("not an integer"), "got {e}"),
            other => panic!("expected error, got {other:?}"),
        }
    }

    // ── Type safety ───────────────────────────────────────────────────────────
    // A string command against a hash must error, not coerce or silently
    // clobber the existing value.

    #[test]
    fn wrong_type_operations_error_and_preserve_the_value() {
        let s = KeyValueStore::new();
        s.execute(Command::HSet("h".into(), vec![("f".into(), "v".into())]));

        for cmd in [
            Command::Get("h".into()),
            Command::Incr("h".into()),
            Command::Append("h".into(), "x".into()),
            Command::LPush("h".into(), vec!["x".into()]),
            Command::SAdd("h".into(), vec!["x".into()]),
        ] {
            assert!(
                matches!(s.execute(cmd.clone()), Value::Error(_)),
                "{cmd:?} against a hash should error"
            );
        }
        // The hash survived every attempt intact.
        assert_eq!(s.execute(Command::HGet("h".into(), "f".into())), bulk("v"));
    }

    // ── Snapshot round-trip ───────────────────────────────────────────────────
    // Restore is the data-loss path: anything that fails to round-trip is gone
    // after a restart.

    #[test]
    fn snapshot_restores_every_value_type() {
        let s = KeyValueStore::new();
        s.execute(Command::Set(
            "str".into(),
            "v".into(),
            SetOptions::default(),
        ));
        s.execute(Command::HSet("h".into(), vec![("f".into(), "v".into())]));
        s.execute(Command::RPush("l".into(), vec!["a".into(), "b".into()]));
        s.execute(Command::SAdd("st".into(), vec!["m".into()]));
        s.execute(Command::ZAdd(
            "z".into(),
            Default::default(),
            vec![(1.5, "m".into())],
        ));
        s.execute(Command::JSet("j".into(), "$".into(), "{\"a\":1}".into()));

        let restored = KeyValueStore::new();
        restored.restore(s.snapshot());

        assert_eq!(restored.execute(Command::Get("str".into())), bulk("v"));
        assert_eq!(
            restored.execute(Command::HGet("h".into(), "f".into())),
            bulk("v")
        );
        assert_eq!(
            restored.execute(Command::LLen("l".into())),
            Value::Integer(2)
        );
        assert_eq!(
            restored.execute(Command::SIsMember("st".into(), "m".into())),
            Value::Integer(1)
        );
        assert_eq!(
            restored.execute(Command::ZScore("z".into(), "m".into())),
            bulk("1.5")
        );
        assert_eq!(
            restored.execute(Command::JGet("j".into(), None)),
            bulk("{\"a\":1}")
        );
    }

    #[test]
    fn snapshot_preserves_ttls_rather_than_making_keys_permanent() {
        let s = KeyValueStore::new();
        s.execute(Command::Set(
            "k".into(),
            "v".into(),
            SetOptions {
                expiry: Some(SetExpiry::Ex(600)),
                ..Default::default()
            },
        ));
        let restored = KeyValueStore::new();
        restored.restore(s.snapshot());
        assert!(
            matches!(restored.execute(Command::Ttl("k".into())), Value::Integer(n) if n > 0),
            "a restored key must keep its expiry, not become permanent"
        );
    }
}

#[cfg(test)]
mod ephemeral_tests {
    use super::*;
    use crate::cmd::Command;

    #[test]
    fn eset_stores_a_value_like_set() {
        // To the engine an ephemeral key is an ordinary string — lifetime is
        // enforced by the server, which is the layer that knows about
        // connections. Keeping the engine unaware is what keeps it I/O-free
        // and identical between native and wasm builds.
        let s = KeyValueStore::new();
        assert_eq!(
            s.execute(Command::ESet("presence:1".into(), "online".into())),
            Value::SimpleString("OK".into())
        );
        assert_eq!(
            s.execute(Command::Get("presence:1".into())),
            Value::BulkString(Some(b"online".to_vec()))
        );
        assert_eq!(
            s.execute(Command::Type("presence:1".into())),
            Value::SimpleString("string".into())
        );
        // No TTL — the engine must not invent one.
        assert_eq!(
            s.execute(Command::Ttl("presence:1".into())),
            Value::Integer(-1)
        );
    }

    #[test]
    fn eset_overwrites_and_is_visible_to_reads_and_live_queries() {
        let s = KeyValueStore::new();
        s.execute(Command::ESet("presence:1".into(), "first".into()));
        s.execute(Command::ESet("presence:1".into(), "second".into()));
        assert_eq!(
            s.get_current("presence:1"),
            Value::BulkString(Some(b"second".to_vec()))
        );
        // Pattern matching picks it up like any other key.
        let matched = s.matching_key_values("presence:*", 10);
        assert_eq!(matched.len(), 1);
    }
}

#[cfg(test)]
mod metrics_tests {
    use super::*;
    use crate::cmd::{Command, SetOptions};

    #[test]
    fn key_count_changes_only_when_active_expiry_removes_the_entry() {
        // Reads and metrics must not silently consume an expiry event before
        // the server's watcher-aware background task can announce it.
        let s = KeyValueStore::new();
        s.execute(Command::Set(
            "live".into(),
            "v".into(),
            SetOptions::default(),
        ));
        s.execute(Command::Set(
            "dead".into(),
            "v".into(),
            SetOptions {
                expiry: Some(crate::cmd::SetExpiry::Px(1)),
                ..Default::default()
            },
        ));
        std::thread::sleep(std::time::Duration::from_millis(15));
        assert_eq!(
            s.key_count(),
            2,
            "counting must not remove the expired entry"
        );
        assert_eq!(s.sweep_expired_reporting_budget(1), vec!["dead"]);
        assert_eq!(s.key_count(), 1);
    }

    #[test]
    fn eviction_counter_starts_at_zero_and_counts_each_eviction() {
        let s = KeyValueStore::with_config(Some(2), None, EvictionPolicy::AllKeysRandom);
        assert_eq!(s.evicted_count(), 0);

        for i in 0..5 {
            s.execute(Command::Set(
                format!("k{i}"),
                "v".into(),
                SetOptions::default(),
            ));
        }
        // Cap of 2 with 5 inserts means 3 evictions were required.
        assert_eq!(s.key_count(), 2);
        assert_eq!(
            s.evicted_count(),
            3,
            "eviction rate is the signal that a cache is thrashing at its cap"
        );
    }

    #[test]
    fn eviction_counter_stays_zero_without_pressure() {
        let s = KeyValueStore::new();
        for i in 0..10 {
            s.execute(Command::Set(
                format!("k{i}"),
                "v".into(),
                SetOptions::default(),
            ));
        }
        assert_eq!(s.evicted_count(), 0, "no cap configured, nothing to evict");
    }

    #[test]
    fn memory_estimate_tracks_the_stored_data() {
        let s = KeyValueStore::new();
        let empty = s.approximate_memory_bytes();
        s.execute(Command::Set(
            "k".into(),
            "x".repeat(4096).into(),
            SetOptions::default(),
        ));
        assert!(s.approximate_memory_bytes() >= empty + 4096);
    }

    #[test]
    fn memory_usage_reports_a_key_and_nil_for_one_that_is_not_there() {
        let s = KeyValueStore::new();
        // Redis answers a missing key with a nil bulk string, not zero: "no
        // such key" and "an empty key" are different facts.
        assert_eq!(
            s.execute(Command::MemoryUsage("ghost".into())),
            Value::BulkString(None)
        );

        // Equal-length key names, so the only difference the reply can reflect
        // is the value.
        s.execute(Command::Set("k1".into(), "x".into(), SetOptions::default()));
        s.execute(Command::Set(
            "k2".into(),
            "x".repeat(4096).into(),
            SetOptions::default(),
        ));

        let (Value::Integer(small), Value::Integer(big)) = (
            s.execute(Command::MemoryUsage("k1".into())),
            s.execute(Command::MemoryUsage("k2".into())),
        ) else {
            panic!("MEMORY USAGE should report an integer for a live key");
        };
        assert!(small > 0, "a stored key costs something");
        // Exact, not "bigger": the whole point of the reply is that the number
        // moves with the value by the amount the value grew (4096 bytes minus
        // the one byte the small key already held).
        assert_eq!(
            big - small,
            4095,
            "the reported size must track the value: {small} vs {big}"
        );
    }

    #[test]
    fn memory_usage_agrees_with_what_eviction_bills_the_key() {
        // The point of reusing `entry_size` rather than writing a second
        // estimator: the answer to "what is this key costing me" and the number
        // eviction acts on cannot drift apart.
        let s = KeyValueStore::new();
        let empty = s.approximate_memory_bytes();
        s.execute(Command::HSet(
            "h".into(),
            vec![("f".into(), "y".repeat(1024).into())],
        ));
        let Value::Integer(reported) = s.execute(Command::MemoryUsage("h".into())) else {
            panic!("expected an integer");
        };
        assert_eq!(
            s.approximate_memory_bytes() - empty,
            reported as usize,
            "MEMORY USAGE must be the same measurement the eviction loop uses"
        );
    }

    #[test]
    fn memory_usage_treats_an_expired_key_as_absent() {
        let s = KeyValueStore::new();
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        s.execute(Command::PExpire("k".into(), 1));
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(
            s.execute(Command::MemoryUsage("k".into())),
            Value::BulkString(None),
            "a key past its TTL is gone, and its footprint with it"
        );
    }
}

#[cfg(test)]
mod rate_limiter_memory_tests {
    use super::*;
    use crate::cmd::Command;

    fn rl(v: Value) -> (i64, u64, u64) {
        match v {
            Value::Array(Some(items)) => match (&items[0], &items[1], &items[2]) {
                (Value::Integer(a), Value::Integer(r), Value::Integer(w)) => {
                    (*a, *r as u64, *w as u64)
                }
                _ => panic!("unexpected RLCHECK reply shape"),
            },
            other => panic!("expected array, got {other:?}"),
        }
    }

    #[test]
    fn memory_is_bounded_regardless_of_limit() {
        // The reason for bucketing: one timestamp per attempt meant ~800 KB for
        // a single `RLSET key 100000 3600` limiter. Buckets cap it at ~1 KB.
        let s = KeyValueStore::new();
        s.execute(Command::RlSet("big".into(), 100_000, 3600));
        for _ in 0..5_000 {
            s.execute(Command::RlCheck("big".into(), None));
        }
        let bytes = s.approximate_memory_bytes();
        assert!(
            bytes < 4_096,
            "5000 attempts against a 100k limiter should stay small, got {bytes} bytes"
        );
    }

    #[test]
    fn a_high_limit_still_admits_every_attempt_under_it() {
        // Bounding memory must not bound throughput.
        let s = KeyValueStore::new();
        s.execute(Command::RlSet("api".into(), 10_000, 3600));
        for i in 0..2_000 {
            let (allowed, _, _) = rl(s.execute(Command::RlCheck("api".into(), None)));
            assert_eq!(allowed, 1, "attempt {i} should be allowed");
        }
    }

    #[test]
    fn the_limit_is_still_enforced_exactly_at_the_boundary() {
        // Bucketing approximates *when* attempts age out, never *how many* are
        // counted inside the window.
        let s = KeyValueStore::new();
        s.execute(Command::RlSet("api".into(), 5, 60));
        for expected in [4, 3, 2, 1, 0] {
            let (allowed, remaining, retry) = rl(s.execute(Command::RlCheck("api".into(), None)));
            assert_eq!(allowed, 1);
            assert_eq!(remaining, expected);
            assert_eq!(retry, 0);
        }
        let (allowed, remaining, retry) = rl(s.execute(Command::RlCheck("api".into(), None)));
        assert_eq!(allowed, 0, "the 6th attempt against a limit of 5 is denied");
        assert_eq!(remaining, 0);
        assert!(retry > 0, "a denied attempt must say when to retry");
    }

    #[test]
    fn retry_after_never_exceeds_the_window() {
        // Retry-After is handed straight to HTTP clients; a value past the
        // window would park them longer than the policy requires.
        let s = KeyValueStore::new();
        s.execute(Command::RlSet("api".into(), 1, 60));
        s.execute(Command::RlCheck("api".into(), None));
        let (_, _, retry) = rl(s.execute(Command::RlCheck("api".into(), None)));
        assert!(
            retry > 0 && retry <= 60_000,
            "retry_after_ms = {retry}, window is 60000"
        );
    }

    #[test]
    fn the_shortest_window_recovers_after_it_elapses() {
        // RLSET takes the window in *seconds*, so one second is the floor. At
        // that width each bucket is ~15 ms; the bucket-width floor of 1 ms
        // exists so an even shorter window could never divide to zero.
        let s = KeyValueStore::new();
        s.execute(Command::RlSet("fast".into(), 2, 1));
        assert_eq!(rl(s.execute(Command::RlCheck("fast".into(), None))).0, 1);
        assert_eq!(rl(s.execute(Command::RlCheck("fast".into(), None))).0, 1);
        assert_eq!(
            rl(s.execute(Command::RlCheck("fast".into(), None))).0,
            0,
            "third attempt against a limit of 2 is denied"
        );

        // Past the window the limiter admits traffic again.
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        assert_eq!(
            rl(s.execute(Command::RlCheck("fast".into(), None))).0,
            1,
            "buckets older than the window must age out"
        );
    }
}

// ── Byte transparency ─────────────────────────────────────────────────────────
//
// Values are byte-transparent; identifiers are text. A stored value may be
// arbitrary bytes — compressed blobs, protobuf, images — and must come back
// exactly as it went in. Keys, hash fields, set and sorted-set members and glob
// patterns are looked up and matched as text, so a non-UTF-8 one is refused
// rather than lossily converted.
//
// These live in-crate rather than in `tests/`: a separate integration binary
// links its own copy of every function into the coverage map, and the copies it
// does not exercise drag the measured figure down without changing what is
// actually tested.
#[cfg(test)]
mod byte_transparency_tests {
    use super::*;
    use crate::cmd::Command;
    use crate::resp::Value;

    /// Invalid UTF-8 in any position: a lone continuation byte and a truncated
    /// sequence, plus an embedded NUL and a byte that is legal only inside one.
    const BINARY: &[u8] = &[0xff, 0xfe, 0x00, 0x41, 0x80, 0xc3];

    fn parse(raw: &[u8]) -> Result<Command, String> {
        let (v, _) = Value::parse(raw).unwrap();
        Command::from_value(v)
    }

    /// Build a RESP array frame from raw byte arguments.
    fn frame(args: &[&[u8]]) -> Vec<u8> {
        let mut out = format!("*{}\r\n", args.len()).into_bytes();
        for a in args {
            out.extend_from_slice(format!("${}\r\n", a.len()).as_bytes());
            out.extend_from_slice(a);
            out.extend_from_slice(b"\r\n");
        }
        out
    }

    #[test]
    fn a_binary_value_round_trips_byte_for_byte() {
        let store = KeyValueStore::new();
        store.execute(parse(&frame(&[b"SET", b"k", BINARY])).unwrap());

        let Value::BulkString(Some(got)) = store.execute(Command::Get("k".into())) else {
            panic!("key missing after SET");
        };
        assert_eq!(got, BINARY, "value must survive unchanged");
    }

    #[test]
    fn binary_values_work_in_lists_and_hashes() {
        let store = KeyValueStore::new();

        store.execute(parse(&frame(&[b"RPUSH", b"l", BINARY, b"plain"])).unwrap());
        let Value::Array(Some(items)) = store.execute(Command::LRange("l".into(), 0, -1)) else {
            panic!("list missing");
        };
        assert_eq!(items[0], Value::BulkString(Some(BINARY.to_vec())));

        store.execute(parse(&frame(&[b"HSET", b"h", b"f", BINARY])).unwrap());
        assert_eq!(
            store.execute(Command::HGet("h".into(), "f".into())),
            Value::BulkString(Some(BINARY.to_vec()))
        );
    }

    #[test]
    fn append_concatenates_bytes_rather_than_text() {
        let store = KeyValueStore::new();
        store.execute(parse(&frame(&[b"SET", b"k", BINARY])).unwrap());
        store.execute(parse(&frame(&[b"APPEND", b"k", BINARY])).unwrap());

        let Value::BulkString(Some(got)) = store.execute(Command::Get("k".into())) else {
            panic!("key missing");
        };
        assert_eq!(got.len(), BINARY.len() * 2);
        assert_eq!(&got[..BINARY.len()], BINARY);
        assert_eq!(&got[BINARY.len()..], BINARY);
    }

    #[test]
    fn strlen_counts_bytes_not_characters() {
        let store = KeyValueStore::new();
        store.execute(parse(&frame(&[b"SET", b"k", BINARY])).unwrap());
        assert_eq!(
            store.execute(Command::Strlen("k".into())),
            Value::Integer(BINARY.len() as i64)
        );
    }

    #[test]
    fn incr_on_a_binary_value_errors_like_any_non_numeric_value() {
        // The bytes are stored faithfully; they are simply not a number. This must
        // read as a type error, not as corruption.
        let store = KeyValueStore::new();
        store.execute(parse(&frame(&[b"SET", b"k", BINARY])).unwrap());
        let Value::Error(e) = store.execute(Command::Incr("k".into())) else {
            panic!("INCR on binary must error");
        };
        assert!(e.contains("not an integer"), "got {e:?}");
    }

    #[test]
    fn a_binary_key_is_rejected() {
        // Keys are matched by glob and checked against sync scopes as text, so a
        // corrupted key would be silently unretrievable.
        let err = parse(&frame(&[b"SET", BINARY, b"v"])).expect_err("binary key must be refused");
        assert!(err.starts_with("ERR "), "{err:?}");
        assert!(err.contains("must be text"), "{err:?}");
    }

    #[test]
    fn binary_fields_and_members_are_rejected() {
        for args in [
            vec![b"HSET".as_slice(), b"h", BINARY, b"v"], // hash field
            vec![b"SADD".as_slice(), b"s", BINARY],       // set member
            vec![b"ZADD".as_slice(), b"z", b"1", BINARY], // zset member
            vec![b"KEYS".as_slice(), BINARY],             // glob pattern
        ] {
            let err = parse(&frame(&args)).expect_err("identifier must be refused");
            assert!(err.contains("must be text"), "{:?} -> {err:?}", args[0]);
        }
    }

    #[test]
    fn the_error_names_which_argument_was_bad() {
        // MSET k1 v1 <binary-key> v2 — index 3 is a key, so it is refused.
        let err =
            parse(&frame(&[b"MSET", b"k1", b"v1", BINARY, b"v2"])).expect_err("must be refused");
        assert!(err.contains("argument 3"), "got {err:?}");
    }

    #[test]
    fn a_rejected_command_stores_nothing() {
        let store = KeyValueStore::new();
        assert!(parse(&frame(&[b"SET", BINARY, b"v"])).is_err());
        assert_eq!(store.execute(Command::DbSize), Value::Integer(0));
    }

    #[test]
    fn utf8_values_survive_unchanged() {
        let store = KeyValueStore::new();
        for value in ["héllo ✓", "日本語", "\u{1F600}", "", "plain"] {
            store.execute(Command::Set("k".into(), value.into(), Default::default()));
            let Value::BulkString(Some(got)) = store.execute(Command::Get("k".into())) else {
                panic!("key missing after SET of {value:?}");
            };
            assert_eq!(String::from_utf8(got).unwrap(), value);
        }
    }
}

// ── Snapshot compatibility ────────────────────────────────────────────────────
//
// `SnapshotValue` held `String` up to 0.2.1 and holds `Blob` from 0.2.2.
// rmp-serde encodes those differently — msgpack `str` versus `bin` — so `Blob`'s
// deserializer accepts either. Without that, upgrading a server would silently
// start from an empty cache, or fail to boot.
#[cfg(test)]
mod snapshot_compat_tests {
    // `super::*` already brings Command, Value and the snapshot types into
    // scope from the store module's own imports.
    use super::*;
    use std::collections::HashMap;

    /// Mirror of the pre-0.2.2 `SnapshotValue`, used to produce a genuine old-format
    /// payload rather than a hand-rolled byte string. Variant order matters:
    /// rmp-serde encodes variants by index.
    #[derive(Serialize)]
    #[allow(dead_code)] // variants exist to fix the discriminant order, not to be built
    enum LegacySnapshotValue {
        Str(String),
        Hash(HashMap<String, String>),
        List(Vec<String>),
        Set(Vec<String>),
        ZSet(Vec<(String, f64)>),
        RateLimiter {
            limit: u64,
            window_ms: u64,
            events: Vec<u64>,
        },
        Json(String),
    }

    #[derive(Serialize)]
    struct LegacyEntry {
        key: String,
        value: LegacySnapshotValue,
        expires_at_ms: Option<u64>,
    }

    #[test]
    fn a_pre_0_2_2_snapshot_still_restores() {
        let legacy = vec![
            LegacyEntry {
                key: "s".into(),
                value: LegacySnapshotValue::Str("hello".into()),
                expires_at_ms: None,
            },
            LegacyEntry {
                key: "l".into(),
                value: LegacySnapshotValue::List(vec!["a".into(), "b".into()]),
                expires_at_ms: None,
            },
            LegacyEntry {
                key: "h".into(),
                value: LegacySnapshotValue::Hash(HashMap::from([(
                    "f".to_string(),
                    "v".to_string(),
                )])),
                expires_at_ms: None,
            },
        ];
        let bytes = rmp_serde::to_vec(&legacy).expect("legacy snapshot must encode");

        // Decode with the *current* types — this is what a restarted server does.
        let entries: Vec<SnapshotEntry> =
            rmp_serde::from_slice(&bytes).expect("a pre-0.2.2 snapshot must still decode");

        let store = KeyValueStore::new();
        store.restore(entries);

        use crate::{cmd::Command, resp::Value};
        assert_eq!(
            store.execute(Command::Get("s".into())),
            Value::BulkString(Some(b"hello".to_vec()))
        );
        assert_eq!(
            store.execute(Command::HGet("h".into(), "f".into())),
            Value::BulkString(Some(b"v".to_vec()))
        );
        assert_eq!(store.execute(Command::LLen("l".into())), Value::Integer(2));
    }

    #[test]
    fn binary_values_survive_a_snapshot_round_trip() {
        let binary = vec![0xff, 0xfe, 0x00, 0x41, 0x80];
        let store = KeyValueStore::new();
        store.restore(vec![SnapshotEntry {
            key: "b".into(),
            value: SnapshotValue::Str(binary.clone().into()),
            expires_at_ms: None,
        }]);

        let bytes = rmp_serde::to_vec(&store.snapshot()).unwrap();
        let entries: Vec<SnapshotEntry> = rmp_serde::from_slice(&bytes).unwrap();

        let restored = KeyValueStore::new();
        restored.restore(entries);

        use crate::{cmd::Command, resp::Value};
        assert_eq!(
            restored.execute(Command::Get("b".into())),
            Value::BulkString(Some(binary)),
            "binary must survive snapshot and restore"
        );
    }

    #[test]
    fn a_binary_value_encodes_as_msgpack_bin_not_an_int_array() {
        // Vec<u8> serializes as an array of integers by default, which would roughly
        // double snapshot size for binary payloads. Blob emits a compact `bin`.
        let store = KeyValueStore::new();
        store.restore(vec![SnapshotEntry {
            key: "b".into(),
            value: SnapshotValue::Str(vec![0xffu8; 1000].into()),
            expires_at_ms: None,
        }]);
        let bytes = rmp_serde::to_vec(&store.snapshot()).unwrap();
        assert!(
            bytes.len() < 1200,
            "1000 bytes encoded to {} — likely an int array, not msgpack bin",
            bytes.len()
        );
    }
}

// ── Concurrency regression tests ──────────────────────────────────────────────

/// The mutating commands used to resolve a key's type with one `get()` (whose
/// read guard was then dropped) and write through a second `entry()` guard.
/// Anything that happened in between was invisible, which produced two distinct
/// faults: an `unreachable!()` when the type had changed, and a silent clobber
/// when an expired key had been rewritten. Both are structural — they only
/// appear when two connections touch one key at once — so these tests hammer
/// the same key from several threads and assert the process survives with a
/// coherent result.
#[cfg(test)]
mod type_race_tests {
    use super::*;
    use crate::cmd::{Command, SetOptions};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::thread;

    /// Number of iterations each racing thread runs. High enough to hit the
    /// window reliably on a multi-core machine, low enough to stay quick.
    const ROUNDS: usize = 20_000;

    fn set(store: &KeyValueStore, key: &str, val: &str) -> Value {
        store.execute(Command::Set(key.into(), val.into(), SetOptions::default()))
    }

    /// Every reply must be either the command's normal answer or WRONGTYPE —
    /// never a panic, and never a success that silently did nothing.
    fn assert_sane(v: &Value) {
        if let Value::Error(e) = v {
            assert!(
                e.starts_with("WRONGTYPE") || e.starts_with("ERR"),
                "unexpected error variant: {e}"
            );
        }
    }

    /// `HSET k f v` on a missing key racing `SET k v`: the old code passed its
    /// type check while the key was absent, then found a `Str` behind the
    /// `entry()` guard and hit `unreachable!()`, panicking the connection task.
    #[test]
    fn hset_racing_set_on_the_same_key_never_panics() {
        let store = Arc::new(KeyValueStore::new());
        let stop = Arc::new(AtomicBool::new(false));

        let writer = {
            let store = Arc::clone(&store);
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    set(&store, "k", "v");
                    store.execute(Command::Del(vec!["k".into()]));
                }
            })
        };

        for _ in 0..ROUNDS {
            let r = store.execute(Command::HSet(
                "k".into(),
                vec![("f".to_string(), b"v".to_vec())],
            ));
            assert_sane(&r);
            store.execute(Command::Del(vec!["k".into()]));
        }

        stop.store(true, Ordering::Relaxed);
        writer.join().expect("writer thread panicked");
    }

    /// The same window on the string path: `INCR` used to read the type, drop
    /// the guard, then assume the value was still a string.
    #[test]
    fn incr_racing_hset_on_the_same_key_never_panics() {
        let store = Arc::new(KeyValueStore::new());
        let stop = Arc::new(AtomicBool::new(false));

        let hasher = {
            let store = Arc::clone(&store);
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    store.execute(Command::Del(vec!["n".into()]));
                    store.execute(Command::HSet(
                        "n".into(),
                        vec![("f".to_string(), b"1".to_vec())],
                    ));
                }
            })
        };

        for _ in 0..ROUNDS {
            assert_sane(&store.execute(Command::Incr("n".into())));
        }

        stop.store(true, Ordering::Relaxed);
        hasher.join().expect("hasher thread panicked");
    }

    /// Same shape again across the list, set and zset constructors, which each
    /// had their own copy of the pattern.
    #[test]
    fn collection_writes_racing_a_string_write_never_panic() {
        let store = Arc::new(KeyValueStore::new());
        let stop = Arc::new(AtomicBool::new(false));

        let writer = {
            let store = Arc::clone(&store);
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    set(&store, "c", "v");
                    store.execute(Command::Del(vec!["c".into()]));
                }
            })
        };

        for _ in 0..ROUNDS {
            assert_sane(&store.execute(Command::LPush("c".into(), vec!["v".into()])));
            assert_sane(&store.execute(Command::SAdd("c".into(), vec!["m".into()])));
            assert_sane(&store.execute(Command::ZAdd(
                "c".into(),
                ZAddOptions::default(),
                vec![(1.0, "m".to_string())],
            )));
            assert_sane(&store.execute(Command::Append("c".into(), "x".into())));
            store.execute(Command::Del(vec!["c".into()]));
        }

        stop.store(true, Ordering::Relaxed);
        writer.join().expect("writer thread panicked");
    }

    /// A wrong-typed key must still be reported as WRONGTYPE now that the check
    /// happens under the write guard rather than ahead of it — and the command
    /// must not have modified anything on the way out.
    #[test]
    fn wrongtype_is_reported_and_leaves_the_value_untouched() {
        let store = KeyValueStore::new();
        set(&store, "s", "hello");

        for cmd in [
            Command::HSet("s".into(), vec![("f".to_string(), b"v".to_vec())]),
            Command::HSetNx("s".into(), "f".into(), b"v".to_vec()),
            Command::LPush("s".into(), vec!["v".into()]),
            Command::RPush("s".into(), vec!["v".into()]),
            Command::SAdd("s".into(), vec!["m".into()]),
            Command::ZAdd(
                "s".into(),
                ZAddOptions::default(),
                vec![(1.0, "m".to_string())],
            ),
            Command::ZIncrBy("s".into(), 1.0, "m".into()),
            Command::HIncrBy("s".into(), "f".into(), 1),
            Command::RlSet("s".into(), 10, 60),
        ] {
            let label = format!("{cmd:?}");
            match store.execute(cmd) {
                Value::Error(e) => assert!(e.starts_with("WRONGTYPE"), "{label}: got {e}"),
                other => panic!("{label}: expected WRONGTYPE, got {other:?}"),
            }
        }

        // The string is intact: no command created, reset or partially wrote it.
        assert_eq!(
            store.execute(Command::Get("s".into())),
            Value::BulkString(Some(b"hello".to_vec()))
        );
        assert_eq!(
            store.execute(Command::Type("s".into())),
            Value::SimpleString("string".to_string())
        );
    }

    /// An expired key is reset to the fresh collection under the same guard, so
    /// a command that creates over an expired key behaves as if the key was
    /// absent — the ordinary single-threaded contract the race broke.
    #[test]
    fn an_expired_key_is_replaced_not_appended_to() {
        let store = KeyValueStore::new();
        store.execute(Command::HSet(
            "h".into(),
            vec![("old".to_string(), b"1".to_vec())],
        ));
        // Expire it in the past.
        store.execute(Command::PExpireAt("h".into(), 1));

        assert_eq!(
            store.execute(Command::HSet(
                "h".into(),
                vec![("new".to_string(), b"2".to_vec())]
            )),
            Value::Integer(1),
            "the field must count as new — the expired hash is gone"
        );
        assert_eq!(store.execute(Command::HLen("h".into())), Value::Integer(1));
        assert_eq!(
            store.execute(Command::HGet("h".into(), "old".into())),
            Value::BulkString(None),
            "a field from the expired generation must not survive"
        );
    }

    /// `SMOVE` used to check the destination's type, then insert through a
    /// second guard; if the destination had been retyped in between, the member
    /// was dropped on the floor and the reply still said `1`. It now reports
    /// WRONGTYPE and returns the member to the source.
    #[test]
    fn smove_to_a_wrongtyped_destination_does_not_lose_the_member() {
        let store = KeyValueStore::new();
        store.execute(Command::SAdd("src".into(), vec!["m".into()]));
        set(&store, "dst", "not-a-set");

        match store.execute(Command::SMove("src".into(), "dst".into(), "m".into())) {
            Value::Error(e) => assert!(e.starts_with("WRONGTYPE"), "got {e}"),
            other => panic!("expected WRONGTYPE, got {other:?}"),
        }
        assert_eq!(
            store.execute(Command::SIsMember("src".into(), "m".into())),
            Value::Integer(1),
            "member must still be in the source after a refused move"
        );
    }
}

// ── Eviction sampling ─────────────────────────────────────────────────────────

#[cfg(test)]
mod eviction_sampling_tests {
    use super::*;
    use crate::cmd::{Command, SetOptions};

    fn fill(store: &KeyValueStore, n: usize) {
        for i in 0..n {
            store.execute(Command::Set(
                format!("k{i}"),
                format!("v{i}").into_bytes(),
                SetOptions::default(),
            ));
        }
    }

    #[test]
    fn execute_reports_implicit_capacity_evictions() {
        let store = KeyValueStore::with_config(Some(1), None, EvictionPolicy::AllKeysRandom);
        assert_eq!(
            store.execute(Command::Set(
                "victim".into(),
                "old".into(),
                SetOptions::default(),
            )),
            Value::SimpleString("OK".into())
        );

        let (response, evicted) = store.execute_reporting(Command::Set(
            "replacement".into(),
            "new".into(),
            SetOptions::default(),
        ));
        assert_eq!(response, Value::SimpleString("OK".into()));
        assert_eq!(evicted, vec!["victim"]);
        assert_eq!(
            store.execute(Command::Get("victim".into())),
            Value::BulkString(None)
        );
        assert_eq!(
            store.execute(Command::Get("replacement".into())),
            Value::BulkString(Some(b"new".to_vec()))
        );
    }

    /// The sampler must still pick a victim under every policy, and must pick
    /// the least-recently-used one when the whole keyspace fits in the sample.
    #[test]
    fn lru_evicts_the_oldest_key_when_the_sample_covers_the_keyspace() {
        // No memory cap: `evict_one` is driven directly here, and a cap would make
        // the write path evict during `fill` before the assertions run.
        let mut store = KeyValueStore::with_config(None, None, EvictionPolicy::AllKeysLru);
        store.eviction_sample = 100;
        fill(&store, 5);

        // Touch everything except k0, making it the oldest.
        let now = now_ms() + 1_000;
        for i in 1..5 {
            if let Some(e) = store.data.get(&format!("k{i}")) {
                e.touch(now);
            }
        }

        assert_eq!(
            store.evict_one(now),
            Some(entry_size("k0", &Entry::new_str("v0")))
        );
        assert!(!store.data.contains_key("k0"), "oldest key should be gone");
        assert_eq!(store.key_count(), 4);
    }

    /// A reservoir of `want` must never return more than `want` candidates'
    /// worth of work, and must still return *something* while eligible keys
    /// exist — the bug being guarded against is a rewrite that samples nothing.
    #[test]
    fn every_policy_finds_a_victim_while_eligible_keys_remain() {
        for policy in [
            EvictionPolicy::AllKeysLru,
            EvictionPolicy::AllKeysRandom,
            EvictionPolicy::VolatileLru,
            EvictionPolicy::VolatileTtl,
        ] {
            let store = KeyValueStore::with_config(None, None, policy);
            fill(&store, 50);
            let volatile = matches!(
                policy,
                EvictionPolicy::VolatileLru | EvictionPolicy::VolatileTtl
            );
            if volatile {
                // Volatile policies only consider keys carrying a TTL.
                for i in 0..50 {
                    store.execute(Command::PExpireAt(format!("k{i}"), now_ms() + 600_000));
                }
            }
            let before = store.key_count();
            assert!(
                store.evict_one(now_ms()).is_some(),
                "{policy:?} found no victim among {before} eligible keys"
            );
            assert_eq!(store.key_count(), before - 1, "{policy:?}");
        }
    }

    /// Volatile policies must leave TTL-less keys alone even though the walk
    /// now filters inside the sampler rather than in an iterator adaptor.
    #[test]
    fn volatile_policies_never_evict_a_key_without_a_ttl() {
        for policy in [EvictionPolicy::VolatileLru, EvictionPolicy::VolatileTtl] {
            let store = KeyValueStore::with_config(None, None, policy);
            fill(&store, 20);
            assert_eq!(
                store.evict_one(now_ms()),
                None,
                "{policy:?} evicted a key with no TTL"
            );
            assert_eq!(store.key_count(), 20, "{policy:?}");
        }
    }

    /// `AllKeysRandom` samples a reservoir of one. Over many runs it must reach
    /// more than a single key, or the rewrite has silently pinned it to
    /// whichever key the iterator happens to yield first.
    #[test]
    fn random_eviction_is_not_biased_to_one_key() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..40 {
            let store = KeyValueStore::with_config(None, None, EvictionPolicy::AllKeysRandom);
            fill(&store, 20);
            let before: std::collections::HashSet<String> =
                store.data.iter().map(|r| r.key().clone()).collect();
            store.evict_one(now_ms());
            let after: std::collections::HashSet<String> =
                store.data.iter().map(|r| r.key().clone()).collect();
            seen.extend(before.difference(&after).cloned());
        }
        assert!(
            seen.len() > 1,
            "40 random evictions only ever removed {seen:?}"
        );
    }

    /// `try_evict_for_memory` drives `evict_one` in a loop; it must converge
    /// rather than spin now that the sampler is a hand-rolled reservoir.
    #[test]
    fn memory_eviction_converges_under_the_limit() {
        let store = KeyValueStore::with_config(None, Some(4096), EvictionPolicy::AllKeysLru);
        fill(&store, 500);
        assert!(store.try_evict_for_memory());
        assert!(
            store.approximate_memory_bytes() <= 4096,
            "still {} bytes after eviction",
            store.approximate_memory_bytes()
        );
    }
}

// ── Write-path memory enforcement ─────────────────────────────────────────────

/// `max_memory_bytes` used to be enforced only by the server's background
/// sweep, so between two ticks a client could write past the cap without limit
/// — while `max_keys`, checked inline on every write, held. These tests never
/// call `try_evict_for_memory` themselves: if the cap holds, it held because
/// the write path enforced it.
#[cfg(test)]
mod write_path_memory_tests {
    use super::*;
    use crate::cmd::{Command, SetOptions};

    fn set(store: &KeyValueStore, key: &str, val: &[u8]) -> Value {
        store.execute(Command::Set(
            key.into(),
            val.to_vec(),
            SetOptions::default(),
        ))
    }

    const LIMIT: usize = 64 * 1024;

    #[test]
    fn a_burst_of_writes_cannot_run_past_the_cap_between_sweeps() {
        let store = KeyValueStore::with_config(None, Some(LIMIT), EvictionPolicy::AllKeysLru);
        let value = vec![b'x'; 1024];
        for i in 0..2_000 {
            set(&store, &format!("k{i}"), &value);
        }
        // No sweep has run. Two megabytes went in against a 64 KB cap.
        let used = store.approximate_memory_bytes();
        assert!(
            used <= LIMIT,
            "{used} bytes held against a {LIMIT} byte cap — the cap did not apply on the write path"
        );
    }

    #[test]
    fn one_oversized_value_is_caught_immediately() {
        let store = KeyValueStore::with_config(None, Some(LIMIT), EvictionPolicy::AllKeysLru);
        set(&store, "filler", &vec![b'x'; LIMIT / 2]);
        set(&store, "big", &vec![b'y'; LIMIT]);
        let used = store.approximate_memory_bytes();
        assert!(used <= LIMIT, "{used} bytes against a {LIMIT} byte cap");
    }

    /// Every payload-carrying write must be accounted for, not just `SET`.
    #[test]
    fn collection_writes_are_accounted_for_too() {
        let chunk = vec![b'z'; 512];
        for (label, mut write) in [
            (
                "rpush",
                Box::new(|s: &KeyValueStore, i: usize, v: &[u8]| {
                    s.execute(Command::RPush(format!("l{i}"), vec![v.to_vec()]));
                }) as Box<dyn FnMut(&KeyValueStore, usize, &[u8])>,
            ),
            (
                "hset",
                Box::new(|s: &KeyValueStore, i: usize, v: &[u8]| {
                    s.execute(Command::HSet(
                        format!("h{i}"),
                        vec![("f".to_string(), v.to_vec())],
                    ));
                }),
            ),
            (
                "sadd",
                Box::new(|s: &KeyValueStore, i: usize, v: &[u8]| {
                    s.execute(Command::SAdd(
                        format!("s{i}"),
                        vec![String::from_utf8_lossy(v).into_owned()],
                    ));
                }),
            ),
            (
                "zadd",
                Box::new(|s: &KeyValueStore, i: usize, v: &[u8]| {
                    s.execute(Command::ZAdd(
                        format!("z{i}"),
                        ZAddOptions::default(),
                        vec![(1.0, String::from_utf8_lossy(v).into_owned())],
                    ));
                }),
            ),
        ] {
            let store = KeyValueStore::with_config(None, Some(LIMIT), EvictionPolicy::AllKeysLru);
            for i in 0..1_000 {
                write(&store, i, &chunk);
            }
            let used = store.approximate_memory_bytes();
            assert!(used <= LIMIT, "{label}: {used} bytes against a {LIMIT} cap");
        }
    }

    /// With no cap configured the gate must stay entirely out of the way — no
    /// eviction, no keyspace walks.
    #[test]
    fn an_unlimited_store_never_evicts() {
        let store = KeyValueStore::new();
        for i in 0..500 {
            set(&store, &format!("k{i}"), b"v");
        }
        assert_eq!(store.key_count(), 500);
        assert_eq!(store.evicted_count(), 0);
    }

    /// A store whose policy cannot free anything refuses the write and restores
    /// the previous value rather than silently growing beyond the cap.
    #[test]
    fn no_eviction_policy_refuses_growth_past_the_memory_cap() {
        let store = KeyValueStore::with_config(None, Some(LIMIT), EvictionPolicy::NoEviction);
        let value = vec![b'x'; 1024];
        let mut rejected = 0;
        for i in 0..300 {
            if matches!(set(&store, &format!("k{i}"), &value), Value::Error(_)) {
                rejected += 1;
            }
        }
        assert!(rejected > 0, "writes past the cap must be rejected");
        assert_eq!(store.evicted_count(), 0);
        assert!(store.approximate_memory_bytes() <= LIMIT);
    }

    #[test]
    fn no_eviction_rollback_restores_an_existing_value() {
        let store = KeyValueStore::with_config(None, Some(512), EvictionPolicy::NoEviction);
        assert_eq!(set(&store, "k", b"small"), Value::SimpleString("OK".into()));
        let response = set(&store, "k", &vec![b'x'; 2_048]);
        assert!(matches!(response, Value::Error(message) if message.starts_with("OOM")));
        assert_eq!(
            store.get_current("k"),
            Value::BulkString(Some(b"small".to_vec()))
        );
        assert!(store.approximate_memory_bytes() <= 512);
    }

    /// An eviction pass sheds past the cap rather than stopping exactly on it.
    /// Without that headroom the store would sit on the limit and the very next
    /// write would trip the gate again and require another eviction pass.
    ///
    /// Loaded through `restore`, which bypasses the write path, so the store is
    /// genuinely over the cap when the pass starts.
    #[test]
    fn an_eviction_pass_leaves_headroom_below_the_cap() {
        let store = KeyValueStore::with_config(None, Some(LIMIT), EvictionPolicy::AllKeysLru);
        store.restore(
            (0..1_000)
                .map(|i| SnapshotEntry {
                    key: format!("k{i}"),
                    value: SnapshotValue::Str(vec![b'x'; 256].into()),
                    expires_at_ms: None,
                })
                .collect(),
        );
        assert!(
            store.approximate_memory_bytes() > LIMIT,
            "restore should have loaded past the cap"
        );

        assert!(store.try_evict_for_memory());
        let used = store.approximate_memory_bytes();
        let low_water = LIMIT - LIMIT / EVICTION_HEADROOM_DIVISOR;
        assert!(
            used <= low_water,
            "{used} bytes stops at the {LIMIT} byte cap instead of shedding to {low_water}"
        );
    }

    /// Reads must not be charged: a read-only workload against a full store
    /// must not trigger eviction.
    #[test]
    fn reads_are_not_charged_against_the_cap() {
        let store = KeyValueStore::with_config(None, Some(LIMIT), EvictionPolicy::AllKeysLru);
        for i in 0..20 {
            set(&store, &format!("k{i}"), b"v");
        }
        let before = store.evicted_count();
        for _ in 0..10_000 {
            store.execute(Command::Get("k0".into()));
            store.execute(Command::Exists(vec!["k1".into()]));
        }
        assert_eq!(
            store.evicted_count(),
            before,
            "reads triggered eviction — write_cost is charging them"
        );
    }

    #[test]
    fn write_cost_is_zero_for_reads_and_positive_for_writes() {
        assert_eq!(write_cost(&Command::Get("k".into())), 0);
        assert_eq!(write_cost(&Command::LRange("k".into(), 0, -1)), 0);
        assert_eq!(write_cost(&Command::Del(vec!["k".into()])), 0);
        assert!(
            write_cost(&Command::Set(
                "k".into(),
                vec![0u8; 4096],
                SetOptions::default()
            )) >= 4096,
            "a 4 KB SET must be charged at least its payload"
        );
        assert!(write_cost(&Command::Incr("k".into())) > 0);
    }
}

// ── Sorted-set index ──────────────────────────────────────────────────────────

/// Every sorted-set range command used to call `rank_asc`, which collected the
/// whole set into a `Vec` and sorted it — O(n log n) and an allocation
/// proportional to the set, per query, while holding the shard guard.
/// `ZRANGE board 0 9` on a million-member leaderboard sorted a million entries
/// to return ten.
///
/// A `(score, member)` index replaces that. These tests pin the behaviour the
/// index has to preserve exactly, and the cost characteristics that justify it.
#[cfg(test)]
mod zset_index_tests {
    use super::*;
    use crate::cmd::{Command, ZAddOptions};

    fn int(n: i64) -> Value {
        Value::Integer(n)
    }

    fn zset(pairs: &[(f64, &str)]) -> ZSetInner {
        let mut z = ZSetInner::new();
        for (s, m) in pairs {
            z.insert(m, *s);
        }
        z
    }

    fn members(z: &ZSetInner) -> Vec<(&str, f64)> {
        z.iter_asc().collect()
    }

    /// The index must not exist until something asks for an ordering — that is
    /// what keeps a write-only workload at pre-index write cost.
    #[test]
    fn a_write_only_workload_never_builds_the_index() {
        let mut z = ZSetInner::new();
        for i in 0..500 {
            z.insert(&format!("m{i}"), i as f64);
        }
        assert!(
            z.index.get().is_none(),
            "writes alone materialised the ordering, so they are paying for an \
             index nothing has asked for"
        );
        // Point reads must not build it either.
        assert_eq!(z.score("m1"), Some(1.0));
        assert_eq!(z.len(), 500);
        assert!(z.index.get().is_none(), "a point lookup built the index");
    }

    /// Once a range query has built the ordering, writes keep it current rather
    /// than throwing it away — otherwise the very common "update a score, read
    /// the leaderboard" loop rebuilds on every single read.
    #[test]
    fn a_write_maintains_an_ordering_that_is_being_read() {
        let mut z = ZSetInner::new();
        for i in 0..50 {
            z.insert(&format!("m{i}"), i as f64);
        }
        assert!(z.index.get().is_none(), "writes alone must not build it");

        let _ = z.iter_asc().count();
        assert!(
            z.index.get().is_some(),
            "a range query should have built it"
        );

        z.insert("new", 99.0);
        assert!(
            z.index.get().is_some(),
            "the ordering was dropped by a write, so an alternating \
             write/read loop would rebuild it on every read"
        );
        assert_eq!(z.iter_asc().last(), Some(("new", 99.0)));

        // A score change re-keys in place.
        z.insert("m0", 1_000.0);
        assert_eq!(z.iter_asc().last(), Some(("m0", 1_000.0)));
        assert_eq!(z.iter_asc().count(), z.len());
    }

    /// The leftover case: one range query long ago, a flood of writes since.
    /// Maintaining the ordering forever would tax every write on behalf of a
    /// reader that has gone away, so it is abandoned and rebuilt if one returns.
    ///
    /// Scored updates to a stable set, not insertions: the threshold scales
    /// with the set, so a *growing* set never trips it — and correctly so,
    /// since n insertions cost about the same maintained as rebuilt once.
    #[test]
    fn a_write_dominated_set_abandons_an_ordering_nobody_reads() {
        let mut z = ZSetInner::new();
        for i in 0..10 {
            z.insert(&format!("m{i}"), i as f64);
        }
        let _ = z.iter_asc().count();
        assert!(z.index.get().is_some());

        for w in 0..(INDEX_ABANDON_FLOOR + 64) {
            z.insert(&format!("m{}", w % 10), (w + 100) as f64);
        }
        assert!(
            z.index.get().is_none(),
            "ordering survived {} writes with no range query in between",
            INDEX_ABANDON_FLOOR + 64
        );

        // Still correct once someone reads again.
        assert_eq!(z.iter_asc().count(), 10);
        assert_eq!(z.len(), 10);
    }

    /// Re-writing the same score cannot change the ordering, so the index is
    /// worth keeping across that write.
    #[test]
    fn an_unchanged_score_keeps_the_index() {
        let mut z = ZSetInner::new();
        z.insert("a", 1.0);
        let _ = z.iter_asc().count();
        assert!(z.index.get().is_some());
        z.insert("a", 1.0);
        assert!(
            z.index.get().is_some(),
            "a no-op write threw away a still-valid ordering"
        );
    }

    /// A rebuilt index must be indistinguishable from one that was never
    /// dropped — this is the invariant the whole scheme rests on.
    #[test]
    fn a_rebuilt_index_matches_one_built_from_scratch() {
        let mut churned = ZSetInner::new();
        for round in 0..15u64 {
            for i in 0..30u64 {
                churned.insert(&format!("m{i}"), ((round * 5 + i * 11) % 13) as f64 - 6.0);
            }
            // Force a build, then let the next round's writes invalidate it.
            let _ = churned.iter_asc().count();
            if round % 3 == 0 {
                churned.remove(&format!("m{}", round % 30));
            }
        }
        let via_rebuild: Vec<(String, f64)> = churned
            .iter_asc()
            .map(|(m, s)| (m.to_string(), s))
            .collect();

        let fresh = ZSetInner::from_pairs(via_rebuild.clone());
        let via_fresh: Vec<(String, f64)> =
            fresh.iter_asc().map(|(m, s)| (m.to_string(), s)).collect();
        assert_eq!(via_rebuild, via_fresh);
    }

    #[test]
    fn iteration_is_ordered_by_score_then_member() {
        // Ties break on the member name, as Redis does.
        let z = zset(&[(2.0, "b"), (1.0, "z"), (2.0, "a"), (-1.5, "m")]);
        assert_eq!(
            members(&z),
            vec![("m", -1.5), ("z", 1.0), ("a", 2.0), ("b", 2.0)]
        );
    }

    #[test]
    fn updating_a_score_moves_the_member_in_the_index() {
        let mut z = zset(&[(1.0, "a"), (2.0, "b"), (3.0, "c")]);
        assert_eq!(
            z.insert("a", 10.0),
            Some(1.0),
            "should report the old score"
        );
        assert_eq!(members(&z), vec![("b", 2.0), ("c", 3.0), ("a", 10.0)]);
        assert_eq!(z.len(), 3, "an update must not duplicate the member");
    }

    #[test]
    fn re_inserting_the_same_score_is_a_no_op() {
        let mut z = zset(&[(1.0, "a")]);
        assert_eq!(z.insert("a", 1.0), Some(1.0));
        assert_eq!(members(&z), vec![("a", 1.0)]);
    }

    #[test]
    fn removal_clears_both_the_map_and_the_index() {
        let mut z = zset(&[(1.0, "a"), (2.0, "b")]);
        assert_eq!(z.remove("a"), Some(1.0));
        assert_eq!(z.remove("a"), None, "second removal finds nothing");
        assert_eq!(members(&z), vec![("b", 2.0)]);
        assert_eq!(z.score("a"), None);
        assert_eq!(z.rank("a"), None);
    }

    /// The index must never disagree with the map, whatever sequence of writes
    /// it has seen — a stale index entry would surface as a phantom member in
    /// every range query.
    #[test]
    fn the_index_stays_consistent_with_the_map_under_churn() {
        let mut z = ZSetInner::new();
        for round in 0..40u64 {
            for i in 0..25u64 {
                let m = format!("m{}", i);
                // Deterministic but jumbled score sequence.
                let score = ((round * 7 + i * 13) % 17) as f64 - 8.0;
                z.insert(&m, score);
            }
            for i in (0..25u64).step_by(3) {
                z.remove(&format!("m{}", i));
            }
        }

        let ordered = members(&z);
        assert_eq!(ordered.len(), z.len(), "index and map disagree on size");
        for (m, s) in &ordered {
            assert_eq!(z.score(m), Some(*s), "index score for {m} is stale");
        }
        let mut sorted = ordered.clone();
        sorted.sort_by(|(m1, s1), (m2, s2)| s1.total_cmp(s2).then(m1.cmp(m2)));
        assert_eq!(ordered, sorted, "index is not in sorted order");
    }

    /// The index walk stops early rather than testing every member, so it must
    /// be checked against the plain definition it replaced.
    #[test]
    fn range_by_score_agrees_with_a_linear_scan() {
        let z = zset(&[
            (-10.0, "a"),
            (0.0, "b"),
            (0.0, "c"),
            (1.5, "d"),
            (2.0, "e"),
            (99.0, "f"),
        ]);
        let bounds = [
            ("-inf", "+inf"),
            ("0", "2"),
            ("(0", "2"),
            ("0", "(2"),
            ("(0", "(2"),
            ("-inf", "0"),
            ("2", "-inf"),
            ("+inf", "+inf"),
            ("3", "1"),
            ("-10", "-10"),
        ];
        for (min_s, max_s) in bounds {
            let min = ScoreBound::parse(min_s).expect("bound parses");
            let max = ScoreBound::parse(max_s).expect("bound parses");
            let via_index: Vec<(&str, f64)> = z.range_by_score(&min, &max).collect();
            let via_scan: Vec<(&str, f64)> = z
                .iter_asc()
                .filter(|(_, s)| in_score_range(*s, &min, &max))
                .collect();
            assert_eq!(via_index, via_scan, "disagreement for {min_s}..{max_s}");
        }
    }

    #[test]
    fn rank_matches_the_position_in_ascending_order() {
        let z = zset(&[(3.0, "c"), (1.0, "a"), (2.0, "b"), (2.0, "bb")]);
        for (i, (m, _)) in members(&z).into_iter().enumerate() {
            assert_eq!(z.rank(m), Some(i), "rank of {m}");
        }
        assert_eq!(z.rank("missing"), None);
    }

    /// A `NaN` must never reach the index, but if one did the ordering must
    /// still be total — `total_cmp` guarantees that where `partial_cmp` did not.
    #[test]
    fn score_ordering_is_total_even_for_nan() {
        let mut v = [
            Score(f64::NAN),
            Score(1.0),
            Score(f64::NEG_INFINITY),
            Score(f64::INFINITY),
            Score(-0.0),
            Score(0.0),
        ];
        v.sort();
        // Sorting must be deterministic and must not panic; the exact position
        // of NaN is unspecified, only that the order is total.
        assert_eq!(v[0], Score(f64::NEG_INFINITY));
        assert!(v.iter().any(|s| s.0.is_nan()));
    }

    // ── Command-level behaviour ───────────────────────────────────────────────

    fn store_with_leaderboard(n: usize) -> KeyValueStore {
        let store = KeyValueStore::new();
        store.execute(Command::ZAdd(
            "board".into(),
            ZAddOptions::default(),
            (0..n).map(|i| (i as f64, format!("p{i:06}"))).collect(),
        ));
        store
    }

    #[test]
    fn zrange_returns_the_same_window_as_before() {
        let store = store_with_leaderboard(10);
        let top = store.execute(Command::ZRange("board".into(), 0, 2, false));
        assert_eq!(
            top,
            Value::Array(Some(vec![
                Value::BulkString(Some(b"p000000".to_vec())),
                Value::BulkString(Some(b"p000001".to_vec())),
                Value::BulkString(Some(b"p000002".to_vec())),
            ]))
        );
        // Negative indices count from the end.
        let last = store.execute(Command::ZRange("board".into(), -1, -1, false));
        assert_eq!(
            last,
            Value::Array(Some(vec![Value::BulkString(Some(b"p000009".to_vec()))]))
        );
        // An empty window stays empty rather than wrapping.
        assert_eq!(
            store.execute(Command::ZRange("board".into(), 5, 2, false)),
            Value::Array(Some(vec![]))
        );
    }

    #[test]
    fn zrevrange_walks_the_index_backwards() {
        let store = store_with_leaderboard(10);
        assert_eq!(
            store.execute(Command::ZRevRange("board".into(), 0, 1, false)),
            Value::Array(Some(vec![
                Value::BulkString(Some(b"p000009".to_vec())),
                Value::BulkString(Some(b"p000008".to_vec())),
            ]))
        );
    }

    /// The cost claim: a top-N query must not touch the whole set. Measured as
    /// wall-clock ratio between a small and a large leaderboard — sorting the
    /// set would make the large one dramatically slower, walking the index
    /// makes it flat.
    #[test]
    fn a_top_n_query_does_not_scale_with_the_size_of_the_set() {
        use std::time::Instant;

        fn time_top_10(n: usize) -> std::time::Duration {
            let store = store_with_leaderboard(n);
            // Warm up, then measure.
            store.execute(Command::ZRange("board".into(), 0, 9, false));
            let start = Instant::now();
            for _ in 0..200 {
                let r = store.execute(Command::ZRange("board".into(), 0, 9, false));
                std::hint::black_box(&r);
            }
            start.elapsed()
        }

        let small = time_top_10(1_000);
        let large = time_top_10(100_000);
        // 100x the members. Sorting per query would be well over 100x slower;
        // a generous 10x ceiling still fails loudly if the sort comes back,
        // without turning into a flaky timing assertion.
        assert!(
            large < small * 10 + std::time::Duration::from_millis(50),
            "top-10 over 100k members took {large:?} vs {small:?} over 1k — \
             ZRANGE looks like it is still sorting the whole set"
        );
    }

    #[test]
    fn zadd_conditions_still_behave() {
        let store = KeyValueStore::new();
        let zadd = |opts: ZAddOptions, pairs: Vec<(f64, String)>| {
            store.execute(Command::ZAdd("k".into(), opts, pairs))
        };

        assert_eq!(
            zadd(ZAddOptions::default(), vec![(1.0, "a".into())]),
            int(1)
        );
        // NX leaves an existing member alone.
        let nx = ZAddOptions {
            condition: Some(crate::cmd::ZAddCondition::Nx),
            ..Default::default()
        };
        assert_eq!(zadd(nx, vec![(9.0, "a".into())]), int(0));
        assert_eq!(
            store.execute(Command::ZScore("k".into(), "a".into())),
            Value::BulkString(Some(b"1".to_vec()))
        );
        // XX will not create one.
        let xx = ZAddOptions {
            condition: Some(crate::cmd::ZAddCondition::Xx),
            ..Default::default()
        };
        assert_eq!(zadd(xx, vec![(5.0, "new".into())]), int(0));
        assert_eq!(store.execute(Command::ZCard("k".into())), int(1));
        // GT only raises.
        let gt = ZAddOptions {
            gt: true,
            ch: true,
            ..Default::default()
        };
        assert_eq!(
            zadd(gt, vec![(0.5, "a".into())]),
            int(0),
            "GT must not lower"
        );
        let gt2 = ZAddOptions {
            gt: true,
            ch: true,
            ..Default::default()
        };
        assert_eq!(zadd(gt2, vec![(7.0, "a".into())]), int(1));
        assert_eq!(
            store.execute(Command::ZScore("k".into(), "a".into())),
            Value::BulkString(Some(b"7".to_vec()))
        );
    }

    #[test]
    fn a_zset_survives_a_snapshot_round_trip_with_its_order() {
        let store = store_with_leaderboard(50);
        let snap = store.snapshot();
        let restored = KeyValueStore::new();
        restored.restore(snap);
        assert_eq!(
            restored.execute(Command::ZRange("board".into(), 0, -1, true)),
            store.execute(Command::ZRange("board".into(), 0, -1, true))
        );
    }
}

// ── TTL rounding ──────────────────────────────────────────────────────────────

/// `TTL` reports whole seconds rounded to nearest, as Redis does.
///
/// It used to truncate, so `SET k v EX 100` followed immediately by `TTL k`
/// answered 99: the microseconds between the two commands took the remainder
/// just below 100_000 ms and `/ 1000` discarded the rest. Every reading was up
/// to a second short, which breaks a ported test suite asserting the value it
/// just set, and makes a client that renews below a threshold renew early on
/// every pass. `PTTL` is unaffected — it reports milliseconds and has nothing
/// to round.
#[cfg(test)]
mod ttl_rounding_tests {
    use super::*;
    use crate::cmd::{SetExpiry, SetOptions};

    fn store_with_expiry_in(ms: u64) -> KeyValueStore {
        let s = KeyValueStore::new();
        s.execute(Command::Set(
            "k".into(),
            "v".into(),
            SetOptions {
                expiry: Some(SetExpiry::Px(ms)),
                ..Default::default()
            },
        ));
        s
    }

    fn ttl(s: &KeyValueStore) -> i64 {
        match s.execute(Command::Ttl("k".into())) {
            Value::Integer(n) => n,
            other => panic!("expected an integer, got {other:?}"),
        }
    }

    #[test]
    fn a_freshly_set_ttl_reads_back_as_the_value_that_was_set() {
        // The regression that motivated this: the number a caller just wrote
        // must be the number they read back.
        //
        // The sleep is load-bearing. In-process, SET and TTL can land in the
        // same millisecond, and with a remainder of exactly `secs * 1000` even
        // truncation answers correctly — so without a gap this test passes
        // against the bug it exists to catch. A single millisecond of real
        // elapsed time is what separates the two: truncation then answers
        // `secs - 1`, rounding still answers `secs`. Over TCP the gap is always
        // there, which is why the bug showed up against a live server first.
        for secs in [1u64, 10, 100, 3600] {
            let s = KeyValueStore::new();
            s.execute(Command::Set(
                "k".into(),
                "v".into(),
                SetOptions {
                    expiry: Some(SetExpiry::Ex(secs)),
                    ..Default::default()
                },
            ));
            std::thread::sleep(std::time::Duration::from_millis(2));
            assert_eq!(
                ttl(&s),
                secs as i64,
                "SET k v EX {secs} then TTL k must answer {secs}"
            );
        }
    }

    #[test]
    fn remainders_round_to_the_nearest_second() {
        // 1600 ms is nearer 2 s than 1 s; 1400 ms is nearer 1 s. Truncation
        // answered 1 for both.
        assert_eq!(ttl(&store_with_expiry_in(1_600)), 2);
        assert_eq!(ttl(&store_with_expiry_in(1_400)), 1);
        // Exactly half a second rounds up, matching Redis's `(ttl + 500) / 1000`.
        assert_eq!(ttl(&store_with_expiry_in(2_500)), 3);
        assert_eq!(ttl(&store_with_expiry_in(600)), 1);
    }

    #[test]
    fn a_sub_second_remainder_still_reports_a_live_key_not_zero_or_minus_two() {
        // A key with 400 ms left is alive. Reporting -2 would say "no such key"
        // and 0 is only correct once it is nearly gone.
        let s = store_with_expiry_in(400);
        assert_eq!(ttl(&s), 0, "400 ms rounds down to 0 whole seconds");
        assert_eq!(
            s.execute(Command::Get("k".into())),
            Value::BulkString(Some(b"v".to_vec())),
            "the key is still readable while TTL reports 0"
        );
    }

    #[test]
    fn the_sentinels_are_unchanged() {
        let s = KeyValueStore::new();
        assert_eq!(s.execute(Command::Ttl("ghost".into())), Value::Integer(-2));
        s.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        assert_eq!(s.execute(Command::Ttl("k".into())), Value::Integer(-1));
    }

    #[test]
    fn pttl_still_reports_exact_milliseconds() {
        // Rounding belongs to TTL alone; PTTL must not gain a half-second bias.
        let s = store_with_expiry_in(1_600);
        match s.execute(Command::PTtl("k".into())) {
            Value::Integer(ms) => assert!(
                (1_400..=1_600).contains(&ms),
                "PTTL should be ~1600 ms, got {ms}"
            ),
            other => panic!("expected an integer, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod storage_regression_tests {
    use super::*;
    use crate::cmd::{SetOptions, ZAddOptions};

    #[test]
    fn replica_snapshot_replacement_drops_stale_keys() {
        let replica = KeyValueStore::new();
        replica.execute(Command::Set(
            "stale".into(),
            b"old".to_vec(),
            SetOptions::default(),
        ));
        let primary = KeyValueStore::new();
        primary.execute(Command::Set(
            "current".into(),
            b"new".to_vec(),
            SetOptions::default(),
        ));

        replica.replace(primary.snapshot());

        assert_eq!(
            replica.execute(Command::Get("stale".into())),
            Value::BulkString(None)
        );
        assert_eq!(
            replica.execute(Command::Get("current".into())),
            Value::BulkString(Some(b"new".to_vec()))
        );
    }

    /// `KeyIndex::sync` skips both `ordered` and `all` on one hash lookup in
    /// `all`, which is only sound while the two hold exactly the same keys.
    ///
    /// That invariant is invisible at the call sites and easy to break — an
    /// edit that touches one collection and forgets the other would leave
    /// `SCAN` silently missing keys that `RANDOMKEY` and eviction can still
    /// see, with nothing failing until a user noticed. So drive every path
    /// that mutates the index and check it directly.
    #[test]
    fn the_ordered_and_dense_key_indexes_never_diverge() {
        let store = KeyValueStore::new();
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state = state.wrapping_mul(0x2545_F491_4F6C_DD1D);
            state
        };

        let check = |label: &str| {
            let index = store.index.lock().unwrap_or_else(|e| e.into_inner());
            let ordered: std::collections::BTreeSet<&str> =
                index.ordered.iter().map(String::as_str).collect();
            let dense: std::collections::BTreeSet<&str> =
                index.all.keys.iter().map(String::as_str).collect();
            assert_eq!(
                ordered,
                dense,
                "ordered and all diverged after {label}: only-in-ordered {:?}, only-in-all {:?}",
                ordered.difference(&dense).collect::<Vec<_>>(),
                dense.difference(&ordered).collect::<Vec<_>>()
            );
            // Every volatile key must still be a live key.
            for key in &index.volatile.keys {
                assert!(
                    dense.contains(key.as_str()),
                    "volatile holds {key}, which is not in the key set, after {label}"
                );
            }
            // The dense position map must agree with its own vector.
            assert_eq!(index.all.keys.len(), index.all.positions.len());
            for (position, key) in index.all.keys.iter().enumerate() {
                assert_eq!(index.all.positions.get(key), Some(&position));
            }
        };

        for step in 0..3000u64 {
            let n = next();
            let key = format!("k{}", n % 48);
            match n % 10 {
                0..=2 => {
                    store.execute(Command::Set(key, "v".into(), SetOptions::default()));
                }
                3 => {
                    // Same key, now volatile.
                    store.execute(Command::SetEx(key, 100, "v".into()));
                }
                4 => {
                    store.execute(Command::Expire(key, 100));
                }
                5 => {
                    store.execute(Command::Persist(key));
                }
                6 => {
                    store.execute(Command::Del(vec![key]));
                }
                7 => {
                    store.execute(Command::HSet(key, vec![("f".into(), "v".into())]));
                }
                8 => {
                    // A TTL short enough to lapse, so the sweeper path runs too.
                    store.execute(Command::PSetEx(key, 1, "v".into()));
                }
                _ => {
                    store.execute(Command::Rename(key, format!("r{}", n % 16)));
                }
            }
            if step % 250 == 0 {
                store.sweep_expired();
                check("a sweep");
            }
        }
        check("the mixed workload");

        store.execute(Command::FlushDb);
        check("FLUSHDB");
    }

    /// The incremental byte counters must agree with a full recount.
    ///
    /// `entry_size` reads a total each collection maintains as it is mutated,
    /// which is what makes writes O(1) instead of O(collection). The risk that
    /// buys is drift: one mutation path that forgets to adjust its counter
    /// silently corrupts `INFO used_memory` and, with a memory cap set,
    /// eviction decisions with it. So drive every collection command over a
    /// deterministic pseudo-random workload and audit the cached total against
    /// the walk it replaced.
    #[test]
    fn incremental_byte_counters_match_a_full_recount() {
        fn audit(value: &EntryValue) -> usize {
            match value {
                EntryValue::Str(s) => s.len(),
                EntryValue::Hash(m) => m.iter().map(|(k, v)| k.len() + v.len()).sum(),
                EntryValue::List(l) => l.iter().map(|s| s.len()).sum(),
                EntryValue::Set(s) => s.iter().map(|m| m.len()).sum::<usize>(),
                EntryValue::ZSet(z) => z.members().map(|(m, _)| m.len() + 8).sum(),
                EntryValue::RateLimiter(rl) => rl.buckets.len() * 16 + 16,
                EntryValue::Json(doc) => json_approx_size(doc),
            }
        }

        let store = KeyValueStore::new();
        // xorshift: a fixed sequence, so a failure reproduces exactly.
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state = state.wrapping_mul(0x2545_F491_4F6C_DD1D);
            state
        };

        for step in 0..4000u64 {
            let n = next();
            let field = format!("f{}", n % 64);
            let member = format!("m{}", n % 64);
            let value = "v".repeat((n % 37) as usize + 1);
            match n % 14 {
                0 => drop(store.execute(Command::HSet(
                    "h".into(),
                    vec![(field, value.clone().into())],
                ))),
                1 => drop(store.execute(Command::HDel("h".into(), vec![field]))),
                2 => drop(store.execute(Command::HIncrBy("h".into(), "counter".into(), 3))),
                3 => drop(store.execute(Command::LPush("l".into(), vec![value.clone().into()]))),
                4 => drop(store.execute(Command::RPush("l".into(), vec![value.clone().into()]))),
                5 => drop(store.execute(Command::LPop("l".into(), Some(n % 3)))),
                6 => drop(store.execute(Command::RPop("l".into(), None))),
                7 => drop(store.execute(Command::LSet("l".into(), 0, value.clone().into()))),
                8 => drop(store.execute(Command::LRem("l".into(), 0, value.clone().into()))),
                9 => drop(store.execute(Command::LTrim("l".into(), 0, 40))),
                10 => drop(store.execute(Command::SAdd("s".into(), vec![member]))),
                11 => drop(store.execute(Command::SRem("s".into(), vec![format!("m{}", n % 32)]))),
                12 => drop(store.execute(Command::ZAdd(
                    "z".into(),
                    ZAddOptions::default(),
                    vec![((n % 100) as f64, member)],
                ))),
                _ => drop(store.execute(Command::ZRem("z".into(), vec![member]))),
            }

            for key in ["h", "l", "s", "z"] {
                if let Some(entry) = store.data.get(key) {
                    let cached = entry_size(key, entry.value()) - key.len() - 64;
                    let walked = audit(&entry.value().value);
                    assert_eq!(
                        cached, walked,
                        "byte counter drifted for {key} at step {step} (cached {cached}, actual {walked})"
                    );
                }
            }
        }

        // And the store-wide total must match the sum of its entries.
        let expected: usize = store
            .data
            .iter()
            .map(|e| entry_size(e.key(), e.value()))
            .sum();
        assert_eq!(store.approximate_memory_bytes(), expected);
    }

    #[test]
    fn collection_inline_storage_does_not_inflate_every_entry_value() {
        // Was 96 until the sorted set was boxed alongside the hash and the
        // set. `EntryValue` is stored inline in every entry, so the largest
        // variant is a tax on every key including plain strings.
        assert!(
            std::mem::size_of::<EntryValue>() <= 48,
            "EntryValue is {} bytes",
            std::mem::size_of::<EntryValue>()
        );
        assert!(
            std::mem::size_of::<CompactHash>() <= 160,
            "CompactHash is {} bytes",
            std::mem::size_of::<CompactHash>()
        );
        assert!(
            std::mem::size_of::<CompactSet>() <= 128,
            "CompactSet is {} bytes",
            std::mem::size_of::<CompactSet>()
        );
    }

    #[test]
    fn small_payloads_hashes_and_sets_stay_inline() {
        let payload = Blob::from("small");
        assert!(!payload.0.spilled());

        let mut hash = CompactHash::new();
        for i in 0..INLINE_HASH_FIELDS {
            hash.insert(format!("f{i}"), Blob::from("v"));
        }
        assert!(matches!(hash.repr, CompactHashRepr::Inline(_)));
        hash.insert("spill".into(), Blob::from("v"));
        assert!(matches!(hash.repr, CompactHashRepr::Table(_)));

        let mut set = CompactSet::new();
        for i in 0..INLINE_SET_MEMBERS {
            set.insert(format!("m{i}"));
        }
        assert!(matches!(set.repr, CompactSetRepr::Inline(_)));
        set.insert("spill".into());
        assert!(matches!(set.repr, CompactSetRepr::Table(_)));
    }
}
