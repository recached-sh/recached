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
//! gains.
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
/// Resumed on another worker (work stealing), there is nothing to compare
/// with on this one: that is neither a gain nor a miss here.
pub(crate) fn end(probe: Probe) -> bool {
    WORKER.with(|w| {
        if std::ptr::from_ref(w) as usize != probe.worker {
            return true;
        }
        if w.batches.get() != probe.batches {
            w.misses.set(0);
            return true;
        }
        let misses = w.misses.get() + 1;
        if misses >= MISSES {
            w.misses.set(0);
            w.skip.set(BACKOFF);
            #[cfg(test)]
            BACKOFFS.with(|b| b.set(b.get() + 1));
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

#[cfg(test)]
thread_local! {
    static BACKOFFS: Cell<u64> = const { Cell::new(0) };
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
