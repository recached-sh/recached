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
//! gains. Back-offs with no gain between them double, up to [`MAX_BACKOFF`]
//! (6O-C); a yield that resumes on another worker is neutral: see [`end`].
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
/// The longest back-off (6O-C). Each back-off that follows another with no
/// gain in between, on a worker that ran batches for one connection only
/// since, doubles, up to this: a worker with one connection probes twice per
/// 1,024 flushes instead of twice per 34. When load returns, the first probe
/// that gains starts over; the worst case is 1,024 flushes uncoalesced, a
/// few milliseconds at these rates.
pub(crate) const MAX_BACKOFF: u32 = 1024;

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
    /// The next back-off's length: [`BACKOFF`] after a gain, doubling with
    /// each back-off since, up to [`MAX_BACKOFF`].
    period: Cell<u32>,
    /// The connection that ran the last read batch here, and whether a
    /// batch from another connection ran since the last back-off began.
    last_conn: Cell<u64>,
    company: Cell<bool>,
}

thread_local! {
    static WORKER: Worker = const {
        Worker {
            batches: Cell::new(0),
            skip: Cell::new(0),
            misses: Cell::new(0),
            period: Cell::new(BACKOFF),
            last_conn: Cell::new(0),
            company: Cell::new(false),
        }
    };
}

/// Record that connection `conn` on this thread ran the commands of one read
/// batch. What a yield compares against: if another connection recorded one
/// while this one was yielded, the yield gained. A batch from a connection
/// other than the previous one also tells the back-off that the worker has
/// company (see [`end`]).
pub(crate) fn note_batch(conn: u64) {
    WORKER.with(|w| {
        w.batches.set(w.batches.get().wrapping_add(1));
        if w.last_conn.replace(conn) != conn {
            w.company.set(true);
        }
    });
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
/// commands first. That clears the misses and the back-off escalation.
///
/// A yield resumed on **another** worker is neutral: it changes nothing on
/// either worker. In Tokio's multi-thread scheduler a yielded task resumes
/// from its own worker's LIFO slot unless several were deferred together and
/// an idle worker stole some of them, so a migration happens almost only
/// when coalescing is paying off; scoring it as a miss on the thief backed
/// the busy case off (6O-C review), and a lone connection does not migrate
/// during its yield (measured: no migrations). The one-client cost on
/// several workers was the probes themselves, which the doubling back-off
/// ([`MAX_BACKOFF`]) now makes rare.
pub(crate) fn end(probe: Probe) -> bool {
    WORKER.with(|w| {
        let migrated = std::ptr::from_ref(w) as usize != probe.worker;
        #[cfg(test)]
        {
            YIELDS.with(|y| y.set(y.get() + 1));
            tally::count(&tally::YIELDS);
            if migrated {
                MIGRATIONS.with(|m| m.set(m.get() + 1));
                tally::count(&tally::MIGRATIONS);
            }
        }
        if migrated {
            return false;
        }
        if w.batches.get() != probe.batches {
            w.misses.set(0);
            w.period.set(BACKOFF);
            return true;
        }
        let misses = w.misses.get() + 1;
        if misses >= MISSES {
            w.misses.set(0);
            // Escalate only a worker that served one connection since its
            // last back-off began. With company its yields may still gain
            // (6O-B's fixed back-off served it), so it starts over instead
            // of settling at the maximum (6O-C review).
            if w.company.replace(false) {
                w.period.set(BACKOFF);
            }
            let period = w.period.get();
            w.skip.set(period);
            w.period.set(period.saturating_mul(2).min(MAX_BACKOFF));
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
/// [`before_flush`]: only after a batch that ran exactly one command. The
/// connection also skips it for a batch that is part of a pipeline split
/// across reads (see `handle_tcp`).
///
/// A batch of several pipelined commands already sends all their replies in
/// one `send(2)`, and its client is waiting for the whole batch: what the
/// yield amortizes is already amortized, and the yield only moves the wait
/// to the end of the pass (6O-B: MSET P=16 p99 +11% at one worker, SET P=16
/// p99 +26% at three). A batch that ran nothing has no reply to coalesce.
///
/// Known limit: a client that writes each pipelined command with its own
/// `write(2)` to a server that keeps up is read one whole frame at a time,
/// which nothing distinguishes from P=1 without timing, so each of those
/// still yields, as in 6O-B.
pub(crate) fn batch_eligible(commands: u32) -> bool {
    commands == 1
}

#[cfg(test)]
thread_local! {
    static BACKOFFS: Cell<u64> = const { Cell::new(0) };
    static MIGRATIONS: Cell<u64> = const { Cell::new(0) };
    static YIELDS: Cell<u64> = const { Cell::new(0) };
}

/// Test-only: yields scored on this thread.
#[cfg(test)]
pub(crate) fn yields() -> u64 {
    YIELDS.with(Cell::get)
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
        w.period.set(BACKOFF);
        w.company.set(false);
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
