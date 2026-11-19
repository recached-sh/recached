//! Counters for the AOF write path, reported under `INFO persistence`.
//!
//! Everything here is updated by whoever *performs* a batch — once per batch,
//! never once per command — so the cost is a handful of relaxed atomic adds
//! per `write(2)`. The exceptions — `lock_contended`, `lead_busy`,
//! `gate_parked`, and the coalescing counters — count a few times at most
//! per connection flush or coalesced append, and mostly events that are
//! already slow paths.
//!
//! They exist to answer one question with numbers instead of guesses: how
//! many logical writes share each physical write, and where the time between
//! a command finishing and its reply leaving goes.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Batch-size histogram buckets: frames per batch, by powers of two.
/// Bucket `i` counts batches of `2^i ..= 2^(i+1)-1` frames; the last bucket
/// is open-ended.
pub(crate) const AOF_BATCH_BUCKETS: usize = 12;

pub(crate) struct AofStats {
    /// Batches performed that wrote at least one byte.
    pub(crate) batches: AtomicU64,
    /// Frames (logical appends) written.
    pub(crate) frames: AtomicU64,
    /// Bytes written.
    pub(crate) bytes: AtomicU64,
    /// `write(2)` calls made (more than `batches` only on short writes).
    pub(crate) write_calls: AtomicU64,
    /// Time spent inside `write(2)`.
    pub(crate) write_ns: AtomicU64,
    /// `fdatasync` calls made by batches (`always`) and by `flush`.
    pub(crate) fsyncs: AtomicU64,
    /// Time spent inside those.
    pub(crate) fsync_ns: AtomicU64,
    /// Sum, over batches, of the time from the batch's first append to the
    /// start of its write: how long frames sat queued.
    pub(crate) queue_wait_ns: AtomicU64,
    /// Sum, over batches, of the tasks holding the batch's completion when
    /// it was performed: the appends and connection gates sharing the write.
    pub(crate) waiters: AtomicU64,
    /// Batches performed inline on an async worker (`everysec`/`no`).
    pub(crate) inline_batches: AtomicU64,
    /// Batches performed on the blocking pool (`always`, truncation).
    pub(crate) blocking_batches: AtomicU64,
    /// Attempts to lead that found a write already in progress.
    pub(crate) lead_busy: AtomicU64,
    /// Queue-lock acquisitions that had to wait for another thread.
    pub(crate) lock_contended: AtomicU64,
    /// Connection-gate waits that found their batch not yet written and
    /// may park for another task's write.
    pub(crate) gate_parked: AtomicU64,
    /// Connection-gate coalescing yields after which the batch had gained a
    /// holder or had been written by another task.
    pub(crate) coalesce_gained: AtomicU64,
    /// Connection-gate coalescing yields that gained nothing.
    pub(crate) coalesce_missed: AtomicU64,
    /// Times the connection gate's coalescing backed off.
    pub(crate) coalesce_backoffs: AtomicU64,
    /// Coalesced fan-out yields (`RECACHED_AOF_FANOUT=coalesced`) after
    /// which another writer's frame had joined the batch after this one.
    pub(crate) fanout_coalesce_gained: AtomicU64,
    /// Coalesced fan-out yields that no frame joined after this one.
    pub(crate) fanout_coalesce_missed: AtomicU64,
    /// Times the coalesced fan-out path backed off.
    pub(crate) fanout_coalesce_backoffs: AtomicU64,
    /// Coalesced fan-out appends that found their batch already completed
    /// (written, or failed) by another writer, and so did not lead.
    pub(crate) fanout_found_written: AtomicU64,
    /// Failed writes that left part of their batch in the file (cut back
    /// to the last whole frame, or held until that succeeds).
    pub(crate) torn_writes: AtomicU64,
    /// Frames per batch, log2 buckets.
    pub(crate) batch_hist: [AtomicU64; AOF_BATCH_BUCKETS],
}

pub(crate) static AOF_STATS: AofStats = AofStats::new();

/// Saturating nanoseconds, so a pathological duration cannot wrap a sum.
pub(crate) fn nanos(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

pub(crate) fn bump(counter: &AtomicU64, by: u64) {
    counter.fetch_add(by, Ordering::Relaxed);
}

impl AofStats {
    pub(crate) const fn new() -> Self {
        #[allow(clippy::declare_interior_mutable_const)]
        const ZERO: AtomicU64 = AtomicU64::new(0);
        AofStats {
            batches: ZERO,
            frames: ZERO,
            bytes: ZERO,
            write_calls: ZERO,
            write_ns: ZERO,
            fsyncs: ZERO,
            fsync_ns: ZERO,
            queue_wait_ns: ZERO,
            waiters: ZERO,
            inline_batches: ZERO,
            blocking_batches: ZERO,
            lead_busy: ZERO,
            lock_contended: ZERO,
            gate_parked: ZERO,
            coalesce_gained: ZERO,
            coalesce_missed: ZERO,
            coalesce_backoffs: ZERO,
            fanout_coalesce_gained: ZERO,
            fanout_coalesce_missed: ZERO,
            fanout_coalesce_backoffs: ZERO,
            fanout_found_written: ZERO,
            torn_writes: ZERO,
            batch_hist: [ZERO; AOF_BATCH_BUCKETS],
        }
    }

    /// Record one performed batch that wrote `frames` frames.
    pub(crate) fn record_batch(&self, frames: u64, bytes: u64, waiters: u64, queued: Duration) {
        bump(&self.batches, 1);
        bump(&self.frames, frames);
        bump(&self.bytes, bytes);
        bump(&self.waiters, waiters);
        bump(&self.queue_wait_ns, nanos(queued));
        // ilog2 of 0 is undefined; a batch with bytes has at least one frame,
        // but a frame count of 0 must not panic either.
        let bucket = usize::try_from(frames.max(1).ilog2())
            .unwrap_or(usize::MAX)
            .min(AOF_BATCH_BUCKETS - 1);
        if let Some(slot) = self.batch_hist.get(bucket) {
            bump(slot, 1);
        }
    }

    /// The `INFO persistence` lines.
    pub(crate) fn render(&self) -> String {
        let get = |c: &AtomicU64| c.load(Ordering::Relaxed);
        let hist = self
            .batch_hist
            .iter()
            .map(|c| get(c).to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "recached_aof_batches:{}\r\n\
             recached_aof_frames:{}\r\n\
             recached_aof_bytes:{}\r\n\
             recached_aof_write_calls:{}\r\n\
             recached_aof_write_usec:{}\r\n\
             recached_aof_fsyncs:{}\r\n\
             recached_aof_fsync_usec:{}\r\n\
             recached_aof_queue_wait_usec:{}\r\n\
             recached_aof_batch_waiters:{}\r\n\
             recached_aof_inline_batches:{}\r\n\
             recached_aof_blocking_batches:{}\r\n\
             recached_aof_lead_busy:{}\r\n\
             recached_aof_lock_contended:{}\r\n\
             recached_aof_gate_parked:{}\r\n\
             recached_aof_coalesce_gained:{}\r\n\
             recached_aof_coalesce_missed:{}\r\n\
             recached_aof_coalesce_backoffs:{}\r\n\
             recached_aof_fanout_coalesce_gained:{}\r\n\
             recached_aof_fanout_coalesce_missed:{}\r\n\
             recached_aof_fanout_coalesce_backoffs:{}\r\n\
             recached_aof_fanout_found_written:{}\r\n\
             recached_aof_torn_writes:{}\r\n\
             recached_aof_batch_frames_log2_hist:{}\r\n",
            get(&self.batches),
            get(&self.frames),
            get(&self.bytes),
            get(&self.write_calls),
            get(&self.write_ns) / 1_000,
            get(&self.fsyncs),
            get(&self.fsync_ns) / 1_000,
            get(&self.queue_wait_ns) / 1_000,
            get(&self.waiters),
            get(&self.inline_batches),
            get(&self.blocking_batches),
            get(&self.lead_busy),
            get(&self.lock_contended),
            get(&self.gate_parked),
            get(&self.coalesce_gained),
            get(&self.coalesce_missed),
            get(&self.coalesce_backoffs),
            get(&self.fanout_coalesce_gained),
            get(&self.fanout_coalesce_missed),
            get(&self.fanout_coalesce_backoffs),
            get(&self.fanout_found_written),
            get(&self.torn_writes),
            hist,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_sizes_land_in_their_power_of_two_bucket() {
        let stats = AofStats::new();
        for frames in [0, 1, 2, 3, 4, 50, u64::MAX] {
            stats.record_batch(frames, 1, 1, Duration::ZERO);
        }
        let hist: Vec<u64> = stats
            .batch_hist
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect();
        // 0 and 1 → bucket 0; 2, 3 → 1; 4 → 2; 50 → 5; u64::MAX → last.
        assert_eq!(hist, vec![2, 2, 1, 0, 0, 1, 0, 0, 0, 0, 0, 1]);
        assert!(stats.render().contains("recached_aof_batches:7\r\n"));
        // The coalesced fan-out path reports apart from the connection gate.
        for field in [
            "coalesce_gained",
            "coalesce_missed",
            "coalesce_backoffs",
            "fanout_coalesce_gained",
            "fanout_coalesce_missed",
            "fanout_coalesce_backoffs",
            "fanout_found_written",
        ] {
            assert!(
                stats
                    .render()
                    .contains(&format!("\r\nrecached_aof_{field}:0\r\n")),
                "INFO lacks {field}"
            );
        }
    }
}
