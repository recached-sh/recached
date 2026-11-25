//! Test-only fault injection: hold a write part-way through, or a reader at
//! a chosen point, so a test can look at the keyspace in between (6O-D, MSET
//! reader atomicity).
//!
//! Compiled into core-engine's own tests and, through the `test-hooks`
//! feature, into the server's: the server enables it as a dev-dependency
//! feature only, so a release build never contains it. Unarmed, a hook point
//! costs one relaxed load.
//!
//! The hook is process-wide but **scoped to one store**: [`after_mutation_of`]
//! names the store, and every hook point passes its store's identity, so a
//! test's stop, counts and reader stops are never triggered by another test's
//! store running in parallel. Arming takes a process-wide lock held by the
//! returned [`Pause`], so tests that arm it run one at a time.

use super::KeyValueStore;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

static ARMED: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<State> = Mutex::new(State {
    store: 0,
    key: None,
    transaction: false,
    readers_of: Vec::new(),
    reached: false,
    released: false,
    readers_blocked: 0,
    locked_reads: 0,
    reader_stop: ReaderStop::Off,
});
static CHANGED: Condvar = Condvar::new();
static EXCLUSIVE: Mutex<()> = Mutex::new(());

struct State {
    /// The armed store's identity (`KeyValueStore::hook_id`).
    store: usize,
    /// The key whose mutation stops the next write that makes it.
    key: Option<String>,
    /// Stop a transaction after its queued write of `key` instead
    /// ([`after_transaction_write_of`], 6O-G), not a write's own mutation.
    transaction: bool,
    /// The write-order stripes of the keys whose readers are counted and
    /// stopped.
    readers_of: Vec<usize>,
    /// A write has stopped after mutating `key`.
    reached: bool,
    /// The stopped write may continue.
    released: bool,
    /// Readers of `readers_of` that found a write in progress and waited.
    readers_blocked: usize,
    /// Multi-key reads of `readers_of` that gave up validating and locked.
    locked_reads: usize,
    /// Where the next multi-key reader of `readers_of` stops, if anywhere.
    reader_stop: ReaderStop,
}

/// A point inside a validated read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReaderPoint {
    /// After it loaded its stripes' words, found no write in progress, and
    /// before it reads anything. Single-key reads stop here too.
    AfterLoad,
    /// After an MGET or EXISTS read its first key, before the others.
    AfterFirstKey,
    /// After a single-key read read, before it validates.
    AfterRead,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReaderStop {
    Off,
    Armed(ReaderPoint),
    Stopped,
    Released,
}

fn state() -> MutexGuard<'static, State> {
    STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// An armed hook; dropping it releases whatever is stopped and disarms it.
pub struct Pause {
    _exclusive: MutexGuard<'static, ()>,
}

/// Arm the hook on `store`: the next write to it that mutates `key` stops
/// right after that mutation — with its stripes held and no shard guard —
/// until the returned [`Pause`] is dropped. Readers of any of `readers_of`
/// that wait for a write are counted ([`Pause::readers_blocked`]).
pub fn after_mutation_of(store: &KeyValueStore, key: &str, readers_of: &[&str]) -> Pause {
    arm(store, key, readers_of, false)
}

fn arm(store: &KeyValueStore, key: &str, readers_of: &[&str], transaction: bool) -> Pause {
    let exclusive = EXCLUSIVE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    {
        let mut state = state();
        state.store = store.hook_id();
        state.key = Some(key.to_string());
        state.transaction = transaction;
        state.readers_of = readers_of
            .iter()
            .map(|key| super::write_order_stripe(key))
            .collect();
        state.reached = false;
        state.released = false;
        state.readers_blocked = 0;
        state.locked_reads = 0;
        state.reader_stop = ReaderStop::Off;
    }
    ARMED.store(true, Ordering::SeqCst);
    Pause {
        _exclusive: exclusive,
    }
}

/// Arm the hook on `store` for a transaction (6O-G): the next transaction
/// applied to it ([`KeyValueStore::apply_transaction`]) stops right after its
/// queued write naming `key` — inside its write span, its store stripes held
/// and marked, synchronously on its thread — until the returned [`Pause`] is
/// dropped or [`Pause::release_write`]. Readers of `readers_of` that wait are
/// counted as for [`after_mutation_of`].
///
/// The asynchronous `exec_pause` of the server stops between queued commands
/// at an `.await`; it cannot stop inside the synchronous write span.
pub fn after_transaction_write_of(store: &KeyValueStore, key: &str, readers_of: &[&str]) -> Pause {
    arm(store, key, readers_of, true)
}

/// The write-order stripe of `key`, so a test can choose keys that share a
/// stripe with, or stay apart from, a transaction's.
pub fn stripe_of(key: &str) -> usize {
    super::write_order_stripe(key)
}

fn wait_until(within: Duration, mut done: impl FnMut(&State) -> bool) -> bool {
    let deadline = Instant::now() + within;
    let mut state = state();
    while !done(&state) {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        state = CHANGED
            .wait_timeout(state, left.min(Duration::from_millis(10)))
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .0;
    }
    true
}

impl Pause {
    /// Wait until the write has stopped, at most `within`.
    pub fn wait_reached(&self, within: Duration) -> bool {
        wait_until(within, |state| state.reached)
    }

    /// Wait until either a reader of `readers_of` is waiting for the
    /// stopped write, or `done()` holds (the reader answered without
    /// waiting), at most `within`. Returns whether a reader is waiting.
    pub fn wait_reader_blocked_or(&self, within: Duration, done: impl Fn() -> bool) -> bool {
        let deadline = Instant::now() + within;
        loop {
            if state().readers_blocked > 0 {
                return true;
            }
            if done() || Instant::now() >= deadline {
                return false;
            }
            let state = state();
            let _ = CHANGED
                .wait_timeout(state, Duration::from_millis(1))
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    /// Readers of `readers_of` that waited for a write so far.
    pub fn readers_blocked(&self) -> usize {
        state().readers_blocked
    }

    /// Multi-key reads of `readers_of` that took their stripes' locks.
    pub fn locked_reads(&self) -> usize {
        state().locked_reads
    }

    /// Also stop the next validated read of `readers_of` at `point`, until
    /// [`release_reader`](Self::release_reader).
    pub fn stop_next_reader(&self, point: ReaderPoint) {
        state().reader_stop = ReaderStop::Armed(point);
    }

    /// Wait until a reader is stopped by [`stop_next_reader`](Self::stop_next_reader).
    pub fn wait_reader_stopped(&self, within: Duration) -> bool {
        wait_until(within, |state| state.reader_stop == ReaderStop::Stopped)
    }

    /// Let the stopped reader continue, to stop again at `point`.
    pub fn release_reader_to(&self, point: ReaderPoint) {
        state().reader_stop = ReaderStop::Armed(point);
        CHANGED.notify_all();
    }

    /// Let the stopped reader continue.
    pub fn release_reader(&self) {
        state().reader_stop = ReaderStop::Released;
        CHANGED.notify_all();
    }

    /// Let the stopped write continue now, keeping the hook armed for the
    /// readers' counts and stops.
    pub fn release_write(&self) {
        state().released = true;
        CHANGED.notify_all();
    }
}

impl Drop for Pause {
    fn drop(&mut self) {
        ARMED.store(false, Ordering::SeqCst);
        let mut state = state();
        state.key = None;
        state.readers_of.clear();
        state.released = true;
        if state.reader_stop != ReaderStop::Off {
            state.reader_stop = ReaderStop::Released;
        }
        CHANGED.notify_all();
    }
}

/// Whether the hook is armed: lets a hook point skip work it only needs then.
pub(crate) fn armed() -> bool {
    ARMED.load(Ordering::Relaxed)
}

/// Hook point: a write to `store` has just mutated `key`. Stops it if the
/// hook is armed for that store and key and no write has stopped yet. The
/// caller must hold no shard guard.
pub(crate) fn mutated(store: usize, key: &str) {
    if !armed() {
        return;
    }
    let state = state();
    if state.transaction
        || state.reached
        || state.store != store
        || state.key.as_deref() != Some(key)
    {
        return;
    }
    stop(state);
}

/// Hook point: a transaction applying to `store` has just run a queued write
/// whose keys are `keys`, inside its write span (6O-G). Stops it if the hook
/// is armed for a transaction on that store and one of `keys`.
pub(crate) fn transaction_step(store: usize, keys: &[&str]) {
    if !armed() {
        return;
    }
    let state = state();
    if !state.transaction
        || state.reached
        || state.store != store
        || !state.key.as_deref().is_some_and(|key| keys.contains(&key))
    {
        return;
    }
    stop(state);
}

/// Stop the calling thread until the test releases it.
fn stop(mut state: MutexGuard<'static, State>) {
    state.reached = true;
    CHANGED.notify_all();
    while !state.released {
        state = CHANGED
            .wait(state)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

/// Whether a reader of `store` on `stripes` is one the armed test watches.
fn watched(state: &State, store: usize, stripes: &[usize]) -> bool {
    state.key.is_some()
        && state.store == store
        && stripes
            .iter()
            .any(|stripe| state.readers_of.contains(stripe))
}

/// Hook point: a reader of `store` is about to wait for a write in progress
/// on `stripes` (a stripe, not a key: readers do not copy their keys to
/// report here). `locked`: a multi-key read taking its stripes' locks.
pub(crate) fn reader_blocked(store: usize, stripes: &[usize], locked: bool) {
    if !armed() {
        return;
    }
    let mut state = state();
    if watched(&state, store, stripes) {
        state.readers_blocked += 1;
        if locked {
            state.locked_reads += 1;
        }
        CHANGED.notify_all();
    }
}

/// Hook point: a validated multi-key read of `store` on `stripes` is at
/// `point`. Stops it there if a test armed that stop.
pub(crate) fn reader_at(store: usize, point: ReaderPoint, stripes: &[usize]) {
    if !armed() {
        return;
    }
    let mut state = state();
    if state.reader_stop != ReaderStop::Armed(point) || !watched(&state, store, stripes) {
        return;
    }
    state.reader_stop = ReaderStop::Stopped;
    CHANGED.notify_all();
    while state.reader_stop == ReaderStop::Stopped {
        state = CHANGED
            .wait(state)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}
