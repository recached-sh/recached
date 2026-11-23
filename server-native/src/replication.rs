//! Replication: the listener that serves replicas, the client that follows a
//! primary, and the authentication throttle guarding the handshake.

use crate::*;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

#[derive(Clone)]
pub(crate) struct ReplFrame {
    offset: u64,
    bytes: Arc<Vec<u8>>,
}

pub(crate) struct QueuedReplFrame {
    frame: ReplFrame,
    charged_bytes: usize,
    pending_bytes: Arc<AtomicUsize>,
}

#[cfg(test)]
impl QueuedReplFrame {
    /// The frame's bytes, for tests outside this module.
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.frame.bytes
    }

    /// The frame's replication offset, for tests outside this module.
    pub(crate) fn offset(&self) -> u64 {
        self.frame.offset
    }
}

impl Drop for QueuedReplFrame {
    fn drop(&mut self) {
        self.pending_bytes
            .fetch_sub(self.charged_bytes, Ordering::AcqRel);
    }
}

pub(crate) type ReplSender = mpsc::Sender<QueuedReplFrame>;

/// A connected replica: its write channel plus the counters that make lag
/// observable.
///
/// Replication was previously one-way, so the primary could only report how
/// many replicas were attached — never how far behind one had fallen. The
/// replica now acknowledges each applied frame, and the difference between
/// what was queued and what was acknowledged is the lag.
pub(crate) struct ReplicaHandle {
    pub(crate) tx: ReplSender,
    /// Frames handed to this replica's channel.
    pub(crate) sent: Arc<AtomicU64>,
    /// Frames the replica reports as applied.
    pub(crate) acked: Arc<AtomicU64>,
    pub(crate) pending_bytes: Arc<AtomicUsize>,
    pub(crate) max_queue_bytes: usize,
    /// Dropped with this handle, which closes the replica's connection (see
    /// [`Registered`]): a replica unregistered for a full queue must not
    /// keep its sender task, socket, connection permit and up to a queue
    /// budget of frames while its peer stops reading (6N-C6 review M1).
    pub(crate) _link: tokio::sync::oneshot::Sender<()>,
}

#[derive(Default)]
struct ReplBacklog {
    frames: VecDeque<ReplFrame>,
    bytes: usize,
}

/// The replication stream's state: the backlog partial resynchronisation
/// reads, and the replicas live frames go to. One lock for both (6L-D R1).
///
/// They were two `tokio::sync::Mutex`es, taken in turn by every replicated
/// write: two async lock round trips per write, and the backlog lock held
/// across the second acquisition anyway, because a frame must reach the
/// backlog and every replica queue before the next offset is assigned. No
/// holder awaits — the fan-out only `try_send`s — so a short synchronous lock
/// serves, and `MutexGuard` being `!Send` keeps any future holder from
/// carrying it across an `.await` in a spawned task.
#[derive(Default)]
pub(crate) struct ReplStream {
    backlog: ReplBacklog,
    pub(crate) senders: Vec<ReplicaHandle>,
}

/// Connected-replica registry. `count` mirrors `senders.len()` (updated by
/// every writer while holding the lock) so the per-write hot path can skip
/// the mutex entirely when no replica is connected.
pub(crate) struct ReplHub {
    /// Backlog and replica queues; see [`ReplStream`].
    stream: std::sync::Mutex<ReplStream>,
    pub(crate) count: AtomicUsize,
    run_id: String,
    offset: AtomicU64,
    backlog_bytes: AtomicUsize,
    queue_bytes: AtomicUsize,
    /// Per-key ordering domains shared by client writes and full-sync
    /// snapshots. Conflicting writes hold their guards until propagation has
    /// completed; independent keys remain concurrent.
    write_order: Vec<tokio::sync::Mutex<()>>,
    /// Lets a write nothing observes skip `write_order`; closed by every
    /// other taker of those stripes. See [`UnorderedGate`].
    pub(crate) unordered: UnorderedGate,
    /// Test-only: how many times a taker found a stripe held and had to wait
    /// for it (6O-E). Lets a test see that a writer is blocked behind a
    /// transaction, rather than inferring it from a timeout.
    #[cfg(test)]
    stripe_waits: AtomicUsize,
    /// Stream DEDUP identities with the writes they belong to
    /// (`RECACHED_REPL_DEDUP`, on by default). Fixed at startup: every
    /// replica of this primary must then accept them, see [`REPL_CAP_DEDUP`].
    dedup: AtomicBool,
    /// On a replica: whether the primary it follows streams DEDUP identities.
    /// False while following a pre-0.6 primary, or one with
    /// `RECACHED_REPL_DEDUP=off` — a promoted replica then holds no marks.
    pub(crate) upstream_dedup: AtomicBool,
    /// Socket flushes by replica senders, summed over replicas (6N-C5). Each
    /// flush is one send burst, so the kernel's share of a replicated write
    /// (the socket send, and the replica's acknowledgement that answers it)
    /// scales with flushes per write: `send_frames / send_flushes` is the
    /// batching a sender achieved.
    send_flushes: AtomicU64,
    /// Frames written by replica senders, summed over replicas. Live
    /// frames only: a partial resynchronization's backlog frames are written
    /// before the stream starts and are not counted.
    send_frames: AtomicU64,
    /// The deepest queue a replica sender found when it woke, in frames:
    /// how far writers ran ahead of it (against the channel capacity,
    /// `RECACHED_REPL_BUFFER`, which disconnects the replica when full).
    send_queue_peak: AtomicU64,
    /// Flushes a sender held to the end of a scheduling round, so the round's
    /// other writers' frames joined them (6N-C5); the rest were full flushes
    /// (`REPL_SEND_BATCH_FRAMES`).
    send_deferred: AtomicU64,
    /// The most bytes a replica sender found charged to its queue when it
    /// woke (6N-C6): the byte-budget counterpart of `send_queue_peak`,
    /// against `RECACHED_REPL_BUFFER_BYTES`.
    send_queue_bytes_peak: AtomicU64,
    /// Replicas disconnected because their queue was full, by bytes or by
    /// frames (6N-C6). Each reconnects and resynchronizes.
    queue_disconnects: AtomicU64,
    /// Synchronizations served to replicas: from the backlog (partial) and
    /// from a snapshot (full).
    syncs_partial: AtomicU64,
    syncs_full: AtomicU64,
}

const WRITE_ORDER_STRIPES: usize = 256;

/// Most replication frames a primary writes before one flush.
pub(crate) const REPL_SEND_BATCH_FRAMES: usize = 512;
/// A replica channel smaller than this flushes on every wake, as before
/// 6N-C5 (review F1). When a held flush's round ends, the sender is queued
/// behind the tasks that round woke, and a `try_send` cannot move it
/// forward: a small channel (`RECACHED_REPL_BUFFER`) could fill meanwhile
/// and disconnect a healthy replica. A channel of four flushes or more has
/// room for a round of ordinary writers. A round of pipelining connections
/// can hold more frames than 4096, the pre-6N-C6 default; the default cap
/// now follows the byte budget (65,536 frames) for that reason
/// (`repl_channel_capacity`, 6N-C6).
pub(crate) const REPL_SEND_DEFER_MIN_CAPACITY: usize = 4 * REPL_SEND_BATCH_FRAMES;

/// Send buffer per replica connection; a burst larger than this is written
/// in several syscalls but still flushed once.
const REPL_SEND_BUFFER_BYTES: usize = 64 * 1024;
/// Receive buffer on a replica's stream from its primary.
const REPL_RECV_BUFFER_BYTES: usize = 64 * 1024;

/// In-flight counters for [`UnorderedGate`], one per cache line so writers on
/// different workers do not contend on one counter.
const UNORDERED_SLOTS: usize = 16;

#[repr(align(128))]
struct PaddedCount(AtomicUsize);

/// The fast path for a write whose order nothing can observe.
///
/// The replication-order stripes exist so that conflicting writes reach the
/// AOF, replicas, WATCH and browser pushes in the order they reached the
/// store, and so that a barrier (snapshot, replica attach, active expiry,
/// QSUB's initial state, EXEC, ephemeral release) sees no write half done. A
/// write on a server with no AOF, no replication stream, no WebSocket
/// receiver, no WATCH, no ephemeral claim and no capacity limit has nothing
/// to order — but it took a fair async mutex anyway, and on a hot key with
/// several workers every release handed the lock to a task parked on another
/// core: a lock convoy that made four workers slower than one.
///
/// Such a write may instead *enter* the gate and run with no stripe. The
/// handshake that keeps that sound is the store-buffer (Dekker) pattern, all
/// `SeqCst`:
///
/// - a writer increments its slot's in-flight count, then reads `closed`;
///   only if it reads 0 does it go on to check for consumers and run;
/// - a closer increments `closed`, then waits until every slot reads 0.
///
/// In the single total order of those operations either the writer's
/// increment comes first — the closer then sees it and waits for the write to
/// finish — or the closer's comes first and the writer sees `closed != 0` and
/// takes the ordered path. A consumer is therefore always registered either
/// under a closed gate or under a barrier that closed it, and a write either
/// finishes before the registration completes or sees the consumer.
pub(crate) struct UnorderedGate {
    closed: AtomicUsize,
    in_flight: [PaddedCount; UNORDERED_SLOTS],
    /// WebSocket push receivers registered through
    /// [`ReplHub::subscribe_pushes`] and [`ReplHub::route_pushes`]: the
    /// connections routed something. Read by every unordered write; a
    /// connection is counted, with the gate closed, before its routes are
    /// published, so registering one keeps every later write on the ordered
    /// path. An unauthenticated socket, routed nothing, is not counted.
    push_receivers: AtomicUsize,
}

/// A registered push receiver; unregisters on drop.
pub(crate) struct PushReceiver {
    hub: ReplRegistry,
}

impl Drop for PushReceiver {
    fn drop(&mut self) {
        self.hub
            .unordered
            .push_receivers
            .fetch_sub(1, Ordering::SeqCst);
    }
}

/// One write's passage through an open [`UnorderedGate`]. Held only across
/// synchronous code; dropping it marks the write finished.
pub(crate) struct UnorderedPass<'a> {
    count: &'a AtomicUsize,
}

impl Drop for UnorderedPass<'_> {
    fn drop(&mut self) {
        // Release: a closer that reads this decrement also sees the write.
        self.count.fetch_sub(1, Ordering::Release);
    }
}

/// Keeps an [`UnorderedGate`] closed; reopens it on drop.
pub(crate) struct GateClosed<'a> {
    gate: &'a UnorderedGate,
}

impl Drop for GateClosed<'_> {
    fn drop(&mut self) {
        self.gate.closed.fetch_sub(1, Ordering::SeqCst);
    }
}

impl UnorderedGate {
    fn new() -> UnorderedGate {
        UnorderedGate {
            closed: AtomicUsize::new(0),
            in_flight: std::array::from_fn(|_| PaddedCount(AtomicUsize::new(0))),
            push_receivers: AtomicUsize::new(0),
        }
    }

    /// Whether any WebSocket connection is receiving write pushes. Checked
    /// by an unordered write only after it entered the gate.
    pub(crate) fn has_push_receivers(&self) -> bool {
        self.push_receivers.load(Ordering::SeqCst) != 0
    }

    /// Enter the fast path, or `None` while anything holds the gate closed.
    ///
    /// The caller must check for consumers *after* this returns `Some`, and
    /// must not await while holding the pass.
    pub(crate) fn try_enter(&self) -> Option<UnorderedPass<'_>> {
        thread_local! {
            static SLOT: usize = {
                static NEXT: AtomicUsize = AtomicUsize::new(0);
                NEXT.fetch_add(1, Ordering::Relaxed) % UNORDERED_SLOTS
            };
        }
        let slot = SLOT.with(|slot| *slot);
        let count = &self.in_flight[slot].0;
        count.fetch_add(1, Ordering::SeqCst);
        if self.closed.load(Ordering::SeqCst) != 0 {
            count.fetch_sub(1, Ordering::Release);
            return None;
        }
        Some(UnorderedPass { count })
    }

    /// Close the fast path and wait for every write already on it to finish.
    ///
    /// Those writes are synchronous and hold nothing a closer holds, so the
    /// wait is bounded by one command's execution.
    pub(crate) async fn close(&self) -> GateClosed<'_> {
        self.closed.fetch_add(1, Ordering::SeqCst);
        let closed = GateClosed { gate: self };
        for slot in &self.in_flight {
            while slot.0.load(Ordering::SeqCst) != 0 {
                tokio::task::yield_now().await;
            }
        }
        closed
    }

    #[cfg(test)]
    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst) != 0
    }
}

/// Capability a 0.6 primary advertises after its run id (`<run-id>;caps=…`)
/// and a 0.6 replica accepts after the run id it requests
/// (`<run-id>;accept=…`): replication frames carry DEDUP identities —
/// `DEDUP <client> <id> <write…>` envelopes, mark-only `RECACHED-DEDUP` and
/// `RECACHED-DEDUP-REFUSED` records, the AOF's own formats — and a full
/// synchronization sends the duplicate-suppression table with the snapshot.
///
/// Carried inside the run-id strings because that is the one field both
/// directions of a 0.5 handshake treat as opaque: a 0.5 replica stores the
/// advertised string and echoes it back unchanged (which carries no
/// `accept`), and a 0.6 replica adds `accept` only when the primary offered
/// the capability, so a 0.5 primary never sees it.
pub(crate) const REPL_CAP_DEDUP: &str = "dedup1";

/// Mode byte: full synchronization followed by the DEDUP table.
const SYNC_MODE_FULL_DEDUP: u8 = b'D';
/// Mode byte: the primary refuses this replica; a length-prefixed reason
/// follows. A 0.5 replica reads it as an invalid mode and retries.
const SYNC_MODE_REFUSED: u8 = b'E';

/// A run id string as sent in the handshake: the id, then `;key=value`
/// parameters.
pub(crate) struct HandshakeId<'a> {
    pub(crate) run_id: &'a str,
    params: &'a str,
}

impl<'a> HandshakeId<'a> {
    pub(crate) fn parse(raw: &'a str) -> Self {
        match raw.split_once(';') {
            Some((run_id, params)) => HandshakeId { run_id, params },
            None => HandshakeId {
                run_id: raw,
                params: "",
            },
        }
    }

    pub(crate) fn has(&self, key: &str, value: &str) -> bool {
        self.params.split(';').any(|p| {
            p.split_once('=')
                .is_some_and(|(k, v)| k == key && v.split(',').any(|item| item == value))
        })
    }
}

/// One client's duplicate-suppression state as a full synchronization sends
/// it: `(client, high-water mark, [(refused id, error)])`.
pub(crate) type DedupTableEntry = (String, Option<u64>, Vec<(u64, String)>);

impl ReplHub {
    /// The stream's lock. Poisoning: a panic while it is held could only come
    /// from an allocation failure in a `VecDeque`/`Vec` push; the state is then
    /// at worst missing that frame for some replicas, which their offset
    /// check turns into a resynchronisation. Every later write panicking
    /// would be worse, so recover the guard.
    pub(crate) fn stream(&self) -> std::sync::MutexGuard<'_, ReplStream> {
        self.stream
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Deepest send queue across connected replicas, in frames.
    ///
    /// Queue depth measures frames waiting to be written to a replica. Applied
    /// offset acknowledgements provide the separate end-to-end lag metric.
    pub(crate) async fn max_queue_depth(&self) -> usize {
        let stream = self.stream();
        stream
            .senders
            .iter()
            .map(|r| r.tx.max_capacity().saturating_sub(r.tx.capacity()))
            .max()
            .unwrap_or(0)
    }

    pub(crate) async fn max_queue_bytes(&self) -> usize {
        let stream = self.stream();
        stream
            .senders
            .iter()
            .map(|replica| replica.pending_bytes.load(Ordering::Relaxed))
            .max()
            .unwrap_or(0)
    }

    /// Send one frame to every attached replica, dropping any that cannot keep
    /// up. Increments each surviving replica's sent counter, which is one half
    /// of the lag calculation.
    pub(crate) async fn fan_out(&self, bytes: Vec<u8>) {
        if !self.is_enabled() {
            return;
        }
        let bytes = Arc::new(bytes);
        // Keep the backlog guard until every replica queue has accepted this
        // frame. Independent-key writes can propagate concurrently; without
        // this serialization offset 2 could be queued before offset 1.
        let mut stream = self.stream();
        let ReplStream {
            backlog,
            senders: reg,
        } = &mut *stream;
        let offset = self.offset.fetch_add(1, Ordering::AcqRel) + 1;
        let frame = ReplFrame {
            offset,
            bytes: Arc::clone(&bytes),
        };
        let backlog_limit = self.backlog_bytes.load(Ordering::Acquire);
        backlog.bytes = backlog.bytes.saturating_add(bytes.len());
        backlog.frames.push_back(frame.clone());
        while backlog.bytes > backlog_limit {
            let Some(expired) = backlog.frames.pop_front() else {
                break;
            };
            backlog.bytes = backlog.bytes.saturating_sub(expired.bytes.len());
        }
        let backlog_bytes = backlog.bytes;

        reg.retain(|r| {
            let charged_bytes = repl_queue_charge(bytes.len());
            let reserved = r
                .pending_bytes
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                    pending
                        .checked_add(charged_bytes)
                        .filter(|next| *next <= r.max_queue_bytes)
                })
                .is_ok();
            if !reserved {
                warn!("Replica byte queue full — disconnecting so it can resync");
                self.queue_disconnects.fetch_add(1, Ordering::Relaxed);
                counter!("recached_replication_disconnects_total", "reason" => "queue_bytes")
                    .increment(1);
                return false;
            }
            let queued = QueuedReplFrame {
                frame: frame.clone(),
                charged_bytes,
                pending_bytes: Arc::clone(&r.pending_bytes),
            };
            match r.tx.try_send(queued) {
            Ok(()) => {
                r.sent.store(offset, Ordering::Release);
                true
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!(
                    "Replica fell too far behind (channel full) — disconnecting so it can resync"
                );
                self.queue_disconnects.fetch_add(1, Ordering::Relaxed);
                counter!("recached_replication_disconnects_total", "reason" => "queue_items")
                    .increment(1);
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => false,
            }
        });
        self.count.store(reg.len(), Ordering::Relaxed);
        drop(stream);
        // Outside the lock: a metrics update is a registry lookup, and every
        // replicated write would otherwise wait out another's.
        gauge!("recached_replication_backlog_bytes").set(backlog_bytes as f64);
    }

    /// Attach an in-process replica that is handed every frame from now on,
    /// for tests and benchmarks that need the fan-out without a socket.
    #[cfg(test)]
    pub(crate) async fn attach_test_replica(
        &self,
        capacity: usize,
        max_queue_bytes: usize,
    ) -> mpsc::Receiver<QueuedReplFrame> {
        let (tx, rx) = mpsc::channel(capacity);
        let mut stream = self.stream();
        let reg = &mut stream.senders;
        reg.push(ReplicaHandle {
            tx,
            sent: Arc::new(AtomicU64::new(self.offset.load(Ordering::Acquire))),
            acked: Arc::new(AtomicU64::new(0)),
            pending_bytes: Arc::new(AtomicUsize::new(0)),
            max_queue_bytes,
            _link: tokio::sync::oneshot::channel().0,
        });
        self.count.store(reg.len(), Ordering::Relaxed);
        rx
    }

    /// Frames the furthest-behind replica has yet to acknowledge.
    ///
    /// This is true lag: how much of what the primary sent has actually been
    /// applied downstream. Queue depth only shows what is stuck locally, and
    /// reads zero for a replica that has received frames but cannot apply them.
    pub(crate) async fn max_lag_frames(&self) -> u64 {
        let stream = self.stream();
        stream
            .senders
            .iter()
            .map(|r| {
                r.sent
                    .load(Ordering::Relaxed)
                    .saturating_sub(r.acked.load(Ordering::Relaxed))
            })
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn new() -> ReplRegistry {
        Arc::new(ReplHub {
            stream: std::sync::Mutex::new(ReplStream::default()),
            count: AtomicUsize::new(0),
            run_id: generate_run_id(),
            offset: AtomicU64::new(0),
            backlog_bytes: AtomicUsize::new(0),
            queue_bytes: AtomicUsize::new(DEFAULT_REPL_QUEUE_BYTES),
            write_order: (0..WRITE_ORDER_STRIPES)
                .map(|_| tokio::sync::Mutex::new(()))
                .collect(),
            unordered: UnorderedGate::new(),
            #[cfg(test)]
            stripe_waits: AtomicUsize::new(0),
            dedup: AtomicBool::new(true),
            upstream_dedup: AtomicBool::new(false),
            send_flushes: AtomicU64::new(0),
            send_frames: AtomicU64::new(0),
            send_queue_peak: AtomicU64::new(0),
            send_deferred: AtomicU64::new(0),
            send_queue_bytes_peak: AtomicU64::new(0),
            queue_disconnects: AtomicU64::new(0),
            syncs_partial: AtomicU64::new(0),
            syncs_full: AtomicU64::new(0),
        })
    }

    /// The replication numbers `INFO` reports.
    pub(crate) async fn info(&self) -> ReplInfo {
        let (send_flushes, send_frames) = self.send_stats();
        let send_queue_peak = self.send_queue_peak();
        let send_deferred = self.send_deferred();
        let (syncs_partial, syncs_full) = self.syncs();
        ReplInfo {
            connected: self.count.load(Ordering::Relaxed),
            queue_depth: self.max_queue_depth().await,
            lag_frames: self.max_lag_frames().await,
            offset: self.current_offset(),
            send_flushes,
            send_frames,
            send_queue_peak,
            send_deferred,
            send_queue_bytes_peak: self.send_queue_bytes_peak(),
            queue_disconnects: self.queue_disconnects(),
            syncs_partial,
            syncs_full,
        }
    }

    /// The offset of the newest frame this node has fanned out.
    pub(crate) fn current_offset(&self) -> u64 {
        self.offset.load(Ordering::Acquire)
    }

    /// The deepest queue a replica sender found when it woke, since startup.
    pub(crate) fn send_queue_peak(&self) -> u64 {
        self.send_queue_peak.load(Ordering::Relaxed)
    }

    /// Note the queue depth a sender found when it woke.
    fn record_wake(&self, queued: usize, queued_bytes: usize) {
        self.send_queue_peak
            .fetch_max(queued as u64, Ordering::Relaxed);
        self.send_queue_bytes_peak
            .fetch_max(queued_bytes as u64, Ordering::Relaxed);
    }

    /// The most bytes a sender found charged to its queue when it woke.
    pub(crate) fn send_queue_bytes_peak(&self) -> u64 {
        self.send_queue_bytes_peak.load(Ordering::Relaxed)
    }

    /// Replicas disconnected for a full queue (bytes or frames).
    pub(crate) fn queue_disconnects(&self) -> u64 {
        self.queue_disconnects.load(Ordering::Relaxed)
    }

    /// `(partial, full)` synchronizations served to replicas.
    pub(crate) fn syncs(&self) -> (u64, u64) {
        (
            self.syncs_partial.load(Ordering::Relaxed),
            self.syncs_full.load(Ordering::Relaxed),
        )
    }

    /// `(flushes, frames)` written by replica senders since startup (live
    /// frames; see `send_frames`).
    pub(crate) fn send_stats(&self) -> (u64, u64) {
        (
            self.send_flushes.load(Ordering::Relaxed),
            self.send_frames.load(Ordering::Relaxed),
        )
    }

    /// Flushes held to the end of a scheduling round, since startup.
    pub(crate) fn send_deferred(&self) -> u64 {
        self.send_deferred.load(Ordering::Relaxed)
    }

    /// Count one sender flush of `frames` frames.
    fn record_send(&self, frames: usize, deferred: bool) {
        self.send_flushes.fetch_add(1, Ordering::Relaxed);
        self.send_frames.fetch_add(frames as u64, Ordering::Relaxed);
        if deferred {
            self.send_deferred.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn configure_dedup(&self, on: bool) {
        self.dedup.store(on, Ordering::Release);
    }

    /// Whether replication frames from this node carry DEDUP identities.
    pub(crate) fn streams_dedup(&self) -> bool {
        self.dedup.load(Ordering::Acquire)
    }

    /// The run id string this primary advertises.
    fn advertised_id(&self) -> String {
        if self.streams_dedup() {
            format!("{};caps={REPL_CAP_DEDUP}", self.run_id)
        } else {
            self.run_id.clone()
        }
    }

    fn stripe_for(key: &str) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish() as usize % WRITE_ORDER_STRIPES
    }

    /// Subscribe to write pushes, routed `routes` to begin with.
    ///
    /// A connection routed something is a consumer of write order: it is
    /// counted as a push receiver (see [`UnorderedGate`]) with the unordered
    /// path closed, so every write either completed before the subscription
    /// or sees the receiver. A connection routed [`Routes::Nothing`] — one
    /// that has not authenticated, or a strict one without a token — is
    /// pushed nothing, so it is not counted and closes nothing (6N-C PR 0e,
    /// RV-12): an unauthenticated socket used to force every writer on the
    /// server off the unordered fast path. It is counted by
    /// [`route_pushes`](Self::route_pushes) once it is routed. The count is
    /// lowered when the subscription drops.
    pub(crate) async fn subscribe_pushes(
        self: &Arc<Self>,
        tx: &SyncTx,
        routes: Routes<'_>,
    ) -> SyncSubscription {
        if matches!(routes, Routes::Nothing) {
            return tx.subscribe(routes);
        }
        let _closed = self.unordered.close().await;
        let mut subscription = tx.subscribe(routes);
        subscription.receiver = Some(self.count_push_receiver());
        subscription
    }

    /// Replace what `subscription` is routed. Returns once the new routes
    /// are published (and counted): every write that routes afterwards sees
    /// them.
    ///
    /// The first time a subscription is routed something it becomes a push
    /// receiver, with the unordered path closed exactly as in
    /// [`subscribe_pushes`](Self::subscribe_pushes): a write still on that
    /// path finishes first, and so is visible to everything the connection
    /// reads after its routes change. Routed nothing again, it stops being
    /// counted; that needs no gate, since a write that still sees the count
    /// only takes the ordered path needlessly.
    pub(crate) async fn route_pushes(
        self: &Arc<Self>,
        subscription: &mut SyncSubscription,
        routes: Routes<'_>,
    ) {
        let routed = !matches!(routes, Routes::Nothing);
        match (routed, subscription.receiver.is_some()) {
            (true, false) => {
                let _closed = self.unordered.close().await;
                subscription.set_routes(routes);
                subscription.receiver = Some(self.count_push_receiver());
            }
            (false, true) => {
                subscription.set_routes(routes);
                subscription.receiver = None;
            }
            _ => subscription.set_routes(routes),
        }
    }

    /// Count one push receiver; uncounted when the returned value drops.
    /// Callers hold the unordered gate closed.
    fn count_push_receiver(self: &Arc<Self>) -> PushReceiver {
        self.unordered.push_receivers.fetch_add(1, Ordering::SeqCst);
        PushReceiver {
            hub: Arc::clone(self),
        }
    }

    /// Push receivers counted now, for tests.
    #[cfg(test)]
    pub(crate) fn push_receiver_count(&self) -> usize {
        self.unordered.push_receivers.load(Ordering::SeqCst)
    }

    /// The stripes for one ordinary write on the ordered path. One of the
    /// two stripe takers that leave [`UnorderedGate`] open (the other is
    /// [`lock_identity`](Self::lock_identity), which touches no key): a write on this path
    /// either has consumers (so no write is unordered) or conflicts only with
    /// unordered writes that nothing observes the order of. The store's own
    /// write-order stripes still serialize the two in the keyspace.
    pub(crate) async fn lock_write<'a>(&'a self, command: &Command) -> OrderGuards<'a> {
        self.lock_stripes(
            std::slice::from_ref(command),
            std::iter::empty::<&str>(),
            None,
        )
        .await
    }

    /// Lock every ordering stripe touched by the supplied commands. Guards are
    /// acquired in numeric order, which also makes multi-key writes deadlock
    /// free. A global command such as FLUSHDB locks every stripe.
    ///
    /// Closes [`UnorderedGate`] for as long as the guards are held, like every
    /// stripe taker other than [`lock_write`](Self::lock_write): EXEC's
    /// isolation, ephemeral release and claims, and DEDUP refusals must not
    /// overlap a write that took no stripe.
    pub(crate) async fn lock_commands<'a>(&'a self, commands: &[Command]) -> OrderGuards<'a> {
        self.lock_commands_and_keys(commands, std::iter::empty::<&str>())
            .await
    }

    /// Lock command ordering domains plus explicit keys. Transactions include
    /// their WATCH set here before checking for invalidation, closing the gap
    /// where a conflicting writer could otherwise land after the check but
    /// before EXEC acquired its command barriers.
    pub(crate) async fn lock_commands_and_keys<'a, 'k, I>(
        &'a self,
        commands: &[Command],
        extra_keys: I,
    ) -> OrderGuards<'a>
    where
        I: IntoIterator<Item = &'k str>,
    {
        let closed = self.unordered.close().await;
        self.lock_stripes(commands, extra_keys, Some(closed)).await
    }

    async fn lock_stripes<'a, 'k, I>(
        &'a self,
        commands: &[Command],
        extra_keys: I,
        closed: Option<GateClosed<'a>>,
    ) -> OrderGuards<'a>
    where
        I: IntoIterator<Item = &'k str>,
    {
        // Inline for the one or two stripes nearly every write takes: the
        // stripe list and the guard list each cost an allocation per write
        // when they were `Vec`s.
        let mut stripes: SmallVec<[usize; 4]> = SmallVec::new();
        let mut lock_all = false;
        for command in commands {
            if !is_write_command(command) {
                continue;
            }
            match ordering_key_refs(command) {
                None => {
                    lock_all = true;
                    break;
                }
                Some(keys) => stripes.extend(keys.iter().map(|key| Self::stripe_for(key))),
            }
        }
        if lock_all {
            let closed = match closed {
                Some(closed) => closed,
                None => self.unordered.close().await,
            };
            return self.lock_every_stripe(closed).await;
        }
        stripes.extend(extra_keys.into_iter().map(Self::stripe_for));
        stripes.sort_unstable();
        stripes.dedup();

        let mut guards = SmallVec::with_capacity(stripes.len());
        for stripe in stripes {
            #[cfg(test)]
            if let Ok(guard) = self.write_order[stripe].try_lock() {
                guards.push(guard);
                continue;
            } else {
                self.stripe_waits.fetch_add(1, Ordering::SeqCst);
            }
            guards.push(self.write_order[stripe].lock().await);
        }
        OrderGuards {
            _stripes: guards,
            _closed: closed,
        }
    }

    /// Times a stripe taker found its stripe held and waited (test builds).
    #[cfg(test)]
    pub(crate) fn stripe_waits(&self) -> usize {
        self.stripe_waits.load(Ordering::SeqCst)
    }

    /// The one stripe of a DEDUP client identity, for recording a DEDUP
    /// outcome that changes no data and runs no command — a WebSocket write
    /// refused before it ran (6N-C PR 0b, RV-6).
    ///
    /// One stripe is enough to exclude every barrier that must not split
    /// such a record: a checkpoint (`ServerState::save`, which holds every
    /// stripe from its sidecar to its AOF truncate), a sub-replica's cut
    /// (`handle_replica`, which reads the offset and the DEDUP table under
    /// every stripe — the record is also a replication frame) and a
    /// promotion. It leaves [`UnorderedGate`] open: the record touches no
    /// key, so it has no order to observe against an unordered write. It
    /// blocks only writers that hash to the same stripe, and only for its
    /// own AOF append. The refused command's own stripes, which for
    /// `FLUSHDB` are all of them, are not taken.
    pub(crate) async fn lock_identity(&self, client: &str) -> OrderGuards<'_> {
        self.lock_stripes(&[], std::iter::once(client), None).await
    }

    /// The all-write barrier: every stripe, with [`UnorderedGate`] closed and
    /// drained first so no unordered write is in flight under it either.
    pub(crate) async fn lock_all_writes(&self) -> OrderGuards<'_> {
        let closed = self.unordered.close().await;
        self.lock_every_stripe(closed).await
    }

    async fn lock_every_stripe<'a>(&'a self, closed: GateClosed<'a>) -> OrderGuards<'a> {
        let mut guards = SmallVec::with_capacity(WRITE_ORDER_STRIPES);
        for lock in &self.write_order {
            guards.push(lock.lock().await);
        }
        OrderGuards {
            _stripes: guards,
            _closed: Some(closed),
        }
    }

    pub(crate) fn configure_backlog(&self, bytes: usize) {
        self.backlog_bytes.store(bytes.max(1), Ordering::Release);
    }

    pub(crate) fn configure_queue_bytes(&self, bytes: usize) {
        self.queue_bytes.store(bytes.max(1), Ordering::Release);
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.backlog_bytes.load(Ordering::Acquire) > 0
    }

    async fn resume_frames(
        &self,
        requested_run_id: &str,
        requested_offset: u64,
    ) -> Option<Vec<ReplFrame>> {
        if requested_run_id != self.run_id {
            return None;
        }
        let current = self.offset.load(Ordering::Acquire);
        if requested_offset > current {
            return None;
        }
        let stream = self.stream();
        let backlog = &stream.backlog;
        if requested_offset == current {
            return Some(Vec::new());
        }
        let first = backlog.frames.front()?.offset;
        if requested_offset.saturating_add(1) < first {
            return None;
        }
        Some(
            backlog
                .frames
                .iter()
                .filter(|frame| frame.offset > requested_offset)
                .cloned()
                .collect(),
        )
    }
}

pub(crate) type ReplRegistry = Arc<ReplHub>;

/// Default per-replica queue budget (`RECACHED_REPL_BUFFER_BYTES`): what
/// frames waiting for a replica's sender may hold, payload plus
/// [`REPL_QUEUE_FRAME_OVERHEAD`] each. A replica whose queue would pass it is
/// disconnected so it can reconnect and request the retained backlog, falling
/// back to a full snapshot if necessary. The primary write path is never
/// blocked.
pub(crate) const DEFAULT_REPL_QUEUE_BYTES: usize = 8 * 1024 * 1024;

/// Bytes charged against the queue budget for each queued frame on top of its
/// payload (6N-C6): the memory a queued frame holds besides its bytes, and the
/// bound on how many frames a budget admits.
///
/// A queued frame is a `QueuedReplFrame` (32 bytes) in a tokio channel block
/// slot (about 40 bytes with its share of the 32-slot block's 1280-byte
/// allocation), plus the frame's `Arc<Vec<u8>>` allocation (40 bytes, a
/// 48-byte jemalloc class) and the payload allocation's rounding (under 16
/// bytes for frames under 128 bytes): about 105 bytes. The `Arc` and payload are shared
/// with the backlog while it retains the frame, so this over-counts what the
/// queue adds then; it is the bound for when the backlog has trimmed it.
/// Payload rounding above that size (jemalloc's size classes, at most a
/// quarter) was not charged before 6N-C6 and is not now.
///
/// Before 6N-C6 the payload alone was charged, and a separate frame count
/// (`RECACHED_REPL_BUFFER`, 4096) bounded the rest. Small frames filled that
/// count with 0.3% of the byte budget used, and one scheduling round of
/// pipelining connections could overrun it and disconnect a healthy replica
/// (6N-C5 review R1). Charging the overhead lets the byte budget bound memory
/// on its own, so the default frame cap can follow it.
pub(crate) const REPL_QUEUE_FRAME_OVERHEAD: usize = 128;

/// Ceiling on a frame cap derived from the byte budget: 128 MiB of budget at
/// [`REPL_QUEUE_FRAME_OVERHEAD`]. A larger budget keeps this cap unless
/// `RECACHED_REPL_BUFFER` sets one. It also keeps the derived cap far below
/// the most permits a tokio channel accepts.
pub(crate) const REPL_QUEUE_MAX_DERIVED_FRAMES: usize = 1 << 20;

/// What a frame of `len` payload bytes charges against a replica's queue
/// budget. Saturates: a charge that cannot be represented exceeds every
/// budget, so the frame is refused rather than wrapped into a small charge.
pub(crate) fn repl_queue_charge(len: usize) -> usize {
    len.saturating_add(REPL_QUEUE_FRAME_OVERHEAD)
}

/// The per-replica channel capacity, in frames.
///
/// An explicit `RECACHED_REPL_BUFFER` is kept exactly as configured: a strict
/// frame cap, as before 6N-C6. Without one, the cap is derived from the byte
/// budget: as many frames as it can charge at [`REPL_QUEUE_FRAME_OVERHEAD`]
/// each, at least one, and at most [`REPL_QUEUE_MAX_DERIVED_FRAMES`]. Every
/// frame charges more than the overhead, so with a derived cap the byte
/// budget is reached first and the count is a backstop.
pub(crate) fn repl_channel_capacity(explicit: Option<usize>, queue_bytes: usize) -> usize {
    explicit.unwrap_or_else(|| {
        (queue_bytes / REPL_QUEUE_FRAME_OVERHEAD).clamp(1, REPL_QUEUE_MAX_DERIVED_FRAMES)
    })
}

/// Largest explicit frame cap honoured: tokio's channels refuse more permits
/// than this (`Semaphore::MAX_PERMITS`), and a larger capacity would panic the
/// replica's task at its handshake.
pub(crate) const REPL_QUEUE_MAX_EXPLICIT_FRAMES: usize = usize::MAX >> 3;

/// `RECACHED_REPL_BUFFER` as configured: a positive frame count, clamped to
/// [`REPL_QUEUE_MAX_EXPLICIT_FRAMES`]. `None` when it is unset, zero or not a
/// number, and the cap is then derived from the byte budget; the caller
/// warns about the last two.
pub(crate) fn parse_repl_frame_cap(raw: Option<&str>) -> Option<usize> {
    raw.and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|&frames| frames > 0)
        .map(|frames| frames.min(REPL_QUEUE_MAX_EXPLICIT_FRAMES))
}

/// The channel capacity with neither setting configured: 65,536 frames.
/// The server derives it with [`repl_channel_capacity`]; tests name it.
#[cfg(test)]
pub(crate) const DEFAULT_REPL_CHANNEL_CAPACITY: usize =
    DEFAULT_REPL_QUEUE_BYTES / REPL_QUEUE_FRAME_OVERHEAD;
/// Replication-order guards held by one write or barrier; released on drop,
/// stripes first, then the gate reopens.
pub(crate) struct OrderGuards<'a> {
    _stripes: SmallVec<[tokio::sync::MutexGuard<'a, ()>; 2]>,
    _closed: Option<GateClosed<'a>>,
}

pub(crate) const DEFAULT_REPL_BACKLOG_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const REPL_MAGIC: &[u8; 4] = b"RCP1";

/// Upper bound on a single length-prefixed replication frame (snapshot or
/// command). The replication port may be unauthenticated and plaintext, so an
/// untrusted peer could otherwise send a 4 GB length prefix and force a matching
/// allocation. 512 MB comfortably covers a large snapshot while bounding abuse.
pub(crate) const MAX_REPL_FRAME_BYTES: usize = 512 * 1024 * 1024;

// ── Server state ──────────────────────────────────────────────────────────────

/// Per-peer replication auth throttle.
///
/// The RESP port drops a connection after `MAX_AUTH_FAILURES` guesses, but the
/// replication handshake is one-shot: a wrong password costs the attacker a
/// single TCP connection and nothing else, so the port offered effectively
/// unlimited guesses at a secret that yields the entire keyspace. Failures are
/// counted per source address over a rolling window, and a peer that exhausts
/// them is refused before the handshake is read at all.
///
/// Keyed by address rather than by connection, which is the whole point — the
/// weakness being closed is that reconnecting reset the count.
pub(crate) struct ReplAuthThrottle {
    pub(crate) failures: std::sync::Mutex<HashMap<IpAddr, (u32, std::time::Instant)>>,
}

impl ReplAuthThrottle {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            failures: std::sync::Mutex::new(HashMap::new()),
        })
    }

    /// True when this peer has spent its attempts and must be refused.
    pub(crate) fn is_blocked(&self, ip: IpAddr) -> bool {
        let Ok(map) = self.failures.lock() else {
            return false;
        };
        match map.get(&ip) {
            Some((count, last)) => *count >= MAX_AUTH_FAILURES && last.elapsed() < REPL_AUTH_WINDOW,
            None => false,
        }
    }

    pub(crate) fn record_failure(&self, ip: IpAddr) {
        let Ok(mut map) = self.failures.lock() else {
            return;
        };
        let now = std::time::Instant::now();
        // Sweep before inserting so a spray across many source addresses cannot
        // grow the map without bound.
        if map.len() >= REPL_AUTH_SWEEP_THRESHOLD {
            map.retain(|_, (_, last)| last.elapsed() < REPL_AUTH_WINDOW);
        }
        let entry = map.entry(ip).or_insert((0, now));
        // A peer that went quiet for longer than the window starts over, so a
        // slow trickle is not punished forever.
        if entry.1.elapsed() >= REPL_AUTH_WINDOW {
            *entry = (0, now);
        }
        entry.0 = entry.0.saturating_add(1);
        entry.1 = now;
    }

    pub(crate) fn record_success(&self, ip: IpAddr) {
        if let Ok(mut map) = self.failures.lock() {
            map.remove(&ip);
        }
    }
}

/// Read the newline-terminated replication auth line.
///
/// Reads until the terminator rather than reading exactly `password.len() + 1`
/// bytes, which is how the previous implementation worked: the number of bytes
/// the server waited for *was* the password length, so an attacker could
/// recover it exactly by drip-feeding one byte at a time and watching when the
/// server replied.
pub(crate) async fn read_repl_auth_line<S>(socket: &mut S) -> std::io::Result<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut line = Vec::with_capacity(64);
    let mut byte = [0u8; 1];
    loop {
        socket.read_exact(&mut byte).await?;
        if byte[0] == b'\n' {
            return Ok(line);
        }
        if line.len() >= MAX_REPL_AUTH_LINE {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "replication auth line too long",
            ));
        }
        line.push(byte[0]);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_repl_server(
    bind_host: String,
    port: u16,
    store: Arc<KeyValueStore>,
    dedup_source: Arc<ServerState>,
    replicas: ReplRegistry,
    repl_password: Option<Arc<String>>,
    repl_channel_capacity: usize,
    allowed_ips: Option<Arc<Vec<IpAddr>>>,
    semaphore: Arc<Semaphore>,
    throttle: Arc<ReplAuthThrottle>,
    tls: Arc<Option<TlsAcceptor>>,
) {
    let listener = match TcpListener::bind(format!("{}:{}", bind_host, port)).await {
        Ok(l) => l,
        Err(e) => {
            warn!("Replication listener failed to bind :{}: {}", port, e);
            return;
        }
    };
    info!(
        "Replication server listening on {}:{} ({})",
        bind_host,
        port,
        if tls.is_some() { "TLS" } else { "plaintext" }
    );
    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                // Frames and acknowledgements are small writes. With Nagle's
                // algorithm on, each waited for the peer's ACK of the last —
                // which Linux delays by up to 40 ms — so every replicated write
                // reached the replica ~41 ms late (6I probe lag p50, every
                // workload). The client ports already set this.
                let _ = socket.set_nodelay(true);
                // The IP allowlist and the connection limit were previously
                // applied on the RESP and WebSocket listeners only, so neither
                // constrained the one port that streams the whole keyspace.
                if let Some(allowed) = &allowed_ips
                    && !allowed.contains(&addr.ip())
                {
                    debug!("Replication: rejected IP {}", addr.ip());
                    continue;
                }
                if throttle.is_blocked(addr.ip()) {
                    warn!(
                        "Replication: {} refused — too many failed auth attempts",
                        addr.ip()
                    );
                    continue;
                }
                let permit = match Arc::clone(&semaphore).try_acquire_owned() {
                    Ok(p) => p,
                    Err(_) => {
                        warn!("Replication: connection limit reached, dropping {}", addr);
                        continue;
                    }
                };
                info!("Replica connected from {}", addr);
                let store = Arc::clone(&store);
                let dedup_source = Arc::clone(&dedup_source);
                let replicas = Arc::clone(&replicas);
                let pwd = repl_password.clone();
                let thr = Arc::clone(&throttle);
                let tls = Arc::clone(&tls);
                tokio::spawn(async move {
                    let _permit = permit;
                    // Bounded like the other listeners: the permit is already
                    // held, so a peer that never negotiates must not keep it.
                    let outcome = if let Some(acceptor) = tls.as_ref() {
                        match tokio::time::timeout(handshake_timeout(), acceptor.accept(socket))
                            .await
                        {
                            Ok(Ok(stream)) => {
                                handle_replica(
                                    stream,
                                    store,
                                    dedup_source,
                                    replicas,
                                    pwd,
                                    repl_channel_capacity,
                                    addr.ip(),
                                    thr,
                                )
                                .await
                            }
                            Ok(Err(e)) => {
                                // The most likely cause by far is a replica that
                                // has not been given RECACHED_REPL_TLS_CA, which
                                // otherwise looks like an unexplained disconnect.
                                warn!(
                                    "Replication TLS handshake failed from {}: {} — is that \
                                     replica configured with RECACHED_REPL_TLS_CA?",
                                    addr, e
                                );
                                return;
                            }
                            Err(_) => {
                                debug!("Replication TLS handshake from {} timed out", addr);
                                return;
                            }
                        }
                    } else {
                        handle_replica(
                            socket,
                            store,
                            dedup_source,
                            replicas,
                            pwd,
                            repl_channel_capacity,
                            addr.ip(),
                            thr,
                        )
                        .await
                    };
                    if let Err(e) = outcome {
                        info!("Replica {} disconnected: {}", addr, e);
                    }
                });
            }
            Err(e) => warn!("Replication accept error: {}", e),
        }
    }
}

/// A replica's connection while it is registered with the hub.
///
/// Once its `ReplicaHandle` is dropped — `fan_out` unregistered it because
/// its queue was full, or the hub itself is gone — every write fails. A
/// sender blocked writing to a peer that stopped reading used to stay there,
/// holding its task, its socket, its connection permit and every frame still
/// queued for it (up to the queue budget, out of every gauge, since the
/// handle was gone), until the peer read again or TCP gave up (6N-C6 review
/// M1). It now returns at once, and dropping it releases all of that. The
/// replica resumes from the last frame it applied: it applies only frames it
/// read whole, so a frame cut off mid-write is never applied.
struct Registered<S> {
    inner: S,
    /// Resolves when the handle is dropped; `None` once it has.
    handle: Option<tokio::sync::oneshot::Receiver<()>>,
}

impl<S> Registered<S> {
    /// `Err` once the replica is unregistered. Otherwise registers `cx` to
    /// be woken when it is, so a write pending on a full socket fails then.
    fn registered(&mut self, cx: &mut std::task::Context<'_>) -> std::io::Result<()> {
        if let Some(handle) = &mut self.handle {
            if std::pin::Pin::new(handle).poll(cx).is_pending() {
                return Ok(());
            }
            self.handle = None;
        }
        Err(std::io::Error::new(
            ErrorKind::ConnectionAborted,
            "replica unregistered (its queue was full, or the primary is stopping)",
        ))
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Registered<S> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Registered<S> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        if let Err(e) = self.registered(cx) {
            return std::task::Poll::Ready(Err(e));
        }
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if let Err(e) = self.registered(cx) {
            return std::task::Poll::Ready(Err(e));
        }
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_replica<S>(
    mut socket: S,
    store: Arc<KeyValueStore>,
    dedup_source: Arc<ServerState>,
    replicas: ReplRegistry,
    repl_password: Option<Arc<String>>,
    repl_channel_capacity: usize,
    peer_ip: IpAddr,
    throttle: Arc<ReplAuthThrottle>,
) -> std::io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    if !replicas.is_enabled() {
        replicas.configure_backlog(DEFAULT_REPL_BACKLOG_BYTES);
    }
    // 0. Auth handshake — replica must send "<password>\n" before anything else.
    //
    // Bounded by a deadline as well as a length: a peer that connects and then
    // says nothing would otherwise hold its connection permit forever.
    if let Some(pwd) = &repl_password {
        let line = match tokio::time::timeout(handshake_timeout(), read_repl_auth_line(&mut socket))
            .await
        {
            Ok(res) => res?,
            Err(_) => {
                return Err(std::io::Error::new(
                    ErrorKind::TimedOut,
                    "replication auth handshake timed out",
                ));
            }
        };
        if !ct_eq_bytes(&line, pwd.as_bytes()) {
            throttle.record_failure(peer_ip);
            let _ = socket
                .write_all(b"-ERR invalid replication password\n")
                .await;
            return Err(std::io::Error::new(
                ErrorKind::PermissionDenied,
                "replication auth failed",
            ));
        }
        throttle.record_success(peer_ip);
        socket.write_all(b"+OK\n").await?;
        socket.flush().await?;
    }

    // Versioned resume handshake. A replica identifies the primary run and
    // last offset it applied; an empty run id requests a full synchronization.
    socket.write_all(REPL_MAGIC).await?;
    let advertised = replicas.advertised_id();
    let advertised = advertised.as_bytes();
    socket
        .write_all(&(advertised.len() as u16).to_le_bytes())
        .await?;
    socket.write_all(advertised).await?;
    socket.flush().await?;
    let (requested_raw, requested_offset) =
        tokio::time::timeout(handshake_timeout(), read_resume_request(&mut socket))
            .await
            .map_err(|_| {
                std::io::Error::new(
                    ErrorKind::TimedOut,
                    "replication resume handshake timed out",
                )
            })??;

    let requested = HandshakeId::parse(&requested_raw);
    let requested_run_id = requested.run_id.to_string();
    let with_dedup = requested.has("accept", REPL_CAP_DEDUP);
    if replicas.streams_dedup() && !with_dedup {
        // A replica that cannot read DEDUP identities would fail on the first
        // envelope frame — or, worse, a future frame could be misread. Refuse
        // it up front, with a reason a 0.6 replica logs; a 0.5 replica reports
        // an invalid synchronization mode and retries with backoff.
        let reason = "this primary replicates DEDUP state (RECACHED_REPL_DEDUP=on) and the replica \
                      does not support it: upgrade the replica to 0.6, or set \
                      RECACHED_REPL_DEDUP=off on the primary";
        error!("Refusing replica {peer_ip}: {reason}");
        counter!("recached_replication_refusals_total", "reason" => "dedup_unsupported")
            .increment(1);
        socket.write_all(&[SYNC_MODE_REFUSED]).await?;
        socket
            .write_all(&(reason.len() as u16).to_le_bytes())
            .await?;
        socket.write_all(reason.as_bytes()).await?;
        socket.flush().await?;
        return Err(std::io::Error::new(
            ErrorKind::Unsupported,
            "replica does not accept DEDUP replication",
        ));
    }

    // Establish one exact snapshot/log boundary. Holding every write-order
    // stripe ensures no mutation can commit between channel registration and
    // snapshot/backlog capture; after the guards drop, every later mutation is
    // queued with a strictly higher offset.
    let sync_started = std::time::Instant::now();
    let write_barrier = replicas.lock_all_writes().await;
    let current_offset = replicas.offset.load(Ordering::Acquire);
    let missing = replicas
        .resume_frames(&requested_run_id, requested_offset)
        .await;
    let partial_sync = missing.is_some();
    let snapshot = if missing.is_none() {
        Some(
            rmp_serde::to_vec(&store.snapshot())
                .map_err(|e| std::io::Error::other(e.to_string()))?,
        )
    } else {
        None
    };
    // Captured under the same barrier as the snapshot: every write it holds
    // committed its mark before releasing its guards, and none can commit one
    // until the barrier drops, so data and marks describe one instant.
    let dedup_table = if snapshot.is_some() && with_dedup {
        Some(
            rmp_serde::to_vec(&dedup_source.dedup_table())
                .map_err(|e| std::io::Error::other(e.to_string()))?,
        )
    } else {
        None
    };

    // Register before releasing the barrier so every offset after the chosen
    // boundary is buffered while the snapshot or backlog is sent.
    let (tx, mut rx) = mpsc::channel::<QueuedReplFrame>(repl_channel_capacity);
    let sent = Arc::new(AtomicU64::new(current_offset));
    let acked = Arc::new(AtomicU64::new(if missing.is_some() {
        requested_offset
    } else {
        0
    }));
    let pending_bytes = Arc::new(AtomicUsize::new(0));
    let max_queue_bytes = replicas.queue_bytes.load(Ordering::Acquire);
    let (link, unregistered) = tokio::sync::oneshot::channel();
    {
        let mut stream = replicas.stream();
        let reg = &mut stream.senders;
        reg.push(ReplicaHandle {
            tx,
            sent: Arc::clone(&sent),
            acked: Arc::clone(&acked),
            pending_bytes: Arc::clone(&pending_bytes),
            max_queue_bytes,
            _link: link,
        });
        replicas.count.store(reg.len(), Ordering::Relaxed);
    }
    drop(write_barrier);
    let mut socket = Registered {
        inner: socket,
        handle: Some(unregistered),
    };

    let mode = match (&snapshot, &dedup_table) {
        (Some(_), Some(_)) => SYNC_MODE_FULL_DEDUP,
        (Some(_), None) => b'F',
        (None, _) => b'P',
    };
    socket.write_all(&[mode]).await?;
    socket.write_all(&current_offset.to_le_bytes()).await?;
    if let Some(snapshot) = snapshot {
        counter!("recached_replication_syncs_total", "type" => "full").increment(1);
        replicas.syncs_full.fetch_add(1, Ordering::Relaxed);
        let len = u32::try_from(snapshot.len()).map_err(|_| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                "snapshot exceeds replication frame limit",
            )
        })?;
        socket.write_all(&len.to_le_bytes()).await?;
        socket.write_all(&snapshot).await?;
        if let Some(table) = dedup_table {
            let len = u32::try_from(table.len()).map_err(|_| {
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "DEDUP table exceeds replication frame limit",
                )
            })?;
            socket.write_all(&len.to_le_bytes()).await?;
            socket.write_all(&table).await?;
        }
    } else if let Some(missing) = missing {
        counter!("recached_replication_syncs_total", "type" => "partial").increment(1);
        replicas.syncs_partial.fetch_add(1, Ordering::Relaxed);
        let count = u32::try_from(missing.len()).map_err(|_| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                "replication backlog has too many frames",
            )
        })?;
        socket.write_all(&count.to_le_bytes()).await?;
        for frame in &missing {
            write_repl_frame(&mut socket, frame).await?;
        }
    }
    socket.flush().await?;
    histogram!(
        "recached_replication_sync_duration_seconds",
        "type" => if partial_sync {
            "partial"
        } else {
            "full"
        }
    )
    .record(sync_started.elapsed().as_secs_f64());

    // 3. Stream buffered + ongoing writes, and read acknowledgements
    //
    // The socket is bidirectional but used to carry frames one way only, which
    // left the primary unable to say how far behind a replica was. The replica
    // now writes back a cumulative count of applied frames; `sent - acked` is
    // the lag. Reading and writing are selected over so a replica that stops
    // acknowledging cannot stall the write side, and vice versa.
    let (mut rd, wr) = tokio::io::split(socket);
    // Buffered, and flushed once per burst: a frame used to cost three
    // unbuffered writes and a flush — four syscalls per replicated write — so
    // a pipelined primary out-ran its replica's link and the replica fell
    // seconds behind (6I production benchmark: probe lag p99 2.8 s).
    let mut wr = tokio::io::BufWriter::with_capacity(REPL_SEND_BUFFER_BYTES, wr);
    let mut acks = AckReader::default();
    // Frames written into `wr` since its last flush, and the end of the
    // scheduling round that will flush them (6N-C5).
    //
    // The first `try_send` after a drain wakes this task into the worker's
    // LIFO slot, so it runs right after that one writer. It used to flush on
    // every such wake: 3.6 frames per flush at P=1, and 3.8 with the
    // coalesced AOF at P=16, where every command's AOF yield handed the
    // worker back to it. Each flush is a socket send plus, on the replica, an
    // acknowledgement this side reads, and that kernel work was most of a
    // replica's cost per write (6N-C5 profile). It still drains the channel
    // on every wake, so within a round the queue stays at about one writer's
    // poll, as before; it only holds the *flush* until the round ends — tokio resumes a
    // `yield_now` once the worker has run its other ready tasks, or at the
    // next maintenance tick (every 61 polls) — or until a full flush
    // (`REPL_SEND_BATCH_FRAMES`) is buffered. The frames of the whole round
    // then leave in one send.
    //
    // Only *when* bytes reach the socket changes: every frame is in the
    // backlog and drained in offset order, after its AOF stage, and nothing
    // a client waits for depends on the send. The cost is replica lag of at
    // most one round of the worker that armed the wait: sub-millisecond in
    // the 6N-C5 benchmarks, but as long as a long synchronous poll on that
    // worker (`KEYS` on a large keyspace) if one runs meanwhile; frames keep
    // draining on other workers, only the send waits (review S3).
    //
    // One difference from before (review S1): when the round ends, tokio
    // wakes the held flush after the tasks that round deferred, so the sender
    // runs behind them, and a `try_send` cannot move a task that is already
    // queued. Writers that end their poll deferred (the coalesced AOF's
    // yield, a pipeline out of coop budget) can then queue up to a round's
    // frames before it drains. Before 6N-C5 the same happened whenever a
    // replica acknowledgement woke the sender behind them. The wake that does
    // it is any the sender gets while its worker parks between polls — the
    // held flush's deferral, or an acknowledgement's readiness — at the
    // maintenance park every 61 polls too, mid-round, which queues it behind
    // the round's writers not yet run (6N-C6 root cause). The queue must
    // hold their frames, so the default cap follows the byte budget rather
    // than a fixed frame count; a round past the byte budget, or an explicit
    // `RECACHED_REPL_BUFFER` smaller than the round, disconnects the replica
    // to resynchronize (`pipelined_rounds_at_default_capacity_with_acks`).
    let mut unflushed = 0usize;
    let mut flush_pending = false;
    let mut round_end = RoundEnd::default();
    loop {
        tokio::select! {
            frame = rx.recv() => {
                let Some(queued) = frame else {
                    // The handle is gone, so `Registered` refuses this flush
                    // and the stream ends; the replica resumes from what it
                    // applied.
                    if unflushed > 0 {
                        wr.flush().await?;
                        replicas.record_send(unflushed, flush_pending);
                    }
                    break;
                };
                replicas.record_wake(rx.len() + 1, pending_bytes.load(Ordering::Relaxed));
                write_repl_frame(&mut wr, &queued.frame).await?;
                unflushed += 1;
                // Everything already queued goes into the buffer too. Frames
                // leave the queue only once written, as before, so the byte
                // accounting of `QueuedReplFrame` is unchanged.
                while unflushed < REPL_SEND_BATCH_FRAMES {
                    match rx.try_recv() {
                        Ok(queued) => {
                            write_repl_frame(&mut wr, &queued.frame).await?;
                            unflushed += 1;
                        }
                        Err(_) => break,
                    }
                }
                if unflushed >= REPL_SEND_BATCH_FRAMES
                    || rx.max_capacity() < REPL_SEND_DEFER_MIN_CAPACITY
                {
                    // A full flush goes out now, so a busy primary still
                    // flushes regularly. So does every flush on a small
                    // channel (see `REPL_SEND_DEFER_MIN_CAPACITY`).
                    wr.flush().await?;
                    replicas.record_send(unflushed, false);
                    unflushed = 0;
                    flush_pending = false;
                } else if !flush_pending {
                    flush_pending = true;
                    round_end = RoundEnd::default();
                }
            }
            () = &mut round_end, if flush_pending => {
                flush_pending = false;
                wr.flush().await?;
                replicas.record_send(unflushed, true);
                unflushed = 0;
            }
            // `read`, not `read_exact`: when another branch wins, a
            // `read_exact` already part-way through an acknowledgement was
            // dropped with the bytes it had consumed, and every later
            // acknowledgement was read across a boundary (6N-C5 review F4).
            // `read` is cancel-safe — a dropped one consumed nothing — and
            // `AckReader` keeps a partial acknowledgement between iterations.
            res = rd.read(acks.spare()) => {
                match res {
                    // The replica closed its side, or the link failed: the
                    // stream ends, as it did before, and the replica is
                    // unregistered at the next write and reconnects. A
                    // replica that predates acks never writes, so for it
                    // this branch never completes.
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if let Some(applied) = acks.take(n) {
                            // Monotonic: a reordered or replayed ack must
                            // never walk the high-water mark backwards and
                            // report negative lag.
                            acked.fetch_max(applied, Ordering::Relaxed);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// A replica's acknowledgements as they arrive: each is the cumulative
/// offset it has applied, 8 bytes little-endian (`run_repl_client`). Bytes
/// read so far are kept here, not in a read future, so a partial
/// acknowledgement survives the `select!` iteration another branch wins.
struct AckReader {
    buf: [u8; ACK_READ_BYTES],
    /// Bytes of `buf` holding received data: after `take`, under 8.
    filled: usize,
}

/// Bytes one read takes from the acknowledgement side: several
/// acknowledgements, when the sender was busy while they arrived.
const ACK_READ_BYTES: usize = 64;
const ACK_BYTES: usize = 8;

impl Default for AckReader {
    fn default() -> Self {
        AckReader {
            buf: [0; ACK_READ_BYTES],
            filled: 0,
        }
    }
}

impl AckReader {
    /// Where the next read goes. Never empty: `take` keeps under one
    /// acknowledgement.
    fn spare(&mut self) -> &mut [u8] {
        &mut self.buf[self.filled..]
    }

    /// Accounts `n` bytes just read into `spare()`. Returns the highest
    /// complete acknowledgement now received, and keeps a trailing partial
    /// one for the next read.
    fn take(&mut self, n: usize) -> Option<u64> {
        self.filled += n;
        let complete = self.filled - self.filled % ACK_BYTES;
        let highest = self.buf[..complete]
            .as_chunks::<ACK_BYTES>()
            .0
            .iter()
            .map(|ack| u64::from_le_bytes(*ack))
            .max();
        self.buf.copy_within(complete..self.filled, 0);
        self.filled -= complete;
        highest
    }
}

/// Completes once the worker has run its other ready tasks — tokio's
/// `yield_now` deferral — however often it is polled before that.
///
/// `yield_now`'s own future completes on its second poll whether or not the
/// deferred wake has fired, so it cannot be kept across a replica sender's
/// wakes: the next writer's wake would end the "round". This polls it once,
/// through a waker that records when the deferred wake really fires.
#[derive(Default)]
struct RoundEnd {
    armed: Option<Arc<RoundWake>>,
}

struct RoundWake {
    fired: AtomicBool,
    /// The waker of the latest poll (review S2: a future must wake the
    /// waker it was last polled with).
    task: std::sync::Mutex<std::task::Waker>,
}

impl std::task::Wake for RoundWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.fired.store(true, Ordering::Release);
        self.task
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .wake_by_ref();
    }
}

impl std::future::Future for RoundEnd {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        if let Some(wake) = &self.armed {
            if wake.fired.load(Ordering::Acquire) {
                return std::task::Poll::Ready(());
            }
            let mut task = wake
                .task
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !task.will_wake(cx.waker()) {
                task.clone_from(cx.waker());
            }
            drop(task);
            // Fired between the check and the update: the old waker was
            // woken, so look again rather than wait for a wake that came.
            return if wake.fired.load(Ordering::Acquire) {
                std::task::Poll::Ready(())
            } else {
                std::task::Poll::Pending
            };
        }
        let wake = Arc::new(RoundWake {
            fired: AtomicBool::new(false),
            task: std::sync::Mutex::new(cx.waker().clone()),
        });
        let waker = std::task::Waker::from(Arc::clone(&wake));
        let mut deferral = std::pin::pin!(tokio::task::yield_now());
        // Tokio keeps the waker in the worker's defer list after the yield
        // future is dropped; off a worker it wakes at once.
        if deferral
            .as_mut()
            .poll(&mut std::task::Context::from_waker(&waker))
            .is_ready()
        {
            return std::task::Poll::Ready(());
        }
        self.armed = Some(wake);
        std::task::Poll::Pending
    }
}

async fn read_resume_request<S>(socket: &mut S) -> std::io::Result<(String, u64)>
where
    S: AsyncRead + Unpin,
{
    let mut len = [0u8; 2];
    socket.read_exact(&mut len).await?;
    let len = u16::from_le_bytes(len) as usize;
    if len > 128 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "replication run id is too long",
        ));
    }
    let mut run_id = vec![0u8; len];
    socket.read_exact(&mut run_id).await?;
    let run_id = String::from_utf8(run_id)
        .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "invalid replication run id"))?;
    let mut offset = [0u8; 8];
    socket.read_exact(&mut offset).await?;
    Ok((run_id, u64::from_le_bytes(offset)))
}

async fn write_repl_frame<S>(socket: &mut S, frame: &ReplFrame) -> std::io::Result<()>
where
    S: AsyncWrite + Unpin,
{
    let len = u32::try_from(frame.bytes.len()).map_err(|_| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            "command exceeds replication frame limit",
        )
    })?;
    socket.write_all(&frame.offset.to_le_bytes()).await?;
    socket.write_all(&len.to_le_bytes()).await?;
    socket.write_all(&frame.bytes).await
}

// ── Replication client (replica side) ────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_repl_client(
    primary_addr: String,
    store: Arc<KeyValueStore>,
    state: Arc<ServerState>,
    repl_password: Option<String>,
    failover_timeout_secs: Option<u64>,
    tx: SyncTx,
    watches: WatchRegistry,
    tls: Option<(TlsConnector, String)>,
) {
    let mut backoff_secs = 2u64;
    let mut resume = ReplResume::default();
    if failover_timeout_secs.is_some() {
        warn!(
            "RECACHED_FAILOVER_TIMEOUT is ignored: automatic promotion without quorum or fencing can create split brain; fence the old primary, then use REPLICAOF NO ONE"
        );
    }

    loop {
        // Stop after an operator promotes this replica with REPLICAOF NO ONE.
        if !state.is_replica() {
            return;
        }

        info!("Replica: connecting to primary at {}", primary_addr);
        match TcpStream::connect(&primary_addr).await {
            Err(e) => {
                warn!("Replica: connect failed: {}", e);
            }
            Ok(socket) => {
                backoff_secs = 2;
                // As on the primary's side: no Nagle delay on small frames
                // and acknowledgements.
                let _ = socket.set_nodelay(true);

                // TLS is what makes the primary's *identity* checked, not just
                // the channel encrypted: without it a DNS hijack or an on-path
                // attacker can feed this replica an arbitrary keyspace, and the
                // replica has no way to tell.
                let result = match &tls {
                    None => {
                        sync_from_primary(
                            &mut { socket },
                            &store,
                            repl_password.as_deref(),
                            &tx,
                            &watches,
                            &state,
                            &mut resume,
                        )
                        .await
                    }
                    Some((connector, servername)) => {
                        match ServerName::try_from(servername.clone()) {
                            Err(_) => {
                                error!(
                                    "Replica: '{}' is not a valid TLS server name — set \
                                     RECACHED_REPL_TLS_SERVERNAME to the name on the primary's \
                                     certificate",
                                    servername
                                );
                                return;
                            }
                            Ok(name) => {
                                match tokio::time::timeout(
                                    handshake_timeout(),
                                    connector.connect(name, socket),
                                )
                                .await
                                {
                                    Err(_) => Err(std::io::Error::new(
                                        ErrorKind::TimedOut,
                                        "TLS handshake with primary timed out",
                                    )),
                                    Ok(Err(e)) => Err(std::io::Error::other(format!(
                                        "TLS handshake with primary failed: {e} — check that the \
                                         primary has RECACHED_TLS_CERT set and that \
                                         RECACHED_REPL_TLS_CA trusts it"
                                    ))),
                                    Ok(Ok(mut stream)) => {
                                        sync_from_primary(
                                            &mut stream,
                                            &store,
                                            repl_password.as_deref(),
                                            &tx,
                                            &watches,
                                            &state,
                                            &mut resume,
                                        )
                                        .await
                                    }
                                }
                            }
                        }
                    }
                };

                if let Err(e) = result {
                    warn!("Replica: sync ended: {}", e);
                    if e.kind() == ErrorKind::InvalidData {
                        // A gap or invalid replicated command means the local
                        // state cannot safely resume from its current offset.
                        resume = ReplResume::default();
                    }
                }
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
        backoff_secs = (backoff_secs * 2).min(30);
    }
}

#[derive(Default)]
pub(crate) struct ReplResume {
    run_id: String,
    offset: u64,
}

pub(crate) async fn sync_from_primary<S>(
    socket: &mut S,
    store: &KeyValueStore,
    repl_password: Option<&str>,
    tx: &SyncTx,
    watches: &WatchRegistry,
    state: &ServerState,
    resume: &mut ReplResume,
) -> std::io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // 0. Send auth password if configured
    if let Some(pwd) = repl_password {
        let msg = format!("{}\n", pwd);
        socket.write_all(msg.as_bytes()).await?;
        socket.flush().await?;
        // Read "+OK\n" (4 bytes)
        let mut resp = [0u8; 4];
        socket.read_exact(&mut resp).await?;
        if &resp != b"+OK\n" {
            return Err(std::io::Error::new(
                ErrorKind::PermissionDenied,
                "replication auth rejected by primary",
            ));
        }
    }

    // 1. Negotiate full or partial synchronization.
    let mut magic = [0u8; 4];
    socket.read_exact(&mut magic).await?;
    if &magic != REPL_MAGIC {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "unsupported replication protocol",
        ));
    }
    let mut run_len = [0u8; 2];
    socket.read_exact(&mut run_len).await?;
    let run_len = u16::from_le_bytes(run_len) as usize;
    if run_len > 128 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "primary replication run id is too long",
        ));
    }
    let mut primary_run_id = vec![0u8; run_len];
    socket.read_exact(&mut primary_run_id).await?;
    let primary_advertised = String::from_utf8(primary_run_id)
        .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "invalid primary run id"))?;
    let advertised = HandshakeId::parse(&primary_advertised);
    let primary_run_id = advertised.run_id.to_string();
    // Accept DEDUP replication whenever the primary offers it, whatever this
    // node's own setting: that governs what it sends its own replicas.
    let with_dedup = advertised.has("caps", REPL_CAP_DEDUP);

    let resume_id = if resume.run_id == primary_run_id {
        resume.run_id.as_str()
    } else {
        ""
    };
    let requested = if with_dedup {
        format!("{resume_id};accept={REPL_CAP_DEDUP}")
    } else {
        resume_id.to_string()
    };
    socket
        .write_all(&(requested.len() as u16).to_le_bytes())
        .await?;
    socket.write_all(requested.as_bytes()).await?;
    socket.write_all(&resume.offset.to_le_bytes()).await?;
    socket.flush().await?;

    let mut mode = [0u8; 1];
    socket.read_exact(&mut mode).await?;
    if mode[0] == SYNC_MODE_REFUSED {
        let mut len = [0u8; 2];
        socket.read_exact(&mut len).await?;
        let mut reason = vec![0u8; usize::from(u16::from_le_bytes(len))];
        socket.read_exact(&mut reason).await?;
        return Err(std::io::Error::new(
            ErrorKind::Unsupported,
            format!(
                "primary refused this replica: {}",
                String::from_utf8_lossy(&reason)
            ),
        ));
    }
    if !with_dedup {
        warn!(
            "Replica: the primary does not replicate DEDUP state (it predates 0.6, or has \
             RECACHED_REPL_DEDUP=off). If this replica is promoted, a client's retried write may run \
             a second time."
        );
    }
    state
        .replicas
        .upstream_dedup
        .store(with_dedup, Ordering::Release);
    let mut boundary = [0u8; 8];
    socket.read_exact(&mut boundary).await?;
    let boundary = u64::from_le_bytes(boundary);

    if mode[0] == b'F' || (mode[0] == SYNC_MODE_FULL_DEDUP && with_dedup) {
        // Receive and atomically install a full snapshot.
        let snap_bytes = read_sized_payload(socket, "snapshot").await?;
        let entries = rmp_serde::from_slice::<Vec<SnapshotEntry>>(&snap_bytes)
            .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e.to_string()))?;
        // Read the DEDUP table before installing anything, so a link that
        // drops between the two leaves this replica's old state whole rather
        // than new data with old marks.
        let table = if mode[0] == SYNC_MODE_FULL_DEDUP {
            let bytes = read_sized_payload(socket, "DEDUP table").await?;
            Some(
                rmp_serde::from_slice::<Vec<DedupTableEntry>>(&bytes)
                    .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e.to_string()))?,
            )
        } else {
            None
        };
        let count = entries.len();
        let clients = {
            // Behind the all-write barrier, as a QSUB snapshot is: a live
            // query registered on this replica must not snapshot a keyspace
            // that is half replaced (the prefix plan would read the fresh,
            // empty index installed at the start of `replace`).
            let _barrier = state.replicas.lock_all_writes().await;
            store.replace(entries);
            // The DEDUP table with the data, under the same barrier (6N-C
            // PR 0d): installed after it, a sub-replica's cut (or a QSUB, or
            // a WebSocket replay on this node) could pair the new keyspace
            // with the old marks, and nothing would stream the new table to
            // that sub-replica afterwards.
            let clients = table.map(|table| {
                let clients = table.len();
                state.replace_dedup_table(table);
                clients
            });
            // Every client of this replica holds a copy, and live-query
            // snapshots, of the keyspace just replaced; keys deleted on the
            // primary while the link was down are simply gone, with no DEL
            // to say so. Close them all with the sync-gap code so they
            // rebuild from the new keyspace, and abort `WATCH`ed `EXEC`s.
            // Still behind the barrier, so no write or QSUB snapshot falls
            // between the replacement and the gap.
            let invalidated = tx.invalidate_all();
            notify_keyspace_replaced(watches, store).await;
            if invalidated > 0 {
                info!(
                    "Replica: full resynchronisation closed {invalidated} WebSocket connection(s) \
                     so they resynchronise"
                );
            }
            counter!("recached_replica_full_resync_total").increment(1);
            clients
        };
        match clients {
            Some(clients) => {
                info!("Replica: full snapshot loaded ({count} entries, {clients} DEDUP client(s))");
            }
            None => info!("Replica: full snapshot loaded ({} entries)", count),
        }
    } else if mode[0] == b'P' {
        let mut count = [0u8; 4];
        socket.read_exact(&mut count).await?;
        let count = u32::from_le_bytes(count);
        for _ in 0..count {
            let expected = resume.offset.checked_add(1).ok_or_else(|| {
                std::io::Error::new(ErrorKind::InvalidData, "replication offset overflow")
            })?;
            let (offset, bytes) = read_repl_frame(socket).await?;
            if offset != expected {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("replication offset gap: expected {expected}, received {offset}"),
                ));
            }
            apply_repl_frame(store, state, tx, watches, &bytes).await?;
            resume.offset = offset;
        }
        if resume.offset != boundary {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "partial synchronization ended at {}, expected {boundary}",
                    resume.offset
                ),
            ));
        }
        info!("Replica: partial synchronization reached offset {boundary}");
    } else {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "invalid replication synchronization mode",
        ));
    }

    resume.run_id = primary_run_id;
    resume.offset = boundary;
    socket.write_all(&boundary.to_le_bytes()).await?;
    socket.flush().await?;

    // 2. Stream write commands from primary, acknowledging applied offsets.
    //
    // Read through a buffer: each frame used to take three `read_exact`
    // calls on the bare socket, plus an 8-byte acknowledgement write — four
    // syscalls per replicated write, which capped how fast a replica could
    // follow a pipelined primary. Nothing was read from the socket before
    // this point beyond what the handshake consumed exactly, so no bytes are
    // stranded by wrapping it here.
    let (rd, mut wr) = tokio::io::split(socket);
    let mut rd = tokio::io::BufReader::with_capacity(REPL_RECV_BUFFER_BYTES, rd);
    loop {
        let (offset, cmd_bytes) = read_repl_frame(&mut rd).await?;
        let expected = resume.offset.saturating_add(1);
        if offset != expected {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!("replication offset gap: expected {expected}, received {offset}"),
            ));
        }
        apply_repl_frame(store, state, tx, watches, &cmd_bytes).await?;
        resume.offset = offset;

        // The acknowledgement is cumulative (the primary keeps the maximum),
        // so one per burst says as much as one per frame: acknowledge when
        // the buffer holds no further frame, i.e. before waiting on the
        // socket again. Lag is then reported at most one burst late.
        if rd.buffer().is_empty() {
            wr.write_all(&offset.to_le_bytes()).await?;
            wr.flush().await?;
        }
    }
}

/// A `u32`-length-prefixed payload of at most [`MAX_REPL_FRAME_BYTES`].
async fn read_sized_payload<S>(socket: &mut S, what: &str) -> std::io::Result<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut len_buf = [0u8; 4];
    socket.read_exact(&mut len_buf).await?;
    let len = usize::try_from(u32::from_le_bytes(len_buf)).unwrap_or(usize::MAX);
    if len > MAX_REPL_FRAME_BYTES {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("{what} frame too large ({len} > {MAX_REPL_FRAME_BYTES} bytes)"),
        ));
    }
    let mut bytes = vec![0u8; len];
    socket.read_exact(&mut bytes).await?;
    Ok(bytes)
}

async fn read_repl_frame<S>(socket: &mut S) -> std::io::Result<(u64, Vec<u8>)>
where
    S: AsyncRead + Unpin,
{
    let mut offset = [0u8; 8];
    socket.read_exact(&mut offset).await?;
    let mut len_buf = [0u8; 4];
    socket.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_REPL_FRAME_BYTES {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("command frame too large ({len} > {MAX_REPL_FRAME_BYTES} bytes)"),
        ));
    }
    let mut bytes = vec![0u8; len];
    socket.read_exact(&mut bytes).await?;
    Ok((u64::from_le_bytes(offset), bytes))
}

/// Refuse a replicated write once this node has been promoted.
///
/// Called with the frame's write-order stripes held, which promotion's
/// all-write barrier excludes (`ServerState::promote_to_primary`): a frame
/// either applies before the promotion, as a follower's, or sees it here and
/// ends the link. Applied after it, a frame would run with the caps on and
/// its evictions would reach none of this node's consumers (6L review, R2).
fn still_following(state: &ServerState) -> std::io::Result<()> {
    if state.is_replica() {
        Ok(())
    } else {
        Err(std::io::Error::other(
            "promoted with REPLICAOF NO ONE: replication link closed",
        ))
    }
}

/// Apply one replicated write on a replica, with the effects a write on the
/// primary has on this node's own clients.
///
/// The write runs under its write-order stripes, as a client write does, and
/// WATCH and live-query (QSUB) subscribers are notified before they are
/// released. Without either, a live query held against a replica was told
/// nothing after its snapshot: the replicated write never reached the watch
/// registry, and the sync push that did reach the connection was dropped as
/// covered by its live query (`forwards_push`), in expectation of a keychange
/// that never came. And without the stripes, a QSUB's all-write barrier did
/// not exclude replicated writes, so one could land between the snapshot and
/// the registration and be missed by both.
async fn apply_repl_frame(
    store: &KeyValueStore,
    state: &ServerState,
    tx: &SyncTx,
    watches: &WatchRegistry,
    bytes: &[u8],
) -> std::io::Result<()> {
    let (value, consumed) = Value::parse(bytes)
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e.to_string()))?;
    if consumed != bytes.len() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "replication frame contains trailing bytes",
        ));
    }
    // DEDUP identities, when the primary streams them: the AOF's own record
    // formats, applied the way AOF replay applies them, and passed on the way
    // a primary passes on its own writes.
    //
    // A record that changes no data still holds a real ordering stripe —
    // the client id's — while it checks the link and applies (6N-C PR 0d,
    // RV-7). Applied with none, as before, it could commit and fan out
    // between a sub-replica's cut (`handle_replica`'s all-write barrier,
    // which reads the offset and the DEDUP table) and that sub-replica's
    // registration, and reach it through neither; and it was applied after
    // `REPLICAOF NO ONE`. `lock_commands(&[])` would take no stripe at all
    // (6N-B round 2, NEW-3), so the client id is passed as an explicit key.
    //
    // Logged before it is remembered, as on the primary: this node's
    // WebSocket clients may read the outcome (early replay), and must not
    // before it is in this node's AOF.
    match dedup_record(&value) {
        Some(DedupRecord::Refused(client, id, error)) => {
            let _order = state
                .replicas
                .lock_commands_and_keys(&[], std::iter::once(client.as_str()))
                .await;
            still_following(state)?;
            state.log_dedup_outcome(&client, id, Some(&error)).await;
            state.remember_dedup_refusal(&client, id, &error);
            return Ok(());
        }
        Some(DedupRecord::Mark(client, id)) if is_dedup_mark_only(&value) => {
            let _order = state
                .replicas
                .lock_commands_and_keys(&[], std::iter::once(client.as_str()))
                .await;
            still_following(state)?;
            state.log_dedup_outcome(&client, id, None).await;
            state.commit_dedup(&client, id);
            return Ok(());
        }
        Some(DedupRecord::Mark(client, id)) => {
            return apply_dedup_envelope(store, state, tx, watches, value, &client, id).await;
        }
        None => {}
    }
    let normalised = match value {
        Value::Push(inner) => Value::Array(Some(inner)),
        other => other,
    };
    let cmd = Command::from_value(normalised)
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))?;
    let _order = state
        .replicas
        .lock_commands(std::slice::from_ref(&cmd))
        .await;
    still_following(state)?;
    if let (Value::Error(error), _) = store.execute_reporting_ref(&cmd) {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("replicated command failed: {error}"),
        ));
    }
    // Logged first, then announced, as a write on the primary is: a client
    // is told of a write only once it is in this node's AOF and replication
    // stream.
    state.on_write(bytes).await;
    notify_write(watches, &cmd, store).await;
    // Built only when a browser is a candidate for it: the push copies the
    // frame, which every replicated write paid for even with no WebSocket
    // client. Checked after the write, so a client routed before it is
    // pushed it and one routed later reads it from the store.
    if tx.would_route(&cmd) {
        tx.send(Arc::new(SyncPush::for_command(
            0,
            &cmd,
            push_keys(&cmd),
            bytes.to_vec(),
        )));
    }
    Ok(())
}

// ── security helpers ─────────────────────────────────────────────────────────

/// Constant-time byte slice equality to prevent timing-based password leaks.
/// Apply a replicated `DEDUP <client> <id> <write…>` envelope: run the write,
/// then commit its mark, as the primary did.
///
/// Browser peers of this replica are pushed the bare write — the envelope
/// names another client's id, which is neither theirs to see nor a frame
/// they understand. The AOF and any replicas of this node get it through
/// `on_write_identified`, wrapped or not by this node's own setting.
async fn apply_dedup_envelope(
    store: &KeyValueStore,
    state: &ServerState,
    tx: &SyncTx,
    watches: &WatchRegistry,
    value: Value,
    client: &str,
    id: u64,
) -> std::io::Result<()> {
    let items = match value {
        Value::Push(items) | Value::Array(Some(items)) => items,
        _ => {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "DEDUP envelope is not an array",
            ));
        }
    };
    let inner: Vec<&[u8]> = items
        .iter()
        .skip(3)
        .map(|item| match item {
            Value::BulkString(Some(bytes)) => Ok(bytes.as_slice()),
            _ => Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "DEDUP envelope holds a non-bulk element",
            )),
        })
        .collect::<std::io::Result<_>>()?;
    let bare = resp_push(&inner);
    let cmd = Command::from_value(Value::Array(Some(items.into_iter().skip(3).collect())))
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))?;
    // As for a bare replicated write (see `apply_repl_frame`).
    let _order = state
        .replicas
        .lock_commands(std::slice::from_ref(&cmd))
        .await;
    still_following(state)?;
    if let (Value::Error(error), _) = store.execute_reporting_ref(&cmd) {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("replicated command failed: {error}"),
        ));
    }
    // Logged, then the mark committed (RV-14: before, the mark was readable
    // before the frame was appended), then announced, then pushed, as for a
    // bare replicated write.
    state.on_write_identified(&bare, Some((client, id))).await;
    state.commit_dedup(client, id);
    notify_write(watches, &cmd, store).await;
    if tx.would_route(&cmd) {
        tx.send(Arc::new(SyncPush::for_command(
            0,
            &cmd,
            push_keys(&cmd),
            bare,
        )));
    }
    Ok(())
}

pub(crate) fn ct_eq_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

// ── sync scoping ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod write_order_tests {
    use super::*;
    use core_engine::cmd::SetOptions;

    fn set(key: &str) -> Command {
        Command::Set(key.to_string(), b"value".to_vec(), SetOptions::default())
    }

    #[tokio::test]
    async fn conflicting_writes_share_one_ordering_domain() {
        let hub = ReplHub::new();
        let first = hub.lock_commands(&[set("same-key")]).await;
        let contender = Arc::clone(&hub);
        let (entered_tx, mut entered_rx) = mpsc::channel(1);
        let task = tokio::spawn(async move {
            let _second = contender.lock_commands(&[set("same-key")]).await;
            entered_tx.send(()).await.expect("receiver alive");
        });

        assert!(
            tokio::time::timeout(Duration::from_millis(25), entered_rx.recv())
                .await
                .is_err(),
            "a conflicting write entered before the first propagated"
        );
        drop(first);
        tokio::time::timeout(Duration::from_secs(1), entered_rx.recv())
            .await
            .expect("contender should be released")
            .expect("contender should signal");
        task.await.expect("contender task panicked");
    }

    #[tokio::test]
    async fn independent_keys_remain_concurrent() {
        let hub = ReplHub::new();
        let _first = hub.lock_commands(&[set("one")]).await;
        let contender = Arc::clone(&hub);
        tokio::time::timeout(Duration::from_secs(1), async move {
            let _second = contender.lock_commands(&[set("two")]).await;
        })
        .await
        .expect("independent keys should not block each other");
    }

    #[tokio::test]
    async fn full_sync_barrier_conflicts_with_every_write() {
        let hub = ReplHub::new();
        let barrier = hub.lock_all_writes().await;
        let contender = Arc::clone(&hub);
        let task = tokio::spawn(async move {
            let _write = contender.lock_commands(&[set("any-key")]).await;
        });
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert!(!task.is_finished(), "write crossed the full-sync barrier");
        drop(barrier);
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("write should resume after snapshot capture")
            .expect("write task panicked");
    }

    #[tokio::test]
    async fn backlog_only_resumes_when_every_missing_offset_is_retained() {
        let hub = ReplHub::new();
        hub.configure_backlog(5);
        hub.fan_out(vec![1, 2, 3]).await;
        hub.fan_out(vec![4, 5, 6]).await;

        assert!(
            hub.resume_frames(&hub.run_id, 0).await.is_none(),
            "offset one was evicted, so offset zero cannot resume"
        );
        let frames = hub
            .resume_frames(&hub.run_id, 1)
            .await
            .expect("offset one should resume from retained offset two");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].offset, 2);
    }

    /// `AckReader` alone: whole, split, several-at-once and trailing partial
    /// acknowledgements, and the buffer never fills.
    #[test]
    fn ack_reader_keeps_partial_acknowledgements() {
        fn feed(acks: &mut AckReader, bytes: &[u8]) -> Option<u64> {
            let spare = acks.spare();
            assert!(spare.len() >= bytes.len(), "the buffer filled");
            spare[..bytes.len()].copy_from_slice(bytes);
            acks.take(bytes.len())
        }
        let mut acks = AckReader::default();
        let seven = 7u64.to_le_bytes();
        assert_eq!(feed(&mut acks, &seven), Some(7));
        assert_eq!(acks.filled, 0);
        // Split three ways.
        let big = (u64::MAX - 1).to_le_bytes();
        assert_eq!(feed(&mut acks, &big[..1]), None);
        assert_eq!(feed(&mut acks, &big[1..6]), None);
        assert_eq!(feed(&mut acks, &big[6..]), Some(u64::MAX - 1));
        // Several at once, out of order, with half of another trailing.
        let mut burst = Vec::new();
        for ack in [30u64, 10, 20] {
            burst.extend_from_slice(&ack.to_le_bytes());
        }
        let next = 40u64.to_le_bytes();
        burst.extend_from_slice(&next[..4]);
        assert_eq!(feed(&mut acks, &burst), Some(30));
        assert_eq!(acks.filled, 4);
        assert_eq!(feed(&mut acks, &next[4..]), Some(40));
        // A full buffer's worth in one read, ending on a boundary.
        let mut full = Vec::new();
        for ack in 1u64..=(ACK_READ_BYTES / ACK_BYTES) as u64 {
            full.extend_from_slice(&ack.to_le_bytes());
        }
        assert_eq!(feed(&mut acks, &full), Some(8));
        assert_eq!(acks.filled, 0);
        // At most seven bytes are ever kept, so a read always has room.
        for n in 1..ACK_BYTES {
            let mut acks = AckReader::default();
            assert_eq!(
                feed(&mut acks, &vec![0xff; ACK_READ_BYTES - ACK_BYTES + n]),
                Some(u64::MAX)
            );
            assert_eq!(acks.filled, n);
            assert!(acks.spare().len() >= ACK_READ_BYTES - ACK_BYTES);
        }
    }

    #[tokio::test]
    async fn replica_queue_enforces_bytes_even_below_its_item_limit() {
        let hub = ReplHub::new();
        hub.configure_backlog(1024);
        hub.configure_queue_bytes(3);
        let (tx, _rx) = mpsc::channel(16);
        hub.stream().senders.push(ReplicaHandle {
            tx,
            sent: Arc::new(AtomicU64::new(0)),
            acked: Arc::new(AtomicU64::new(0)),
            pending_bytes: Arc::new(AtomicUsize::new(0)),
            max_queue_bytes: 3,
            _link: tokio::sync::oneshot::channel().0,
        });
        hub.count.store(1, Ordering::Relaxed);

        hub.fan_out(vec![0; 4]).await;
        assert_eq!(hub.count.load(Ordering::Relaxed), 0);
    }

    async fn drain_frames(
        rx: &mut mpsc::Receiver<QueuedReplFrame>,
        want: usize,
    ) -> Vec<(u64, Vec<u8>)> {
        let mut got = Vec::with_capacity(want);
        while got.len() < want {
            match tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await {
                Ok(Some(q)) => got.push((q.frame.offset, q.frame.bytes.to_vec())),
                _ => break,
            }
        }
        got
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_fan_out_reaches_two_replicas_in_offset_order() {
        // 6L-D R1: one lock now orders the offset, the backlog and every
        // replica queue. Eight concurrent writers; each replica must receive
        // offsets 1..=n contiguously, the same payload per offset, and the
        // backlog must hold them in the same order.
        const WRITERS: usize = 8;
        const EACH: usize = 500;
        let hub = ReplHub::new();
        hub.configure_backlog(16 << 20);
        let mut a = hub.attach_test_replica(1 << 14, usize::MAX >> 1).await;
        let mut b = hub.attach_test_replica(1 << 14, usize::MAX >> 1).await;
        let writers: Vec<_> = (0..WRITERS)
            .map(|w| {
                let hub = Arc::clone(&hub);
                tokio::spawn(async move {
                    for i in 0..EACH {
                        hub.fan_out(format!("w{w}:{i}").into_bytes()).await;
                    }
                })
            })
            .collect();
        for w in writers {
            w.await.unwrap();
        }
        let n = WRITERS * EACH;
        let (got_a, got_b) = (drain_frames(&mut a, n).await, drain_frames(&mut b, n).await);
        assert_eq!(got_a.len(), n);
        for (i, (offset, _)) in got_a.iter().enumerate() {
            assert_eq!(*offset, i as u64 + 1, "replica A: offsets out of order");
        }
        assert_eq!(got_a, got_b, "both replicas see the same stream");
        let backlog: Vec<(u64, Vec<u8>)> = hub
            .stream()
            .backlog
            .frames
            .iter()
            .map(|f| (f.offset, f.bytes.to_vec()))
            .collect();
        assert_eq!(backlog, got_a, "the backlog holds the stream as sent");
        // Each writer's frames keep their own order within the stream.
        for w in 0..WRITERS {
            let mine: Vec<&Vec<u8>> = got_a
                .iter()
                .map(|(_, b)| b)
                .filter(|b| b.starts_with(format!("w{w}:").as_bytes()))
                .collect();
            assert_eq!(mine.len(), EACH);
            for (i, b) in mine.iter().enumerate() {
                assert_eq!(&b[..], format!("w{w}:{i}").as_bytes());
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_slow_replica_is_dropped_without_stalling_the_others() {
        // Backpressure: a replica that stops reading is disconnected once its
        // queue is full; the healthy one keeps every frame, in order, and the
        // writers never block on the slow one.
        let hub = ReplHub::new();
        hub.configure_backlog(16 << 20);
        let _slow = hub.attach_test_replica(8, usize::MAX >> 1).await;
        let mut healthy = hub.attach_test_replica(1 << 14, usize::MAX >> 1).await;
        assert_eq!(hub.count.load(Ordering::Relaxed), 2);
        let started = std::time::Instant::now();
        for i in 0..2_000 {
            hub.fan_out(format!("f{i}").into_bytes()).await;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "writers stalled"
        );
        assert_eq!(
            hub.count.load(Ordering::Relaxed),
            1,
            "the slow replica was dropped"
        );
        let got = drain_frames(&mut healthy, 2_000).await;
        assert_eq!(got.len(), 2_000);
        assert!(got.iter().enumerate().all(|(i, (o, _))| *o == i as u64 + 1));
    }

    #[tokio::test]
    async fn retained_offset_negotiates_a_partial_sync() {
        let store = Arc::new(KeyValueStore::new());
        let hub = ReplHub::new();
        // The 0.5 wire format, which a primary with DEDUP replication off
        // still speaks unchanged.
        hub.configure_dedup(false);
        hub.configure_backlog(1024);
        hub.fan_out(b"one".to_vec()).await;
        hub.fan_out(b"two".to_vec()).await;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_hub = Arc::clone(&hub);
        let server_store = Arc::clone(&store);
        tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let _ = handle_replica(
                socket,
                server_store,
                ServerState::for_tests(
                    Arc::new(SnapshotConfig {
                        path: PathBuf::from("unused.rdb"),
                        last_save: AtomicI64::new(0),
                        checkpoint_id: AtomicU64::new(0),
                    }),
                    Arc::clone(&server_hub),
                    false,
                ),
                server_hub,
                None,
                16,
                IpAddr::from([127, 0, 0, 1]),
                ReplAuthThrottle::new(),
            )
            .await;
        });

        let mut socket = TcpStream::connect(addr).await.unwrap();
        let mut magic = [0u8; 4];
        socket.read_exact(&mut magic).await.unwrap();
        assert_eq!(&magic, REPL_MAGIC);
        let mut run_len = [0u8; 2];
        socket.read_exact(&mut run_len).await.unwrap();
        let mut run_id = vec![0; u16::from_le_bytes(run_len) as usize];
        socket.read_exact(&mut run_id).await.unwrap();
        socket
            .write_all(&(run_id.len() as u16).to_le_bytes())
            .await
            .unwrap();
        socket.write_all(&run_id).await.unwrap();
        socket.write_all(&1u64.to_le_bytes()).await.unwrap();

        let mut mode = [0u8; 1];
        socket.read_exact(&mut mode).await.unwrap();
        assert_eq!(mode[0], b'P');
        let mut boundary = [0u8; 8];
        socket.read_exact(&mut boundary).await.unwrap();
        assert_eq!(u64::from_le_bytes(boundary), 2);
        let mut count = [0u8; 4];
        socket.read_exact(&mut count).await.unwrap();
        assert_eq!(u32::from_le_bytes(count), 1);
        let (offset, bytes) = read_repl_frame(&mut socket).await.unwrap();
        assert_eq!(offset, 2);
        assert_eq!(bytes, b"two");
    }
}

/// 6N-C PR 0d (RV-7, NEW-3, RV-14): replicated DEDUP records that change no
/// data are ordered like writes on a replica, and are refused once it stops
/// following.
#[cfg(test)]
mod dedup_order_tests {
    use super::*;

    struct Replica {
        store: KeyValueStore,
        state: Arc<ServerState>,
        tx: SyncTx,
        watches: WatchRegistry,
    }

    fn replica() -> Replica {
        let hub = ReplHub::new();
        hub.configure_dedup(true);
        // Chained: this replica serves sub-replicas of its own.
        hub.configure_backlog(1 << 20);
        let state = ServerState::for_tests(
            Arc::new(SnapshotConfig {
                path: std::env::temp_dir().join("recached_0d_unused.rdb"),
                last_save: std::sync::atomic::AtomicI64::new(0),
                checkpoint_id: AtomicU64::new(0),
            }),
            hub,
            true,
        );
        let store = KeyValueStore::new();
        store.set_follower(true);
        Replica {
            store,
            state,
            tx: SyncRouter::new(16),
            watches: WatchHub::new(),
        }
    }

    fn knows(state: &ServerState, client: &str, id: u64) -> bool {
        state.dedup_table().into_iter().any(|(c, hwm, refused)| {
            c == client && (hwm.is_some_and(|h| h >= id) || refused.iter().any(|(r, _)| *r == id))
        })
    }

    fn apply(r: &Arc<Replica>, frame: Vec<u8>) -> tokio::task::JoinHandle<std::io::Result<()>> {
        let r = Arc::clone(r);
        tokio::spawn(async move {
            apply_repl_frame(&r.store, &r.state, &r.tx, &r.watches, &frame).await
        })
    }

    /// The RV-7 counterexample, in the order that loses it: under the
    /// all-write barrier a sub-replica's attach reads the DEDUP table, then
    /// registers its stream. A record arriving after the table was read must
    /// not commit and fan out before the registration — it would reach the
    /// sub-replica through neither — so it must wait for the barrier and
    /// arrive in the stream.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_record_racing_a_sub_replica_cut_reaches_the_sub_replica() {
        for (name, frame) in [
            ("mark", dedup_mark_frame("racer-0d", 5)),
            (
                "refusal",
                dedup_refusal_frame("racer-0d", 5, "ERR refused for good"),
            ),
        ] {
            let r = Arc::new(replica());
            let barrier = r.state.replicas.lock_all_writes().await;
            assert!(
                !knows(&r.state, "racer-0d", 5),
                "{name}: in the cut's table"
            );
            let task = apply(&r, frame);
            tokio::time::sleep(Duration::from_millis(50)).await;
            assert!(!task.is_finished(), "{name}: applied under the barrier");
            let mut stream = r
                .state
                .replicas
                .attach_test_replica(64, usize::MAX >> 1)
                .await;
            drop(barrier);
            task.await.unwrap().unwrap();
            let in_stream = tokio::time::timeout(Duration::from_secs(2), stream.recv())
                .await
                .ok()
                .flatten()
                .is_some_and(|q| q.frame.bytes.windows(8).any(|w| w == b"racer-0d"));
            assert!(in_stream, "{name}: reached the sub-replica through neither");
            assert!(
                knows(&r.state, "racer-0d", 5),
                "{name}: applied after the cut"
            );
        }
    }

    /// A record still waiting for its stripe when the node is promoted
    /// (`REPLICAOF NO ONE` flips the role under the all-write barrier) is
    /// refused, which ends the link, and leaves no trace.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_record_pending_across_a_promotion_is_refused() {
        for (name, frame) in [
            ("mark", dedup_mark_frame("late-0d", 9)),
            (
                "refusal",
                dedup_refusal_frame("late-0d", 9, "ERR refused for good"),
            ),
        ] {
            let r = Arc::new(replica());
            let barrier = r.state.replicas.lock_all_writes().await;
            let task = apply(&r, frame);
            tokio::time::sleep(Duration::from_millis(30)).await;
            // What `promote_to_primary` does under its barrier.
            r.state.is_replica.store(false, Ordering::Relaxed);
            drop(barrier);
            assert!(
                task.await.unwrap().is_err(),
                "{name}: applied after promotion"
            );
            assert!(!knows(&r.state, "late-0d", 9), "{name}: left a trace");
        }
    }

    /// After `REPLICAOF NO ONE` has completed, a record is refused outright.
    #[tokio::test]
    async fn a_record_after_replicaof_no_one_is_refused() {
        let r = Arc::new(replica());
        r.state.promote_to_primary(&r.store).await;
        assert!(
            apply(&r, dedup_mark_frame("after-0d", 1))
                .await
                .unwrap()
                .is_err()
        );
        assert!(
            apply(&r, dedup_refusal_frame("after-0d", 2, "ERR x"))
                .await
                .unwrap()
                .is_err()
        );
        assert!(!knows(&r.state, "after-0d", 1));
        assert!(!knows(&r.state, "after-0d", 2));
    }

    /// The stripe is the client id's own: a record for one client waits for
    /// a write holding that stripe and does not wait for unrelated keys.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_record_takes_its_clients_stripe_only() {
        let r = Arc::new(replica());
        let held = r
            .state
            .replicas
            .lock_commands_and_keys(&[], std::iter::once("striped-0d"))
            .await;
        let task = apply(&r, dedup_mark_frame("striped-0d", 1));
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(!task.is_finished(), "took no stripe (NEW-3)");
        drop(held);
        task.await.unwrap().unwrap();
        // An unrelated stripe does not block it.
        let unrelated = loop {
            let key = format!("unrelated-{}", next_conn_id());
            if ReplHub::stripe_for(&key) != ReplHub::stripe_for("striped-0d") {
                break key;
            }
        };
        let _other = r
            .state
            .replicas
            .lock_commands_and_keys(&[], std::iter::once(unrelated.as_str()))
            .await;
        tokio::time::timeout(
            Duration::from_secs(2),
            apply(&r, dedup_mark_frame("striped-0d", 2)),
        )
        .await
        .expect("blocked by an unrelated stripe")
        .unwrap()
        .unwrap();
        assert!(knows(&r.state, "striped-0d", 2));
    }
}
