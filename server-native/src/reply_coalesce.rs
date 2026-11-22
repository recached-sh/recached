//! Cross-connection reply coalescing (6O-B).
//!
//! A TCP connection with replies buffered yields to the scheduler once
//! before flushing them, so the other connections already runnable on the
//! same worker run their commands first and every reply of that pass leaves
//! back to back. Redis does the same by construction: its event loop reads
//! and executes every ready client, then writes all their replies in
//! `beforeSleep`. The AOF gate has done this since 6M for connections with
//! AOF frames queued ([`crate::persistence::AofGatedWriter`]); this is the
//! same yield for every other flush.
//!
//! **Why it matters.** With 50 clients on one worker, replying the moment
//! each connection's command ran spread the replies across the whole pass.
//! The benchmark's client threads (two, 25 connections each) then received
//! one reply, answered it, and went back to sleep before the next arrived:
//! 0.30–0.48 client sleeps per command against Redis's 0.05. Every one of
//! those wake-ups is a remote `try_to_wake_up` the kernel runs inside the
//! server's `send(2)`, in softirq on the server's CPU: Recached's `send`
//! cost 4.1–4.8 µs a call against Redis's 3.6 µs for the same 5 bytes, and
//! the server's CPU was the bottleneck. See
//! `benchmarks/v0.6.0/6o-b/V0.6O-B-CPU-PROFILE.md`.
//!
//! **What it changes.** Only *when* a reply's bytes reach the kernel, within
//! one scheduler pass. Never what a reply says, the order of replies on a
//! connection, or what a reply waits for. Tokio wakes a yielded task once
//! its worker has run every other task in its queue (including tasks that
//! input arriving meanwhile made ready, and whatever it steals) and polled
//! the I/O driver — or at the maintenance tick, every `event_interval` (61)
//! polls, the hard bound even for a worker that never idles. There is no
//! timer, and nothing waits for input that has not arrived.
//!
//! **When nobody else is ready** (one client, or a quiet moment), the yield
//! is pure cost: a scheduler round trip and a zero-timeout poll of the I/O
//! driver. Two yields in a row during which no other connection on the
//! worker processed a read batch back off for [`BACKOFF`] flushes, as the
//! AOF gate's coalescer does (`COALESCE_MISSES`, `COALESCE_BACKOFF`). The
//! state is per worker thread: no shared cache line on the hot path, and a
//! worker's own recent experience is what predicts whether its next yield
//! gains. A yield that resumes on another worker is a miss there (6O-C):
//! see [`end`].
//!
//! **Only single-command batches yield** (6O-C, [`batch_eligible`]). A read
//! batch that ran several pipelined commands already sends their replies
//! together, so the yield would only delay them.
//!
//! `RECACHED_REPLY_COALESCE=off` turns it off (every flush immediate, the
//! behaviour before 6O-B).

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

/// Yields in a row that gained nothing before coalescing backs off.
pub(crate) const MISSES: u32 = 2;
/// Flushes that go out at once after a back-off, before the next probe.
pub(crate) const BACKOFF: u32 = 32;

static ENABLED: AtomicBool = AtomicBool::new(true);

/// Turn coalescing on or off for the process (`RECACHED_REPLY_COALESCE`).
pub(crate) fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Whether coalescing is on for the process.
pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// One worker thread's view: how many read batches its connections have
/// processed, and its back-off state.
struct Worker {
    batches: Cell<u64>,
    skip: Cell<u32>,
    misses: Cell<u32>,
}

thread_local! {
    static WORKER: Worker = const {
        Worker {
            batches: Cell::new(0),
            skip: Cell::new(0),
            misses: Cell::new(0),
        }
    };
}

/// Record that a connection on this thread is about to run the commands of
/// one read batch. What a yield compares against: if another connection
/// recorded one while this one was yielded, the yield gained.
pub(crate) fn note_batch() {
    WORKER.with(|w| w.batches.set(w.batches.get().wrapping_add(1)));
}

/// A yield in progress: which thread it started on, and that thread's
/// batch count when it did.
#[derive(Debug)]
pub(crate) struct Probe {
    worker: usize,
    batches: u64,
}

/// Whether this flush should yield first; `Some` when it should.
pub(crate) fn begin() -> Option<Probe> {
    if !enabled() {
        return None;
    }
    WORKER.with(|w| {
        let skip = w.skip.get();
        if skip > 0 {
            w.skip.set(skip - 1);
            return None;
        }
        Some(Probe {
            worker: std::ptr::from_ref(w) as usize,
            batches: w.batches.get(),
        })
    })
}

/// Record whether the yield `probe` started gained; returns whether it did.
///
/// A gain is evidence on the worker the yield started on: its batch counter
/// moved while the task was deferred, so another connection there ran its
/// commands first. Anything else is a miss, scored on the worker the task
/// resumed on — including a resume on *another* worker (work stealing).
/// A task is stolen while yielded only by a worker with nothing of its own
/// to run, and the origin's counter cannot be read from here, so there is no
/// evidence of a gain; until 6O-C a migration counted as one, and a lone
/// connection bouncing between idle workers yielded on every flush without
/// ever backing off (+3% CPU per op at one client and three workers, 6O-B).
/// Scoring it on the resumed worker keeps the state per thread and backs
/// off where the task now runs.
pub(crate) fn end(probe: Probe) -> bool {
    WORKER.with(|w| {
        let migrated = std::ptr::from_ref(w) as usize != probe.worker;
        #[cfg(test)]
        {
            tally::count(&tally::YIELDS);
            if migrated {
                MIGRATIONS.with(|m| m.set(m.get() + 1));
                tally::count(&tally::MIGRATIONS);
            }
        }
        if !migrated && w.batches.get() != probe.batches {
            w.misses.set(0);
            return true;
        }
        let misses = w.misses.get() + 1;
        if misses >= MISSES {
            w.misses.set(0);
            w.skip.set(BACKOFF);
            #[cfg(test)]
            {
                BACKOFFS.with(|b| b.set(b.get() + 1));
                tally::count(&tally::BACKOFFS);
            }
        } else {
            w.misses.set(misses);
        }
        false
    })
}

/// Yield once before a flush, when coalescing says to.
pub(crate) async fn before_flush() {
    if let Some(probe) = begin() {
        tokio::task::yield_now().await;
        end(probe);
    }
}

/// Whether a flush that ends a read batch should go through
/// [`before_flush`]: only after a batch that ran exactly one command.
///
/// A batch of several pipelined commands already sends all their replies in
/// one `send(2)`, and its client is waiting for the whole batch: what the
/// yield amortizes is already amortized, and the yield only moves the wait
/// to the end of the pass (6O-B: MSET P=16 p99 +11% at one worker, SET P=16
/// p99 +26% at three). A batch that ran nothing has no reply to coalesce.
pub(crate) fn batch_eligible(commands: u32) -> bool {
    commands == 1
}

#[cfg(test)]
thread_local! {
    static BACKOFFS: Cell<u64> = const { Cell::new(0) };
    static MIGRATIONS: Cell<u64> = const { Cell::new(0) };
}

/// Test-only: yields this thread scored as resumed from another worker.
#[cfg(test)]
pub(crate) fn migrations() -> u64 {
    MIGRATIONS.with(Cell::get)
}

/// Test-only: back-offs this thread has entered.
#[cfg(test)]
pub(crate) fn backoffs() -> u64 {
    BACKOFFS.with(Cell::get)
}

/// Test-only: this thread's back-off state, `(skip, misses)`.
#[cfg(test)]
pub(crate) fn state() -> (u32, u32) {
    WORKER.with(|w| (w.skip.get(), w.misses.get()))
}

/// Test-only: reset this thread's state.
#[cfg(test)]
pub(crate) fn reset() {
    WORKER.with(|w| {
        w.skip.set(0);
        w.misses.set(0);
    });
}

/// Test-only counters for a multi-worker runtime, whose workers' thread-local
/// state a test cannot reach: counted only on threads named with [`PREFIX`],
/// so tests running in parallel on other threads do not add to them.
#[cfg(test)]
pub(crate) mod tally {
    use std::sync::atomic::{AtomicU64, Ordering};

    /// The thread-name prefix of a runtime whose yields are tallied.
    pub(crate) const PREFIX: &str = "rc6oc-tally";
    /// Yields scored by [`super::end`].
    pub(crate) static YIELDS: AtomicU64 = AtomicU64::new(0);
    /// Of those, yields resumed on another worker.
    pub(crate) static MIGRATIONS: AtomicU64 = AtomicU64::new(0);
    /// Back-offs entered.
    pub(crate) static BACKOFFS: AtomicU64 = AtomicU64::new(0);

    pub(super) fn count(counter: &AtomicU64) {
        if std::thread::current()
            .name()
            .is_some_and(|n| n.starts_with(PREFIX))
        {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// `(yields, migrations, backoffs)` so far.
    pub(crate) fn snapshot() -> (u64, u64, u64) {
        (
            YIELDS.load(Ordering::Relaxed),
            MIGRATIONS.load(Ordering::Relaxed),
            BACKOFFS.load(Ordering::Relaxed),
        )
    }
}
