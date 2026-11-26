//! Two-phase `EXEC` (6O-G): a transaction's writes become visible to readers
//! outside it all at once.
//!
//! Design: `benchmarks/v0.6.0/6o-f/V0.6O-F-TRANSACTION-ATOMICITY-DESIGN.md`,
//! Option E revision 5. The connection holds the transaction's guards
//! throughout (phase 0, `ReplHub::lock_transaction`: the unordered gate
//! closed, and the replication-order stripes of every key it reads, writes
//! or watches), then:
//!
//! 1. **Apply**, synchronously ([`KeyValueStore::apply_transaction`]): every
//!    queued command's store call, in order, at the transaction's instant,
//!    with the store stripes of its writes held and marked from its first
//!    write to its last. Everything else a write needs that is not I/O runs
//!    here too, inside that hold: the claimable-members check, the
//!    deadline read-back, the frame, the removal kind, eviction's
//!    forgetting of claims and the ephemeral reconciliation (design R6).
//! 2. **Effects**, asynchronously, in command order: the AOF (one batch),
//!    replication, WATCH/QSUB notifications coalesced per key, WebSocket
//!    pushes, eviction `DEL`s after their command, `PUBLISH` delivery.
//! 3. **Release**: the caller unregisters the watches, drops the guards and
//!    writes the reply, still gated on durability.
//!
//! No store stripe is held across an `.await`: a slow `fdatasync` in phase 2
//! holds back writers of the transaction's keys (server stripes), never a
//! reader.

use crate::*;

/// What phase 1 recorded about one queued write, for phase 2.
pub(crate) struct AppliedWrite<'q> {
    /// Position in the queue (and in the reply).
    pub(crate) index: usize,
    pub(crate) cmd: &'q Command,
    /// [`broadcast_for_at`]'s frame: `Some(None)` when the write changed
    /// nothing. `None` while not built: phase 1 builds it only when
    /// something observed writes as the transaction began, and phase 2 then
    /// builds any it needs from the same inputs.
    pub(crate) frame: Option<Option<Vec<u8>>>,
    /// The deadline a relative expiry set, read back inside phase 1.
    pub(crate) exact_deadline: Option<u64>,
    /// Whether, if it changed anything, the write removed its keys (`DEL`,
    /// `UNLINK`, a `*STORE` that left its destination absent): decided in
    /// phase 1, at the write, not by a read in phase 2.
    pub(crate) removes: bool,
    /// Keys capacity eviction removed for it.
    pub(crate) evicted: Vec<String>,
    /// Keys it found expired and removed before it ran (6O-H): logged as a
    /// `DEL` before its frame, whether or not it was effective.
    pub(crate) expired: Vec<String>,
}

impl AppliedWrite<'_> {
    /// Whether the write is *effective* (design Q1): it produced a frame.
    fn effective(&self) -> bool {
        matches!(self.frame, Some(Some(_)))
    }
}

/// Phase 1 of one queued write: everything synchronous that
/// `execute_locked_write_identified` and `apply_write_effects_identified`
/// do for it, in the same order, with the I/O left for phase 2. Runs inside
/// [`KeyValueStore::apply_transaction`]'s scope, so its reads (the
/// claimable check, the deadline read-back, the `*STORE` check) see the
/// transaction's own earlier writes and wait on no stripe.
pub(crate) fn apply_queued_write<'q>(
    index: usize,
    cmd: &'q Command,
    now: u64,
    origin: u64,
    state: &ServerState,
    store: &KeyValueStore,
    build_frame: bool,
) -> (Value, AppliedWrite<'q>) {
    let claimable_members = claimable_eadd_members(cmd, state, store);
    // Observed exactly when the frames are built here: when something
    // observes writes (6O-H). At the transaction's instant, which
    // `apply_transaction` holds.
    let ((response, evicted), expired) =
        observing_if(build_frame, || execute_and_record_ref(store, cmd));
    forget_expired(state, &expired);
    let expired = expiries_to_log(cmd, &response, expired);
    // Read back only when the frame is built here (review B-4). A frame
    // built in phase 2 instead falls back to `now` plus the relative expiry:
    // the very deadline the store set, since it ran with its clock at `now`.
    let exact_deadline = build_frame
        .then(|| exact_deadline_of(cmd, store, Some(now)))
        .flatten();
    let frame = build_frame.then(|| broadcast_for_at(cmd, &response, now, exact_deadline));
    let removes = removes_named_keys(cmd) || stored_nothing(cmd, store);
    // As `apply_eviction_effects` and then `reconcile_ephemeral_state` do,
    // in that order, for the ordinary write path.
    for key in &evicted {
        state.forget_ephemeral_keys(std::iter::once(key.as_str()));
    }
    reconcile_ephemeral_state(cmd, &response, &claimable_members, origin, state);
    (
        response,
        AppliedWrite {
            index,
            cmd,
            frame,
            exact_deadline,
            removes,
            evicted,
            expired,
        },
    )
}

/// The keys `cmd` writes, each once, in the order it names them. Linear in
/// the keys: a command may name a million (`DEL`, `MSET`), and a pairwise
/// check made one wide command take minutes of a worker (review B-1).
fn distinct_keys(cmd: &Command) -> KeyRefs<'_> {
    let mut keys = primary_key_refs(cmd);
    if keys.len() <= 8 {
        let mut kept = 0;
        for i in 0..keys.len() {
            if !keys[..kept].contains(&keys[i]) {
                keys.swap(kept, i);
                kept += 1;
            }
        }
        keys.truncate(kept);
    } else {
        let mut seen = HashSet::with_capacity(keys.len());
        keys.retain(|key| seen.insert(*key));
    }
    keys
}

/// How a queued write changed a key, in the order its effects are emitted:
/// the expired entry it removed first (6O-H), its own write, then the
/// evictions it caused.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
enum Change {
    Expiry,
    #[default]
    Write,
    Victim,
}

/// Each written key's effective writes in a transaction, for coalescing its
/// WATCH/QSUB notification (design N1, P1, Q1).
#[derive(Default, Clone, Copy)]
struct KeyWrites {
    count: u32,
    /// The write it was last changed by (its position among the queue's
    /// writes), and how.
    last: (usize, Change),
    /// Whether any of them set the key (was not a removal).
    set: bool,
}

/// Where each key's one notification goes.
#[derive(Default)]
struct NotificationPlan {
    keys: HashMap<String, KeyWrites>,
    /// For each effective `FLUSHDB` (by write position): the keys set before
    /// it in the transaction. Those keys are notified by the flush, not by
    /// their earlier writes.
    flushes: HashMap<usize, HashSet<String>>,
}

impl NotificationPlan {
    fn new(writes: &[AppliedWrite<'_>]) -> Self {
        let mut plan = NotificationPlan::default();
        for (at, write) in writes.iter().enumerate() {
            // An expired entry the write removed is a removal of its key,
            // before the write's own change to it (6O-H).
            for key in &write.expired {
                let entry = plan.keys.entry(key.clone()).or_default();
                entry.count = entry.count.saturating_add(1);
                entry.last = (at, Change::Expiry);
            }
            if write.effective() {
                if matches!(write.cmd, Command::FlushDb) {
                    let set = plan
                        .keys
                        .drain()
                        .filter(|(_, writes)| writes.set)
                        .map(|(key, _)| key)
                        .collect();
                    plan.flushes.insert(at, set);
                } else {
                    for key in distinct_keys(write.cmd) {
                        let entry = plan.keys.entry(key.to_string()).or_default();
                        entry.count = entry.count.saturating_add(1);
                        entry.last = (at, Change::Write);
                        entry.set |= !write.removes;
                    }
                }
            }
            // A victim is removed whether or not its write took effect.
            for victim in &write.evicted {
                let entry = plan.keys.entry(victim.clone()).or_default();
                entry.count = entry.count.saturating_add(1);
                entry.last = (at, Change::Victim);
            }
        }
        plan
    }

    /// The changes write `at` announces: each of its keys whose last
    /// effective write it is. Written once in the transaction, a key is
    /// announced as an ordinary write announces it (a delta where it has
    /// one). Written more than once, it is announced once, at its last
    /// write, as its full value — a delta of the last write alone would land
    /// on a client copy that never saw the earlier ones — or as a removal
    /// that still invalidates a WATCH if an earlier write set it.
    fn changes<'s>(
        &self,
        at: usize,
        write: &AppliedWrite<'_>,
        store: &'s KeyValueStore,
    ) -> Vec<KeyChange<'s>> {
        let mut changes = Vec::new();
        if !write.effective() {
            return changes;
        }
        let delta = delta_for(write.cmd);
        for key in distinct_keys(write.cmd) {
            let Some(writes) = self.keys.get(key) else {
                continue;
            };
            if writes.last != (at, Change::Write) {
                continue;
            }
            changes.push(match (writes.count, write.removes) {
                (1, true) => KeyChange::removed(key.to_string()),
                (1, false) => {
                    let delta = delta
                        .as_ref()
                        .filter(|(delta_key, _)| delta_key == key)
                        .map(|(_, delta)| delta.clone());
                    KeyChange::current(key.to_string(), delta, store)
                }
                (_, true) if writes.set => KeyChange::removed_after_set(key.to_string()),
                (_, true) => KeyChange::removed(key.to_string()),
                (_, false) => KeyChange::current(key.to_string(), None, store),
            });
        }
        changes
    }

    /// The removal write `at`'s eviction of `victim` announces, if it is the
    /// victim's last change in the transaction.
    fn eviction(&self, at: usize, victim: &str) -> Option<KeyChange<'static>> {
        self.removal(at, victim, Change::Victim)
    }

    /// The removal write `at`'s expired entry `key` announces (6O-H), if
    /// nothing later in the transaction changed the key again: otherwise
    /// that later change announces it, as its full value.
    fn expiry(&self, at: usize, key: &str) -> Option<KeyChange<'static>> {
        self.removal(at, key, Change::Expiry)
    }

    fn removal(&self, at: usize, key: &str, by: Change) -> Option<KeyChange<'static>> {
        let writes = self.keys.get(key)?;
        (writes.last == (at, by)).then(|| {
            if writes.set {
                KeyChange::removed_after_set(key.to_string())
            } else {
                KeyChange::removed(key.to_string())
            }
        })
    }
}

/// The expired entries a queued write removed before it ran, as phase 2
/// propagates them: one `DEL` (6O-H).
struct Expiry {
    /// Whether some sync client was routed it when phase 2 began.
    routed: bool,
    deletion: Command,
    frame: Vec<u8>,
}

/// One eviction a queued write caused, as phase 2 propagates it: a `DEL`.
struct Eviction {
    victim: String,
    /// Whether some sync client was routed it when phase 2 began.
    routed: bool,
    deletion: Command,
    frame: Vec<u8>,
}

/// The reply of a queued command that did not run because an earlier one
/// failed internally (6O-G panic containment).
const NOT_RUN: &str = "ERR command not executed: an earlier command of the transaction failed";
/// The reply of the queued write that failed internally.
const FAILED: &str =
    "ERR internal error executing the command; writes are disabled until SAVE succeeds";
/// The reply of a queued read that failed internally: nothing changed, so
/// nothing fails closed (review N-1).
const FAILED_READ: &str = "ERR internal error executing the command";

/// What a transaction's run asks of the connection that ran it.
pub(crate) struct TransactionRun {
    /// `EXEC`'s reply, one item per queued command.
    pub(crate) replies: Vec<Value>,
    /// A queued write panicked part-way: close every WebSocket sync client
    /// with the sync-gap code ([`resynchronise_clients`]) — right away,
    /// whether or not `EXEC`'s reply can be written (reviews C-1, C-2), but
    /// after the guards are released. A WebSocket connection running the
    /// transaction still sends its reply first: the gap closes a connection
    /// at its next poll of its stream.
    pub(crate) resynchronise_clients: bool,
}

/// Close every WebSocket sync client with the sync-gap code, so each
/// rebuilds its copy from the keyspace as it now is (6O-G convergence).
pub(crate) fn resynchronise_clients(tx: &SyncTx) {
    let clients = tx.invalidate_all();
    warn!("after a part-applied write: closed {clients} WebSocket connection(s) to resynchronise");
}

/// Run a transaction's queued commands under the guards `EXEC` holds
/// ([`ReplHub::lock_transaction`]), every one at `now`, the instant the
/// transaction runs at. Shared by the TCP and WebSocket `EXEC`, so the two
/// cannot drift apart. See the module documentation for the phases.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_transaction_queue(
    queue: Vec<Command>,
    now: u64,
    tx: &SyncTx,
    conn_id: u64,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
    pubsub: &SharedPubSub,
) -> TransactionRun {
    // Whether anything observed writes as the transaction began: then the
    // frames are built inside phase 1 with the rest of each write's work.
    // Something registered since builds the rest in phase 2.
    let observed = state.needs_write_log()
        || !watch_registry.is_empty()
        || state.replicas.unordered.has_push_receivers();

    // ── Phase 1: apply ──────────────────────────────────────────────────
    let mut replies: Vec<Value> = Vec::with_capacity(queue.len());
    let mut writes: Vec<AppliedWrite<'_>> = Vec::new();
    let applied = store.apply_transaction(&queue, now, |index, cmd| {
        let reply = match cmd {
            // Delivered in phase 2 (it awaits the pub/sub lock); answered
            // there. `store.execute(Publish)` is a stub that sends nothing.
            Command::Publish(..) => Value::Integer(0),
            _ if is_write_command(cmd) => {
                let (reply, write) =
                    apply_queued_write(index, cmd, now, conn_id, state, store, observed);
                writes.push(write);
                reply
            }
            _ => execute_and_record_ref(store, cmd).0,
        };
        replies.push(reply);
    });
    let ran = applied.panicked.unwrap_or(queue.len());
    if let Some(failed) = applied.panicked {
        // What the panicking command recorded before it unwound is dropped:
        // it has no reply and no frame. The rest never ran.
        replies.truncate(failed);
        writes.retain(|write| write.index < failed);
        replies.push(Value::Error(
            if applied.diverged {
                FAILED
            } else {
                FAILED_READ
            }
            .to_string(),
        ));
        replies.extend((failed + 1..queue.len()).map(|_| Value::Error(NOT_RUN.to_string())));
        fail_closed(state, &queue[failed], applied);
    }
    // ── Phase 2: effects, in command order ──────────────────────────────
    let needs_log = state.needs_write_log();
    // The writer's half of the WATCH registration handshake, after every
    // write of the transaction (6O-E `watched_after_write`). A queue with
    // no writes has nothing to announce (review B-4).
    let has_watch = !writes.is_empty() && watch_registry.watched_after_write();
    // Nothing to log, notify, push or deliver — a read-only queue, or any
    // queue on a server nothing observes — has no phase 2 at all: no route
    // checks, no allocations. A consumer registering after these checks
    // reads the store, as it would after any write (`UnorderedGate`).
    if !needs_log
        && !has_watch
        && !tx.is_active()
        && writes
            .iter()
            .all(|write| write.evicted.is_empty() && write.expired.is_empty())
        && !queue
            .iter()
            .take(ran)
            .any(|cmd| matches!(cmd, Command::Publish(..)))
    {
        #[cfg(test)]
        for cmd in queue.iter().take(ran) {
            crate::propagation::exec_pause::after(cmd).await;
        }
        if let Some(failed) = applied.panicked
            && applied.diverged
        {
            converge(&queue[failed], state, watch_registry, store).await;
        }
        return TransactionRun {
            replies,
            resynchronise_clients: applied.diverged,
        };
    }
    let routed: Vec<bool> = writes
        .iter()
        .map(|write| tx.would_route(write.cmd))
        .collect();
    // Eviction DELs, built once, with their routes decided now, as the
    // writes' are, before the AOF deferral depends on them (review B-2):
    // logged after their command's frame, and pushed after its push.
    let victims: Vec<Vec<Eviction>> = writes
        .iter()
        .map(|write| {
            write
                .evicted
                .iter()
                .filter_map(|victim| {
                    let deletion = Command::Del(vec![victim.clone()]);
                    let frame = broadcast_for_at(&deletion, &Value::Integer(1), now, None)?;
                    Some(Eviction {
                        victim: victim.clone(),
                        routed: tx.would_route(&deletion),
                        deletion,
                        frame,
                    })
                })
                .collect()
        })
        .collect();
    // The `DEL` of what each write found expired, logged and pushed before
    // its frame (6O-H): the server's deletion (origin 0), as an eviction's.
    let expiries: Vec<Option<Expiry>> = writes
        .iter()
        .map(|write| {
            expired_deletion(&write.expired).map(|(deletion, frame)| Expiry {
                routed: tx.would_route(&deletion),
                deletion,
                frame,
            })
        })
        .collect();
    let has_ws = routed.iter().any(|routed| *routed)
        || victims.iter().flatten().any(|eviction| eviction.routed)
        || expiries.iter().flatten().any(|expiry| expiry.routed);
    if needs_log || has_watch || has_ws {
        for write in &mut writes {
            if write.frame.is_none() {
                write.frame = Some(broadcast_for_at(
                    write.cmd,
                    &replies[write.index],
                    now,
                    write.exact_deadline,
                ));
            }
        }
    }
    if needs_log {
        let mut frames: Vec<&[u8]> = Vec::new();
        for ((write, victims), expiry) in writes.iter().zip(&victims).zip(&expiries) {
            if let Some(expiry) = expiry {
                frames.push(&expiry.frame);
            }
            if let Some(Some(frame)) = &write.frame {
                frames.push(frame);
            }
            frames.extend(victims.iter().map(|eviction| eviction.frame.as_slice()));
        }
        state
            .on_transaction_logged(&frames, !has_ws && !has_watch)
            .await;
    }
    let plan = if has_watch {
        NotificationPlan::new(&writes)
    } else {
        NotificationPlan::default()
    };
    let mut next_write = 0usize;
    for (index, cmd) in queue.iter().enumerate().take(ran) {
        if let Command::Publish(channel, message) = cmd {
            let count = pubsub.lock().await.publish(channel, message);
            replies[index] = Value::Integer(count);
        } else if writes.get(next_write).is_some_and(|w| w.index == index) {
            let at = next_write;
            next_write += 1;
            let write = &mut writes[at];
            if let Some(expiry) = &expiries[at] {
                if has_watch {
                    let changes: Vec<KeyChange<'_>> = write
                        .expired
                        .iter()
                        .filter_map(|key| plan.expiry(at, key))
                        .collect();
                    if !changes.is_empty() {
                        fan_out(watch_registry, &changes).await;
                    }
                }
                if expiry.routed {
                    tx.send(Arc::new(SyncPush::for_command(
                        0,
                        &expiry.deletion,
                        push_keys(&expiry.deletion),
                        expiry.frame.clone(),
                    )));
                }
            }
            if has_watch && write.effective() {
                if matches!(write.cmd, Command::FlushDb) {
                    let watched: Vec<String> = {
                        let reg = watch_registry.map.lock().await;
                        reg.keys().cloned().collect()
                    };
                    let empty = HashSet::new();
                    let set_earlier = plan.flushes.get(&at).unwrap_or(&empty);
                    notify_flushdb_after(watch_registry, watched, set_earlier).await;
                } else {
                    let changes = plan.changes(at, write, store);
                    if !changes.is_empty() {
                        fan_out(watch_registry, &changes).await;
                    }
                }
            }
            if routed[at]
                && let Some(Some(frame)) = write.frame.take()
            {
                tx.send(Arc::new(SyncPush::for_command(
                    conn_id,
                    write.cmd,
                    push_keys(write.cmd),
                    frame,
                )));
            }
            // Its evictions, after it, as `apply_eviction_effects` sends
            // them: the server's (origin 0), not the writer's.
            for eviction in &victims[at] {
                if has_watch && let Some(change) = plan.eviction(at, &eviction.victim) {
                    fan_out(watch_registry, std::slice::from_ref(&change)).await;
                }
                if eviction.routed {
                    tx.send(Arc::new(SyncPush::for_command(
                        0,
                        &eviction.deletion,
                        push_keys(&eviction.deletion),
                        eviction.frame.clone(),
                    )));
                }
            }
        }
        // Test-only stop between queued commands, every guard still held.
        #[cfg(test)]
        crate::propagation::exec_pause::after(cmd).await;
    }
    if let Some(failed) = applied.panicked
        && applied.diverged
    {
        // After the applied prefix's effects: it reaches replicas and
        // clients before they are made to resynchronise.
        converge(&queue[failed], state, watch_registry, store).await;
    }
    TransactionRun {
        replies,
        resynchronise_clients: applied.diverged,
    }
}

/// A queued command panicked inside phase 1 (design R3/N2/P2). The store
/// kept the applied prefix, rebuilt its metadata if the command was a write
/// (`repaired`), and released its stripes; phase 2 still logs, replicates
/// and notifies that prefix. A write may have been applied part-way and has
/// no frame, so the AOF no longer describes the keyspace: writes are
/// refused (`MISCONF`) until a `SAVE` snapshots the actual state, which
/// also truncates the AOF ([`converge`] handles the other copies).
///
/// Known limit (review C-6): what the failing command itself would have
/// done to ephemeral ownership is lost with the unwind — an `ESET` that
/// panicked after its mutation leaves its key unclaimed (not removed on
/// disconnect), and its eviction victims' claims are not forgotten (a later
/// disconnect deletes an absent key). The keyspace stays consistent with
/// what the primary shows; only connection-scoped ownership is affected.
fn fail_closed(
    state: &ServerState,
    cmd: &Command,
    applied: core_engine::store::TransactionApplied,
) {
    let name = command_name(cmd);
    counter!("recached_transaction_panics_total", "command" => name).increment(1);
    if !applied.diverged {
        error!(
            command = name,
            "a read queued in a transaction panicked; the transaction stopped there"
        );
        return;
    }
    error!(
        command = name,
        metadata_rebuilt = applied.repaired,
        "a write queued in a transaction panicked and may be applied part-way; its \
         effect is not logged or replicated. Client writes are disabled until SAVE \
         writes a snapshot of the actual state"
    );
    state.record_internal_failure("transaction_panic");
}

/// Bring every other copy of the keyspace back to the primary's actual state
/// after a write panicked part-way inside a transaction (design P2, Q2). Its
/// partial effect has no frame, so no stream can carry it:
///
/// - replicas: [`ReplHub::reset_stream`] rotates the run id and closes every
///   link, so each reconnects and takes a **full** resynchronisation (and,
///   being a replica that took one, resets its own sub-replicas the same
///   way). Closing the links alone would let a replica resume partially
///   past the missing change;
/// - WebSocket sync clients: closed with the sync-gap code once `EXEC`'s
///   reply is written ([`TransactionRun::resynchronise_clients`]);
/// - watchers and live queries: the command's keys announced as their full
///   values now (or, for a `FLUSHDB`, every watched key).
///
/// Runs with the transaction's guards still held, after the applied prefix's
/// effects, so a replica attach (all-write barrier) waits for it.
async fn converge(
    cmd: &Command,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
) {
    let replicas = state.replicas.reset_stream();
    if matches!(cmd, Command::FlushDb) {
        notify_keyspace_replaced(watch_registry, store).await;
    } else if watch_registry.watched_after_write() {
        let changes: Vec<KeyChange<'_>> = primary_keys(cmd)
            .into_iter()
            .map(|key| KeyChange::current(key, None, store))
            .collect();
        fan_out(watch_registry, &changes).await;
    }
    error!(
        command = command_name(cmd),
        replicas, "after a part-applied write: replicas made to resynchronise in full"
    );
}
