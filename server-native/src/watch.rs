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
    /// The key's whole current value.
    Full(Value),
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
    /// `delta` is the mutation's compact form when it has one; this subscriber
    /// takes it only if its connection asked for deltas.
    pub(crate) fn notify(&self, key: &str, value: &Value, delta: Option<&KeyDelta>) -> bool {
        let payload = match delta {
            Some(delta) if self.deltas.load(Ordering::Relaxed) => {
                NotifPayload::Delta(delta.clone())
            }
            _ => NotifPayload::Full(value.clone()),
        };
        let payload_bytes = match &payload {
            NotifPayload::Full(value) => value_heap_bytes(value),
            NotifPayload::Delta(delta) => delta.heap_bytes(),
        };
        let charged_bytes = key
            .len()
            .saturating_add(payload_bytes)
            .saturating_add(std::mem::size_of::<WatchNotif>());
        if !self.budget.reserve(charged_bytes) {
            self.signal_overflow();
            return false;
        }

        let notification = WatchNotif {
            key: key.to_string(),
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

fn value_heap_bytes(value: &Value) -> usize {
    match value {
        Value::SimpleString(value) | Value::Error(value) => value.len(),
        Value::Integer(_) => std::mem::size_of::<i64>(),
        Value::BulkString(value) => value.as_ref().map_or(0, Vec::len),
        Value::Array(value) => value
            .as_ref()
            .map_or(0, |items| items.iter().map(value_heap_bytes).sum()),
        Value::Push(items) => items.iter().map(value_heap_bytes).sum(),
        Value::Map(entries) => entries
            .iter()
            .map(|(key, value)| value_heap_bytes(key).saturating_add(value_heap_bytes(value)))
            .sum(),
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
