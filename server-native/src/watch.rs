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
    /// to rather than cloned per connection. Each subscriber is still charged
    /// its full size, so per-connection budgets and overflow are unchanged.
    Full(Arc<Value>),
    /// The mutation, for a connection that sent `CLIENT DELTA ON`.
    Delta(KeyDelta),
}

#[derive(Debug)]
pub(crate) struct WatchNotif {
    pub(crate) key: String,
    pub(crate) payload: NotifPayload,
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
        let delta = change
            .delta
            .as_ref()
            .filter(|_| self.deltas.load(Ordering::Relaxed));
        let payload_bytes = match delta {
            Some(delta) => delta.heap_bytes(),
            None => change.full().1,
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
            None => NotifPayload::Full(Arc::clone(&change.full().0)),
        };
        let notification = WatchNotif {
            key: change.key.clone(),
            payload,
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
    full: OnceLock<(Arc<Value>, usize)>,
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

    fn full(&self) -> &(Arc<Value>, usize) {
        self.full.get_or_init(|| {
            let value = match self.source {
                Some(store) => store.get_current(&self.key),
                None => Value::BulkString(None),
            };
            let bytes = value_heap_bytes(&value);
            (Arc::new(value), bytes)
        })
    }
}

pub(crate) type WatchMap = HashMap<String, Vec<WatchSubscriber>>;

/// Watched-key and live-query registry. `watched_keys` / `watched_patterns`
/// mirror the map lengths (updated by every writer while holding the lock) so
/// the per-write hot path can skip the mutexes entirely when nothing is
/// watched.
pub(crate) struct WatchHub {
    /// Exact-key watchers (WATCH).
    pub(crate) map: tokio::sync::Mutex<WatchMap>,
    pub(crate) watched_keys: AtomicUsize,
    /// Glob-pattern subscribers (QSUB live queries), keyed by pattern.
    pub(crate) patterns: tokio::sync::Mutex<WatchMap>,
    pub(crate) watched_patterns: AtomicUsize,
}

impl WatchHub {
    pub(crate) fn new() -> WatchRegistry {
        Arc::new(WatchHub {
            map: tokio::sync::Mutex::new(HashMap::new()),
            watched_keys: AtomicUsize::new(0),
            patterns: tokio::sync::Mutex::new(HashMap::new()),
            watched_patterns: AtomicUsize::new(0),
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

    /// Call after mutating the pattern map, while still holding the lock.
    pub(crate) fn sync_patterns_len(&self, map: &WatchMap) {
        self.watched_patterns.store(map.len(), Ordering::Relaxed);
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
        if let Some(subs) = pats.get_mut(&p) {
            subs.retain(|sub| sub.conn_id != conn_id);
            if subs.is_empty() {
                pats.remove(&p);
            }
        }
    }
    registry.sync_patterns_len(&pats);
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
