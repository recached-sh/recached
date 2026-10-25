//! Hold a live slice of a Recached cache in your own process.
//!
//! `recached-edge` lets a browser keep a local copy of the cache and receive
//! pushes when it changes. This crate is the same thing for a Rust service:
//! reads come from local memory, and the WebSocket carries only writes out and
//! change notices in.
//!
//! ```no_run
//! # async fn demo() -> Result<(), recached_embed::Error> {
//! use recached_embed::Cache;
//!
//! let cache = Cache::connect("ws://127.0.0.1:6380").await?;
//!
//! // Declare the working set once, at start-up. Returns after the initial
//! // state has landed, so the first request never races hydration.
//! cache.watch("fare:*").await?;
//!
//! // Hot path: no network, no await, no lock.
//! let fare = cache.get("fare:MNL-CEB")?;
//! # Ok(())
//! # }
//! ```
//!
//! # What this is for
//!
//! Small, hot, shared, read-heavy, **slowly-changing** data: fare tables,
//! feature flags, tenant settings, entitlement checks, rate-limit tiers. The
//! win is that a read costs a pointer chase instead of a round-trip, while the
//! server still tells every instance the moment something changes.
//!
//! It is the wrong tool for data that changes constantly (live counters, seat
//! inventory), working sets that do not fit in each process, keys read once, or
//! anything where being stale by milliseconds is unacceptable. Use the server
//! directly for those.
//!
//! # Hydration is part of the contract
//!
//! The local store holds only what your [`watch`](Cache::watch) patterns
//! hydrated. Reading anything else returns [`Error::NotHydrated`] rather than
//! `None`, because a silent `None` is indistinguishable from "the key does not
//! exist" — which is how an embedded cache serves confidently wrong answers.
//! [`get_or_fetch`](Cache::get_or_fetch) is the escape hatch when you would
//! rather pay for a round-trip than declare a pattern.
//!
//! # What survives an outage, and what does not
//!
//! Writes issued while the socket is down are queued **in memory** and
//! replayed on reconnect; they return [`Error::Disconnected`] in the meantime,
//! meaning unacknowledged rather than lost. Two limits on that, both
//! deliberate and both worth sizing for:
//!
//! - The queue is capped ([`CacheBuilder::max_pending`]). Past the cap a new
//!   write is refused with [`Error::OutboxFull`] — nothing queued is touched,
//!   and nothing is sent. Until 0.5 the *oldest* queued write was discarded
//!   instead; [`CacheBuilder::evict_oldest_when_full`] restores that, counted
//!   in [`pending_dropped`](Cache::pending_dropped) and logged at `WARN`.
//! - The queue does not outlive the process. A pod that restarts mid-outage
//!   loses whatever it had queued. The browser SDK backs the same queue with
//!   IndexedDB; there is no equivalent here.
//!
//! Local reads keep working throughout — they just stop receiving updates, so
//! check [`status`](Cache::status) if staleness matters: an open socket is
//! not yet [`SyncPhase::Ready`], which also waits for authentication, the
//! replay of queued writes and every live query's snapshot. On
//! reconnect each live query is re-hydrated from a fresh snapshot, and keys the
//! snapshot no longer carries are dropped locally, so a delete or expiry that
//! happened during the outage is not served afterwards.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod conn;
mod error;

pub use core_engine::resp::Value;
pub use error::{Error, Result};
pub use sync_client::{SubscriptionFailure, SubscriptionState, SyncError, SyncPhase, SyncStatus};

use conn::{Ack, Op, Shared, Task};
pub use core_engine::store::EvictionPolicy;
use core_engine::store::{KeyValueStore, glob_match};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use sync_client::{OverflowPolicy, SyncClient, to_resp, to_resp_bytes};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

/// A handle to the local cache. Cheap to clone and safe to share across tasks;
/// every clone reads the same store.
#[derive(Clone)]
pub struct Cache {
    inner: Arc<Inner>,
}

struct Inner {
    /// The local copy. `execute` takes `&self`, so reads need no outer lock.
    store: Arc<KeyValueStore>,
    ops: mpsc::Sender<Op>,
    /// Live-query patterns whose initial state has landed.
    ///
    /// A `std` lock on purpose: [`get`](Cache::get) consults this on every
    /// read and must not have to `.await` to do it. It is never held across
    /// one — [`watch_lock`](Inner::watch_lock) is what covers the round-trip.
    hydrated: RwLock<Vec<String>>,
    /// Serialises `watch`/`unwatch` so their round-trip and their update of
    /// `hydrated` cannot interleave.
    ///
    /// Without it the two take `hydrated` in separate critical sections either
    /// side of an `.await`: an `unwatch` that finishes inside a slower
    /// `watch`'s round-trip is overwritten by it, leaving a pattern marked
    /// hydrated that the server is no longer tracking — permanently stale
    /// reads, reported as authoritative. Readers never take this lock.
    watch_lock: tokio::sync::Mutex<()>,
    shared: Arc<Shared>,
    request_timeout: Duration,
}

/// Configuration for [`Cache::builder`].
pub struct CacheBuilder {
    url: String,
    password: Option<String>,
    sync_token: Option<String>,
    max_pending: Option<usize>,
    evict_oldest_when_full: bool,
    max_keys: Option<usize>,
    max_memory_bytes: Option<usize>,
    eviction_policy: EvictionPolicy,
    request_timeout: Duration,
    connect_timeout: Duration,
    max_queued_ops: usize,
}

/// Default ceiling on a round-trip to the server.
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Default ceiling on establishing the socket.
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Default depth of the queue between `Cache` handles and the connection task.
const DEFAULT_QUEUED_OPS: usize = 1024;

impl CacheBuilder {
    /// `RECACHED_PASSWORD` for the server.
    #[must_use]
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// A scoped sync token (see `docs/server/sync-scopes.md`). Required when
    /// the server runs with `RECACHED_SYNC_SECRET` set.
    #[must_use]
    pub fn sync_token(mut self, token: impl Into<String>) -> Self {
        self.sync_token = Some(token.into());
        self
    }

    /// Cap on writes queued and not yet acknowledged. Beyond it a new write
    /// is refused with [`Error::OutboxFull`]. The default suits a client that
    /// is briefly offline; a service taking heavy write traffic through a
    /// long partition should size this deliberately.
    #[must_use]
    pub fn max_pending(mut self, max: usize) -> Self {
        self.max_pending = Some(max);
        self
    }

    /// Past [`max_pending`](Self::max_pending), evict the oldest queued
    /// write to admit the new one — the behaviour before 0.5 — instead of
    /// refusing the new one. An evicted write never reaches the server;
    /// [`Cache::pending_dropped`] counts them.
    #[must_use]
    pub fn evict_oldest_when_full(mut self, evict: bool) -> Self {
        self.evict_oldest_when_full = evict;
        self
    }

    /// Cap the memory the local copy may occupy, in bytes.
    ///
    /// **Set this.** Without a cap the size of your process's cache is decided
    /// by whatever the server's keyspace grows to under your `watch` patterns
    /// — a remote party sizing your heap. Pair it with
    /// [`eviction_policy`](Self::eviction_policy); the default policy evicts
    /// nothing, so the cap alone only stops new entries being admitted.
    #[must_use]
    pub fn max_memory(mut self, bytes: usize) -> Self {
        self.max_memory_bytes = Some(bytes);
        self
    }

    /// Cap the number of keys the local copy may hold.
    #[must_use]
    pub fn max_keys(mut self, keys: usize) -> Self {
        self.max_keys = Some(keys);
        self
    }

    /// What to drop when a cap is reached. Defaults to
    /// [`EvictionPolicy::NoEviction`], matching the server.
    #[must_use]
    pub fn eviction_policy(mut self, policy: EvictionPolicy) -> Self {
        self.eviction_policy = policy;
        self
    }

    /// How long a round-trip to the server may take before it returns
    /// [`Error::Timeout`]. Defaults to 10s.
    ///
    /// This is the only thing standing between a caller and an indefinite
    /// hang: a reply the server never sends leaves the awaiting task parked
    /// forever otherwise.
    #[must_use]
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// How long establishing the socket may take. Defaults to 10s.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Depth of the queue between `Cache` handles and the connection task.
    /// Defaults to 1024.
    ///
    /// Bounded so that callers issuing work faster than the socket drains it
    /// wait, rather than the queue growing until the process runs out of
    /// memory.
    #[must_use]
    pub fn max_queued_ops(mut self, depth: usize) -> Self {
        self.max_queued_ops = depth.max(1);
        self
    }

    /// Connect, returning once the socket is open. A later drop reconnects in
    /// the background with jittered backoff.
    pub async fn connect(self) -> Result<Cache> {
        let store = Arc::new(KeyValueStore::with_config(
            self.max_keys,
            self.max_memory_bytes,
            self.eviction_policy,
        ));
        let client_id = format!("{:032x}", rand::random::<u128>());
        let mut client = SyncClient::new(Arc::clone(&store), client_id);

        if let Some(max) = self.max_pending {
            client.set_max_pending(max);
        }
        if self.evict_oldest_when_full {
            client.set_overflow_policy(OverflowPolicy::EvictOldest);
        }
        // Recorded, not sent: `on_open` emits these in the right order on this
        // connection and on every reconnect.
        if let Some(password) = &self.password {
            client.set_password(password, false);
        }
        if let Some(token) = &self.sync_token {
            client.set_sync_token(token, false);
        }

        let shared = Arc::new(Shared {
            connected: AtomicBool::new(false),
            outbox_len: AtomicUsize::new(0),
            dropped_writes: AtomicU64::new(0),
            status: watch::channel(client.status()).0,
            subscriptions: std::sync::Mutex::new(Default::default()),
        });
        let (ops_tx, ops_rx) = mpsc::channel(self.max_queued_ops);
        let (open_tx, open_rx) = oneshot::channel();

        let task = Task::new(
            client,
            Arc::clone(&shared),
            self.url,
            open_tx,
            self.connect_timeout,
        );
        tokio::spawn(task.run(ops_rx));

        open_rx.await.map_err(|_| Error::Closed)??;

        Ok(Cache {
            inner: Arc::new(Inner {
                store,
                ops: ops_tx,
                hydrated: RwLock::new(Vec::new()),
                watch_lock: tokio::sync::Mutex::new(()),
                shared,
                request_timeout: self.request_timeout,
            }),
        })
    }
}

impl Cache {
    /// Start configuring a connection to `url` (`ws://host:6380`).
    ///
    /// Note the port: live queries and change pushes travel on the WebSocket
    /// sync path, not RESP on 6379. A plain TCP client is never sent keychange
    /// notifications, so it cannot hold a coherent local copy.
    #[must_use]
    pub fn builder(url: impl Into<String>) -> CacheBuilder {
        CacheBuilder {
            url: url.into(),
            password: None,
            sync_token: None,
            max_pending: None,
            evict_oldest_when_full: false,
            max_keys: None,
            max_memory_bytes: None,
            eviction_policy: EvictionPolicy::NoEviction,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            max_queued_ops: DEFAULT_QUEUED_OPS,
        }
    }

    /// Connect with defaults.
    pub async fn connect(url: impl Into<String>) -> Result<Cache> {
        Self::builder(url).connect().await
    }

    // ── working set ───────────────────────────────────────────────────────

    /// Hydrate every key matching `pattern` and keep it current.
    ///
    /// Returns only after the server's initial state has been applied, so this
    /// doubles as a start-up barrier. The server caps initial state at 10,000
    /// keys per pattern; a pattern matching more is a sign the working set is
    /// too big to embed.
    ///
    /// Re-subscription after a reconnect is automatic.
    pub async fn watch(&self, pattern: impl Into<String>) -> Result<()> {
        let pattern = pattern.into();
        // Held across the round-trip so a concurrent `unwatch` of the same
        // pattern cannot land between the reply and the registry update.
        let _serialised = self.inner.watch_lock.lock().await;

        self.round_trip(|ack| Op::Watch {
            pattern: pattern.clone(),
            ack,
        })
        .await?;

        // Poisoning cannot leave this inconsistent: the only writes are a
        // `push` and a `retain` on a `Vec<String>`, neither of which can panic
        // partway through and leave a half-updated entry behind.
        let mut hydrated = self
            .inner
            .hydrated
            .write()
            .unwrap_or_else(|e| e.into_inner());
        if !hydrated.contains(&pattern) {
            hydrated.push(pattern);
        }
        Ok(())
    }

    /// Stop tracking `pattern`. Keys it hydrated stay in local memory but stop
    /// receiving updates, so they are dropped from the hydrated set — reading
    /// them now returns [`Error::NotHydrated`] rather than a stale value.
    pub async fn unwatch(&self, pattern: impl Into<String>) -> Result<()> {
        let pattern = pattern.into();
        let _serialised = self.inner.watch_lock.lock().await;

        self.round_trip(|ack| Op::Unwatch {
            pattern: pattern.clone(),
            ack,
        })
        .await?;

        let mut hydrated = self
            .inner
            .hydrated
            .write()
            .unwrap_or_else(|e| e.into_inner());
        hydrated.retain(|p| p != &pattern);
        Ok(())
    }

    /// Whether `key` is covered by a live query, i.e. whether [`get`](Self::get)
    /// can answer for it.
    pub fn is_hydrated(&self, key: &str) -> bool {
        let hydrated = self
            .inner
            .hydrated
            .read()
            .unwrap_or_else(|e| e.into_inner());
        hydrated.iter().any(|p| glob_match(p, key))
    }

    // ── local reads (no network) ──────────────────────────────────────────

    /// Read a string value from local memory.
    ///
    /// `Ok(None)` means the key genuinely does not exist. A key outside every
    /// watched pattern is [`Error::NotHydrated`], never `None`.
    ///
    /// While the socket is down this answers from the last state it saw; the
    /// reconnect re-hydrates each live query from a fresh snapshot and drops
    /// keys the snapshot no longer carries, so a delete or expiry missed
    /// during the outage does not outlive it. Check
    /// [`is_connected`](Self::is_connected) if you need to know which of the
    /// two you are reading.
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.ensure_hydrated(key)?;
        as_bytes(key, self.inner.store.get_current(key))
    }

    /// [`get`](Self::get) with UTF-8 decoding.
    pub fn get_str(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .get(key)?
            .map(|b| String::from_utf8_lossy(&b).into_owned()))
    }

    /// The raw local value, including collection types (returned as the
    /// type-tagged arrays described in `docs/server/protocol.md`).
    pub fn get_value(&self, key: &str) -> Result<Value> {
        self.ensure_hydrated(key)?;
        Ok(self.inner.store.get_current(key))
    }

    /// Whether the key exists locally.
    pub fn contains(&self, key: &str) -> Result<bool> {
        self.ensure_hydrated(key)?;
        Ok(!matches!(
            self.inner.store.get_current(key),
            Value::BulkString(None)
        ))
    }

    /// Every hydrated key matching `pattern`, from local memory.
    ///
    /// This scans the local keyspace, so it is O(n) in the number of cached
    /// keys — fine at start-up or on a background tick, not on a hot path.
    ///
    /// **Collections come back as a type marker, not their contents**: a hash
    /// key yields the simple string `"hash"` (likewise `"list"`, `"set"`,
    /// `"zset"`, `"json"`, `"ratelimit"`), which is indistinguishable from a
    /// string key whose value happens to be `hash`. Call
    /// [`get_value`](Self::get_value) for the real contents of any key this
    /// reports as a collection — it is the accurate reader, and this is the
    /// cheap enumerator.
    pub fn matching(&self, pattern: &str) -> Vec<(String, Value)> {
        self.inner.store.matching_key_values(pattern, usize::MAX)
    }

    /// Every hydrated key matching `pattern`, without its value.
    ///
    /// Prefer this to [`matching`](Self::matching) when you only need the key
    /// names: it clones no values and has no collection-marker caveat.
    pub fn matching_keys(&self, pattern: &str) -> Vec<String> {
        self.inner.store.matching_keys(pattern)
    }

    // ── round-trips ───────────────────────────────────────────────────────

    /// Local read, falling back to one round-trip when the key is not covered
    /// by a watched pattern.
    ///
    /// The fetched value is **not** cached: nothing is keeping it current, and
    /// storing an unwatched key locally would make it stale on the first
    /// change. Prefer [`watch`](Self::watch) for anything read repeatedly.
    pub async fn get_or_fetch(&self, key: &str) -> Result<Option<Vec<u8>>> {
        match self.get(key) {
            Err(Error::NotHydrated { .. }) => {}
            other => return other,
        }
        let reply = self
            .round_trip(|ack| Op::Read {
                frame: to_resp(&["GET", key]),
                ack,
            })
            .await?;
        as_bytes(key, reply)
    }

    // ── writes ────────────────────────────────────────────────────────────

    /// Set a key. Resolves when the server acknowledges the write.
    ///
    /// While disconnected the write is queued in memory and replayed on
    /// reconnect, and this returns [`Error::Disconnected`] — unacknowledged
    /// rather than lost. Two caveats, both covered in the crate docs: the
    /// queue is capped, and a write past the cap is refused with
    /// [`Error::OutboxFull`] instead of queued; and the queue does not
    /// survive a process restart.
    ///
    /// `Ok` means the server applied the write — in its memory, not
    /// necessarily on its disk: what survives a server crash is the server's
    /// persistence configuration, not this acknowledgment.
    pub async fn set(&self, key: &str, value: impl AsRef<[u8]>) -> Result<()> {
        self.write(to_resp_bytes(&[b"SET", key.as_bytes(), value.as_ref()]))
            .await
            .map(drop)
    }

    /// Set a key with a time-to-live.
    pub async fn set_ex(&self, key: &str, value: impl AsRef<[u8]>, ttl: Duration) -> Result<()> {
        let ms = ttl.as_millis().to_string();
        self.write(to_resp_bytes(&[
            b"SET",
            key.as_bytes(),
            value.as_ref(),
            b"PX",
            ms.as_bytes(),
        ]))
        .await
        .map(drop)
    }

    /// Delete a key.
    pub async fn del(&self, key: &str) -> Result<()> {
        self.write(to_resp(&["DEL", key])).await.map(drop)
    }

    /// Add to a counter, returning its new value.
    pub async fn incr_by(&self, key: &str, delta: i64) -> Result<i64> {
        let reply = self
            .write(to_resp(&["INCRBY", key, &delta.to_string()]))
            .await?;
        match reply {
            Value::Integer(n) => Ok(n),
            _ => Err(Error::WrongType {
                key: key.to_string(),
            }),
        }
    }

    /// Send an arbitrary mutation, returning the server's reply.
    ///
    /// Goes through the outbox: it is replayed on reconnect and wrapped in an
    /// duplicate-suppression envelope, like [`set`](Self::set). Use it for
    /// commands the typed helpers do not cover (`HSET`, `SADD`, `ZADD`, …).
    ///
    /// Only pass mutations. A read sent here would be replayed pointlessly
    /// after a reconnect — use [`read_command`](Self::read_command) instead.
    pub async fn write_command(&self, args: &[&str]) -> Result<Value> {
        self.write(to_resp(args)).await
    }

    /// Send an arbitrary command as a one-shot round-trip, never replayed.
    ///
    /// The reply is returned as-is and nothing is applied to the local store.
    pub async fn read_command(&self, args: &[&str]) -> Result<Value> {
        let frame = to_resp(args);
        self.round_trip(|ack| Op::Read { frame, ack }).await
    }

    // ── pub/sub ───────────────────────────────────────────────────────────

    /// Subscribe to a channel. Re-subscribed automatically after a reconnect;
    /// messages published during the gap are lost.
    pub async fn subscribe(
        &self,
        channel: impl Into<String>,
    ) -> Result<broadcast::Receiver<Vec<u8>>> {
        let (tx, rx) = oneshot::channel();
        self.send_op(Op::Subscribe {
            channel: channel.into(),
            ack: tx,
        })
        .await?;
        tokio::time::timeout(self.inner.request_timeout, rx)
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(|_| Error::Closed)?
    }

    /// Publish to a channel.
    ///
    /// Sent one-shot rather than through the outbox: replaying a publish after
    /// a reconnect would deliver it twice.
    pub async fn publish(&self, channel: &str, message: impl AsRef<[u8]>) -> Result<()> {
        self.round_trip(|ack| Op::Read {
            frame: to_resp_bytes(&[b"PUBLISH", channel.as_bytes(), message.as_ref()]),
            ack,
        })
        .await
        .map(drop)
    }

    // ── introspection ─────────────────────────────────────────────────────

    /// Whether the sync socket is currently up. Local reads keep working
    /// either way — they just stop receiving updates.
    ///
    /// An open socket is not a synchronized one: see [`status`](Self::status).
    pub fn is_connected(&self) -> bool {
        self.inner.shared.connected.load(Ordering::Acquire)
    }

    /// Where synchronization stands: [`SyncPhase::Ready`] once the session
    /// is authenticated, queued writes have been replayed and answered, and
    /// every live query has its snapshot.
    pub fn status(&self) -> SyncStatus {
        self.inner.shared.status.borrow().clone()
    }

    /// Every status change as it happens, without polling.
    ///
    /// ```no_run
    /// # async fn demo(cache: recached_embed::Cache) {
    /// let mut changes = cache.status_changes();
    /// while changes.changed().await.is_ok() {
    ///     let status = changes.borrow().clone();
    ///     tracing::info!(?status, "sync status");
    /// }
    /// # }
    /// ```
    pub fn status_changes(&self) -> watch::Receiver<SyncStatus> {
        self.inner.shared.status.subscribe()
    }

    /// One live query's progress, or `None` when `pattern` is not watched.
    pub fn subscription_state(&self, pattern: &str) -> Option<SubscriptionState> {
        self.inner
            .shared
            .subscriptions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(pattern)
            .cloned()
    }

    /// Writes queued but not yet acknowledged by the server.
    pub fn pending_writes(&self) -> usize {
        self.inner.shared.outbox_len.load(Ordering::Acquire)
    }

    /// Writes discarded because the pending queue was full, since start-up.
    /// Always zero unless [`CacheBuilder::evict_oldest_when_full`] is set.
    ///
    /// Non-zero means writes were lost, not merely delayed: the queue evicted
    /// its oldest entry to admit a new one, and an evicted write is never
    /// replayed. Each one is also logged at `WARN`. If this moves, raise
    /// [`CacheBuilder::max_pending`] or send fewer writes through an outage.
    pub fn pending_dropped(&self) -> u64 {
        self.inner.shared.dropped_writes.load(Ordering::Acquire)
    }

    /// Approximate bytes the local copy occupies in this process.
    pub fn local_bytes(&self) -> usize {
        self.inner.store.approximate_memory_bytes()
    }

    /// The underlying local store, for reads this API does not wrap.
    ///
    /// Bypasses the hydration check — anything read here may be absent simply
    /// because it was never watched.
    pub fn store(&self) -> &Arc<KeyValueStore> {
        &self.inner.store
    }

    // ── internals ─────────────────────────────────────────────────────────

    fn ensure_hydrated(&self, key: &str) -> Result<()> {
        if self.is_hydrated(key) {
            Ok(())
        } else {
            Err(Error::NotHydrated {
                key: key.to_string(),
            })
        }
    }

    async fn round_trip(&self, make: impl FnOnce(Ack) -> Op) -> Result<Value> {
        let (tx, rx) = oneshot::channel();
        self.send_op(make(tx)).await?;
        self.await_reply(rx).await
    }

    async fn write(&self, encoded: Vec<u8>) -> Result<Value> {
        let (tx, rx) = oneshot::channel();
        self.send_op(Op::Write {
            encoded,
            ack: Some(tx),
        })
        .await?;
        self.await_reply(rx).await
    }

    /// Hand one op to the connection task, waiting if its queue is full.
    ///
    /// The queue is bounded, so this is where backpressure lands: a caller
    /// outrunning the socket waits here instead of growing a queue until the
    /// process dies.
    async fn send_op(&self, op: Op) -> Result<()> {
        tokio::time::timeout(self.inner.request_timeout, self.inner.ops.send(op))
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(|_| Error::Closed)
    }

    /// Wait for the connection task's answer, but never indefinitely.
    ///
    /// A reply that never arrives — a desynchronised FIFO, a server that
    /// answered nothing — would otherwise park the caller's task forever.
    /// Dropping the `Ack` here is safe: the task's `send` on it simply fails,
    /// and the reply slot it occupies is still consumed in order.
    async fn await_reply(&self, rx: oneshot::Receiver<Result<Value>>) -> Result<Value> {
        tokio::time::timeout(self.inner.request_timeout, rx)
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(|_| Error::Closed)?
    }
}

fn as_bytes(key: &str, value: Value) -> Result<Option<Vec<u8>>> {
    match value {
        Value::BulkString(Some(bytes)) => Ok(Some(bytes)),
        Value::BulkString(None) => Ok(None),
        Value::SimpleString(s) => Ok(Some(s.into_bytes())),
        Value::Integer(n) => Ok(Some(n.to_string().into_bytes())),
        _ => Err(Error::WrongType {
            key: key.to_string(),
        }),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────
//
// The end-to-end behaviour lives in `tests/live.rs`, which needs a running
// server and is driven by CI. What is here is the logic that decides what a
// caller sees, which should not need a socket to verify.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_key_is_none_but_a_collection_is_an_error() {
        // The distinction the whole crate rests on: absent is `None`; the
        // wrong shape is an error, never a plausible-looking empty answer.
        assert_eq!(as_bytes("k", Value::BulkString(None)), Ok(None));
        assert_eq!(
            as_bytes("k", Value::BulkString(Some(b"v".to_vec()))),
            Ok(Some(b"v".to_vec()))
        );
        assert_eq!(
            as_bytes("k", Value::Array(Some(vec![]))),
            Err(Error::WrongType {
                key: "k".to_string()
            })
        );
    }

    #[test]
    fn an_integer_reads_back_as_its_digits() {
        // `INCRBY` replies with an integer; a caller doing `get()` on the same
        // key should see what they would have seen from the server.
        assert_eq!(
            as_bytes("n", Value::Integer(-42)),
            Ok(Some(b"-42".to_vec()))
        );
    }

    #[test]
    fn hydration_is_decided_by_pattern_not_by_presence() {
        // `is_hydrated` must answer for keys that do not exist: that is the
        // whole point — "watched and absent" has to be distinguishable from
        // "never watched".
        assert!(glob_match("fare:*", "fare:MNL-CEB"));
        assert!(glob_match("fare:*", "fare:anything-at-all"));
        assert!(!glob_match("fare:*", "user:42"));
    }

    #[test]
    fn the_builder_defaults_are_the_documented_ones() {
        // These are load-bearing: an unset request timeout is an indefinite
        // hang, and an unbounded op queue is a memory-growth path.
        let b = Cache::builder("ws://127.0.0.1:6380");
        assert_eq!(b.request_timeout, DEFAULT_REQUEST_TIMEOUT);
        assert_eq!(b.connect_timeout, DEFAULT_CONNECT_TIMEOUT);
        assert_eq!(b.max_queued_ops, DEFAULT_QUEUED_OPS);
        assert_eq!(b.max_memory_bytes, None);
    }

    #[test]
    fn a_zero_depth_op_queue_is_clamped_rather_than_accepted() {
        // `mpsc::channel(0)` panics, and the caller's intent ("as small as
        // possible") is servable without one.
        let b = Cache::builder("ws://127.0.0.1:6380").max_queued_ops(0);
        assert_eq!(b.max_queued_ops, 1);
    }
}
