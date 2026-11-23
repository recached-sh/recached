//! Watched keys: the registry behind WATCH's compare-and-set transactions and
//! the live-query (QSUB) subscriptions that share it.

use crate::*;

/// A single connection may buffer at most this many keychange notifications
/// and this many payload bytes. Count and byte limits complement each other:
/// many tiny updates cannot create an unbounded allocation, and a handful of
/// maximum-size values cannot consume hundreds of megabytes.
const NOTIFICATION_QUEUE_ITEMS: usize = 256;
const NOTIFICATION_QUEUE_BYTES: usize = 8 * 1024 * 1024;

/// What a queued notification carries. The choice is made per subscriber, at
/// queue time, so a connection receiving deltas is charged for the bytes it
/// will actually be sent — an agent appending to a 1 MiB key must not be
/// disconnected for backpressure it is not causing.
#[derive(Debug)]
pub(crate) enum NotifPayload {
    /// The key's whole current value, shared by every subscriber it is sent
    /// to rather than cloned per connection, and its remaining time to live in
    /// milliseconds when it has one. Each subscriber is still charged the
    /// value's full size, so per-connection budgets and overflow are unchanged.
    Full(Arc<Value>, Option<u64>),
    /// The mutation, for a connection that sent `CLIENT DELTA ON`.
    Delta(KeyDelta),
}

#[derive(Debug)]
pub(crate) struct WatchNotif {
    pub(crate) key: String,
    pub(crate) payload: NotifPayload,
    /// Whether this change invalidates a WATCH of the key (6O-E): false only
    /// for the removal of a key that was not live when it was watched, which
    /// Redis treats as no change. A WebSocket watcher is still pushed it.
    pub(crate) invalidates: bool,
    charged_bytes: usize,
    budget: Arc<NotificationBudget>,
}

impl Drop for WatchNotif {
    fn drop(&mut self) {
        self.budget
            .pending_bytes
            .fetch_sub(self.charged_bytes, Ordering::AcqRel);
    }
}

#[derive(Debug)]
struct NotificationBudget {
    pending_bytes: AtomicUsize,
}

impl NotificationBudget {
    fn reserve(&self, bytes: usize) -> bool {
        self.pending_bytes
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                pending
                    .checked_add(bytes)
                    .filter(|next| *next <= NOTIFICATION_QUEUE_BYTES)
            })
            .is_ok()
    }
}

#[derive(Clone)]
pub(crate) struct WatchSubscriber {
    pub(crate) conn_id: u64,
    tx: mpsc::Sender<WatchNotif>,
    overflow: mpsc::Sender<()>,
    budget: Arc<NotificationBudget>,
    /// Mirrors this connection's `CLIENT DELTA` setting. Shared rather than
    /// copied because the toggle can flip while subscriptions are live.
    deltas: Arc<AtomicBool>,
    /// For a WATCH registration: whether the key was live (present and not
    /// expired) when it was watched (6O-E). Redis 7.2 does not count the
    /// removal of a key that was absent or already expired at `WATCH` as a
    /// change (`touchWatchedKey`'s `expired` flag, `touchAllWatchedKeysInDb`),
    /// and does abort when a key that was live has since expired
    /// (`isWatchedKeyExpired`). True for every other registration, so live
    /// queries are unaffected.
    live_at_watch: bool,
}

impl WatchSubscriber {
    /// Queue a notification without ever waiting behind a slow socket. A full
    /// queue signals the owning connection and removes this registration.
    ///
    /// The change's compact delta is used when it has one and this
    /// subscriber's connection asked for deltas; otherwise the full value,
    /// which is materialised on first need. The charge is measured and
    /// reserved before anything is cloned.
    pub(crate) fn notify(&self, change: &KeyChange<'_>) -> bool {
        let invalidates = self.live_at_watch || !change.leaves_key_absent();
        let delta = change
            .delta
            .as_ref()
            .filter(|_| self.deltas.load(Ordering::Relaxed));
        let payload_bytes = match delta {
            Some(delta) => delta.heap_bytes(),
            None => change.full().bytes,
        };
        let charged_bytes = change
            .key
            .len()
            .saturating_add(payload_bytes)
            .saturating_add(std::mem::size_of::<WatchNotif>());
        if !self.budget.reserve(charged_bytes) {
            self.signal_overflow();
            return false;
        }

        let payload = match delta {
            Some(delta) => NotifPayload::Delta(delta.clone()),
            None => {
                let full = change.full();
                NotifPayload::Full(Arc::clone(&full.value), full.ttl_ms)
            }
        };
        let notification = WatchNotif {
            key: change.key.clone(),
            payload,
            invalidates,
            charged_bytes,
            budget: Arc::clone(&self.budget),
        };
        if self.tx.try_send(notification).is_err() {
            // The failed item is dropped by TrySendError, releasing its charge.
            self.signal_overflow();
            return false;
        }
        true
    }

    /// This subscriber, registered for a key that was (`live`) or was not
    /// live when watched.
    pub(crate) fn watching(&self, live: bool) -> WatchSubscriber {
        WatchSubscriber {
            live_at_watch: live,
            ..self.clone()
        }
    }

    fn signal_overflow(&self) {
        counter!("recached_notification_overflows_total").increment(1);
        let _ = self.overflow.try_send(());
    }
}

pub(crate) fn notification_channel(
    conn_id: u64,
    overflow: mpsc::Sender<()>,
    deltas: Arc<AtomicBool>,
) -> (WatchSubscriber, mpsc::Receiver<WatchNotif>) {
    let (tx, rx) = mpsc::channel(NOTIFICATION_QUEUE_ITEMS);
    (
        WatchSubscriber {
            conn_id,
            tx,
            overflow,
            budget: Arc::new(NotificationBudget {
                pending_bytes: AtomicUsize::new(0),
            }),
            deltas,
            live_at_watch: true,
        },
        rx,
    )
}

/// Bytes a queued value keeps allocated: string and bulk payloads by
/// capacity, plus the backing storage of every array, push and map — each
/// element slot is a whole `Value` even when the element itself owns nothing.
/// Counting only leaf payloads charged an array of 300,000 empty strings as
/// zero bytes while it held 9.6 MB, which let 256 queued items retain
/// gigabytes against an 8 MiB budget.
fn value_heap_bytes(value: &Value) -> usize {
    fn slots(capacity: usize, slot: usize) -> usize {
        capacity.saturating_mul(slot)
    }
    match value {
        Value::SimpleString(value) | Value::Error(value) => value.capacity(),
        Value::Integer(_) => 0,
        Value::BulkString(value) => value.as_ref().map_or(0, Vec::capacity),
        Value::Array(None) => 0,
        Value::Array(Some(items)) | Value::Push(items) => items.iter().fold(
            slots(items.capacity(), std::mem::size_of::<Value>()),
            |total, item| total.saturating_add(value_heap_bytes(item)),
        ),
        Value::Map(entries) => entries.iter().fold(
            slots(entries.capacity(), std::mem::size_of::<(Value, Value)>()),
            |total, (key, value)| {
                total
                    .saturating_add(value_heap_bytes(key))
                    .saturating_add(value_heap_bytes(value))
            },
        ),
    }
}

/// One changed key on its way to watchers and live queries.
///
/// The full value is read from the store at most once, and only if some
/// recipient will be sent it: a subscriber that asked for deltas never needs
/// it, so a one-byte `APPEND` watched only by delta clients no longer copies
/// and encodes the whole value just to throw it away. Its size is measured
/// once alongside, rather than walked again per subscriber.
pub(crate) struct KeyChange<'a> {
    pub(crate) key: String,
    pub(crate) delta: Option<KeyDelta>,
    /// Where the current value comes from; `None` for a removal, whose value
    /// is nil.
    source: Option<&'a KeyValueStore>,
    full: OnceLock<FullValue>,
}

/// A change's full form: the value, its expiry, and the value's size, read
/// together so the expiry always belongs to the value it travels with.
struct FullValue {
    value: Arc<Value>,
    ttl_ms: Option<u64>,
    bytes: usize,
}

impl<'a> KeyChange<'a> {
    /// A write: the value is the key's current contents in `store`. The
    /// caller must hold the key's ordering guard until delivery finishes, so
    /// a lazy read still observes this write and not a later one.
    pub(crate) fn current(key: String, delta: Option<KeyDelta>, store: &'a KeyValueStore) -> Self {
        Self {
            key,
            delta,
            source: Some(store),
            full: OnceLock::new(),
        }
    }

    /// A removal (expiry, eviction, flush), announced as a nil value.
    pub(crate) fn removed(key: String) -> Self {
        Self {
            key,
            delta: None,
            source: None,
            full: OnceLock::new(),
        }
    }

    /// Whether the key is absent once this change is applied: a removal, or a
    /// write that left nothing (`DEL` of a key that was not there). Read only
    /// for a WATCH of a key that was not live when watched; the caller holds
    /// the key's ordering guard, so the read sees this change.
    fn leaves_key_absent(&self) -> bool {
        match self.source {
            None => true,
            Some(store) => !key_is_live(store, &self.key),
        }
    }

    fn full(&self) -> &FullValue {
        self.full.get_or_init(|| {
            let (value, ttl_ms) = match self.source {
                Some(store) => store.get_current_with_ttl(&self.key),
                None => (Value::BulkString(None), None),
            };
            let bytes = value_heap_bytes(&value);
            FullValue {
                value: Arc::new(value),
                ttl_ms,
                bytes,
            }
        })
    }
}

pub(crate) type WatchMap = HashMap<String, Vec<WatchSubscriber>>;

/// Live-query (QSUB) registrations, indexed by which keys each pattern can
/// match, so a write is checked against the patterns that could match its
/// key rather than against every pattern registered.
///
/// The index is a candidate filter and nothing more — the same scheme
/// [`SyncRouter`](crate::sync_router) uses for grants. The glob knows two
/// metacharacters, `*` and `?`, and matches bytes, so every byte of a pattern
/// before its first metacharacter must equal the same byte of any key it
/// matches:
///
/// | Pattern | Indexed as | A key is a candidate when |
/// |---|---|---|
/// | `cart:42` (no metacharacter) | exact | it equals the pattern |
/// | `cart:*`, `cart:?:x` | prefix `cart:` | it starts with the prefix |
/// | `*`, `*:x`, `?x` | broad | always |
///
/// A candidate is then matched with `glob_match` before anything is
/// delivered, so the index can only ever narrow the patterns checked, never
/// admit a key a pattern does not match. Exact patterns need no glob: a
/// pattern without metacharacters matches exactly the key equal to it.
///
/// Lives inside [`WatchHub::patterns`]' mutex, so it changes in the same
/// critical section as the subscriptions it indexes and adds no lock.
#[derive(Default)]
pub(crate) struct LiveQueries {
    /// Patterns without a metacharacter, and their subscribers.
    exact: WatchMap,
    /// Patterns beginning with a metacharacter, and their subscribers.
    broad: WatchMap,
    /// Literal prefix → the patterns sharing it, and their subscribers.
    prefixes: HashMap<Box<[u8]>, WatchMap>,
    /// Each distinct length in `prefixes`, with how many prefixes have it: a
    /// key is looked up once per length no longer than itself.
    prefix_lens: BTreeMap<usize, usize>,
    /// Distinct patterns registered, across all three.
    len: usize,
}

/// Where [`LiveQueries`] files a pattern.
enum PatternClass<'a> {
    Exact,
    Broad,
    Prefix(&'a [u8]),
}

fn classify(pattern: &str) -> PatternClass<'_> {
    let bytes = pattern.as_bytes();
    match bytes.iter().position(|b| matches!(b, b'*' | b'?')) {
        None => PatternClass::Exact,
        Some(0) => PatternClass::Broad,
        Some(at) => bytes
            .get(..at)
            .map_or(PatternClass::Broad, PatternClass::Prefix),
    }
}

/// Deliver `change` to `subs` (one pattern's subscribers), skipping and
/// recording connections in `delivered`; drops overflowed subscribers.
/// Returns whether the pattern is left with none.
fn deliver_to(
    subs: &mut Vec<WatchSubscriber>,
    change: &KeyChange<'_>,
    delivered: &mut HashSet<u64>,
) -> bool {
    subs.retain(|sub| {
        if delivered.contains(&sub.conn_id) {
            return true;
        }
        let alive = sub.notify(change);
        if alive {
            delivered.insert(sub.conn_id);
        }
        alive
    });
    subs.is_empty()
}

impl LiveQueries {
    /// Distinct patterns registered.
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Register `sub` for `pattern`.
    pub(crate) fn add(&mut self, pattern: &str, sub: WatchSubscriber) {
        let bucket = match classify(pattern) {
            PatternClass::Exact => &mut self.exact,
            PatternClass::Broad => &mut self.broad,
            PatternClass::Prefix(prefix) => {
                if !self.prefixes.contains_key(prefix) {
                    *self.prefix_lens.entry(prefix.len()).or_default() += 1;
                }
                self.prefixes.entry(Box::from(prefix)).or_default()
            }
        };
        match bucket.get_mut(pattern) {
            Some(subs) => subs.push(sub),
            None => {
                bucket.insert(pattern.to_string(), vec![sub]);
                self.len = self.len.saturating_add(1);
            }
        }
    }

    /// Remove `conn_id`'s subscription to `pattern`, and the pattern itself
    /// when that was its last.
    pub(crate) fn remove_conn(&mut self, pattern: &str, conn_id: u64) {
        let emptied = self.bucket_mut(pattern).is_some_and(|bucket| {
            bucket.get_mut(pattern).is_some_and(|subs| {
                subs.retain(|sub| sub.conn_id != conn_id);
                subs.is_empty()
            })
        });
        if emptied {
            self.remove_pattern(pattern);
        }
    }

    /// The bucket `pattern` is filed in, if it exists.
    fn bucket_mut(&mut self, pattern: &str) -> Option<&mut WatchMap> {
        match classify(pattern) {
            PatternClass::Exact => Some(&mut self.exact),
            PatternClass::Broad => Some(&mut self.broad),
            PatternClass::Prefix(prefix) => self.prefixes.get_mut(prefix),
        }
    }

    /// Drop `pattern` and everything indexed for it.
    fn remove_pattern(&mut self, pattern: &str) {
        let removed = self
            .bucket_mut(pattern)
            .is_some_and(|bucket| bucket.remove(pattern).is_some());
        if !removed {
            return;
        }
        self.len = self.len.saturating_sub(1);
        if let PatternClass::Prefix(prefix) = classify(pattern)
            && self.prefixes.get(prefix).is_some_and(HashMap::is_empty)
        {
            self.prefixes.remove(prefix);
            if let Some(count) = self.prefix_lens.get_mut(&prefix.len()) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.prefix_lens.remove(&prefix.len());
                }
            }
        }
    }

    /// Deliver `change` to every subscriber of every pattern matching its
    /// key, skipping connections in `delivered` (already sent this key by a
    /// WATCH or another of their patterns) and adding the ones reached.
    /// Subscribers whose queue overflowed are dropped, and patterns left with
    /// none are unregistered. Returns (candidates checked, patterns matched).
    ///
    /// Only patterns that could match are visited (see the table above);
    /// each is confirmed with `glob_match` (exact ones need not be) and its
    /// subscribers are reached in place, with no second lookup and nothing
    /// allocated per match. Only a pattern emptied by overflow (rare) is
    /// copied, to unregister it afterwards.
    pub(crate) fn deliver(
        &mut self,
        change: &KeyChange<'_>,
        delivered: &mut HashSet<u64>,
    ) -> (usize, usize) {
        let key = change.key.as_str();
        let (mut candidates, mut matched) = (0usize, 0usize);
        let mut emptied: smallvec::SmallVec<[String; 1]> = smallvec::SmallVec::new();
        // An exact pattern is the key itself: no glob needed.
        if let Some(subs) = self.exact.get_mut(key) {
            candidates = 1;
            matched = 1;
            if deliver_to(subs, change, delivered) {
                emptied.push(key.to_string());
            }
        }
        let mut visit = |bucket: &mut WatchMap, needs_glob: bool| {
            for (pattern, subs) in bucket.iter_mut() {
                candidates = candidates.saturating_add(1);
                if needs_glob && !core_engine::store::glob_match(pattern, key) {
                    continue;
                }
                matched = matched.saturating_add(1);
                if deliver_to(subs, change, delivered) {
                    emptied.push(pattern.clone());
                }
            }
        };
        visit(&mut self.broad, true);
        let bytes = key.as_bytes();
        // `..=len`: `BTreeMap::range` panics when its start exceeds its end,
        // and the empty key is a key. Every stored prefix is at least one
        // byte (an empty one is broad).
        for &len in self.prefix_lens.range(..=bytes.len()).map(|(len, _)| len) {
            if let Some(bucket) = bytes.get(..len).and_then(|p| self.prefixes.get_mut(p)) {
                visit(bucket, true);
            }
        }
        for pattern in emptied {
            self.remove_pattern(&pattern);
        }
        (candidates, matched)
    }

    /// Call `f` with every pattern and its subscribers (FLUSHDB announces
    /// itself to every pattern), then unregister patterns left with none.
    pub(crate) fn for_each_pattern(&mut self, mut f: impl FnMut(&str, &mut Vec<WatchSubscriber>)) {
        let mut emptied = Vec::new();
        let buckets = std::iter::once(&mut self.exact)
            .chain(std::iter::once(&mut self.broad))
            .chain(self.prefixes.values_mut());
        for bucket in buckets {
            for (pattern, subs) in bucket.iter_mut() {
                f(pattern, subs);
                if subs.is_empty() {
                    emptied.push(pattern.clone());
                }
            }
        }
        for pattern in emptied {
            self.remove_pattern(&pattern);
        }
    }

    /// The index's shape, for INFO: (exact, prefixes, broad).
    pub(crate) fn shape(&self) -> (usize, usize, usize) {
        (self.exact.len(), self.prefixes.len(), self.broad.len())
    }
}

/// Live-query routing counters since start, for `INFO recached`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LiveQueryStats {
    /// Distinct patterns registered now.
    pub(crate) patterns: u64,
    /// Of those, indexed as exact keys, literal prefixes, and broad.
    pub(crate) exact: u64,
    pub(crate) prefixes: u64,
    pub(crate) broad: u64,
    /// Changed keys offered to live queries.
    pub(crate) events: u64,
    /// Patterns checked for them (the work the index left).
    pub(crate) candidates: u64,
    /// Patterns that matched.
    pub(crate) matches: u64,
}

/// Watched-key and live-query registry. `watched_keys` / `watched_patterns`
/// mirror the map lengths (updated by every writer while holding the lock) so
/// the per-write hot path can skip the mutexes entirely when nothing is
/// watched.
pub(crate) struct WatchHub {
    /// Exact-key watchers (WATCH).
    pub(crate) map: tokio::sync::Mutex<WatchMap>,
    pub(crate) watched_keys: AtomicUsize,
    /// Glob-pattern subscribers (QSUB live queries), indexed by pattern.
    pub(crate) patterns: tokio::sync::Mutex<LiveQueries>,
    pub(crate) watched_patterns: AtomicUsize,
    /// Routing counters ([`LiveQueryStats`]); updated under `patterns`.
    pub(crate) route_events: AtomicU64,
    pub(crate) route_candidates: AtomicU64,
    pub(crate) route_matches: AtomicU64,
    /// The index's shape as of the last change, packed for a lock-free read:
    /// exact, prefixes and broad counts, 21 bits each (saturating).
    route_shape: AtomicU64,
}

impl WatchHub {
    pub(crate) fn new() -> WatchRegistry {
        Arc::new(WatchHub {
            map: tokio::sync::Mutex::new(HashMap::new()),
            watched_keys: AtomicUsize::new(0),
            patterns: tokio::sync::Mutex::new(LiveQueries::default()),
            watched_patterns: AtomicUsize::new(0),
            route_events: AtomicU64::new(0),
            route_candidates: AtomicU64::new(0),
            route_matches: AtomicU64::new(0),
            route_shape: AtomicU64::new(0),
        })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.watched_keys.load(Ordering::Relaxed) == 0
            && self.watched_patterns.load(Ordering::Relaxed) == 0
    }

    /// Call after mutating the key map, while still holding the lock.
    pub(crate) fn sync_len(&self, map: &WatchMap) {
        self.watched_keys.store(map.len(), Ordering::Relaxed);
    }

    /// Call after mutating the live-query registry, while still holding the
    /// lock.
    pub(crate) fn sync_patterns_len(&self, queries: &LiveQueries) {
        self.watched_patterns
            .store(queries.len(), Ordering::Relaxed);
        const FIELD: u64 = (1 << 21) - 1;
        let pack = |n: usize| u64::try_from(n).unwrap_or(FIELD).min(FIELD);
        let (exact, prefixes, broad) = queries.shape();
        self.route_shape.store(
            pack(exact) | (pack(prefixes) << 21) | (pack(broad) << 42),
            Ordering::Relaxed,
        );
    }

    /// Live-query routing counters for INFO. Lock-free; each field is
    /// individually current, not a consistent snapshot.
    pub(crate) fn live_query_stats(&self) -> LiveQueryStats {
        const FIELD: u64 = (1 << 21) - 1;
        let shape = self.route_shape.load(Ordering::Relaxed);
        LiveQueryStats {
            patterns: self.watched_patterns.load(Ordering::Relaxed) as u64,
            exact: shape & FIELD,
            prefixes: (shape >> 21) & FIELD,
            broad: (shape >> 42) & FIELD,
            events: self.route_events.load(Ordering::Relaxed),
            candidates: self.route_candidates.load(Ordering::Relaxed),
            matches: self.route_matches.load(Ordering::Relaxed),
        }
    }
}

pub(crate) type WatchRegistry = Arc<WatchHub>;

/// Drop all of `conn_id`'s live-query subscriptions. Called on QUNSUB (all
/// form) and on connection close.
pub(crate) async fn unregister_all_qsubs(
    registry: &WatchRegistry,
    conn_id: u64,
    qsub_patterns: &mut HashSet<String>,
) {
    if qsub_patterns.is_empty() {
        return;
    }
    let mut pats = registry.patterns.lock().await;
    for p in qsub_patterns.drain() {
        pats.remove_conn(&p, conn_id);
    }
    registry.sync_patterns_len(&pats);
}

/// Whether `key` is present and not expired now (at the store's clock, so
/// inside [`core_engine::store::at_instant`], at that instant).
pub(crate) fn key_is_live(store: &KeyValueStore, key: &str) -> bool {
    matches!(
        store.execute(Command::Exists(vec![key.to_string()])),
        Value::Integer(1)
    )
}

/// Whether a key `conn_id` watched while it was live is no longer live at
/// `now`, the instant its transaction runs at: Redis 7.2 aborts `EXEC` then
/// (`isWatchedKeyExpired`), though no command touched the key. A key that
/// expires lazily changes nothing in the store, so no notification ever told
/// the connection.
///
/// Called by `EXEC` under its guards, which include every watched key's
/// stripe, so no write to those keys runs meanwhile. A key whose
/// registration is gone was dropped for overflow, and `EXEC` checks that
/// first.
pub(crate) async fn watched_key_expired(
    registry: &WatchRegistry,
    conn_id: u64,
    watched_keys: &HashSet<String>,
    store: &KeyValueStore,
    now: u64,
) -> bool {
    if watched_keys.is_empty() {
        return false;
    }
    let reg = registry.map.lock().await;
    watched_keys.iter().any(|key| {
        reg.get(key)
            .and_then(|subs| subs.iter().find(|sub| sub.conn_id == conn_id))
            .is_some_and(|sub| sub.live_at_watch)
            && !core_engine::store::at_instant(now, || key_is_live(store, key))
    })
}

/// Drop all of `conn_id`'s WATCH registrations and clear `watched_keys`.
/// Called at every transaction boundary (EXEC, DISCARD) and on connection close,
/// matching Redis semantics that WATCH state is flushed by EXEC/DISCARD.
pub(crate) async fn unregister_all_watches(
    registry: &WatchRegistry,
    conn_id: u64,
    watched_keys: &mut HashSet<String>,
) {
    if watched_keys.is_empty() {
        return;
    }
    let mut reg = registry.map.lock().await;
    for key in watched_keys.drain() {
        if let Some(subs) = reg.get_mut(&key) {
            subs.retain(|sub| sub.conn_id != conn_id);
            if subs.is_empty() {
                reg.remove(&key);
            }
        }
    }
    registry.sync_len(&reg);
}

// ── helpers ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn aggregate_storage_is_charged_not_just_its_leaves() {
        let items = vec![Value::BulkString(Some(Vec::new())); 300_000];
        let slots = items.capacity() * std::mem::size_of::<Value>();
        let value = Value::Array(Some(items));
        assert!(value_heap_bytes(&value) >= slots);

        let map = Value::Map(vec![(Value::Integer(1), Value::Integer(2)); 1_000]);
        assert!(value_heap_bytes(&map) >= 1_000 * std::mem::size_of::<(Value, Value)>());
    }

    #[test]
    fn one_oversized_aggregate_overflows_the_queue_budget() {
        let (overflow, mut overflowed) = mpsc::channel(1);
        let (subscriber, _rx) = notification_channel(1, overflow, Arc::new(AtomicBool::new(false)));
        let store = KeyValueStore::new();
        let members: Vec<String> = (0..300_000).map(|i| i.to_string()).collect();
        store.execute(Command::SAdd("big".into(), members));
        let change = KeyChange::current("big".into(), None, &store);
        assert!(
            !subscriber.notify(&change),
            "a value past the byte budget must not queue"
        );
        assert!(
            overflowed.try_recv().is_ok(),
            "the connection must be told it overflowed"
        );
    }

    #[test]
    fn a_delta_subscriber_never_materialises_the_full_value() {
        let (overflow, _overflowed) = mpsc::channel(1);
        let (subscriber, mut rx) =
            notification_channel(1, overflow, Arc::new(AtomicBool::new(true)));
        let store = KeyValueStore::new();
        store.execute(Command::Append("k".into(), b"x".to_vec()));
        let delta = KeyDelta {
            op: "append",
            args: vec![b"x".to_vec()],
        };
        let change = KeyChange::current("k".into(), Some(delta), &store);
        assert!(subscriber.notify(&change));
        assert!(
            change.full.get().is_none(),
            "the full value was read for nobody"
        );
        let queued = rx.try_recv().expect("the delta was queued");
        assert!(matches!(queued.payload, NotifPayload::Delta(_)));
    }
}

#[cfg(test)]
mod live_query_index_tests {
    use super::*;
    use rand::{Rng, SeedableRng};

    /// A subscriber on its own connection, and the end its notifications
    /// arrive on.
    fn subscriber(conn_id: u64) -> (WatchSubscriber, mpsc::Receiver<WatchNotif>) {
        let (overflow, _) = mpsc::channel(1);
        notification_channel(conn_id, overflow, Arc::new(AtomicBool::new(false)))
    }

    fn drain(rx: &mut mpsc::Receiver<WatchNotif>) -> Vec<String> {
        let mut keys = Vec::new();
        while let Ok(n) = rx.try_recv() {
            keys.push(n.key.clone());
        }
        keys
    }

    /// Nothing indexed for a pattern outlives its last subscriber.
    fn assert_no_residue(q: &LiveQueries) {
        assert_eq!(q.len(), 0);
        assert!(q.exact.is_empty() && q.broad.is_empty());
        assert!(q.prefixes.is_empty() && q.prefix_lens.is_empty());
    }

    #[test]
    fn delivers_to_exactly_the_patterns_a_full_glob_matches() {
        // Small alphabet, so prefixes nest and collide and keys contain the
        // metacharacters themselves.
        let mut rng = rand::rngs::StdRng::seed_from_u64(0x6b42);
        let alphabet = ['a', 'b', ':', '*', '?', 'é'];
        let word = |rng: &mut rand::rngs::StdRng, max: usize| -> String {
            (0..rng.random_range(0..=max))
                .map(|_| alphabet[rng.random_range(0..alphabet.len())])
                .collect()
        };
        for round in 0..30 {
            let mut q = LiveQueries::default();
            // One connection per pattern, so who was notified says which
            // patterns matched.
            let mut patterns: Vec<String> = (0..rng.random_range(1..60))
                .map(|_| word(&mut rng, 5))
                .collect();
            patterns.sort();
            patterns.dedup();
            let mut rxs = Vec::new();
            for (i, p) in patterns.iter().enumerate() {
                let (sub, rx) = subscriber(i as u64 + 1);
                q.add(p, sub);
                rxs.push(rx);
            }
            for _ in 0..80 {
                let key = word(&mut rng, 6);
                let mut reached = HashSet::new();
                let (candidates, matched) =
                    q.deliver(&KeyChange::removed(key.clone()), &mut reached);
                let expected: Vec<usize> = patterns
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| core_engine::store::glob_match(p, &key))
                    .map(|(i, _)| i)
                    .collect();
                assert_eq!(matched, expected.len(), "round {round} key {key:?}");
                assert!(candidates >= matched && candidates <= patterns.len());
                for (i, rx) in rxs.iter_mut().enumerate() {
                    let got = drain(rx);
                    let want = if expected.contains(&i) {
                        vec![key.clone()]
                    } else {
                        vec![]
                    };
                    assert_eq!(
                        got, want,
                        "round {round}: pattern {:?} key {key:?}",
                        patterns[i]
                    );
                }
            }
        }
    }

    #[test]
    fn an_unrelated_write_checks_no_pattern_however_many_are_registered() {
        let mut q = LiveQueries::default();
        let mut rxs = Vec::new();
        for i in 0..10_000u64 {
            let (sub, rx) = subscriber(i + 1);
            q.add(&format!("tenant:{i}:*"), sub);
            rxs.push(rx);
        }
        let (exact, prefixes, broad) = q.shape();
        assert_eq!((exact, prefixes, broad), (0, 10_000, 0));

        let mut reached = HashSet::new();
        let (candidates, matched) = q.deliver(&KeyChange::removed("other:1".into()), &mut reached);
        assert_eq!((candidates, matched), (0, 0), "a full pattern walk is back");

        // `tenant:5:x` is a candidate for `tenant:5:*` only — not
        // `tenant:50:*`, whose prefix it does not start with.
        let (candidates, matched) =
            q.deliver(&KeyChange::removed("tenant:5:x".into()), &mut reached);
        assert_eq!((candidates, matched), (1, 1));
        assert_eq!(drain(&mut rxs[5]), ["tenant:5:x"]);
        assert!(drain(&mut rxs[50]).is_empty());

        // A broad pattern is a candidate for everything, as it must be.
        let (sub, _rx) = subscriber(99_999);
        q.add("*:x", sub);
        let mut reached = HashSet::new();
        let (candidates, _) = q.deliver(&KeyChange::removed("other:x".into()), &mut reached);
        assert_eq!(candidates, 1);
    }

    #[test]
    fn a_connection_gets_each_key_once_across_overlapping_patterns() {
        let mut q = LiveQueries::default();
        let (sub, mut rx) = subscriber(7);
        for p in ["a:*", "a:b:*", "*", "a:b:c", "a:?:c", "a*"] {
            q.add(p, sub.clone());
        }
        let mut reached = HashSet::new();
        let (_, matched) = q.deliver(&KeyChange::removed("a:b:c".into()), &mut reached);
        assert_eq!(matched, 6);
        assert_eq!(drain(&mut rx), ["a:b:c"], "one delivery per connection");
        // A connection already sent the key (by WATCH, say) is skipped.
        let mut reached = HashSet::from([7]);
        q.deliver(&KeyChange::removed("a:b:c".into()), &mut reached);
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn keys_holding_metacharacters_and_the_empty_key() {
        let mut q = LiveQueries::default();
        let (lit, mut lit_rx) = subscriber(1);
        let (empty, mut empty_rx) = subscriber(2);
        let (star, mut star_rx) = subscriber(3);
        q.add("a*", lit); // prefix `a`: matches `a*`, `ab`, `a`…
        q.add("", empty); // exact: only the empty key
        q.add("*", star);
        // One `reached` set per changed key, as `fan_out` keeps them.
        q.deliver(&KeyChange::removed("a*".into()), &mut HashSet::new());
        q.deliver(&KeyChange::removed(String::new()), &mut HashSet::new());
        assert_eq!(drain(&mut lit_rx), ["a*"]);
        assert_eq!(drain(&mut empty_rx), [""]);
        assert_eq!(drain(&mut star_rx), ["a*", ""]);
    }

    #[test]
    fn unregistering_leaves_no_index_behind() {
        let mut q = LiveQueries::default();
        let patterns = ["cart:*", "cart:1", "*", "cart:*:x", "c?rt", "cart:*"];
        for conn in 1..=3u64 {
            for p in patterns {
                q.add(p, subscriber(conn).0);
            }
        }
        // A second subscriber on a pattern keeps it registered.
        for p in patterns {
            q.remove_conn(p, 1);
            q.remove_conn(p, 2);
        }
        assert_eq!(q.len(), 5);
        // Exact `cart:1`; prefixes `cart:` (two patterns) and `c` (`c?rt`);
        // broad `*`.
        assert_eq!(q.shape(), (1, 2, 1));
        for p in patterns {
            q.remove_conn(p, 3);
        }
        assert_no_residue(&q);
        // Removing what is not there is a no-op.
        q.remove_conn("nope:*", 9);
        assert_no_residue(&q);
    }

    #[test]
    fn an_overflowed_subscriber_is_dropped_and_its_pattern_unindexed() {
        let mut q = LiveQueries::default();
        let (overflow, mut overflowed) = mpsc::channel(1);
        let (sub, rx) = notification_channel(1, overflow, Arc::new(AtomicBool::new(false)));
        q.add("q:*", sub);
        drop(rx); // the connection is gone: every send fails
        let mut reached = HashSet::new();
        q.deliver(&KeyChange::removed("q:1".into()), &mut reached);
        assert!(overflowed.try_recv().is_ok());
        assert_no_residue(&q);

        // FLUSHDB's walk drops dead subscribers the same way.
        let (sub, rx) = subscriber(2);
        q.add("f:*", sub);
        drop(rx);
        q.for_each_pattern(|pattern, subs| {
            let sentinel = KeyChange::removed(pattern.to_string());
            subs.retain(|s| s.notify(&sentinel));
        });
        assert_no_residue(&q);
    }
}

#[cfg(test)]
mod live_query_routing_bench {
    use super::*;

    /// The pre-6K-B pattern loop, verbatim but for the registry type: every
    /// pattern globbed against every changed key.
    fn pre_6kb_deliver(
        pats: &mut WatchMap,
        change: &KeyChange<'_>,
        delivered: &mut HashSet<u64>,
    ) -> usize {
        let mut matched = 0;
        for (pattern, subs) in pats.iter_mut() {
            if core_engine::store::glob_match(pattern, &change.key) {
                matched += 1;
                subs.retain(|sub| {
                    if delivered.contains(&sub.conn_id) {
                        return true;
                    }
                    let alive = sub.notify(change);
                    if alive {
                        delivered.insert(sub.conn_id);
                    }
                    alive
                });
            }
        }
        matched
    }

    /// Per-write live-query matching cost, pre-6K-B loop vs `LiveQueries`.
    /// Run with:
    ///
    /// ```text
    /// cargo test --release -p recached --bin recached-server -- --ignored \
    ///     bench_live_query_routing --nocapture
    /// ```
    ///
    /// Patterns are `tenant:<i>:*`, one subscriber connection each. Cases:
    /// `none` (key matches no pattern), `one` (one tenant's key), `many`
    /// (also 50 overlapping `tenant:7*` patterns), `broad` (plus one `*:x`
    /// pattern; the key matches no tenant). Timed in batches of 100
    /// deliveries; receivers are drained between batches, untimed.
    #[test]
    #[ignore = "benchmark; run explicitly in release"]
    fn bench_live_query_routing() {
        println!("patterns,case,pre6kb_ns_per_write,k6b_ns_per_write,speedup,matched");
        for &n in &[10usize, 100, 1_000, 10_000] {
            for case in ["none", "one", "many", "broad"] {
                let mut old = WatchMap::new();
                let mut new = LiveQueries::default();
                let mut rxs = Vec::new();
                let mut add = |p: String, id: u64, old: &mut WatchMap, new: &mut LiveQueries| {
                    let (overflow, _) = mpsc::channel(1);
                    let (sub, rx) =
                        notification_channel(id, overflow, Arc::new(AtomicBool::new(false)));
                    old.entry(p.clone()).or_default().push(sub.clone());
                    new.add(&p, sub);
                    rxs.push(rx);
                };
                for i in 0..n {
                    add(format!("tenant:{i}:*"), i as u64 + 1, &mut old, &mut new);
                }
                if case == "many" {
                    for j in 0..50u64 {
                        add(format!("tenant:7*{j}*"), 100_000 + j, &mut old, &mut new);
                    }
                }
                if case == "broad" {
                    add("*:x".to_string(), 200_000, &mut old, &mut new);
                }
                let key = match case {
                    "none" | "broad" => "other:42:k".to_string(),
                    _ => "tenant:7:k12345".to_string(),
                };
                let change = KeyChange::removed(key);
                let batches = if n >= 10_000 { 20 } else { 200 };
                let (mut old_ns, mut new_ns, mut matched) = (0u128, 0u128, 0usize);
                for _ in 0..batches {
                    let t = std::time::Instant::now();
                    for _ in 0..100 {
                        matched = pre_6kb_deliver(&mut old, &change, &mut HashSet::new());
                    }
                    old_ns += t.elapsed().as_nanos();
                    for rx in &mut rxs {
                        while rx.try_recv().is_ok() {}
                    }
                    let t = std::time::Instant::now();
                    for _ in 0..100 {
                        let (_, m) = new.deliver(&change, &mut HashSet::new());
                        assert_eq!(m, matched);
                    }
                    new_ns += t.elapsed().as_nanos();
                    for rx in &mut rxs {
                        while rx.try_recv().is_ok() {}
                    }
                }
                let writes = (batches * 100) as f64;
                let (o, k) = (old_ns as f64 / writes, new_ns as f64 / writes);
                println!("{n},{case},{o:.0},{k:.0},{:.1},{matched}", o / k.max(1.0));
            }
        }
    }
}
