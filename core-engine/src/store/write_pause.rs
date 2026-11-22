//! Test-only fault injection: hold a multi-key write part-way through, so a
//! test can look at the keyspace between the write's first mutation and its
//! last (6O-D, MSET reader atomicity).
//!
//! Compiled into core-engine's own tests and, through the `test-hooks`
//! feature, into the server's: the server enables it as a dev-dependency
//! feature only, so a release build never contains it. Unarmed, a hook point
//! costs one relaxed load.
//!
//! There is one hook per process. [`after_mutation_of`] takes a process-wide
//! lock held by the returned [`Pause`], so tests that arm it run one at a
//! time; tests that never arm it are unaffected, since a hook only stops the
//! first write that mutates the armed key.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

static ARMED: AtomicBool = AtomicBool::new(false);
static READER_ARMED: AtomicBool = AtomicBool::new(false);
/// Multi-key reads that gave up validating and took their keys' locks, since
/// start: lets a test check it exercised that path.
pub static LOCKED_READS: AtomicUsize = AtomicUsize::new(0);
static STATE: Mutex<State> = Mutex::new(State {
    key: None,
    readers_of: Vec::new(),
    reached: false,
    released: false,
    readers_blocked: 0,
    reader_stop: ReaderStop::Off,
});
static CHANGED: Condvar = Condvar::new();
static EXCLUSIVE: Mutex<()> = Mutex::new(());

struct State {
    /// The key whose mutation stops the next write that makes it.
    key: Option<String>,
    /// The write-order stripes of the keys whose readers [`reader_blocked`]
    /// counts.
    readers_of: Vec<usize>,
    /// A write has stopped after mutating `key`.
    reached: bool,
    /// The stopped write may continue.
    released: bool,
    /// Readers of `readers_of` that found a write in progress and waited.
    readers_blocked: usize,
    /// Where the next validated multi-key reader stops, if anywhere.
    reader_stop: ReaderStop,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReaderStop {
    Off,
    /// Stop the next multi-key reader after its first validation load.
    Armed,
    /// A reader is stopped there.
    Stopped,
    /// The stopped reader may continue.
    Released,
}

fn state() -> MutexGuard<'static, State> {
    STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// An armed hook; dropping it releases the stopped write (if any) and
/// disarms the hook.
pub struct Pause {
    _exclusive: MutexGuard<'static, ()>,
}

/// Arm the hook: the next multi-key write that mutates `key` stops right
/// after that mutation — with its stripes held and the shard guard released —
/// until the returned [`Pause`] is dropped. Readers of any of `readers_of`
/// that wait for a write are counted ([`Pause::readers_blocked`]).
pub fn after_mutation_of(key: &str, readers_of: &[&str]) -> Pause {
    let exclusive = EXCLUSIVE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    {
        let mut state = state();
        state.key = Some(key.to_string());
        state.readers_of = readers_of
            .iter()
            .map(|key| super::write_order_stripe(key))
            .collect();
        state.reached = false;
        state.released = false;
        state.readers_blocked = 0;
        state.reader_stop = ReaderStop::Off;
    }
    ARMED.store(true, Ordering::SeqCst);
    Pause {
        _exclusive: exclusive,
    }
}

impl Pause {
    /// Wait until the write has stopped, at most `within`.
    pub fn wait_reached(&self, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        let mut state = state();
        while !state.reached {
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

    /// Wait until either a reader of `readers_of` is waiting for the
    /// stopped write, or `done()` holds (the reader answered without
    /// waiting), at most `within`. Returns whether a reader is waiting.
    pub fn wait_reader_blocked_or(&self, within: Duration, done: impl Fn() -> bool) -> bool {
        let deadline = Instant::now() + within;
        loop {
            {
                let state = state();
                if state.readers_blocked > 0 {
                    return true;
                }
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

    /// Readers of the armed key that waited for the stopped write so far.
    pub fn readers_blocked(&self) -> usize {
        state().readers_blocked
    }

    /// Also stop the next validated multi-key read right after it loaded its
    /// stripes' sequence words and found no write in progress — before it
    /// reads anything — until [`release_reader`](Self::release_reader).
    pub fn stop_next_reader_after_load(&self) {
        state().reader_stop = ReaderStop::Armed;
        READER_ARMED.store(true, Ordering::SeqCst);
    }

    /// Wait until a reader is stopped by
    /// [`stop_next_reader_after_load`](Self::stop_next_reader_after_load).
    pub fn wait_reader_stopped(&self, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        let mut state = state();
        while state.reader_stop != ReaderStop::Stopped {
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

    /// Let the stopped reader continue.
    pub fn release_reader(&self) {
        READER_ARMED.store(false, Ordering::SeqCst);
        state().reader_stop = ReaderStop::Released;
        CHANGED.notify_all();
    }
}

impl Drop for Pause {
    fn drop(&mut self) {
        ARMED.store(false, Ordering::SeqCst);
        READER_ARMED.store(false, Ordering::SeqCst);
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

/// Hook point: a write has just mutated `key`. Stops it if the hook is armed
/// for `key` and no write has stopped yet. The caller must hold no shard
/// guard.
pub(crate) fn mutated(key: &str) {
    if !ARMED.load(Ordering::Relaxed) {
        return;
    }
    let mut state = state();
    if state.reached || state.key.as_deref() != Some(key) {
        return;
    }
    state.reached = true;
    CHANGED.notify_all();
    while !state.released {
        state = CHANGED
            .wait(state)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

/// Hook point: a reader is about to wait for a write in progress on
/// `stripes`. Counted when one of them is a stripe of the armed `readers_of`
/// (a stripe, not a key: readers do not copy their keys to report here).
pub(crate) fn reader_blocked(stripes: impl IntoIterator<Item = usize>) {
    if !ARMED.load(Ordering::Relaxed) {
        return;
    }
    let mut state = state();
    if state.key.is_none() {
        return;
    }
    let counted = stripes
        .into_iter()
        .any(|stripe| state.readers_of.contains(&stripe));
    if counted {
        state.readers_blocked += 1;
        CHANGED.notify_all();
    }
}

/// Hook point: a validated multi-key read has loaded its sequence words and
/// is about to read. Stops it if [`Pause::stop_next_reader_after_load`]
/// armed a stop.
pub(crate) fn reader_loaded() {
    if !READER_ARMED.load(Ordering::Relaxed) {
        return;
    }
    let mut state = state();
    if state.reader_stop != ReaderStop::Armed {
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
