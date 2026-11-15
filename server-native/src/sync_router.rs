//! Scope-aware routing of WebSocket sync pushes.
//!
//! Until 0.6 every mutation was sent on one `tokio::broadcast` channel that
//! every WebSocket connection received from. Each connection woke for every
//! write on the server, checked it against its own scopes and, for a tenant
//! that could not see it, threw it away — so the work per write grew with the
//! number of connected browsers, whoever they belonged to.
//!
//! [`SyncRouter`] keeps an index from what a connection may see to the
//! connection's own bounded mailbox, and delivers a push only to the
//! connections the index names as *candidates*. It is a candidate-selection
//! optimisation and nothing more:
//!
//! * **The receiving connection's check stays authoritative.** Every push a
//!   connection takes from its mailbox still goes through
//!   [`forwards_push`] against the scopes, watches and
//!   live queries it holds *at that moment*. The router can therefore only
//!   ever widen what is checked, never authorise anything.
//! * **Routing never under-selects.** A connection whose grants could match a
//!   push is always a candidate. Grants the index cannot place precisely
//!   (no literal prefix, such as `*:user1`) put the connection in the *broad*
//!   set, which is offered every push. Broad grants cost what every grant cost
//!   before; they are not routed in constant time and nothing here claims so.
//!
//! # What the index holds
//!
//! The server's glob (`core_engine::store::glob_match`) knows exactly two
//! metacharacters, `*` and `?`, and matches bytes. Every byte of a pattern
//! before its first metacharacter must equal the same byte of any key it
//! matches, so that *literal prefix* is a sound necessary condition:
//!
//! | Grant pattern | Indexed as | A key is a candidate when |
//! |---|---|---|
//! | `tenant:a:book:1` (no metacharacter) | exact | it equals the pattern |
//! | `tenant:a:*`, `tenant:a:?:x` | prefix `tenant:a:` | it starts with the prefix |
//! | `*`, `*:user1`, `?x` | broad | always |
//!
//! A push is routed on the first key it names. Visibility needs a grant for
//! *every* named key ([`scopes_match`]), so a connection
//! that cannot match the first key cannot see the push; checking the other
//! keys is left to the authoritative filter. A push naming no key goes to the
//! broad set only (open-mode connections without scopes live there), unless it
//! is keyspace-wide (`FLUSHDB`), which is offered to every connection.
//!
//! # Ordering and lost updates
//!
//! Every write routes *after* its mutation is in the store, and a
//! connection's routes are published (the table's write lock released) before
//! that connection handles anything else. So a write either finds the new
//! routes, or completed before they were published and is visible to every
//! read the connection makes afterwards — the same argument the
//! `UnorderedGate` makes for registering a receiver at all. Narrowing routes
//! cannot leak: a push routed under the old routes still meets the
//! connection's *current* scopes in the authoritative filter.
//!
//! Per-key order is unchanged: writes to one key are serialised by their
//! replication stripe until their push is offered, and each mailbox is FIFO.
//!
//! # Backpressure
//!
//! A mailbox holds at most `capacity` pushes and [`MAILBOX_BYTE_BUDGET`]
//! bytes of them (always at least one push, however large). When a push does
//! not fit, the mailbox records a gap and the connection is closed with the
//! sync-gap code once it notices — the same contract the broadcast channel's
//! `Lagged` error had, except that only pushes a connection is a candidate for
//! count towards its limit now. A writer never waits on a mailbox.
//!
//! The gap and the queue's occupancy are one atomic word, so deciding that a
//! push does not fit and recording the gap are a single step, and so are
//! taking a push and learning whether a gap opened before it. Nothing
//! admitted after a dropped push is ever delivered.

use crate::*;
use indexmap::IndexMap;
use std::sync::RwLock;
use tokio::sync::{Notify, mpsc};

/// Most bytes of push frames one mailbox may hold queued. A mailbox with
/// nothing queued always takes one push, however large, so a big value is
/// never undeliverable; past that, a connection that stops reading pins at
/// most this much (plus the one frame it is stuck sending) until it is closed.
///
/// The broadcast channel this replaced kept 512 frames for the whole server
/// and pinned nothing for a lagging receiver; a mailbox per connection would
/// otherwise let each stalled connection pin 512 frames of its own.
pub(crate) const MAILBOX_BYTE_BUDGET: u64 = 16 << 20;

/// `Mailbox::state`: bit 63 is the gap, bits 40–62 the queued pushes, bits
/// 0–39 their bytes.
const GAP_BIT: u64 = 1 << 63;
const COUNT_SHIFT: u32 = 40;
const COUNT_ONE: u64 = 1 << COUNT_SHIFT;
const COUNT_MAX: u64 = (1 << 23) - 1;
const BYTES_MAX: u64 = COUNT_ONE - 1;

/// What a push counts against a mailbox's byte budget.
fn push_bytes(push: &SyncPush) -> u64 {
    u64::try_from(push.resp.len())
        .unwrap_or(BYTES_MAX)
        .min(BYTES_MAX)
}

/// One connection's queue of candidate pushes.
struct Mailbox {
    tx: mpsc::Sender<SyncMsg>,
    /// Gap flag, queued count and queued bytes, as one word (see the
    /// constants above). Every push queued was admitted by a successful
    /// compare-exchange on it, and once the gap bit is set nothing is.
    state: AtomicU64,
    capacity: u64,
    byte_budget: u64,
    /// Wakes a connection idle in [`SyncSubscription::recv`] when a gap opens
    /// with nothing else queued for it.
    gap_notify: Notify,
}

impl Mailbox {
    fn has_gap(&self) -> bool {
        self.state.load(Ordering::Acquire) & GAP_BIT != 0
    }

    /// Offer one push. False when it was not queued: the mailbox already has
    /// a gap, it just got one, or its connection is gone.
    fn offer(&self, push: &SyncMsg) -> bool {
        let len = push_bytes(push);
        let mut cur = self.state.load(Ordering::Acquire);
        let admitted = loop {
            if cur & GAP_BIT != 0 {
                return false;
            }
            let count = (cur >> COUNT_SHIFT) & COUNT_MAX;
            let bytes = cur & BYTES_MAX;
            // `bytes + len` stays within the byte field: either the mailbox
            // is empty (`len ≤ BYTES_MAX`) or the sum is within the budget.
            let fits = count < self.capacity
                && (bytes == 0 || bytes.saturating_add(len) <= self.byte_budget);
            let next = match fits.then(|| cur.checked_add(COUNT_ONE)?.checked_add(len)) {
                Some(Some(next)) => next,
                _ => cur | GAP_BIT,
            };
            match self
                .state
                .compare_exchange_weak(cur, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => break next & GAP_BIT == 0,
                Err(actual) => cur = actual,
            }
        };
        if !admitted {
            self.gap_notify.notify_one();
            return false;
        }
        match self.tx.try_send(Arc::clone(push)) {
            Ok(()) => true,
            // Not reachable: the count admitted above never exceeds the
            // channel's capacity, and a slot is freed before its count is.
            // Should it happen, it is a gap like any other.
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.state.fetch_or(GAP_BIT, Ordering::AcqRel);
                self.gap_notify.notify_one();
                false
            }
            // The connection is gone; its mailbox goes with it.
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        }
    }

    /// Account for a push taken from the queue. True when a gap had opened
    /// by then: the push must not be delivered.
    fn release(&self, push: &SyncPush) -> bool {
        let prev = self
            .state
            .fetch_sub(COUNT_ONE.saturating_add(push_bytes(push)), Ordering::AcqRel);
        prev & GAP_BIT != 0
    }
}

/// More grants than this and a connection is routed as broad instead of
/// indexed grant by grant: still never under-selected, and registering it
/// cannot hold the router's write lock for long. A token or `SYNC` list this
/// long is not a normal shape; its connection pays what every connection
/// paid before routing.
pub(crate) const MAX_INDEXED_GRANTS: usize = 256;

/// What a connection may currently be pushed, as handed to the router.
#[derive(Clone, Copy)]
pub(crate) enum Routes<'a> {
    /// Nothing: a connection that has not authenticated, or a strict one
    /// that has not presented a token.
    Nothing,
    /// Every push: an open-mode connection that set no scopes.
    Everything,
    /// Pushes these grants could make visible.
    Grants(&'a [Grant]),
}

/// Where one subscriber sits in the index, so it can be taken out exactly.
///
/// Always reduced to a prefix-free set: no kept prefix starts with another,
/// and no exact key starts with a kept prefix. Any key then matches at most
/// one of a subscriber's entries, so a push reaches a mailbox at most once
/// without any per-push de-duplication.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Placement {
    /// Counted in `routed`: the connection holds scopes (possibly none for
    /// keys — a channel-only token — which still sees `FLUSHDB`) or is open.
    routed: bool,
    broad: bool,
    exact: Vec<Box<str>>,
    prefixes: Vec<Box<[u8]>>,
}

impl Placement {
    fn is_empty(&self) -> bool {
        !self.routed
    }

    fn broad() -> Placement {
        Placement {
            routed: true,
            broad: true,
            ..Placement::default()
        }
    }

    fn of(routes: Routes<'_>) -> Placement {
        let grants = match routes {
            Routes::Nothing => return Placement::default(),
            Routes::Everything => return Placement::broad(),
            Routes::Grants(grants) if grants.len() > MAX_INDEXED_GRANTS => {
                return Placement::broad();
            }
            Routes::Grants(grants) => grants,
        };
        let mut prefixes: Vec<&[u8]> = Vec::new();
        let mut exact: Vec<&str> = Vec::new();
        for grant in grants {
            let pattern = grant.pattern.as_bytes();
            match pattern.iter().position(|b| matches!(b, b'*' | b'?')) {
                None => exact.push(&grant.pattern),
                // No literal prefix: any key may match.
                Some(0) => return Placement::broad(),
                Some(at) => {
                    if let Some(prefix) = pattern.get(..at) {
                        prefixes.push(prefix);
                    }
                }
            }
        }
        // Keys sharing a prefix are contiguous in byte order, so after sorting
        // a prefix is covered exactly when the last one kept is a prefix of it.
        prefixes.sort_unstable();
        prefixes.dedup();
        let mut kept: Vec<&[u8]> = Vec::with_capacity(prefixes.len());
        for prefix in prefixes {
            if !kept.last().is_some_and(|k| prefix.starts_with(k)) {
                kept.push(prefix);
            }
        }
        exact.sort_unstable();
        exact.dedup();
        // `kept` is sorted and prefix-free, so the only kept prefix that can
        // cover a key is the greatest one not after it.
        exact.retain(|key| {
            let key = key.as_bytes();
            let at = kept.partition_point(|k| *k <= key);
            !at.checked_sub(1)
                .and_then(|i| kept.get(i))
                .is_some_and(|k| key.starts_with(k))
        });
        Placement {
            routed: true,
            broad: false,
            exact: exact.into_iter().map(Box::from).collect(),
            prefixes: kept.into_iter().map(Box::from).collect(),
        }
    }
}

type Bucket = IndexMap<u64, Arc<Mailbox>>;

struct Subscriber {
    mailbox: Arc<Mailbox>,
    placement: Placement,
}

#[derive(Default)]
struct RouteTable {
    /// Every live subscription, routed or not.
    subscribers: HashMap<u64, Subscriber>,
    broad: Bucket,
    exact: HashMap<Box<str>, Bucket>,
    prefixes: HashMap<Box<[u8]>, Bucket>,
    /// Each distinct prefix length in `prefixes`, with how many prefixes have
    /// it: a key is looked up once per length no longer than itself.
    prefix_lens: BTreeMap<usize, usize>,
}

impl RouteTable {
    fn place(&mut self, id: u64, mailbox: &Arc<Mailbox>, placement: &Placement) {
        if placement.broad {
            self.broad.insert(id, Arc::clone(mailbox));
        }
        for key in &placement.exact {
            self.exact
                .entry(key.clone())
                .or_default()
                .insert(id, Arc::clone(mailbox));
        }
        for prefix in &placement.prefixes {
            let bucket = self.prefixes.entry(prefix.clone()).or_default();
            if bucket.is_empty() {
                *self.prefix_lens.entry(prefix.len()).or_default() += 1;
            }
            bucket.insert(id, Arc::clone(mailbox));
        }
    }

    fn unplace(&mut self, id: u64, placement: &Placement) {
        if placement.broad {
            self.broad.swap_remove(&id);
        }
        for key in &placement.exact {
            if let Some(bucket) = self.exact.get_mut(key) {
                bucket.swap_remove(&id);
                if bucket.is_empty() {
                    self.exact.remove(key);
                }
            }
        }
        for prefix in &placement.prefixes {
            let Some(bucket) = self.prefixes.get_mut(prefix) else {
                continue;
            };
            bucket.swap_remove(&id);
            if !bucket.is_empty() {
                continue;
            }
            self.prefixes.remove(prefix);
            if let Some(count) = self.prefix_lens.get_mut(&prefix.len()) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.prefix_lens.remove(&prefix.len());
                }
            }
        }
        // A reconnect storm grows these to its peak; give the memory back
        // once they are mostly empty again.
        shrink_if_sparse(&mut self.exact);
        shrink_if_sparse(&mut self.prefixes);
        shrink_if_sparse(&mut self.subscribers);
        shrink_bucket_if_sparse(&mut self.broad);
    }

    /// Call `f` on every candidate mailbox for a push whose first named key
    /// is `first` (none for a keyless push). Each mailbox at most once.
    fn for_each_candidate(
        &self,
        first: Option<&str>,
        keyspace_wide: bool,
        mut f: impl FnMut(&Arc<Mailbox>),
    ) {
        if keyspace_wide {
            for sub in self.subscribers.values() {
                f(&sub.mailbox);
            }
            return;
        }
        for mailbox in self.broad.values() {
            f(mailbox);
        }
        let Some(key) = first else {
            return;
        };
        if let Some(bucket) = self.exact.get(key) {
            for mailbox in bucket.values() {
                f(mailbox);
            }
        }
        let bytes = key.as_bytes();
        // `..=len`, not `1..=len`: `BTreeMap::range` panics when the start
        // exceeds the end, and the empty key is a key. Every stored prefix
        // is at least one byte (an empty one is the broad set).
        for &len in self.prefix_lens.range(..=bytes.len()).map(|(len, _)| len) {
            if let Some(bucket) = bytes.get(..len).and_then(|p| self.prefixes.get(p)) {
                for mailbox in bucket.values() {
                    f(mailbox);
                }
            }
        }
    }

    /// Whether any mailbox is a candidate for such a push.
    fn has_candidate(&self, first: Option<&str>, keyspace_wide: bool) -> bool {
        if keyspace_wide {
            return !self.subscribers.is_empty();
        }
        if !self.broad.is_empty() {
            return true;
        }
        let Some(key) = first else {
            return false;
        };
        if self.exact.contains_key(key) {
            return true;
        }
        let bytes = key.as_bytes();
        self.prefix_lens.range(..=bytes.len()).any(|(&len, _)| {
            bytes
                .get(..len)
                .is_some_and(|p| self.prefixes.contains_key(p))
        })
    }
}

fn shrink_bucket_if_sparse(bucket: &mut Bucket) {
    if bucket.capacity() >= 1024 && bucket.len().saturating_mul(8) < bucket.capacity() {
        bucket.shrink_to_fit();
    }
}

fn shrink_if_sparse<K: std::hash::Hash + Eq, V>(map: &mut HashMap<K, V>) {
    if map.capacity() >= 1024 && map.len().saturating_mul(8) < map.capacity() {
        map.shrink_to_fit();
    }
}

/// Counters striped across cache lines so writers on different workers do not
/// contend on one: these are bumped on every routed write.
struct StripedCounter {
    slots: [PaddedU64; COUNTER_SLOTS],
}

const COUNTER_SLOTS: usize = 16;

#[repr(align(128))]
struct PaddedU64(AtomicU64);

impl StripedCounter {
    fn new() -> Self {
        StripedCounter {
            slots: std::array::from_fn(|_| PaddedU64(AtomicU64::new(0))),
        }
    }

    fn add(&self, n: u64) {
        thread_local! {
            static SLOT: usize = {
                static NEXT: AtomicUsize = AtomicUsize::new(0);
                NEXT.fetch_add(1, Ordering::Relaxed) % COUNTER_SLOTS
            };
        }
        let slot = SLOT.with(|slot| *slot);
        if let Some(PaddedU64(count)) = self.slots.get(slot) {
            count.fetch_add(n, Ordering::Relaxed);
        }
    }

    fn sum(&self) -> u64 {
        self.slots
            .iter()
            .map(|PaddedU64(c)| c.load(Ordering::Relaxed))
            .fold(0u64, u64::wrapping_add)
    }
}

/// Router counters, reported by `INFO`.
pub(crate) struct RouteStats {
    /// Pushes offered for routing (after the cheap early-outs).
    events: StripedCounter,
    /// Mailboxes offered a push, summed over pushes.
    candidates: StripedCounter,
    /// Writes that built no push because no connection was a candidate.
    skipped: StripedCounter,
    /// Pushes a connection took from its mailbox and sent.
    forwarded: StripedCounter,
    /// Pushes a connection took from its mailbox and dropped (not visible to
    /// it, its own write, or superseded by a keychange it also gets).
    filtered: StripedCounter,
}

/// A snapshot of [`RouteStats`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RouteStatsSnapshot {
    pub(crate) subscribers: usize,
    pub(crate) routed: usize,
    pub(crate) broad: usize,
    pub(crate) exact_keys: usize,
    pub(crate) prefixes: usize,
    pub(crate) events: u64,
    pub(crate) candidates: u64,
    pub(crate) skipped: u64,
    pub(crate) forwarded: u64,
    pub(crate) filtered: u64,
}

/// The router: the index plus every subscriber's mailbox. Shared as [`SyncTx`].
pub(crate) struct SyncRouter {
    capacity: usize,
    byte_budget: u64,
    table: RwLock<RouteTable>,
    next_id: AtomicU64,
    /// Subscribers whose placement is not empty. Zero means no push can be
    /// delivered, which a write checks with one atomic load.
    routed: AtomicUsize,
    pub(crate) stats: RouteStats,
}

/// How the rest of the server holds the router.
pub(crate) type SyncTx = Arc<SyncRouter>;

impl SyncRouter {
    /// A router whose mailboxes each hold up to `capacity` pushes.
    pub(crate) fn new(capacity: usize) -> SyncTx {
        SyncRouter::with_byte_budget(capacity, MAILBOX_BYTE_BUDGET)
    }

    /// [`new`](Self::new) with a mailbox byte budget other than
    /// [`MAILBOX_BYTE_BUDGET`].
    pub(crate) fn with_byte_budget(capacity: usize, byte_budget: u64) -> SyncTx {
        Arc::new(SyncRouter {
            // The count field of a mailbox's state word bounds its capacity.
            capacity: capacity.clamp(1, usize::try_from(COUNT_MAX).unwrap_or(usize::MAX)),
            byte_budget: byte_budget.clamp(1, BYTES_MAX),
            table: RwLock::new(RouteTable::default()),
            next_id: AtomicU64::new(1),
            routed: AtomicUsize::new(0),
            stats: RouteStats {
                events: StripedCounter::new(),
                candidates: StripedCounter::new(),
                skipped: StripedCounter::new(),
                forwarded: StripedCounter::new(),
                filtered: StripedCounter::new(),
            },
        })
    }

    // Poisoning: a panic while the table is held would come from an
    // allocation failure inside a map operation. The table is then at worst
    // missing or holding entries for one subscriber; a stale entry offers to
    // a closed mailbox (a no-op), and the alternative — every later write
    // panicking — is worse. Recover the guard.
    fn read(&self) -> std::sync::RwLockReadGuard<'_, RouteTable> {
        self.table.read().unwrap_or_else(|p| p.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, RouteTable> {
        self.table.write().unwrap_or_else(|p| p.into_inner())
    }

    /// Register a subscriber with its initial routes. Callers that need the
    /// `UnorderedGate` guarantee go through `ReplHub::subscribe_pushes`.
    pub(crate) fn subscribe(self: &Arc<Self>, routes: Routes<'_>) -> SyncSubscription {
        let (tx, rx) = mpsc::channel(self.capacity);
        let mailbox = Arc::new(Mailbox {
            tx,
            state: AtomicU64::new(0),
            capacity: u64::try_from(self.capacity).unwrap_or(COUNT_MAX),
            byte_budget: self.byte_budget,
            gap_notify: Notify::new(),
        });
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let placement = Placement::of(routes);
        {
            let mut table = self.write();
            table.place(id, &mailbox, &placement);
            if !placement.is_empty() {
                self.routed.fetch_add(1, Ordering::SeqCst);
            }
            table.subscribers.insert(
                id,
                Subscriber {
                    mailbox: Arc::clone(&mailbox),
                    placement,
                },
            );
        }
        SyncSubscription {
            router: Arc::clone(self),
            id,
            rx,
            mailbox,
            receiver: None,
        }
    }

    fn set_routes(&self, id: u64, routes: Routes<'_>) {
        // Computed outside the lock; a renewal granting the same scopes —
        // the usual case — then costs one read of the subscriber's entry.
        // The read-then-write is not a race: a subscriber's routes are only
        // changed by its own connection task, and removed by its own drop.
        let placement = Placement::of(routes);
        if self
            .read()
            .subscribers
            .get(&id)
            .is_some_and(|sub| sub.placement == placement)
        {
            return;
        }
        let mut table = self.write();
        let Some(sub) = table.subscribers.get_mut(&id) else {
            return;
        };
        let old = std::mem::replace(&mut sub.placement, placement.clone());
        let mailbox = Arc::clone(&sub.mailbox);
        // Out, then in: buckets are keyed by subscriber, so placing first
        // would have the removal take out entries the new routes share with
        // the old. No push routes while the write lock is held, so nothing
        // observes the moment in between.
        table.unplace(id, &old);
        table.place(id, &mailbox, &placement);
        match (old.is_empty(), placement.is_empty()) {
            (true, false) => {
                self.routed.fetch_add(1, Ordering::SeqCst);
            }
            (false, true) => {
                self.routed.fetch_sub(1, Ordering::SeqCst);
            }
            _ => {}
        }
    }

    fn remove(&self, id: u64) {
        let mut table = self.write();
        if let Some(sub) = table.subscribers.remove(&id) {
            table.unplace(id, &sub.placement);
            if !sub.placement.is_empty() {
                self.routed.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    /// Whether any connection could currently be pushed anything.
    pub(crate) fn is_active(&self) -> bool {
        self.routed.load(Ordering::SeqCst) != 0
    }

    /// Whether a push for `cmd` would have a candidate. Lets a write skip
    /// building a push nobody would be offered. Uses the same keys
    /// [`SyncPush::for_command`] is built from.
    pub(crate) fn would_route(&self, cmd: &Command) -> bool {
        if !self.is_active() {
            return false;
        }
        let keys = push_key_refs(cmd);
        let wide = is_keyspace_wide(cmd);
        let found = self.read().has_candidate(keys.first().copied(), wide);
        if !found {
            self.stats.skipped.add(1);
        }
        found
    }

    /// Offer `push` to every candidate mailbox. Returns how many queued it.
    pub(crate) fn send(&self, push: SyncMsg) -> usize {
        if !self.is_active() {
            return 0;
        }
        let mut offered = 0u64;
        let mut queued = 0usize;
        self.read().for_each_candidate(
            push.keys.first().map(String::as_str),
            push.keyspace_wide,
            |mailbox| {
                offered = offered.saturating_add(1);
                if mailbox.offer(&push) {
                    queued = queued.saturating_add(1);
                }
            },
        );
        self.stats.events.add(1);
        self.stats.candidates.add(offered);
        queued
    }

    pub(crate) fn stats(&self) -> RouteStatsSnapshot {
        let table = self.read();
        RouteStatsSnapshot {
            subscribers: table.subscribers.len(),
            routed: self.routed.load(Ordering::SeqCst),
            broad: table.broad.len(),
            exact_keys: table.exact.len(),
            prefixes: table.prefixes.len(),
            events: self.stats.events.sum(),
            candidates: self.stats.candidates.sum(),
            skipped: self.stats.skipped.sum(),
            forwarded: self.stats.forwarded.sum(),
            filtered: self.stats.filtered.sum(),
        }
    }
}

/// Why [`SyncSubscription::recv`] returned no push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PushRecvError {
    /// A push did not fit in the mailbox: the stream has a gap.
    Lagged,
    /// The queue's sender is gone. Not produced today — the subscription
    /// holds its mailbox, and so its sender, for as long as it lives — but a
    /// closed queue must never read as a push.
    Closed,
}

/// Why [`SyncSubscription::try_recv`] returned no push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TryPushRecvError {
    Empty,
    Lagged,
    Closed,
}

/// One connection's registration with the router. Unregisters on drop.
pub(crate) struct SyncSubscription {
    router: SyncTx,
    id: u64,
    rx: mpsc::Receiver<SyncMsg>,
    mailbox: Arc<Mailbox>,
    /// Keeps the connection counted as a push receiver for `UnorderedGate`.
    pub(crate) receiver: Option<PushReceiver>,
}

impl SyncSubscription {
    /// Replace what this connection is routed. Returns once the new routes
    /// are published: every write that routes afterwards sees them.
    pub(crate) fn set_routes(&self, routes: Routes<'_>) {
        self.router.set_routes(self.id, routes);
    }

    /// The next push, waiting for one. Cancel safe.
    pub(crate) async fn recv(&mut self) -> Result<SyncMsg, PushRecvError> {
        loop {
            if self.mailbox.has_gap() {
                return Err(PushRecvError::Lagged);
            }
            tokio::select! {
                biased;
                () = self.mailbox.gap_notify.notified() => {}
                push = self.rx.recv() => {
                    return match push {
                        // Released and checked in one step: a gap that opened
                        // before this push was taken may have dropped one it
                        // would follow, so it is not delivered.
                        Some(push) if self.mailbox.release(&push) => Err(PushRecvError::Lagged),
                        Some(push) => Ok(push),
                        None => Err(PushRecvError::Closed),
                    };
                }
            }
        }
    }

    /// The next queued push, without waiting.
    pub(crate) fn try_recv(&mut self) -> Result<SyncMsg, TryPushRecvError> {
        if self.mailbox.has_gap() {
            return Err(TryPushRecvError::Lagged);
        }
        match self.rx.try_recv() {
            Ok(push) if self.mailbox.release(&push) => Err(TryPushRecvError::Lagged),
            Ok(push) => Ok(push),
            Err(mpsc::error::TryRecvError::Empty) => Err(TryPushRecvError::Empty),
            Err(mpsc::error::TryRecvError::Disconnected) => Err(TryPushRecvError::Closed),
        }
    }

    /// Count what the connection did with a push it took.
    pub(crate) fn record(&self, forwarded: bool) {
        if forwarded {
            self.router.stats.forwarded.add(1);
        } else {
            self.router.stats.filtered.add(1);
        }
    }
}

impl Drop for SyncSubscription {
    fn drop(&mut self) {
        self.router.remove(self.id);
    }
}

/// Whether `cmd`'s push is keyspace-wide (`FLUSHDB`, bare or in a DEDUP
/// envelope): offered to every connection although it names no key.
pub(crate) fn is_keyspace_wide(cmd: &Command) -> bool {
    matches!(cmd, Command::FlushDb)
        || matches!(cmd, Command::Dedup(_, _, inner) if matches!(**inner, Command::FlushDb))
}

#[cfg(test)]
#[path = "tests/sync_router.rs"]
mod tests;
