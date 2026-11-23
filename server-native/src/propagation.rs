//! Turning an executed command into the frame that reaches the AOF, replicas
//! and browser sync clients — and the watch notifications that go with it.

use crate::*;
use core_engine::resp::{format_i64, write_bulk, write_push_header};

pub(crate) fn is_write_command(cmd: &Command) -> bool {
    if let Command::Dedup(_, _, inner) = cmd {
        return is_write_command(inner);
    }
    matches!(
        cmd,
        Command::Set(..)
            | Command::ESet(..)
            | Command::EAdd(..)
            | Command::Del(..)
            | Command::Unlink(..)
            | Command::Append(..)
            | Command::GetSet(..)
            | Command::MSet(..)
            | Command::SetNx(..)
            | Command::SetEx(..)
            | Command::PSetEx(..)
            | Command::Incr(..)
            | Command::Decr(..)
            | Command::IncrBy(..)
            | Command::DecrBy(..)
            | Command::Expire(..)
            | Command::PExpire(..)
            | Command::ExpireAt(..)
            | Command::PExpireAt(..)
            | Command::Persist(..)
            | Command::FlushDb
            | Command::Rename(..)
            | Command::HSet(..)
            | Command::HDel(..)
            | Command::HIncrBy(..)
            | Command::HIncrByFloat(..)
            | Command::HSetNx(..)
            | Command::LPush(..)
            | Command::RPush(..)
            | Command::LPushX(..)
            | Command::RPushX(..)
            | Command::LPop(..)
            | Command::RPop(..)
            | Command::LSet(..)
            | Command::LRem(..)
            | Command::LTrim(..)
            | Command::SAdd(..)
            | Command::SRem(..)
            | Command::SInterStore(..)
            | Command::SUnionStore(..)
            | Command::SDiffStore(..)
            | Command::SPop(..)
            | Command::SMove(..)
            | Command::ZAdd(..)
            | Command::ZRem(..)
            | Command::ZIncrBy(..)
            | Command::RlSet(..)
            | Command::RlCheck(..)
            | Command::JSet(..)
            | Command::JMerge(..)
            // Redis flags GETEX `write` even without an option, so a replica
            // refuses it either way and scope requires write for it.
            | Command::GetDel(..)
            | Command::GetEx(..)
            | Command::RenameNx(..)
            | Command::Copy(..)
            | Command::ZPopMin(..)
            | Command::ZPopMax(..)
    )
}

// ── Save conditions ───────────────────────────────────────────────────────────

/// Extract the key(s) that `cmd` writes to, without inspecting the response.
/// Used together with `broadcast_for()` — only call this when `broadcast_for`
/// already confirmed a mutation occurred.
pub(crate) fn primary_keys(cmd: &Command) -> Vec<String> {
    primary_key_refs(cmd)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// Keys borrowed from a command, inline for the usual one or two.
pub(crate) type KeyRefs<'a> = SmallVec<[&'a str; 2]>;

/// [`primary_keys`] without copying them: the ordering path only hashes the
/// keys, and cloning each into a `Vec<String>` cost two allocations per write.
pub(crate) fn primary_key_refs(cmd: &Command) -> KeyRefs<'_> {
    if let Command::Dedup(_, _, inner) = cmd {
        return primary_key_refs(inner);
    }
    match cmd {
        Command::ESet(k, _)
        | Command::EAdd(k, _)
        | Command::Set(k, _, _)
        | Command::Append(k, _)
        | Command::GetSet(k, _)
        | Command::SetNx(k, _)
        | Command::SetEx(k, _, _)
        | Command::PSetEx(k, _, _)
        | Command::Incr(k)
        | Command::Decr(k)
        | Command::IncrBy(k, _)
        | Command::DecrBy(k, _)
        | Command::Expire(k, _)
        | Command::PExpire(k, _)
        | Command::ExpireAt(k, _)
        | Command::PExpireAt(k, _)
        | Command::Persist(k)
        | Command::HSet(k, _)
        | Command::HDel(k, _)
        | Command::HSetNx(k, _, _)
        | Command::HIncrBy(k, _, _)
        | Command::HIncrByFloat(k, _, _)
        | Command::LPush(k, _)
        | Command::RPush(k, _)
        | Command::LPushX(k, _)
        | Command::RPushX(k, _)
        | Command::LPop(k, _)
        | Command::RPop(k, _)
        | Command::LSet(k, _, _)
        | Command::LRem(k, _, _)
        | Command::LTrim(k, _, _)
        | Command::SAdd(k, _)
        | Command::SRem(k, _)
        | Command::SPop(k, _)
        | Command::SInterStore(k, _)
        | Command::SUnionStore(k, _)
        | Command::SDiffStore(k, _)
        | Command::ZAdd(k, _, _)
        | Command::ZRem(k, _)
        | Command::ZIncrBy(k, _, _)
        | Command::RlSet(k, _, _)
        // RLCHECK records the attempt in its key. It is never broadcast (see
        // `broadcast_for`), but it is a write, and listing it keeps it in its
        // key's ordering domain rather than in none.
        | Command::RlCheck(k, _)
        | Command::JSet(k, _, _)
        | Command::JMerge(k, _)
        | Command::GetDel(k)
        | Command::GetEx(k, _)
        | Command::ZPopMin(k, _)
        | Command::ZPopMax(k, _) => smallvec::smallvec![k.as_str()],
        // COPY writes only its destination; the source is read, and is in its
        // ordering domain through `ordering_key_refs`.
        Command::Copy(_, dst, _) => smallvec::smallvec![dst.as_str()],
        Command::Del(keys) | Command::Unlink(keys) => keys.iter().map(String::as_str).collect(),
        Command::MSet(pairs) => pairs.iter().map(|(k, _)| k.as_str()).collect(),
        Command::Rename(src, dst) | Command::RenameNx(src, dst) | Command::SMove(src, dst, _) => {
            smallvec::smallvec![src.as_str(), dst.as_str()]
        }
        _ => KeyRefs::new(),
    }
}

/// Keys whose state can affect a write's result.
///
/// This is deliberately broader than [`primary_keys`]: the source sets of a
/// `*STORE` command are read while computing the destination, so they must be
/// in the same ordering domain as that destination. `FLUSHDB` is represented
/// by `None` because it conflicts with every key.
///
/// What the replication stripes are chosen from, borrowed from the command.
pub(crate) fn ordering_key_refs(cmd: &Command) -> Option<KeyRefs<'_>> {
    if let Command::Dedup(_, _, inner) = cmd {
        return ordering_key_refs(inner);
    }
    match cmd {
        Command::FlushDb => None,
        Command::SInterStore(dst, sources)
        | Command::SUnionStore(dst, sources)
        | Command::SDiffStore(dst, sources) => Some(
            std::iter::once(dst.as_str())
                .chain(sources.iter().map(String::as_str))
                .collect(),
        ),
        Command::Copy(src, dst, _) => Some(smallvec::smallvec![dst.as_str(), src.as_str()]),
        _ => Some(primary_key_refs(cmd)),
    }
}

/// Every key a pushed frame names, for the sync fan-out filter.
///
/// A WebSocket push is the command's own frame, so a scoped socket may receive
/// it only when it may read every key that frame mentions — not just the keys
/// the write changes. `COPY src dst` and the `*STORE` commands write only their
/// destination but name their sources: filtered by [`primary_keys`] alone, a
/// socket scoped to the destination tenant was pushed the names of another
/// tenant's keys (and a frame it could not apply, lacking the sources). Such a
/// socket now receives no push for that write; its live queries and watches on
/// the destination still get the value through their keychange. Empty for
/// `FLUSHDB`, which the filter handles as keyspace-wide.
pub(crate) fn push_keys(cmd: &Command) -> Vec<String> {
    push_key_refs(cmd).into_iter().map(str::to_owned).collect()
}

/// [`push_keys`], borrowed: what [`SyncRouter`] routes a write on before
/// deciding whether to build its push at all. One definition, so the keys
/// routed on and the keys the push carries cannot drift apart.
pub(crate) fn push_key_refs(cmd: &Command) -> KeyRefs<'_> {
    ordering_key_refs(cmd).unwrap_or_default()
}

fn claimable_eadd_members(
    cmd: &Command,
    state: &ServerState,
    store: &KeyValueStore,
) -> Vec<String> {
    let Command::EAdd(key, members) = cmd else {
        return Vec::new();
    };
    members
        .iter()
        .filter(|member| {
            state.is_ephemeral_member(key, member)
                || matches!(
                    store.execute(Command::SIsMember(key.clone(), (*member).clone())),
                    Value::Integer(0)
                )
        })
        .cloned()
        .collect()
}

fn reconcile_ephemeral_state(
    cmd: &Command,
    response: &Value,
    claimable_members: &[String],
    origin: u64,
    state: &ServerState,
) {
    if matches!(response, Value::Error(_)) {
        return;
    }
    match cmd {
        Command::ESet(key, _) => {
            state.forget_ephemeral_members_for_key(key);
            state.claim_ephemeral(key, origin);
        }
        Command::EAdd(key, _) => {
            state.forget_ephemeral_key(key);
            state.claim_ephemeral_members(key, claimable_members, origin);
        }
        Command::Del(keys) | Command::Unlink(keys) => {
            state.forget_ephemeral_keys(keys.iter().map(String::as_str));
        }
        Command::FlushDb => state.forget_all_ephemeral(),
        Command::Set(key, _, opts) => {
            if set_applied(opts, response) {
                state.forget_ephemeral_keys(std::iter::once(key.as_str()));
            }
        }
        Command::GetSet(key, _)
        | Command::SetEx(key, _, _)
        | Command::PSetEx(key, _, _)
        | Command::Incr(key)
        | Command::Decr(key)
        | Command::IncrBy(key, _)
        | Command::DecrBy(key, _) => {
            state.forget_ephemeral_keys(std::iter::once(key.as_str()));
        }
        Command::SetNx(key, _) if matches!(response, Value::Integer(1)) => {
            state.forget_ephemeral_keys(std::iter::once(key.as_str()));
        }
        Command::MSet(pairs) => {
            state.forget_ephemeral_keys(pairs.iter().map(|(key, _)| key.as_str()));
        }
        Command::SRem(key, members) => state.forget_ephemeral_members(key, members),
        Command::SPop(key, _) => {
            let members: Vec<String> = match response {
                Value::BulkString(Some(member)) => {
                    vec![String::from_utf8_lossy(member).into_owned()]
                }
                Value::Array(Some(values)) => values
                    .iter()
                    .filter_map(|value| match value {
                        Value::BulkString(Some(member)) => {
                            Some(String::from_utf8_lossy(member).into_owned())
                        }
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            state.forget_ephemeral_members(key, &members);
        }
        Command::SMove(source, destination, member) if matches!(response, Value::Integer(1)) => {
            state.forget_ephemeral_members(source, std::slice::from_ref(member));
            state.forget_ephemeral_members(destination, std::slice::from_ref(member));
        }
        Command::Rename(source, destination) if matches!(response, Value::SimpleString(_)) => {
            state.forget_ephemeral_keys([source.as_str(), destination.as_str()]);
        }
        Command::RenameNx(source, destination) if matches!(response, Value::Integer(1)) => {
            state.forget_ephemeral_keys([source.as_str(), destination.as_str()]);
        }
        // The value left the key, or the key now belongs to whoever wrote its
        // copy; either way no connection's close should delete it.
        Command::GetDel(key) if matches!(response, Value::BulkString(Some(_))) => {
            state.forget_ephemeral_keys(std::iter::once(key.as_str()));
        }
        Command::Copy(_, destination, _) if matches!(response, Value::Integer(1)) => {
            state.forget_ephemeral_keys(std::iter::once(destination.as_str()));
        }
        Command::SInterStore(destination, _)
        | Command::SUnionStore(destination, _)
        | Command::SDiffStore(destination, _) => {
            state.forget_ephemeral_keys(std::iter::once(destination.as_str()));
        }
        _ => {}
    }
}

/// Execute a write while the caller already holds all of its ordering guards.
/// Transaction execution and disconnect cleanup share this with the ordinary
/// write path so ephemeral ownership cannot be bypassed.
pub(crate) async fn execute_locked_write(
    cmd: &Command,
    tx: &SyncTx,
    origin: u64,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
) -> Value {
    execute_locked_write_identified(cmd, None, None, tx, origin, state, watch_registry, store).await
}

/// [`execute_locked_write`] for a write queued in a transaction: the store
/// runs it with its clock fixed at `now`, the instant the transaction runs at
/// (see [`core_engine::store::at_instant`]).
pub(crate) async fn execute_locked_write_at(
    cmd: &Command,
    now: u64,
    tx: &SyncTx,
    origin: u64,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
) -> Value {
    execute_locked_write_identified(
        cmd,
        None,
        Some(now),
        tx,
        origin,
        state,
        watch_registry,
        store,
    )
    .await
}

/// [`execute_locked_write`] for the write inside a DEDUP envelope, whose
/// identity the AOF records with it.
#[allow(clippy::too_many_arguments)]
async fn execute_locked_write_identified(
    cmd: &Command,
    dedup: Option<(&str, u64)>,
    clock: Option<u64>,
    tx: &SyncTx,
    origin: u64,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
) -> Value {
    let claimable_members = match clock {
        Some(now) => {
            core_engine::store::at_instant(now, || claimable_eadd_members(cmd, state, store))
        }
        None => claimable_eadd_members(cmd, state, store),
    };
    // By reference: the command is needed again below, and cloning it so the
    // store could consume the copy cost a key and a value per write.
    let (response, evicted) = match clock {
        Some(now) => core_engine::store::at_instant(now, || execute_and_record_ref(store, cmd)),
        None => execute_and_record_ref(store, cmd),
    };
    apply_write_effects_identified(
        cmd,
        &response,
        dedup,
        clock,
        tx,
        origin,
        state,
        watch_registry,
        store,
    )
    .await;
    apply_eviction_effects(&evicted, tx, state, watch_registry, store).await;
    reconcile_ephemeral_state(cmd, &response, &claimable_members, origin, state);
    response
}

/// Execute one write while its key-order guards remain held through durable
/// logging and fan-out. This makes the store commit order the observable AOF,
/// replica, watcher, and browser-sync order for every pair of conflicting
/// writes.
pub(crate) async fn execute_ordered_write(
    cmd: &Command,
    tx: &SyncTx,
    origin: u64,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
) -> Value {
    if !state.persistence_is_healthy() {
        let name = command_name(cmd);
        record_command(name);
        counter!("recached_command_errors_total", "command" => name).increment(1);
        return Value::Error(
            "MISCONF persistence is unhealthy; writes are disabled until SAVE succeeds".to_string(),
        );
    }
    let (effective, dedup) = match cmd {
        Command::Dedup(client, id, inner) => (inner.as_ref(), Some((client.as_str(), *id))),
        other => (other, None),
    };
    let _dedup_order = if dedup.is_some() {
        Some(state.dedup_order.lock().await)
    } else {
        None
    };
    if let Some((client, id)) = dedup
        && let Some(replay) = state.dedup_replay(client, id)
    {
        record_command(command_name(effective));
        return dedup_replay_reply(replay);
    }

    if dedup.is_none()
        && let Some(response) = try_execute_unordered(effective, state, watch_registry, store)
    {
        return response;
    }

    // Capacity eviction may choose any key, and its DEL must reach replicas
    // in order with writes to that key, so a write that may evict reserves
    // every propagation ordering domain. One that cannot — the store says so
    // under its capacity gate, and runs it in the same hold — needs only its
    // own keys' domains, like a write to an uncapped store. Taking all 256
    // stripes (and closing the unordered gate) for every write on a capped
    // store serialized every writer of every multitenant deployment.
    if store.has_capacity_limits() && !matches!(effective, Command::ESet(..) | Command::EAdd(..)) {
        let guards = state.replicas.lock_write(effective).await;
        if let Some(response) = execute_locked_write_within_capacity(
            effective,
            dedup,
            tx,
            origin,
            state,
            watch_registry,
            store,
        )
        .await
        {
            // Committed inside the write's ordering domain, as below.
            commit_dedup_outcome(state, dedup, &response).await;
            drop(guards);
            return response;
        }
    }
    let _guards = if store.has_capacity_limits() {
        state.replicas.lock_all_writes().await
    } else if matches!(effective, Command::ESet(..) | Command::EAdd(..)) {
        // A claim turns on `ephemeral_in_use`, which every unordered write
        // checks; closing the gate while claiming means none is in flight
        // when it does. See `UnorderedGate`.
        state
            .replicas
            .lock_commands(std::slice::from_ref(effective))
            .await
    } else {
        state.replicas.lock_write(effective).await
    };
    let response = execute_locked_write_identified(
        effective,
        dedup,
        None,
        tx,
        origin,
        state,
        watch_registry,
        store,
    )
    .await;
    commit_dedup_outcome(state, dedup, &response).await;
    response
}

/// A DEDUP write's outcome: its refusal remembered, or its mark committed.
async fn commit_dedup_outcome(state: &ServerState, dedup: Option<(&str, u64)>, response: &Value) {
    if let Some((client, id)) = dedup {
        match response {
            Value::Error(error) => record_dedup_refusal(state, client, id, error).await,
            _ => state.commit_dedup(client, id),
        }
    }
}

/// [`execute_locked_write_identified`] on a capped store for a write that
/// cannot evict, under only its own keys' ordering guards; `None`, having
/// run and propagated nothing, when it might evict — the caller then takes
/// every guard and runs it the ordinary way. With no eviction the write's
/// effects are its own, exactly as on an uncapped store.
async fn execute_locked_write_within_capacity(
    cmd: &Command,
    dedup: Option<(&str, u64)>,
    tx: &SyncTx,
    origin: u64,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
) -> Option<Value> {
    let claimable_members = claimable_eadd_members(cmd, state, store);
    let (response, evicted) = execute_and_record_ref_within_capacity(store, cmd)?;
    debug_assert!(
        evicted.is_empty(),
        "a write within capacity evicted {evicted:?}"
    );
    apply_write_effects_identified(
        cmd,
        &response,
        dedup,
        None,
        tx,
        origin,
        state,
        watch_registry,
        store,
    )
    .await;
    reconcile_ephemeral_state(cmd, &response, &claimable_members, origin, state);
    Some(response)
}

/// Run a write with no replication-order stripe when nothing can observe the
/// order it commits in, or `None` when it must take the ordered path.
///
/// Nothing observes it when the server has no AOF, no replication stream or
/// backlog, no WebSocket receiver, no WATCH or live query, no ephemeral claim
/// and no capacity limit — a plain cache, which is how most deployments and
/// every Redis comparison run. Its effects are then the store mutation alone,
/// which the store's own write-order stripes and shard locks keep atomic and
/// per-key ordered. DEDUP writes and ESET/EADD always take the ordered path:
/// they record state of their own that barriers capture.
///
/// Soundness rests on [`UnorderedGate`]: the consumer checks happen *after*
/// entering the gate, and every consumer is registered with the gate closed
/// (WebSocket subscription, TCP WATCH) or under a barrier that closes it
/// (replica attach, QSUB, snapshot, EXEC, ephemeral claims and release). So
/// a write either sees the consumer or finishes before the registration does.
/// The pass is held only across synchronous code.
pub(crate) fn try_execute_unordered(
    cmd: &Command,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
) -> Option<Value> {
    if store.has_capacity_limits()
        || matches!(
            cmd,
            Command::Dedup(..) | Command::ESet(..) | Command::EAdd(..)
        )
    {
        return None;
    }
    let _pass = enter_unordered(state, watch_registry)?;
    #[cfg(test)]
    unordered_pause::pause(|| primary_key_refs(cmd));
    let (response, evicted) = execute_and_record_ref(store, cmd);
    // An uncapped store never evicts; nothing to propagate either way.
    debug_assert!(evicted.is_empty(), "uncapped store evicted {evicted:?}");
    Some(response)
}

/// Enter the unordered gate, then check that nothing observes write order
/// (see [`UnorderedGate`]: the checks must follow the entry). `None` when the
/// write has to take the ordered path. The caller checks the store is
/// uncapped and holds the pass only across synchronous code.
fn enter_unordered<'a>(
    state: &'a ServerState,
    watch_registry: &WatchRegistry,
) -> Option<UnorderedPass<'a>> {
    let pass = state.replicas.unordered.try_enter()?;
    // `has_push_receivers` counts every WebSocket connection routed
    // something. One routed nothing (not authenticated, or no token yet) is
    // pushed nothing; it is counted, with this gate closed, before its first
    // routes are published (`ReplHub::route_pushes`).
    if state.replicas.unordered.has_push_receivers()
        || state.needs_write_log()
        || !watch_registry.is_empty()
        || state.ephemeral_in_use.load(Ordering::Acquire)
    {
        return None;
    }
    Some(pass)
}

/// Run a borrowed hot command straight from the read buffer, appending its
/// reply to `out`; `None` (nothing run, nothing written) when it must take
/// the owned path instead.
///
/// A read always can. A write can exactly when the owned write would take
/// the unordered path: persistence healthy, a primary, an uncapped store, and
/// nothing observing write order — so it has no effects beyond the store, and
/// running it from borrowed arguments changes nothing but the copies. Its
/// metrics and dirty count are recorded as the owned path records them.
pub(crate) fn try_execute_borrowed(
    cmd: &CommandRef<'_>,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
    out: &mut Vec<u8>,
) -> Option<()> {
    #[cfg(test)]
    if borrowed_switch::OFF.load(Ordering::Relaxed) {
        return None;
    }
    let is_write = cmd.is_write();
    // A write the owned path will take anyway is turned away before the
    // clock and the gate: with the AOF on, a replica, push receivers,
    // watchers or ephemeral keys, entering the gate only to leave it again
    // made every declined write pay it twice (6O-C review). This check can
    // only decline; the one after entering the gate stays authoritative.
    if is_write
        && (!state.persistence_is_healthy()
            || state.is_replica()
            || store.has_capacity_limits()
            || state.needs_write_log()
            || state.replicas.unordered.has_push_receivers()
            || !watch_registry.is_empty()
            || state.ephemeral_in_use.load(Ordering::Acquire))
    {
        return None;
    }
    let started = std::time::Instant::now();
    let outcome = if is_write {
        let _pass = enter_unordered(state, watch_registry)?;
        #[cfg(test)]
        unordered_pause::pause(|| cmd.write_key().into_iter().collect());
        store.execute_ref_into(cmd, out)?
    } else {
        store.execute_ref_into(cmd, out)?
    };
    record_metrics(
        store,
        command_ref_name(cmd),
        is_write,
        started,
        outcome.error,
        outcome.hit,
        false,
    );
    Some(())
}

/// One second's active expiry, in chunks; returns the keys removed.
///
/// Each chunk examines the next [`EXPIRY_CHUNK`] keys of the TTL-only dense
/// index (so a large persistent keyspace costs nothing here) and takes the
/// all-write barrier it shares with snapshots and QSUB's initial state, so a
/// removal and its notification are one observable step; writers run between
/// chunks.
///
/// One chunk per second used to be all there was: an expiry rate above 256
/// keys/s accumulated expired entries without bound, since reads only mask
/// them (6I: a `SET … EX 1` load grew the keyspace ~3,000 entries/s while
/// Redis stayed flat). As Redis does, this keeps sweeping while the last
/// sample was at least a quarter expired, within [`EXPIRY_TICK_BUDGET`].
pub(crate) async fn sweep_expired_tick(
    store: &KeyValueStore,
    state: &ServerState,
    watch_registry: &WatchRegistry,
) -> usize {
    let started = std::time::Instant::now();
    let mut total = 0usize;
    loop {
        let examined = store.volatile_key_count().min(EXPIRY_CHUNK);
        if examined == 0 {
            return total;
        }
        let removed = {
            let _write_guards = state.replicas.lock_all_writes().await;
            let expired = store.sweep_expired_reporting_budget(EXPIRY_CHUNK);
            state.forget_ephemeral_keys(expired.iter().map(String::as_str));
            if !watch_registry.is_empty() {
                notify_removed(watch_registry, &expired).await;
            }
            expired.len()
        };
        total = total.saturating_add(removed);
        if removed.saturating_mul(4) < examined || started.elapsed() >= EXPIRY_TICK_BUDGET {
            return total;
        }
    }
}

/// Test-only: turn the borrowed path off, so a test can compare the replies
/// of the two paths over the same input. Global, but harmless to tests
/// running alongside: the owned path answers identically, which is what the
/// comparison checks.
#[cfg(test)]
pub(crate) mod borrowed_switch {
    pub(crate) static OFF: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
}

/// Test-only fault injection: stall an unordered write on one key between
/// passing the gate and touching the store, so a test can land a consumer
/// registration or a barrier exactly inside that window.
#[cfg(test)]
pub(crate) mod unordered_pause {
    use super::*;

    static PAUSE: std::sync::Mutex<Option<(String, std::time::Duration)>> =
        std::sync::Mutex::new(None);
    /// Checked before the mutex, so an unarmed hook costs one relaxed load:
    /// the request-path benchmark runs in test builds, and a process-wide
    /// lock per write here would contend exactly like the locks it measures.
    static ARMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    /// Stall unordered writes to `key` for `pause` until [`clear`].
    pub(crate) fn set(key: &str, pause: std::time::Duration) {
        *PAUSE.lock().unwrap_or_else(|p| p.into_inner()) = Some((key.to_string(), pause));
        ARMED.store(true, Ordering::SeqCst);
    }

    pub(crate) fn clear() {
        ARMED.store(false, Ordering::SeqCst);
        *PAUSE.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }

    /// `written` lists the write's keys; it is only called when armed, so
    /// an unarmed hook costs the benchmarks (which run in test builds) no
    /// allocation.
    pub(super) fn pause<'k>(written: impl FnOnce() -> SmallVec<[&'k str; 2]>) {
        if !ARMED.load(Ordering::Relaxed) {
            return;
        }
        let armed = PAUSE.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if let Some((key, pause)) = armed
            && written().contains(&key.as_str())
        {
            std::thread::sleep(pause);
        }
    }
}

/// Test-only stop inside EXEC (6O-E): an EXEC that has just run a queued
/// command naming an armed key waits there, with every guard it holds still
/// held, until the test releases it. Used to place a concurrent write
/// *between* two of a transaction's queued commands, deterministically.
#[cfg(test)]
pub(crate) mod exec_pause {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    struct Inner {
        key: String,
        taken: AtomicBool,
        reached: AtomicBool,
        release: tokio::sync::Semaphore,
    }

    static ARMED: Mutex<Vec<Arc<Inner>>> = Mutex::new(Vec::new());
    /// Checked before the mutex, so an unarmed hook costs one load.
    static COUNT: AtomicUsize = AtomicUsize::new(0);

    /// Released (and disarmed) on drop.
    pub(crate) struct Pause {
        inner: Arc<Inner>,
    }

    /// Stop the first EXEC that runs a queued command naming `key`, right
    /// after that command. Keys are unique per test, so parallel tests do
    /// not stop each other's transactions.
    pub(crate) fn after_command_naming(key: &str) -> Pause {
        let inner = Arc::new(Inner {
            key: key.to_string(),
            taken: AtomicBool::new(false),
            reached: AtomicBool::new(false),
            release: tokio::sync::Semaphore::new(0),
        });
        ARMED
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(Arc::clone(&inner));
        COUNT.fetch_add(1, Ordering::SeqCst);
        Pause { inner }
    }

    impl Pause {
        /// Wait (polling, bounded) until an EXEC has stopped here.
        pub(crate) async fn reached(&self, within: Duration) -> bool {
            let deadline = Instant::now() + within;
            while Instant::now() < deadline {
                if self.inner.reached.load(Ordering::SeqCst) {
                    return true;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            false
        }
    }

    impl Drop for Pause {
        fn drop(&mut self) {
            ARMED
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .retain(|i| !Arc::ptr_eq(i, &self.inner));
            COUNT.fetch_sub(1, Ordering::SeqCst);
            self.inner.release.close();
        }
    }

    /// Called by EXEC after each queued command.
    pub(crate) async fn after(cmd: &core_engine::cmd::Command) {
        if COUNT.load(Ordering::SeqCst) == 0 {
            return;
        }
        let text = format!("{cmd:?}");
        let hit = ARMED
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .find(|i| text.contains(i.key.as_str()) && !i.taken.swap(true, Ordering::SeqCst))
            .cloned();
        if let Some(inner) = hit {
            inner.reached.store(true, Ordering::SeqCst);
            // Closed by `Pause::drop`.
            let _ = inner.release.acquire().await;
        }
    }
}

/// The reply to a DEDUP write seen before.
pub(crate) fn dedup_replay_reply(replay: DedupReplay) -> Value {
    match replay {
        DedupReplay::Duplicate => {
            counter!("recached_dedup_replays_total", "outcome" => "duplicate").increment(1);
            Value::SimpleString("DUP".to_string())
        }
        DedupReplay::Refused(error) => {
            counter!("recached_dedup_replays_total", "outcome" => "refused").increment(1);
            Value::Error(error)
        }
    }
}

/// A DEDUP write was refused. A permanent refusal is remembered — in memory
/// and in the AOF — so its replay is refused again; a transient one is left
/// for the client to retry.
///
/// Logged first, then remembered (6N-C, RV-9): a replay answered from the
/// remembered refusal — the WebSocket early-replay path reads it without
/// `dedup_order` — must not see it before its frame has reached the AOF
/// stage a write would wait for. Remembered first, a replay could be told
/// "refused" and a crash then lose the refusal, after which the same replay
/// was answered `+DUP` (or ran) instead.
pub(crate) async fn record_dedup_refusal(state: &ServerState, client: &str, id: u64, error: &str) {
    if core_engine::resp::is_transient_refusal(error) {
        counter!("recached_sync_write_refusals_total", "class" => "transient").increment(1);
        return;
    }
    counter!("recached_sync_write_refusals_total", "class" => "permanent").increment(1);
    state.log_dedup_outcome(client, id, Some(error)).await;
    state.remember_dedup_refusal(client, id, error);
}

/// Record the DEDUP refusal of a command refused before it ran, which so
/// holds none of its own stripes: a WebSocket command refused on a scoped
/// connection, as administrative or for a key outside its scopes (6N-C PR
/// 0b, RV-6).
///
/// It used to take the refused command's stripes: for `FLUSHDB` every
/// stripe with the unordered gate closed, and for a keyed command one per
/// key, so a refused `DEL` of a few thousand keys took nearly all of them.
/// Any token holder could take a global write barrier, plus an fsync under
/// `always`, once per message.
/// The refusal changes no data; what it must not do is fall inside a
/// checkpoint or a sub-replica cut, which the identity's single stripe
/// already prevents ([`ReplHub::lock_identity`]). `dedup_order` orders it
/// with DEDUP writes of the same identity, as on the write path; the lock
/// order is the write path's, `dedup_order` → stripe.
pub(crate) async fn record_unexecuted_dedup_refusal(
    state: &ServerState,
    client: &str,
    id: u64,
    error: &str,
) {
    let _dedup_order = state.dedup_order.lock().await;
    let _identity = state.replicas.lock_identity(client).await;
    // A replica records no DEDUP outcome of its own: its table follows its
    // primary's (replicated records, full-sync tables), and a refusal it
    // recorded would be fanned out to its sub-replicas and outlive its role
    // with no `still_following` check (6N-C integration review, L1). Checked
    // under the stripe, which a promotion's all-write barrier excludes. The
    // client is still answered, and that permanent answer is remembered
    // nowhere: a replay is evaluated afresh. That is safe for a client that
    // replays in order (the SDK outbox does), since the primary then sees the
    // refused id first and records the refusal itself; a client that skips
    // ahead after a lost reply could later be answered `+DUP` for it. The
    // reply is not changed to `READONLY`, so no client sees a new answer.
    if state.is_replica() {
        return;
    }
    record_dedup_refusal(state, client, id, error).await;
}

/// Release one connection's ephemeral holds while keeping the affected keys
/// locked from the last-holder decision through the final DEL/SREM. This closes
/// the disconnect race where a concurrent writer could recreate a key between
/// cleanup's cardinality check and delete.
pub(crate) async fn release_ephemeral_state(
    conn_id: u64,
    tx: &SyncTx,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
) {
    let (candidate_keys, candidate_members) = state.ephemeral_claims_for(conn_id);
    if candidate_keys.is_empty() && candidate_members.is_empty() {
        return;
    }
    let mut commands = Vec::new();
    if !candidate_keys.is_empty() {
        commands.push(Command::Del(candidate_keys));
    }
    commands.extend(
        candidate_members
            .into_iter()
            .map(|(key, members)| Command::SRem(key, members)),
    );
    let _guards = if store.has_capacity_limits() {
        state.replicas.lock_all_writes().await
    } else {
        state.replicas.lock_commands(&commands).await
    };

    let expired = state.take_ephemeral_for(conn_id);
    if !expired.is_empty() {
        let del = Command::Del(expired);
        execute_locked_write(&del, tx, conn_id, state, watch_registry, store).await;
    }
    for (key, members) in state.take_ephemeral_members_for(conn_id) {
        let srem = Command::SRem(key.clone(), members);
        execute_locked_write(&srem, tx, conn_id, state, watch_registry, store).await;
        if matches!(
            store.execute(Command::SCard(key.clone())),
            Value::Integer(0)
        ) {
            let del = Command::Del(vec![key]);
            execute_locked_write(&del, tx, conn_id, state, watch_registry, store).await;
        }
    }
}

/// Propagate implicit capacity victims as DEL operations. The caller holds the
/// same ordering guards as the write that selected them, so local mutation,
/// AOF, replication, browser sync, and watcher delivery agree on the order.
///
/// The deletions are the server's, not the writer's: pushed with origin 0.
/// They used to carry the id of the connection whose write caused them, and
/// a connection never receives its own pushes — so the one browser that
/// triggered an eviction was the one never told that a different key it
/// held had gone, and kept it in its local copy.
pub(crate) async fn apply_eviction_effects(
    evicted: &[String],
    tx: &SyncTx,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
) {
    for key in evicted {
        state.forget_ephemeral_keys(std::iter::once(key.as_str()));
        let deletion = Command::Del(vec![key.clone()]);
        apply_write_effects(
            &deletion,
            &Value::Integer(1),
            tx,
            0,
            state,
            watch_registry,
            store,
        )
        .await;
    }
}

#[cfg(test)]
mod eviction_propagation_tests {
    use super::*;
    use core_engine::cmd::SetOptions;
    use core_engine::store::{EvictionPolicy, KeyValueStore};
    use std::sync::atomic::AtomicBool;

    fn tmp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("recached_test_{name}_{}", std::process::id()))
    }

    #[tokio::test]
    async fn implicit_capacity_eviction_is_replayed_as_a_delete() {
        let path = tmp_path("capacity_eviction.aof");
        let _ = tokio::fs::remove_file(&path).await;
        let aof = AofWriter::open(path.clone(), AofSync::No).await.unwrap();
        let state = ServerState {
            snap: Arc::new(SnapshotConfig {
                path: tmp_path("capacity_eviction.rdb"),
                last_save: AtomicI64::new(0),
                checkpoint_id: AtomicU64::new(0),
            }),
            aof: Some(Arc::new(aof)),
            replicas: ReplHub::new(),
            is_replica: AtomicBool::new(false),
            dedup: std::sync::Mutex::new(HashMap::new()),
            ephemeral: std::sync::Mutex::new(HashMap::new()),
            ephemeral_members: std::sync::Mutex::new(HashMap::new()),
            ephemeral_in_use: std::sync::atomic::AtomicBool::new(false),
            dedup_dirty: AtomicBool::new(false),
            dedup_order: tokio::sync::Mutex::new(()),
            save_lock: tokio::sync::Mutex::new(()),
            persistence_healthy: AtomicBool::new(true),
            persistence_failures: AtomicU64::new(0),
        };
        let store = KeyValueStore::with_config(Some(1), None, EvictionPolicy::AllKeysRandom);
        let tx = SyncRouter::new(8);
        let watches = WatchHub::new();

        for (key, value) in [("victim", "old"), ("replacement", "new")] {
            let command = Command::Set(key.into(), value.into(), SetOptions::default());
            assert_eq!(
                execute_ordered_write(&command, &tx, 0, &state, &watches, &store).await,
                Value::SimpleString("OK".into())
            );
        }
        state.aof.as_ref().unwrap().flush().await.unwrap();

        let replayed = KeyValueStore::new();
        assert_eq!(replay_aof(&replayed, &path).await.unwrap(), 3);
        assert_eq!(
            replayed.execute(Command::Get("victim".into())),
            Value::BulkString(None),
            "the implicit victim must not return after AOF replay"
        );
        assert_eq!(
            replayed.execute(Command::Get("replacement".into())),
            Value::BulkString(Some(b"new".to_vec()))
        );

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn the_writer_whose_write_evicted_a_key_is_pushed_the_deletion() {
        // Regression (pre-6J, found by the 6J-E review): the eviction's DEL
        // carried the writer's connection id, and a connection never receives
        // its own pushes, so the browser that caused the eviction kept the
        // victim in its local copy.
        let state = ServerState {
            snap: Arc::new(SnapshotConfig {
                path: tmp_path("eviction_origin.rdb"),
                last_save: AtomicI64::new(0),
                checkpoint_id: AtomicU64::new(0),
            }),
            aof: None,
            replicas: ReplHub::new(),
            is_replica: AtomicBool::new(false),
            dedup: std::sync::Mutex::new(HashMap::new()),
            ephemeral: std::sync::Mutex::new(HashMap::new()),
            ephemeral_members: std::sync::Mutex::new(HashMap::new()),
            ephemeral_in_use: std::sync::atomic::AtomicBool::new(false),
            dedup_dirty: AtomicBool::new(false),
            dedup_order: tokio::sync::Mutex::new(()),
            save_lock: tokio::sync::Mutex::new(()),
            persistence_healthy: AtomicBool::new(true),
            persistence_failures: AtomicU64::new(0),
        };
        let store = KeyValueStore::with_config(Some(1), None, EvictionPolicy::AllKeysRandom);
        let tx = SyncRouter::new(8);
        let watches = WatchHub::new();
        let mut browser = state
            .replicas
            .subscribe_pushes(&tx, Routes::Everything)
            .await;
        const WRITER: u64 = 42;
        for key in ["victim", "replacement"] {
            let command = Command::Set(key.into(), "v".into(), SetOptions::default());
            execute_ordered_write(&command, &tx, WRITER, &state, &watches, &store).await;
        }
        let mut deletion = None;
        while let Ok(push) = browser.try_recv() {
            if push.keys == ["victim"]
                && push.resp.windows(3).any(|w| w.eq_ignore_ascii_case(b"DEL"))
            {
                deletion = Some(push.origin);
            }
        }
        assert_eq!(
            deletion,
            Some(0),
            "the eviction must be pushed as the server's"
        );
    }
}

/// The compact form of a mutation: an operation and its arguments, which a
/// client replays against its own copy of the key.
///
/// A delta is the *command*, not a computed diff, which is what makes it safe
/// — the client runs the same engine, so replaying `sadd k a` reproduces
/// exactly what the server did. It also keeps the set of delta-able commands
/// honest: if an operation cannot be replayed verbatim to the same result, it
/// has no delta and the key falls back to a whole-value `keychange`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeyDelta {
    pub(crate) op: &'static str,
    pub(crate) args: Vec<Vec<u8>>,
}

impl KeyDelta {
    /// Bytes this delta will occupy on the wire, for queue accounting.
    pub(crate) fn heap_bytes(&self) -> usize {
        self.args
            .iter()
            .map(|a| a.len().saturating_add(std::mem::size_of::<Vec<u8>>()))
            .sum::<usize>()
            .saturating_add(self.op.len())
    }
}

/// The compact form of `cmd`, if it has one.
///
/// Only commands that are **replayable verbatim** appear here. Two rules keep
/// that true:
///
/// * The caller has already confirmed the mutation happened (`broadcast_for`),
///   so a conditional push such as `LPUSHX` on a missing key never reaches
///   this point and cannot be replayed into a key that should not exist.
/// * Where a command's outcome depends on options — `ZADD` with `NX`, `XX`,
///   `GT`, `LT` or `INCR` — the delta is withheld rather than replayed without
///   them, because the client would compute a different score.
///
/// `SET`, `DEL`, expiry and everything else keep sending whole values: they
/// are already as small as their result, so a delta would buy nothing.
pub(crate) fn delta_for(cmd: &Command) -> Option<(String, KeyDelta)> {
    fn bytes(items: &[String]) -> Vec<Vec<u8>> {
        items.iter().map(|s| s.clone().into_bytes()).collect()
    }
    let (key, op, args) = match cmd {
        Command::Dedup(_, _, inner) => return delta_for(inner),

        // A token appended to an agent's output re-sent the whole transcript
        // to every viewer before this.
        Command::Append(k, v) => (k, "append", vec![v.clone()]),

        Command::SAdd(k, members) | Command::EAdd(k, members) => (k, "sadd", bytes(members)),
        Command::SRem(k, members) => (k, "srem", bytes(members)),
        Command::LPush(k, values) => (k, "lpush", values.clone()),
        Command::RPush(k, values) => (k, "rpush", values.clone()),
        Command::LPushX(k, values) => (k, "lpush", values.clone()),
        Command::RPushX(k, values) => (k, "rpush", values.clone()),
        Command::HDel(k, fields) => (k, "hdel", bytes(fields)),
        Command::HSet(k, pairs) => {
            let mut args = Vec::with_capacity(pairs.len() * 2);
            for (field, value) in pairs {
                args.push(field.clone().into_bytes());
                args.push(value.clone());
            }
            (k, "hset", args)
        }
        Command::ZRem(k, members) => (k, "zrem", bytes(members)),
        Command::ZAdd(k, options, entries) => {
            // `CH` only changes the integer reply, not the resulting zset, so
            // it is safe to replay without. The rest change what is stored.
            if options.condition.is_some() || options.gt || options.lt || options.incr {
                return None;
            }
            let mut args = Vec::with_capacity(entries.len() * 2);
            for (score, member) in entries {
                args.push(score.to_string().into_bytes());
                args.push(member.clone().into_bytes());
            }
            (k, "zadd", args)
        }
        _ => return None,
    };
    Some((key.clone(), KeyDelta { op, args }))
}

/// Encode a delta push: `["keydelta", key, op, arg...]`.
///
/// A distinct tag rather than a variant of `keychange`, so a client that does
/// not understand it can be detected by its absence of `CLIENT DELTA ON`
/// rather than by silently mis-parsing a frame it half-recognises.
pub(crate) fn encode_keydelta(key: &str, delta: &KeyDelta) -> Vec<u8> {
    let mut parts = Vec::with_capacity(delta.args.len() + 3);
    parts.push(Value::BulkString(Some(b"keydelta".to_vec())));
    parts.push(Value::BulkString(Some(key.as_bytes().to_vec())));
    parts.push(Value::BulkString(Some(delta.op.as_bytes().to_vec())));
    parts.extend(
        delta
            .args
            .iter()
            .map(|a| Value::BulkString(Some(a.clone()))),
    );
    Value::Array(Some(parts)).serialize()
}

/// Encode a queued notification in whichever form it was queued as.
/// `expiry` is the connection's `CLIENT EXPIRY` setting.
pub(crate) fn encode_notification(notif: &WatchNotif, expiry: bool) -> Vec<u8> {
    match &notif.payload {
        NotifPayload::Full(value, ttl_ms) => match ttl_ms.filter(|_| expiry) {
            Some(ms) => encode_keychange(&notif.key, &with_expiry((**value).clone(), Some(ms))),
            None => encode_keychange(&notif.key, value),
        },
        NotifPayload::Delta(delta) => encode_keydelta(&notif.key, delta),
    }
}

/// A key's value as a connection that sent `CLIENT EXPIRY ON` receives it in a
/// keychange or qstate: wrapped as `["px", <ms>, value]` while the key has a
/// time to live, so the client's copy expires when the server's does. Without
/// it, applying the value would make an expiring key permanent on the client.
///
/// Opt-in because an older client reads the wrapper as an unknown collection
/// type, which clears its copy of the key. Deltas need no wrapper: the commands
/// they carry keep the key's expiry, on the server and when replayed.
pub(crate) fn with_expiry(value: Value, ttl_ms: Option<u64>) -> Value {
    match ttl_ms {
        Some(ms) => Value::Array(Some(vec![
            Value::BulkString(Some(b"px".to_vec())),
            Value::Integer(i64::try_from(ms).unwrap_or(i64::MAX)),
            value,
        ])),
        None => value,
    }
}

pub(crate) fn encode_keychange(key: &str, value: &Value) -> Vec<u8> {
    Value::Array(Some(vec![
        Value::BulkString(Some(b"keychange".to_vec())),
        Value::BulkString(Some(key.as_bytes().to_vec())),
        value.clone(),
    ]))
    .serialize()
}

/// Tell WATCH and live-query subscribers about a confirmed write: per key, or
/// for `FLUSHDB` — which names no keys, so the per-key notifier would announce
/// nothing and subscribers would miss the wipe — every watched key and one
/// sentinel per live-query pattern. Shared by client writes on a primary and
/// replicated writes on a replica, so the two cannot drift apart.
pub(crate) async fn notify_write(registry: &WatchRegistry, cmd: &Command, store: &KeyValueStore) {
    if !registry.watched_after_write() {
        return;
    }
    if matches!(cmd, Command::FlushDb) {
        let watched: Vec<String> = {
            let reg = registry.map.lock().await;
            reg.keys().cloned().collect()
        };
        notify_flushdb(registry, watched).await;
    } else {
        notify_watchers(registry, cmd, store).await;
    }
}

/// Push keychange notifications for a *confirmed* mutation. Callers must have
/// already established that `cmd` mutated the store (via `broadcast_for`).
pub(crate) async fn notify_watchers(
    registry: &WatchRegistry,
    cmd: &Command,
    store: &KeyValueStore,
) {
    if registry.is_empty() {
        return;
    }
    let keys = primary_keys(cmd);
    if keys.is_empty() {
        return;
    }
    // The compact form, for subscribers that asked for deltas. Computed once
    // here rather than per subscriber, and only ever for the key it names —
    // a command touching several keys (MSET, RENAME) has no delta.
    let delta = delta_for(cmd);
    // The full value is read lazily, inside the fan-out, and only when some
    // recipient takes it rather than the delta. That read happens under the
    // registry lock; it cannot deadlock, because no DashMap guard is ever held
    // across an await, and it cannot see a later write to this key, because
    // the caller holds the key's ordering guard until this returns.
    // `DEL` and `UNLINK` announce removals: a WATCH of a key that was not
    // live at WATCH sees no change in one, as in Redis, whether or not the
    // key was there to delete (the store's reply counts keys, not which).
    // So does a `*STORE` whose result was empty: it deleted (or never
    // created) its destination, which Redis signals only if it existed. Read
    // under the writer's guard; a STORE's result carries no TTL, so absent
    // now means empty, not expired since (review round 2, Low).
    let removes = removes_named_keys(cmd) || stored_nothing(cmd, store);
    let changes: Vec<KeyChange<'_>> = keys
        .into_iter()
        .map(|k| {
            if removes {
                return KeyChange::removed(k);
            }
            let delta = delta
                .as_ref()
                .filter(|(delta_key, _)| *delta_key == k)
                .map(|(_, delta)| delta.clone());
            KeyChange::current(k, delta, store)
        })
        .collect();
    fan_out(registry, &changes).await;
}

/// Whether `cmd` is a `*STORE` that left its destination absent.
fn stored_nothing(cmd: &Command, store: &KeyValueStore) -> bool {
    match cmd {
        Command::Dedup(_, _, inner) => stored_nothing(inner, store),
        Command::SInterStore(dst, _)
        | Command::SUnionStore(dst, _)
        | Command::SDiffStore(dst, _) => !key_is_live(store, dst),
        _ => false,
    }
}

/// Whether `cmd` only removes the keys it names (`DEL`, `UNLINK`).
fn removes_named_keys(cmd: &Command) -> bool {
    match cmd {
        Command::Dedup(_, _, inner) => removes_named_keys(inner),
        Command::Del(_) | Command::Unlink(_) => true,
        _ => false,
    }
}

/// Announce keys the store removed on its own initiative rather than on a
/// client's command — today, expiry.
///
/// Without this the removal is invisible to anything replicating the keyspace:
/// no command ran, so no keychange was ever emitted, and a volatile key
/// survives forever in every replica's local copy with its last value. A nil
/// value is already how a delete is encoded, so this needs no new frame shape
/// and existing clients apply it unchanged.
pub(crate) async fn notify_removed(registry: &WatchRegistry, keys: &[String]) {
    if registry.is_empty() || keys.is_empty() {
        return;
    }
    let changes: Vec<KeyChange<'_>> = keys.iter().map(|k| KeyChange::removed(k.clone())).collect();
    fan_out(registry, &changes).await;
}

/// Deliver one batch of changed keys to exact-key watchers and to every live
/// query whose pattern matches.
async fn fan_out(registry: &WatchRegistry, changes: &[KeyChange<'_>]) {
    // A connection may WATCH a key and QSUB one or more overlapping patterns.
    // Deliver each changed key once per connection: deltas such as APPEND and
    // INCR are not idempotent when clients apply them to a local replica.
    let mut delivered: HashMap<String, HashSet<u64>> = HashMap::new();
    if registry.watched_keys.load(Ordering::Relaxed) > 0 {
        let mut reg = registry.map.lock().await;
        for change in changes {
            let key = &change.key;
            if let Some(subs) = reg.get_mut(key) {
                subs.retain(|sub| {
                    let alive = sub.notify(change);
                    if alive {
                        delivered
                            .entry(key.clone())
                            .or_default()
                            .insert(sub.conn_id);
                    }
                    alive
                });
                if subs.is_empty() {
                    reg.remove(key);
                }
            }
        }
        registry.sync_len(&reg);
    }
    // Live queries: every registered pattern matching a touched key gets the
    // same keychange notification. Only the patterns that could match the key
    // are checked (see `LiveQueries`), so a write no live query can match
    // costs a few hash lookups however many patterns are registered. Changes
    // are delivered in command order, so a connection sees a multi-key
    // command's keys in the order it named them.
    if registry.watched_patterns.load(Ordering::Relaxed) > 0 {
        let mut pats = registry.patterns.lock().await;
        let (mut candidates, mut matches) = (0usize, 0usize);
        for change in changes {
            // A key named twice in one command is delivered once, so
            // connections reached are remembered per key — but a key is only
            // copied into the map once somebody was reached.
            let (checked, matched) = match delivered.get_mut(&change.key) {
                Some(reached) => pats.deliver(change, reached),
                None => {
                    let mut reached = HashSet::new();
                    let counts = pats.deliver(change, &mut reached);
                    if !reached.is_empty() {
                        delivered.insert(change.key.clone(), reached);
                    }
                    counts
                }
            };
            candidates = candidates.saturating_add(checked);
            matches = matches.saturating_add(matched);
        }
        registry
            .route_events
            .fetch_add(changes.len() as u64, Ordering::Relaxed);
        registry
            .route_candidates
            .fetch_add(candidates as u64, Ordering::Relaxed);
        registry
            .route_matches
            .fetch_add(matches as u64, Ordering::Relaxed);
        registry.sync_patterns_len(&pats);
    }
}

/// Announce a `FLUSHDB` to live queries.
///
/// Emitting a keychange per deleted key would mean one frame per key in the
/// keyspace — potentially millions — for a single command. Instead each
/// registered pattern receives one sentinel, delivered as a keychange whose key
/// is the pattern and whose value is nil. Subscribers treat it as "every key
/// matching this pattern is gone", which is exactly what happened, at O(patterns)
/// instead of O(keys).
///
/// Explicitly `WATCH`ed keys are notified individually — that set is bounded by
/// the connection limit and callers expect per-key precision there.
pub(crate) async fn notify_flushdb(registry: &WatchRegistry, watched_before: Vec<String>) {
    if registry.watched_keys.load(Ordering::Relaxed) > 0 && !watched_before.is_empty() {
        let mut reg = registry.map.lock().await;
        for key in watched_before {
            if let Some(subs) = reg.get_mut(&key) {
                let change = KeyChange::removed(key);
                subs.retain(|sub| sub.notify(&change));
            }
        }
        registry.sync_len(&reg);
    }
    if registry.watched_patterns.load(Ordering::Relaxed) > 0 {
        let mut pats = registry.patterns.lock().await;
        pats.for_each_pattern(|pattern, subs| {
            let sentinel = KeyChange::removed(pattern.to_string());
            subs.retain(|sub| sub.notify(&sentinel));
        });
        registry.sync_patterns_len(&pats);
    }
}

/// Announce a keyspace replaced wholesale (a replica's full
/// resynchronisation) to exact-key watchers: each watched key with its value
/// as it now stands, so a `WATCH` before the resync aborts the `EXEC` after
/// it, as any write to the key would.
///
/// The current value, not FLUSHDB's nil: a WebSocket watcher may read this
/// frame before the sync-gap close [`SyncRouter::invalidate_all`] sends it,
/// and the current value is true where a nil would claim the key is gone.
/// Live queries need nothing here — they exist only on WebSocket
/// connections, which that close makes resubscribe.
pub(crate) async fn notify_keyspace_replaced(registry: &WatchRegistry, store: &KeyValueStore) {
    if registry.watched_keys.load(Ordering::Relaxed) == 0 {
        return;
    }
    let watched: Vec<String> = {
        let reg = registry.map.lock().await;
        reg.keys().cloned().collect()
    };
    let changes: Vec<KeyChange<'_>> = watched
        .into_iter()
        // A key the resynchronised keyspace does not have is announced as
        // removed: a WATCH of a key absent before and after sees no change,
        // as in Redis (`touchAllWatchedKeysInDb`).
        .map(|key| {
            if key_is_live(store, &key) {
                KeyChange::current(key, None, store)
            } else {
                KeyChange::removed(key)
            }
        })
        .collect();
    fan_out(registry, &changes).await;
}

/// Post-write fan-out shared by the TCP and WS command paths: WebSocket sync
/// broadcast, AOF/replication log, and watch notifications. Structured so that
/// with no WS clients, no replicas, no AOF, and no watched keys — the common
/// standalone-server case — a write costs zero locks and zero allocations here.
pub(crate) async fn apply_write_effects(
    cmd: &Command,
    response: &Value,
    tx: &SyncTx,
    origin: u64,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
) {
    apply_write_effects_identified(
        cmd,
        response,
        None,
        None,
        tx,
        origin,
        state,
        watch_registry,
        store,
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
async fn apply_write_effects_identified(
    cmd: &Command,
    response: &Value,
    dedup: Option<(&str, u64)>,
    clock: Option<u64>,
    tx: &SyncTx,
    origin: u64,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
) {
    let needs_log = state.needs_write_log();
    let has_watch = watch_registry.watched_after_write();
    // Only when some connection is a candidate for this write's push (see
    // `SyncRouter`): a write to keys no browser may see builds no push, and
    // with nothing else to do builds no frame either. Checked after the
    // write, so a connection routed later reads the write from the store.
    let has_ws = tx.would_route(cmd);
    if !has_ws && !needs_log && !has_watch {
        return;
    }
    // Read once, after the early-out above, so a standalone server with nothing
    // listening still pays no clock read per write.
    // Still under the write's key guards, so this is the deadline the write
    // set, not one a later write replaced.
    //
    // A write queued in a transaction ran at the transaction's instant
    // (`clock`): its deadline is read back at that instant too, and the
    // fallback counts from it, so a deadline that has passed by now still
    // propagates as the one the store set (6O-E review).
    let exact_deadline = relative_expiry_key(cmd).and_then(|key| {
        let read = || store.execute(Command::PExpireTime(key.to_string()));
        let at = match clock {
            Some(now) => core_engine::store::at_instant(now, read),
            None => read(),
        };
        match at {
            Value::Integer(at) if at > 0 => u64::try_from(at).ok(),
            _ => None,
        }
    });
    let now = clock.unwrap_or_else(now_unix_ms);
    let Some(msg) = broadcast_for_at(cmd, response, now, exact_deadline) else {
        // A DEDUP write that changed nothing still has an outcome to keep, or
        // a crash would let its retry run where the first try did not.
        // Refusals are recorded by the caller.
        if let Some((client, id)) = dedup
            && !matches!(response, Value::Error(_))
        {
            state.log_dedup_outcome(client, id, None).await;
        }
        return;
    };
    if needs_log {
        state
            .on_write_logged(&msg, dedup, !has_ws && !has_watch)
            .await;
    }
    if has_watch {
        notify_write(watch_registry, cmd, store).await;
    }
    if has_ws {
        tx.send(Arc::new(SyncPush::for_command(
            origin,
            cmd,
            push_keys(cmd),
            msg,
        )));
    }
}

/// Encodes a list of string parts as a RESP3 Push frame for WebSocket fan-out.
/// Uses `>` prefix so clients can distinguish server-initiated pushes from command responses.
/// Build a RESP3 Push frame from raw byte arguments.
///
/// Bytes rather than `&str` because these frames carry stored values, which may
/// be arbitrary binary. Building them as a `String` would have required a lossy
/// conversion — silently corrupting the replicated, AOF-logged and
/// browser-synced copy of a value the store itself holds faithfully.
///
/// One allocation of exactly the frame's size (6O-A), every byte written
/// straight into it by the reply encoder's own stack-formatting helpers. It
/// used to `format!` a temporary `String` per part and grow the `Vec` as it
/// went: eight allocator calls for a `SET`'s 75-byte frame, on every
/// logged, replicated or pushed write. The bytes are the same; the tests
/// compare them with that original encoder.
pub(crate) fn resp_push(parts: &[&[u8]]) -> Vec<u8> {
    // Only a total past `usize::MAX`, which no frame in memory comes near,
    // falls back to growing as it goes.
    let mut out = Vec::with_capacity(resp_push_len(parts).unwrap_or(0));
    write_push_header(&mut out, parts.len());
    for part in parts {
        write_bulk(&mut out, Some(part));
    }
    out
}

/// The exact length [`resp_push`] encodes `parts` to, or `None` if it does not
/// fit in a `usize`.
fn resp_push_len(parts: &[&[u8]]) -> Option<usize> {
    let digits = |n: usize| n.checked_ilog10().map_or(1, |d| d as usize + 1);
    // `>` count CRLF, then per part `$` length CRLF bytes CRLF.
    parts
        .iter()
        .try_fold(3 + digits(parts.len()), |total, part| {
            total
                .checked_add(5 + digits(part.len()))?
                .checked_add(part.len())
        })
}

/// Whether a `SET` that did not error actually wrote, read from its reply.
///
/// With `GET` the reply is the previous value, but together with the
/// condition it still settles the outcome: `NX` writes only when the key was
/// absent, so exactly when the old value is nil; `XX` writes only when it was
/// present, so exactly when it is not. Treating any `GET` as a write sent
/// `SET k new` to replicas after `SET k new NX GET` had left the primary's
/// value alone.
fn set_applied(opts: &SetOptions, response: &Value) -> bool {
    let nil = matches!(response, Value::BulkString(None));
    match (opts.get, &opts.condition) {
        (false, _) => !nil,
        (true, None) => true,
        (true, Some(SetCondition::Nx)) => nil,
        (true, Some(SetCondition::Xx)) => !nil,
    }
}

/// Returns the RESP-encoded mutation to broadcast to WebSocket peers, or `None`
/// if the command mutated nothing (read-only or conditional-and-failed).
///
/// `now_ms` is the propagation timestamp, and every *relative* TTL is rewritten
/// against it into an absolute `PXAT`/`PEXPIREAT` deadline.
///
/// That rewrite is load-bearing, because this one buffer is what the AOF, the
/// replication log and the browser sync fan-out all receive. Propagating the
/// relative form meant each of them re-based the TTL onto *its own* clock at
/// *its own* arrival time, so a key's lifetime silently restarted on every hop:
///
/// - **AOF** — replay happens at startup, so a key written with `EX 5` and
///   replayed an hour later came back alive with a fresh 5 seconds. A revoked
///   session, a distributed lock or an idempotency key that had long since
///   expired was resurrected by a restart.
/// - **Replicas** — a replica's copy expired later than the primary's by the
///   replication delay, and the gap reopened on every re-send.
/// - **Browsers** — the sync socket delivers on connect *and* on outbox replay,
///   so a reconnecting tab reset the TTL of every key it received.
///
/// An absolute deadline is idempotent under replay: applying it once or a
/// thousand times, now or after an hour of downtime, yields the same instant.
/// A deadline already in the past is not a special case — the store treats such
/// an entry as expired on read and the sweeper reaps it, which is precisely the
/// "it should already be gone" behaviour that was missing.
///
/// The deadline is computed from `now_ms` rather than read back out of the
/// store: the store's own expiry was computed microseconds earlier from the
/// same clock, and re-reading it would cost a lookup per write and still race
/// another thread overwriting the key. This is also what Redis does — it
/// rewrites relative expiries to absolute ones at propagation time.
/// [`broadcast_for_at`] without a stored deadline: tests drive it with a fixed clock.
#[cfg(test)]
pub(crate) fn broadcast_for(cmd: &Command, response: &Value, now_ms: u64) -> Option<Vec<u8>> {
    broadcast_for_at(cmd, response, now_ms, None)
}

/// The key whose deadline a write sets *relative* to the clock — `EX`/`PX`
/// on `SET` and `GETEX`, `SETEX`, `PSETEX`, `EXPIRE`, `PEXPIRE` — if any.
pub(crate) fn relative_expiry_key(cmd: &Command) -> Option<&str> {
    match cmd {
        Command::Dedup(_, _, inner) => relative_expiry_key(inner),
        Command::Set(k, _, opts)
            if matches!(opts.expiry, Some(SetExpiry::Ex(_)) | Some(SetExpiry::Px(_))) =>
        {
            Some(k)
        }
        Command::SetEx(k, _, _)
        | Command::PSetEx(k, _, _)
        | Command::Expire(k, _)
        | Command::PExpire(k, _)
        | Command::GetEx(k, Some(GetExOption::Ex(_) | GetExOption::Px(_))) => Some(k),
        _ => None,
    }
}

/// [`broadcast_for`], given the deadline the store actually set.
///
/// A relative expiry used to be re-derived here from a second clock reading,
/// taken after the store's own: the AOF, every replica and every browser peer
/// could hold a deadline a millisecond or two later than the primary's. With
/// `exact_deadline` (read back while the write's key guard is still held) they
/// get the primary's, to the millisecond.
pub(crate) fn broadcast_for_at(
    cmd: &Command,
    response: &Value,
    now_ms: u64,
    exact_deadline: Option<u64>,
) -> Option<Vec<u8>> {
    let at = |computed: u64| exact_deadline.unwrap_or(computed);
    // An error reply means the store refused the write (WRONGTYPE, OOM, a
    // key-cap refusal, an overflow) and left the keyspace as it was. Emitting
    // it anyway would make every replica and AOF replay run a command that
    // fails there too — and both treat a failed command as corruption, so a
    // single mistyped `HSET` would stop the server from restarting.
    if matches!(response, Value::Error(_)) {
        return None;
    }
    match cmd {
        // Keep the ephemeral verb in the durable stream so AOF replay can
        // discard any connection-scoped state whose owner vanished in a crash.
        // Replicas and browser stores still apply it as an ordinary mutation;
        // only the owning server tracks the connection lifetime.
        Command::ESet(k, v) => Some(resp_push(&[b"ESET", k.as_bytes(), v.as_slice()])),
        Command::EAdd(k, members) => match response {
            Value::Integer(n) if *n > 0 => {
                let mut parts: Vec<&[u8]> = vec![b"EADD", k.as_bytes()];
                let m_refs: Vec<&[u8]> = members.iter().map(|s| s.as_bytes()).collect();
                parts.extend_from_slice(&m_refs);
                Some(resp_push(&parts))
            }
            _ => None,
        },
        Command::Set(k, v, opts) => {
            if !set_applied(opts, response) {
                return None;
            }
            match &opts.expiry {
                None => Some(resp_push(&[b"SET", k.as_bytes(), v.as_slice()])),
                // Relative → absolute: see the note on `broadcast_for`.
                Some(SetExpiry::Ex(s)) => {
                    let pxat = at(now_ms.saturating_add(s.saturating_mul(1000))).to_string();
                    Some(resp_push(&[
                        b"SET",
                        k.as_bytes(),
                        v.as_slice(),
                        b"PXAT",
                        pxat.as_bytes(),
                    ]))
                }
                Some(SetExpiry::Px(ms)) => {
                    let pxat = at(now_ms.saturating_add(*ms)).to_string();
                    Some(resp_push(&[
                        b"SET",
                        k.as_bytes(),
                        v.as_slice(),
                        b"PXAT",
                        pxat.as_bytes(),
                    ]))
                }
                Some(SetExpiry::Exat(ts)) => {
                    let pxat = ts.saturating_mul(1000).to_string();
                    Some(resp_push(&[
                        b"SET",
                        k.as_bytes(),
                        v.as_slice(),
                        b"PXAT",
                        pxat.as_bytes(),
                    ]))
                }
                Some(SetExpiry::Pxat(ts)) => {
                    let ts_s = ts.to_string();
                    Some(resp_push(&[
                        b"SET",
                        k.as_bytes(),
                        v.as_slice(),
                        b"PXAT",
                        ts_s.as_bytes(),
                    ]))
                }
                Some(SetExpiry::KeepTtl) => {
                    Some(resp_push(&[b"SET", k.as_bytes(), v.as_slice(), b"KEEPTTL"]))
                }
            }
        }
        Command::Del(keys) | Command::Unlink(keys) => {
            let mut parts: Vec<&[u8]> = vec![b"DEL"];
            let key_refs: Vec<&[u8]> = keys.iter().map(|s| s.as_bytes()).collect();
            parts.extend_from_slice(&key_refs);
            Some(resp_push(&parts))
        }
        Command::MSet(pairs) => {
            // Borrowed: the frame is the only copy of the keys and values.
            let mut parts: Vec<&[u8]> = Vec::with_capacity(1 + 2 * pairs.len());
            parts.push(b"MSET");
            for (k, v) in pairs {
                parts.push(k.as_bytes());
                parts.push(v);
            }
            Some(resp_push(&parts))
        }
        Command::SetNx(k, v) => match response {
            Value::Integer(1) => Some(resp_push(&[b"SET", k.as_bytes(), v.as_slice()])),
            _ => None,
        },
        Command::SetEx(k, secs, v) => {
            let pxat = at(now_ms.saturating_add(secs.saturating_mul(1000))).to_string();
            Some(resp_push(&[
                b"SET",
                k.as_bytes(),
                v.as_slice(),
                b"PXAT",
                pxat.as_bytes(),
            ]))
        }
        Command::PSetEx(k, ms, v) => {
            let pxat = at(now_ms.saturating_add(*ms)).to_string();
            Some(resp_push(&[
                b"SET",
                k.as_bytes(),
                v.as_slice(),
                b"PXAT",
                pxat.as_bytes(),
            ]))
        }
        Command::Append(k, v) => match response {
            Value::Integer(_) => Some(resp_push(&[b"APPEND", k.as_bytes(), v.as_slice()])),
            _ => None,
        },
        // GETSET clears any TTL, as in Redis, so a bare SET is the faithful
        // replay — unlike the counters below.
        Command::GetSet(k, v) => Some(resp_push(&[b"SET", k.as_bytes(), v.as_slice()])),
        // Counters replay as `SET <new value> KEEPTTL`.
        //
        // A counter is propagated by value rather than as `INCR`, so that a
        // replica that missed a frame converges on the primary's number instead
        // of compounding its own. But a bare `SET` also *clears* the TTL, and
        // `INCR` in Redis leaves it untouched — so the single most common
        // expiring-counter idiom, `INCR key` + `EXPIRE key window`, replayed as
        // a key with no expiry at all. The rate-limit bucket, the per-minute
        // quota and the retry counter all became permanent on the replica, in
        // the AOF and in every synced browser, and the next window never reset
        // because the key it keyed on never went away. `KEEPTTL` keeps the
        // by-value convergence while leaving the deadline where the primary
        // has it.
        Command::Incr(k) | Command::Decr(k) | Command::IncrBy(k, _) | Command::DecrBy(k, _) => {
            match response {
                Value::Integer(n) => {
                    let mut digits = [0u8; 20];
                    let n = format_i64(*n, &mut digits);
                    Some(resp_push(&[b"SET", k.as_bytes(), n, b"KEEPTTL"]))
                }
                _ => None,
            }
        }
        // Relative → absolute: see the note on `broadcast_for`.
        Command::Expire(k, secs) => match response {
            Value::Integer(1) => {
                let ts = at(now_ms.saturating_add(secs.saturating_mul(1000))).to_string();
                Some(resp_push(&[b"PEXPIREAT", k.as_bytes(), ts.as_bytes()]))
            }
            _ => None,
        },
        Command::PExpire(k, ms) => match response {
            Value::Integer(1) => {
                let ts = at(now_ms.saturating_add(*ms)).to_string();
                Some(resp_push(&[b"PEXPIREAT", k.as_bytes(), ts.as_bytes()]))
            }
            _ => None,
        },
        Command::ExpireAt(k, ts) => match response {
            Value::Integer(1) => {
                let ts_ms = ts.saturating_mul(1000).to_string();
                Some(resp_push(&[b"PEXPIREAT", k.as_bytes(), ts_ms.as_bytes()]))
            }
            _ => None,
        },
        Command::PExpireAt(k, ts) => match response {
            Value::Integer(1) => {
                let ts_s = ts.to_string();
                Some(resp_push(&[b"PEXPIREAT", k.as_bytes(), ts_s.as_bytes()]))
            }
            _ => None,
        },
        Command::Persist(k) => match response {
            Value::Integer(1) => Some(resp_push(&[b"PERSIST", k.as_bytes()])),
            _ => None,
        },
        Command::FlushDb => Some(resp_push(&[b"FLUSHDB"])),

        // ── 0.6 commands, propagated as primitives every consumer already
        // replays (replicas, AOF, and 0.5 browser and mobile clients) ─────
        Command::GetDel(k) => match response {
            Value::BulkString(Some(_)) => Some(resp_push(&[b"DEL", k.as_bytes()])),
            _ => None,
        },
        Command::GetEx(k, Some(option)) => {
            if !matches!(response, Value::BulkString(Some(_))) {
                return None;
            }
            let deadline = match option {
                GetExOption::Persist => {
                    return Some(resp_push(&[b"PERSIST", k.as_bytes()]));
                }
                GetExOption::Ex(secs) => at(secs.saturating_mul(1000).saturating_add(now_ms)),
                GetExOption::Px(ms) => at(ms.saturating_add(now_ms)),
                GetExOption::ExAt(secs) => secs.saturating_mul(1000),
                GetExOption::PxAt(ms) => *ms,
            };
            if deadline <= now_ms {
                // The store deleted it; say so rather than ship a past
                // deadline for each consumer to interpret.
                Some(resp_push(&[b"DEL", k.as_bytes()]))
            } else {
                let at = deadline.to_string();
                Some(resp_push(&[b"PEXPIREAT", k.as_bytes(), at.as_bytes()]))
            }
        }
        Command::RenameNx(src, dst) => match response {
            Value::Integer(1) => Some(resp_push(&[b"RENAME", src.as_bytes(), dst.as_bytes()])),
            _ => None,
        },
        // Replayed verbatim, like the STORE family: deterministic against the
        // same prior state, which ordering guarantees. REPLACE always, so a
        // consumer that somehow holds the destination converges on the copy.
        Command::Copy(src, dst, _) => match response {
            Value::Integer(1) => Some(resp_push(&[
                b"COPY",
                src.as_bytes(),
                dst.as_bytes(),
                b"REPLACE",
            ])),
            _ => None,
        },
        Command::ZPopMin(k, _) | Command::ZPopMax(k, _) => match response {
            Value::Array(Some(items)) if !items.is_empty() => {
                let mut parts: Vec<&[u8]> = vec![b"ZREM", k.as_bytes()];
                parts.extend(items.iter().step_by(2).filter_map(|item| match item {
                    Value::BulkString(Some(member)) => Some(member.as_slice()),
                    _ => None,
                }));
                Some(resp_push(&parts))
            }
            _ => None,
        },
        Command::Rename(src, dst) => match response {
            Value::Error(_) => None,
            _ => Some(resp_push(&[b"RENAME", src.as_bytes(), dst.as_bytes()])),
        },

        // ── Hash ─────────────────────────────────────────────────────────────
        Command::HSet(k, pairs) => {
            // Borrowed: the frame is the only copy of the fields and values.
            let mut parts: Vec<&[u8]> = Vec::with_capacity(2 + 2 * pairs.len());
            parts.push(b"HSET");
            parts.push(k.as_bytes());
            for (f, v) in pairs {
                parts.push(f.as_bytes());
                parts.push(v);
            }
            Some(resp_push(&parts))
        }
        Command::HDel(k, fields) => match response {
            Value::Integer(n) if *n > 0 => {
                let mut parts: Vec<&[u8]> = vec![b"HDEL", k.as_bytes()];
                let field_refs: Vec<&[u8]> = fields.iter().map(|s| s.as_bytes()).collect();
                parts.extend_from_slice(&field_refs);
                Some(resp_push(&parts))
            }
            _ => None,
        },
        Command::HIncrBy(k, f, _) => match response {
            Value::Integer(n) => {
                let s = n.to_string();
                Some(resp_push(&[
                    b"HSET",
                    k.as_bytes(),
                    f.as_bytes(),
                    s.as_bytes(),
                ]))
            }
            _ => None,
        },
        Command::HIncrByFloat(k, f, _) => match response {
            Value::BulkString(Some(data)) => {
                let s = String::from_utf8_lossy(data);
                Some(resp_push(&[
                    b"HSET",
                    k.as_bytes(),
                    f.as_bytes(),
                    s.as_bytes(),
                ]))
            }
            _ => None,
        },
        Command::HSetNx(k, f, v) => match response {
            Value::Integer(1) => Some(resp_push(&[
                b"HSET",
                k.as_bytes(),
                f.as_bytes(),
                v.as_slice(),
            ])),
            _ => None,
        },

        // ── List ─────────────────────────────────────────────────────────────
        Command::LPush(k, vals) | Command::RPush(k, vals) => {
            let cmd_name = if matches!(cmd, Command::LPush(_, _)) {
                "LPUSH"
            } else {
                "RPUSH"
            };
            let mut parts: Vec<&[u8]> = vec![cmd_name.as_bytes(), k.as_bytes()];
            let val_refs: Vec<&[u8]> = vals.iter().map(|v| v.as_slice()).collect();
            parts.extend_from_slice(&val_refs);
            Some(resp_push(&parts))
        }
        Command::LPushX(k, vals) | Command::RPushX(k, vals) => match response {
            Value::Integer(n) if *n > 0 => {
                let cmd_name = if matches!(cmd, Command::LPushX(_, _)) {
                    "LPUSH"
                } else {
                    "RPUSH"
                };
                let mut parts: Vec<&[u8]> = vec![cmd_name.as_bytes(), k.as_bytes()];
                let val_refs: Vec<&[u8]> = vals.iter().map(|v| v.as_slice()).collect();
                parts.extend_from_slice(&val_refs);
                Some(resp_push(&parts))
            }
            _ => None,
        },
        Command::LPop(k, count) => match response {
            Value::BulkString(None) => None,
            Value::Array(Some(items)) if items.is_empty() => None,
            _ => {
                let n = count.map(|c| c.to_string());
                match &n {
                    Some(ns) => Some(resp_push(&[b"LPOP", k.as_bytes(), ns.as_bytes()])),
                    None => Some(resp_push(&[b"LPOP", k.as_bytes()])),
                }
            }
        },
        Command::RPop(k, count) => match response {
            Value::BulkString(None) => None,
            Value::Array(Some(items)) if items.is_empty() => None,
            _ => {
                let n = count.map(|c| c.to_string());
                match &n {
                    Some(ns) => Some(resp_push(&[b"RPOP", k.as_bytes(), ns.as_bytes()])),
                    None => Some(resp_push(&[b"RPOP", k.as_bytes()])),
                }
            }
        },
        Command::LSet(k, idx, v) => match response {
            Value::SimpleString(_) => {
                let idx_s = idx.to_string();
                Some(resp_push(&[
                    b"LSET",
                    k.as_bytes(),
                    idx_s.as_bytes(),
                    v.as_slice(),
                ]))
            }
            _ => None,
        },
        Command::LRem(k, count, elem) => match response {
            Value::Integer(n) if *n > 0 => {
                let count_s = count.to_string();
                Some(resp_push(&[
                    b"LREM",
                    k.as_bytes(),
                    count_s.as_bytes(),
                    elem.as_slice(),
                ]))
            }
            _ => None,
        },
        Command::LTrim(k, start, stop) => {
            let start_s = start.to_string();
            let stop_s = stop.to_string();
            Some(resp_push(&[
                b"LTRIM",
                k.as_bytes(),
                start_s.as_bytes(),
                stop_s.as_bytes(),
            ]))
        }

        // ── Set ───────────────────────────────────────────────────────────────
        Command::SAdd(k, members) => match response {
            Value::Integer(n) if *n > 0 => {
                let mut parts: Vec<&[u8]> = vec![b"SADD", k.as_bytes()];
                let m_refs: Vec<&[u8]> = members.iter().map(|s| s.as_bytes()).collect();
                parts.extend_from_slice(&m_refs);
                Some(resp_push(&parts))
            }
            _ => None,
        },
        Command::SRem(k, members) => match response {
            Value::Integer(n) if *n > 0 => {
                let mut parts: Vec<&[u8]> = vec![b"SREM", k.as_bytes()];
                let m_refs: Vec<&[u8]> = members.iter().map(|s| s.as_bytes()).collect();
                parts.extend_from_slice(&m_refs);
                Some(resp_push(&parts))
            }
            _ => None,
        },
        Command::SPop(k, count) => {
            let popped: Vec<String> = match response {
                Value::BulkString(Some(data)) => {
                    vec![String::from_utf8_lossy(data).into_owned()]
                }
                Value::Array(Some(items)) => items
                    .iter()
                    .filter_map(|v| {
                        if let Value::BulkString(Some(d)) = v {
                            Some(String::from_utf8_lossy(d).into_owned())
                        } else {
                            None
                        }
                    })
                    .collect(),
                _ => vec![],
            };
            if popped.is_empty() {
                let _ = count;
                None
            } else {
                let mut parts: Vec<&[u8]> = vec![b"SREM", k.as_bytes()];
                let m_refs: Vec<&[u8]> = popped.iter().map(|s| s.as_bytes()).collect();
                parts.extend_from_slice(&m_refs);
                Some(resp_push(&parts))
            }
        }
        Command::SMove(src, dst, member) => match response {
            Value::Integer(1) => Some(resp_push(&[
                b"SMOVE",
                src.as_bytes(),
                dst.as_bytes(),
                member.as_bytes(),
            ])),
            _ => None,
        },
        Command::SInterStore(dst, keys) => {
            let mut parts: Vec<&[u8]> = vec![b"SINTERSTORE", dst.as_bytes()];
            let k_refs: Vec<&[u8]> = keys.iter().map(|s| s.as_bytes()).collect();
            parts.extend_from_slice(&k_refs);
            Some(resp_push(&parts))
        }
        Command::SUnionStore(dst, keys) => {
            let mut parts: Vec<&[u8]> = vec![b"SUNIONSTORE", dst.as_bytes()];
            let k_refs: Vec<&[u8]> = keys.iter().map(|s| s.as_bytes()).collect();
            parts.extend_from_slice(&k_refs);
            Some(resp_push(&parts))
        }
        Command::SDiffStore(dst, keys) => {
            let mut parts: Vec<&[u8]> = vec![b"SDIFFSTORE", dst.as_bytes()];
            let k_refs: Vec<&[u8]> = keys.iter().map(|s| s.as_bytes()).collect();
            parts.extend_from_slice(&k_refs);
            Some(resp_push(&parts))
        }

        // ── Sorted Set ────────────────────────────────────────────────────────
        Command::ZAdd(k, opts, pairs) => {
            // A nil reply is an `INCR` whose condition aborted it: nothing
            // was written.
            if opts.incr && matches!(response, Value::BulkString(None)) {
                return None;
            }
            // Every condition travels with the command. The reply cannot say
            // which members a conditional ZADD actually updated — a score
            // update returns 0 just as a skipped member does — but replaying
            // the same conditions against the same prior state reproduces
            // exactly the primary's outcome. Dropping GT/LT made `ZADD z GT 1
            // m` lower the score on every replica while the primary kept it.
            let mut parts: Vec<String> = vec!["ZADD".into(), k.clone()];
            if let Some(cond) = &opts.condition {
                parts.push(match cond {
                    ZAddCondition::Nx => "NX".into(),
                    ZAddCondition::Xx => "XX".into(),
                });
            }
            if opts.gt {
                parts.push("GT".into());
            }
            if opts.lt {
                parts.push("LT".into());
            }
            if opts.ch {
                parts.push("CH".into());
            }
            if opts.incr {
                parts.push("INCR".into());
            }
            for (score, member) in pairs {
                parts.push(format_f64_score(*score));
                parts.push(member.clone());
            }
            let refs: Vec<&[u8]> = parts.iter().map(|s| s.as_bytes()).collect();
            Some(resp_push(&refs))
        }
        Command::ZRem(k, members) => match response {
            Value::Integer(n) if *n > 0 => {
                let mut parts: Vec<&[u8]> = vec![b"ZREM", k.as_bytes()];
                let m_refs: Vec<&[u8]> = members.iter().map(|s| s.as_bytes()).collect();
                parts.extend_from_slice(&m_refs);
                Some(resp_push(&parts))
            }
            _ => None,
        },
        Command::ZIncrBy(k, delta, member) => {
            let delta_s = format_f64_score(*delta);
            Some(resp_push(&[
                b"ZINCRBY",
                k.as_bytes(),
                delta_s.as_bytes(),
                member.as_bytes(),
            ]))
        }

        // ── JSON ─────────────────────────────────────────────────────────────
        // Replayable as-is on replicas, AOF, and browser stores. Only
        // successful writes replicate (errors reply -ERR, not +OK).
        Command::JSet(k, path, value) => match response {
            Value::SimpleString(_) => Some(resp_push(&[
                b"JSET",
                k.as_bytes(),
                path.as_bytes(),
                value.as_bytes(),
            ])),
            _ => None,
        },
        Command::JMerge(k, patch) => match response {
            Value::SimpleString(_) => Some(resp_push(&[b"JMERGE", k.as_bytes(), patch.as_bytes()])),
            _ => None,
        },

        // ── Rate limiting ────────────────────────────────────────────────────
        // RLSET replicates so limiter *config* survives AOF replay / reaches
        // replicas. RLCHECK is deliberately not replicated: attempt state is
        // transient and high-frequency — streaming every check would flood the
        // AOF and the sync fan-out for state that expires within one window.
        Command::RlSet(k, limit, window_secs) => {
            let limit_s = limit.to_string();
            let window_s = window_secs.to_string();
            Some(resp_push(&[
                b"RLSET",
                k.as_bytes(),
                limit_s.as_bytes(),
                window_s.as_bytes(),
            ]))
        }

        // Pub/Sub and transactions carry no store state — no broadcast needed.
        _ => None,
    }
}

pub(crate) fn format_f64_score(s: f64) -> String {
    if s == f64::INFINITY {
        "inf".into()
    } else if s == f64::NEG_INFINITY {
        "-inf".into()
    } else if s.fract() == 0.0 && s.abs() < 1e15 {
        format!("{}", s as i64)
    } else {
        format!("{}", s)
    }
}

/// `broadcast_for` emits one buffer that the AOF, the replication log and the
/// browser sync fan-out all consume. It used to propagate *relative* TTLs
/// (`PX 5000`), so each consumer re-based the deadline onto its own clock at its
/// own arrival time and a key's lifetime silently restarted on every hop — most
/// visibly at AOF replay, where a long-dead key came back with a full fresh TTL.
///
/// These tests pin the replacement contract: relative in, absolute out.
#[cfg(test)]
mod expiry_propagation_tests {
    use super::*;
    use core_engine::cmd::SetOptions;
    use core_engine::store::KeyValueStore;

    fn tmp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("recached_test_{name}_{}", std::process::id()))
    }

    /// Decode a broadcast frame into its argument strings.
    fn parts(frame: &[u8]) -> Vec<String> {
        let (v, n) = Value::parse(frame).expect("frame must parse");
        assert_eq!(n, frame.len(), "frame must be exactly one value");
        let items = match v {
            Value::Push(items) | Value::Array(Some(items)) => items,
            other => panic!("expected an aggregate, got {other:?}"),
        };
        items
            .into_iter()
            .map(|i| match i {
                Value::BulkString(Some(b)) => String::from_utf8_lossy(&b).into_owned(),
                other => panic!("expected a bulk string, got {other:?}"),
            })
            .collect()
    }

    fn frame_of(cmd: &Command, now_ms: u64) -> Vec<String> {
        let f = broadcast_for(cmd, &Value::SimpleString("OK".into()), now_ms)
            .expect("command must propagate");
        parts(&f)
    }

    const NOW: u64 = 1_700_000_000_000;

    #[test]
    fn relative_set_expiries_propagate_as_an_absolute_deadline() {
        let set = |exp| {
            Command::Set(
                "k".into(),
                "v".into(),
                SetOptions {
                    expiry: Some(exp),
                    ..Default::default()
                },
            )
        };
        // EX seconds and PX milliseconds both land on the same instant.
        for (cmd, want) in [
            (set(SetExpiry::Ex(5)), NOW + 5_000),
            (set(SetExpiry::Px(1_500)), NOW + 1_500),
            (Command::SetEx("k".into(), 5, "v".into()), NOW + 5_000),
            (Command::PSetEx("k".into(), 1_500, "v".into()), NOW + 1_500),
        ] {
            let p = frame_of(&cmd, NOW);
            assert_eq!(p[0], "SET", "{p:?}");
            assert_eq!(
                p[3], "PXAT",
                "a relative TTL must not reach the log as PX: {p:?}"
            );
            assert_eq!(p[4], want.to_string(), "{p:?}");
        }
    }

    #[test]
    fn relative_expire_commands_propagate_as_an_absolute_deadline() {
        for (cmd, want) in [
            (Command::Expire("k".into(), 30), NOW + 30_000),
            (Command::PExpire("k".into(), 250), NOW + 250),
        ] {
            let f = broadcast_for(&cmd, &Value::Integer(1), NOW).expect("must propagate");
            let p = parts(&f);
            assert_eq!(p[0], "PEXPIREAT", "{p:?}");
            assert_eq!(p[2], want.to_string(), "{p:?}");
        }
    }

    #[test]
    fn absolute_expiries_are_still_passed_through_unchanged() {
        // These arms were already correct; the rewrite must not double-convert
        // them by adding `now` to a stamp that is already absolute.
        let set = |exp| {
            Command::Set(
                "k".into(),
                "v".into(),
                SetOptions {
                    expiry: Some(exp),
                    ..Default::default()
                },
            )
        };
        let p = frame_of(&set(SetExpiry::Pxat(999)), NOW);
        assert_eq!((p[3].as_str(), p[4].as_str()), ("PXAT", "999"), "{p:?}");
        let p = frame_of(&set(SetExpiry::Exat(999)), NOW);
        assert_eq!((p[3].as_str(), p[4].as_str()), ("PXAT", "999000"), "{p:?}");

        let f = broadcast_for(
            &Command::PExpireAt("k".into(), 999),
            &Value::Integer(1),
            NOW,
        )
        .expect("must propagate");
        assert_eq!(parts(&f)[2], "999");
        let f = broadcast_for(&Command::ExpireAt("k".into(), 999), &Value::Integer(1), NOW)
            .expect("must propagate");
        assert_eq!(parts(&f)[2], "999000");
    }

    #[test]
    fn a_write_without_an_expiry_still_carries_none() {
        // A plain SET clears any TTL, and KEEPTTL defers to whatever the
        // receiving store already holds — neither may gain a deadline.
        let p = frame_of(
            &Command::Set("k".into(), "v".into(), SetOptions::default()),
            NOW,
        );
        assert_eq!(p, vec!["SET", "k", "v"], "{p:?}");

        let p = frame_of(
            &Command::Set(
                "k".into(),
                "v".into(),
                SetOptions {
                    expiry: Some(SetExpiry::KeepTtl),
                    ..Default::default()
                },
            ),
            NOW,
        );
        assert_eq!(p, vec!["SET", "k", "v", "KEEPTTL"], "{p:?}");
    }

    #[test]
    fn the_propagated_frame_is_a_command_the_replay_path_can_parse() {
        // The frame is fed straight back through `Command::from_value` on AOF
        // replay and on replicas, so an encoding no parser accepts would be a
        // silent data-loss bug rather than a compile error.
        for cmd in [
            Command::Set(
                "k".into(),
                "v".into(),
                SetOptions {
                    expiry: Some(SetExpiry::Ex(5)),
                    ..Default::default()
                },
            ),
            Command::SetEx("k".into(), 5, "v".into()),
            Command::PSetEx("k".into(), 5_000, "v".into()),
        ] {
            let f = broadcast_for(&cmd, &Value::SimpleString("OK".into()), NOW).unwrap();
            let (v, _) = Value::parse(&f).unwrap();
            let arr = match v {
                Value::Push(i) | Value::Array(Some(i)) => Value::Array(Some(i)),
                other => panic!("unexpected {other:?}"),
            };
            let parsed = Command::from_value(arr).expect("replay must parse the frame");
            assert!(
                matches!(&parsed, Command::Set(_, _, o) if matches!(o.expiry, Some(SetExpiry::Pxat(_)))),
                "replayed command lost its absolute deadline: {parsed:?}"
            );
        }

        let f = broadcast_for(&Command::Expire("k".into(), 5), &Value::Integer(1), NOW).unwrap();
        let (v, _) = Value::parse(&f).unwrap();
        let arr = match v {
            Value::Push(i) => Value::Array(Some(i)),
            other => panic!("unexpected {other:?}"),
        };
        assert!(matches!(
            Command::from_value(arr).unwrap(),
            Command::PExpireAt(_, _)
        ));
    }

    /// The property the whole change exists for: the deadline is a point in
    /// time, so *when* the frame is applied cannot change *when* it expires.
    #[test]
    fn replaying_the_same_write_later_does_not_extend_the_key() {
        let cmd = Command::SetEx("k".into(), 60, "v".into());
        let frame = broadcast_for(&cmd, &Value::SimpleString("OK".into()), NOW).unwrap();

        // Same write, propagated a full hour later, is a *different* deadline —
        // but any single frame carries exactly one, whenever it is applied.
        let later =
            broadcast_for(&cmd, &Value::SimpleString("OK".into()), NOW + 3_600_000).unwrap();
        assert_ne!(frame, later);
        assert_eq!(parts(&frame)[4], (NOW + 60_000).to_string());
        assert_eq!(parts(&later)[4], (NOW + 3_600_000 + 60_000).to_string());
    }

    /// The bug, end to end through the real AOF path: a key whose deadline has
    /// already passed must stay dead when the log is replayed.
    #[tokio::test]
    async fn an_expired_key_is_not_resurrected_by_aof_replay() {
        let path = tmp_path("expiry_replay.aof");
        let _ = tokio::fs::remove_file(&path).await;
        let aof = AofWriter::open(path.clone(), AofSync::No).await.unwrap();

        // A write whose 5-second TTL elapsed long ago — the shape of any
        // short-lived key written before a restart that outlasted it.
        let long_ago = now_unix_ms() - 3_600_000;
        let frame = broadcast_for(
            &Command::SetEx("session:revoked".into(), 5, "tok".into()),
            &Value::SimpleString("OK".into()),
            long_ago,
        )
        .unwrap();
        aof.append(&frame).await.unwrap();
        // A live key, to prove replay still works at all.
        let live = broadcast_for(
            &Command::SetEx("session:live".into(), 600, "tok".into()),
            &Value::SimpleString("OK".into()),
            now_unix_ms(),
        )
        .unwrap();
        aof.append(&live).await.unwrap();
        aof.flush().await.unwrap();

        let store = KeyValueStore::new();
        assert_eq!(replay_aof(&store, &path).await.unwrap(), 2);

        assert_eq!(
            store.execute(Command::Get("session:revoked".into())),
            Value::BulkString(None),
            "a key dead for an hour was resurrected by replay"
        );
        assert_eq!(
            store.execute(Command::Exists(vec!["session:revoked".into()])),
            Value::Integer(0)
        );
        assert_eq!(
            store.execute(Command::Ttl("session:revoked".into())),
            Value::Integer(-2)
        );
        // ...while the key that had not expired survives with its remaining TTL.
        assert_eq!(
            store.execute(Command::Get("session:live".into())),
            Value::BulkString(Some(b"tok".to_vec()))
        );
        assert!(
            matches!(
                store.execute(Command::Ttl("session:live".into())),
                Value::Integer(n) if (0..=600).contains(&n)
            ),
            "a live key must keep its original deadline, not gain a fresh one"
        );

        let _ = tokio::fs::remove_file(&path).await;
    }

    /// EXPIRE has the same shape as SET..EX and the same failure mode.
    #[tokio::test]
    async fn an_elapsed_expire_does_not_extend_the_key_on_replay() {
        let path = tmp_path("expiry_replay_expire.aof");
        let _ = tokio::fs::remove_file(&path).await;
        let aof = AofWriter::open(path.clone(), AofSync::No).await.unwrap();

        aof.append(b">3\r\n$3\r\nSET\r\n$1\r\nk\r\n$1\r\nv\r\n")
            .await
            .unwrap();
        let long_ago = now_unix_ms() - 3_600_000;
        let frame = broadcast_for(
            &Command::Expire("k".into(), 30),
            &Value::Integer(1),
            long_ago,
        )
        .unwrap();
        aof.append(&frame).await.unwrap();
        aof.flush().await.unwrap();

        let store = KeyValueStore::new();
        replay_aof(&store, &path).await.unwrap();
        assert_eq!(
            store.execute(Command::Get("k".into())),
            Value::BulkString(None),
            "an EXPIRE that elapsed before the restart was re-armed by replay"
        );

        let _ = tokio::fs::remove_file(&path).await;
    }
}

// ── Transaction abort ─────────────────────────────────────────────────────────

/// Counters propagate by value, but must not clear the key's deadline.
///
/// `INCR` is replicated as `SET key <new value>` so a replica that missed a
/// frame converges on the primary's number rather than compounding its own —
/// but a bare `SET` also clears the TTL, and Redis's `INCR` leaves it alone.
/// The single most common expiring-counter idiom, `INCR key` + `EXPIRE key
/// window`, therefore replayed as a key with *no* expiry: the rate-limit
/// bucket, the per-minute quota and the retry counter all became permanent on
/// the replica, in the AOF and in every synced browser, and the window never
/// reset because the key it keyed on never went away.
#[cfg(test)]
mod counter_ttl_propagation_tests {
    use super::*;
    use core_engine::cmd::{SetExpiry, SetOptions};
    use core_engine::store::KeyValueStore;

    fn tmp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("recached_test_{name}_{}", std::process::id()))
    }

    fn frame_args(cmd: &Command, response: &Value) -> Vec<String> {
        let f = broadcast_for(cmd, response, 0).expect("counter must propagate");
        let (v, _) = Value::parse(&f).unwrap();
        let items = match v {
            Value::Push(i) | Value::Array(Some(i)) => i,
            other => panic!("unexpected {other:?}"),
        };
        items
            .into_iter()
            .map(|i| match i {
                Value::BulkString(Some(b)) => String::from_utf8_lossy(&b).into_owned(),
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[test]
    fn every_counter_propagates_with_keepttl() {
        for cmd in [
            Command::Incr("c".into()),
            Command::Decr("c".into()),
            Command::IncrBy("c".into(), 5),
            Command::DecrBy("c".into(), 5),
        ] {
            let args = frame_args(&cmd, &Value::Integer(7));
            assert_eq!(
                args,
                vec!["SET", "c", "7", "KEEPTTL"],
                "{} must not clear the key's deadline",
                command_name(&cmd)
            );
        }
    }

    #[test]
    fn a_counter_that_did_not_run_still_propagates_nothing() {
        // INCR on a non-numeric value errors and changes nothing; replaying a
        // SET for it would invent a value the primary never stored.
        assert!(
            broadcast_for(
                &Command::Incr("c".into()),
                &Value::Error("ERR not an integer".into()),
                0
            )
            .is_none()
        );
    }

    #[test]
    fn getset_still_clears_the_ttl() {
        // GETSET *does* clear the TTL in Redis, so it must keep propagating a
        // bare SET — the KEEPTTL change applies to counters only.
        let args = frame_args(
            &Command::GetSet("k".into(), "v".into()),
            &Value::BulkString(None),
        );
        assert_eq!(args, vec!["SET", "k", "v"]);
    }

    /// The behaviour, through the real replay path: `INCR` + `EXPIRE` survives
    /// a restart still holding its deadline.
    #[tokio::test]
    async fn an_expiring_counter_keeps_its_deadline_across_aof_replay() {
        let path = tmp_path("counter_ttl.aof");
        let _ = tokio::fs::remove_file(&path).await;
        let aof = AofWriter::open(path.clone(), AofSync::No).await.unwrap();

        // The rate-limiter idiom: create with a window, then count into it.
        let now = now_unix_ms();
        for frame in [
            broadcast_for(
                &Command::Set(
                    "rate:user:42".into(),
                    "1".into(),
                    SetOptions {
                        expiry: Some(SetExpiry::Ex(60)),
                        ..Default::default()
                    },
                ),
                &Value::SimpleString("OK".into()),
                now,
            ),
            broadcast_for(
                &Command::Incr("rate:user:42".into()),
                &Value::Integer(2),
                now,
            ),
            broadcast_for(
                &Command::Incr("rate:user:42".into()),
                &Value::Integer(3),
                now,
            ),
        ] {
            aof.append(&frame.unwrap()).await.unwrap();
        }
        aof.flush().await.unwrap();

        let store = KeyValueStore::new();
        replay_aof(&store, &path).await.unwrap();

        assert_eq!(
            store.execute(Command::Get("rate:user:42".into())),
            Value::BulkString(Some(b"3".to_vec())),
            "the counter must converge on the primary's value"
        );
        match store.execute(Command::Ttl("rate:user:42".into())) {
            Value::Integer(n) => assert!(
                (1..=60).contains(&n),
                "the rate-limit window was lost on replay — TTL is {n}, so the \
                 bucket would never reset"
            ),
            other => panic!("expected an integer, got {other:?}"),
        }

        let _ = tokio::fs::remove_file(&path).await;
    }
}

#[cfg(test)]
mod write_outcome_propagation_tests {
    use super::*;
    use core_engine::cmd::{SetOptions, ZAddOptions};
    use core_engine::store::KeyValueStore;

    /// Run `cmd` on `primary` and, if it propagates, on `replica` exactly as a
    /// replica or AOF replay would. Returns whether a frame was emitted.
    fn replicate(primary: &KeyValueStore, replica: &KeyValueStore, cmd: Command) -> bool {
        let response = primary.execute(cmd.clone());
        let Some(frame) = broadcast_for(&cmd, &response, 0) else {
            return false;
        };
        let (value, _) = Value::parse(&frame).expect("frame parses");
        let Value::Push(items) = value else {
            panic!("expected a push frame, got {value:?}");
        };
        let replayed = Command::from_value(Value::Array(Some(items))).expect("frame is a command");
        let replayed_response = replica.execute(replayed);
        assert!(
            !matches!(replayed_response, Value::Error(_)),
            "replaying {cmd:?} failed: {replayed_response:?}"
        );
        true
    }

    fn pair() -> (KeyValueStore, KeyValueStore) {
        (KeyValueStore::new(), KeyValueStore::new())
    }

    /// A refused write left the primary unchanged, and replaying it fails —
    /// which a replica treats as a broken stream and AOF replay as corruption
    /// that stops the server from starting.
    #[test]
    fn a_refused_write_is_never_propagated() {
        let store = KeyValueStore::new();
        store.execute(Command::Set(
            "k".into(),
            b"v".to_vec(),
            SetOptions::default(),
        ));
        for cmd in [
            Command::HSet("k".into(), vec![("f".into(), b"v".to_vec())]),
            Command::RPush("k".into(), vec![b"v".to_vec()]),
            Command::LPush("k".into(), vec![b"v".to_vec()]),
            Command::SAdd("k".into(), vec!["m".into()]),
            Command::ZAdd("k".into(), ZAddOptions::default(), vec![(1.0, "m".into())]),
        ] {
            let response = store.execute(cmd.clone());
            assert!(
                matches!(response, Value::Error(_)),
                "{cmd:?} → {response:?}"
            );
            assert!(
                broadcast_for(&cmd, &response, 0).is_none(),
                "{cmd:?} was emitted"
            );
        }

        let capped = KeyValueStore::with_config(
            None,
            Some(70),
            core_engine::store::EvictionPolicy::NoEviction,
        );
        capped.execute(Command::Set(
            "k".into(),
            b"old".to_vec(),
            SetOptions::default(),
        ));
        let oversized = Command::Set(
            "k".into(),
            b"value-too-large".to_vec(),
            SetOptions::default(),
        );
        let response = capped.execute(oversized.clone());
        assert!(
            matches!(response, Value::Error(_)),
            "expected OOM, got {response:?}"
        );
        assert!(broadcast_for(&oversized, &response, 0).is_none());
    }

    #[test]
    fn set_get_with_a_condition_propagates_only_when_it_wrote() {
        let get = |condition| SetOptions {
            get: true,
            condition: Some(condition),
            ..Default::default()
        };
        let (primary, replica) = pair();
        for store in [&primary, &replica] {
            store.execute(Command::Set(
                "k".into(),
                b"old".to_vec(),
                SetOptions::default(),
            ));
        }
        // NX on a present key: no write, no frame.
        let nx = Command::Set("k".into(), b"new".to_vec(), get(SetCondition::Nx));
        assert!(!replicate(&primary, &replica, nx));
        // XX on a missing key: no write, no frame.
        let xx = Command::Set("absent".into(), b"new".to_vec(), get(SetCondition::Xx));
        assert!(!replicate(&primary, &replica, xx));
        // The writing forms still propagate.
        let nx = Command::Set("fresh".into(), b"v".to_vec(), get(SetCondition::Nx));
        assert!(replicate(&primary, &replica, nx));
        let xx = Command::Set("k".into(), b"newer".to_vec(), get(SetCondition::Xx));
        assert!(replicate(&primary, &replica, xx));
        for key in ["k", "absent", "fresh"] {
            assert_eq!(
                primary.execute(Command::Get(key.into())),
                replica.execute(Command::Get(key.into())),
                "{key} diverged"
            );
        }
    }

    #[test]
    fn conditional_zadd_leaves_replicas_where_the_primary_is() {
        let (primary, replica) = pair();
        for store in [&primary, &replica] {
            store.execute(Command::ZAdd(
                "z".into(),
                ZAddOptions::default(),
                vec![(2.0, "m".into())],
            ));
        }
        let cases = [
            (
                ZAddOptions {
                    gt: true,
                    ..Default::default()
                },
                1.0,
            ),
            (
                ZAddOptions {
                    lt: true,
                    ..Default::default()
                },
                3.0,
            ),
            (
                ZAddOptions {
                    gt: true,
                    ..Default::default()
                },
                5.0,
            ),
            (
                ZAddOptions {
                    lt: true,
                    incr: true,
                    ..Default::default()
                },
                1.0,
            ),
            (
                ZAddOptions {
                    gt: true,
                    incr: true,
                    ..Default::default()
                },
                1.0,
            ),
        ];
        for (options, score) in cases {
            let cmd = Command::ZAdd("z".into(), options, vec![(score, "m".into())]);
            replicate(&primary, &replica, cmd.clone());
            assert_eq!(
                primary.execute(Command::ZScore("z".into(), "m".into())),
                replica.execute(Command::ZScore("z".into(), "m".into())),
                "{cmd:?} diverged"
            );
        }
    }

    #[test]
    fn an_aborted_zadd_incr_is_not_propagated() {
        let (primary, replica) = pair();
        let nx_incr = ZAddOptions {
            condition: Some(ZAddCondition::Nx),
            incr: true,
            ..Default::default()
        };
        primary.execute(Command::ZAdd(
            "z".into(),
            ZAddOptions::default(),
            vec![(1.0, "m".into())],
        ));
        let cmd = Command::ZAdd("z".into(), nx_incr, vec![(1.0, "m".into())]);
        assert!(!replicate(&primary, &replica, cmd));
    }
}

// ── Port configuration ────────────────────────────────────────────────────────
