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
    let (response, evicted) = execute_and_record_ref(store, cmd);
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

/// Each written key's effective writes in a transaction, for coalescing its
/// WATCH/QSUB notification (design N1, P1, Q1).
#[derive(Default, Clone, Copy)]
struct KeyWrites {
    count: u32,
    /// The write it was last changed by (its position among the queue's
    /// writes), and whether as an eviction victim of that write.
    last: (usize, bool),
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
                        entry.last = (at, false);
                        entry.set |= !write.removes;
                    }
                }
            }
            // A victim is removed whether or not its write took effect.
            for victim in &write.evicted {
                let entry = plan.keys.entry(victim.clone()).or_default();
                entry.count = entry.count.saturating_add(1);
                entry.last = (at, true);
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
            if writes.last != (at, false) {
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
        let writes = self.keys.get(victim)?;
        (writes.last == (at, true)).then(|| {
            if writes.set {
                KeyChange::removed_after_set(victim.to_string())
            } else {
                KeyChange::removed(victim.to_string())
            }
        })
    }
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
/// The reply of the queued command that failed internally.
const FAILED: &str =
    "ERR internal error executing the command; writes are disabled until SAVE succeeds";

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
) -> Vec<Value> {
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
        replies.push(Value::Error(FAILED.to_string()));
        replies.extend((failed + 1..queue.len()).map(|_| Value::Error(NOT_RUN.to_string())));
        contain_transaction_panic(state, &queue[failed], applied.repaired);
    }

    // ── Phase 2: effects, in command order ──────────────────────────────
    let needs_log = state.needs_write_log();
    // The writer's half of the WATCH registration handshake, after every
    // write of the transaction (6O-E `watched_after_write`). A queue with
    // no writes has nothing to announce (review B-4).
    let has_watch = !writes.is_empty() && watch_registry.watched_after_write();
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
    let has_ws = routed.iter().any(|routed| *routed)
        || victims.iter().flatten().any(|eviction| eviction.routed);
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
        for (write, victims) in writes.iter().zip(&victims) {
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
    replies
}

/// A queued command panicked inside phase 1 (design R3/N2/P2). The store
/// kept the applied prefix, rebuilt its metadata if the command was a write
/// (`repaired`), and released its stripes; phase 2 still logs, replicates
/// and notifies that prefix. A write may have been applied part-way and has
/// no frame, so the AOF no longer describes the keyspace: writes are
/// refused (`MISCONF`) until a `SAVE` snapshots the actual state.
fn contain_transaction_panic(state: &ServerState, cmd: &Command, repaired: bool) {
    let name = command_name(cmd);
    counter!("recached_transaction_panics_total", "command" => name).increment(1);
    if !repaired {
        error!(
            command = name,
            "a read queued in a transaction panicked; the transaction stopped there"
        );
        return;
    }
    error!(
        command = name,
        "a write queued in a transaction panicked and may be applied part-way; its \
         effect is not logged or replicated. Client writes are disabled until SAVE \
         writes a snapshot of the actual state"
    );
    state.record_internal_failure("transaction_panic");
}
