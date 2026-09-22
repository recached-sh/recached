//! Turning an executed command into the frame that reaches the AOF, replicas
//! and browser sync clients — and the watch notifications that go with it.

use crate::*;

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
    )
}

// ── Save conditions ───────────────────────────────────────────────────────────

/// Extract the key(s) that `cmd` writes to, without inspecting the response.
/// Used together with `broadcast_for()` — only call this when `broadcast_for`
/// already confirmed a mutation occurred.
pub(crate) fn primary_keys(cmd: &Command) -> Vec<String> {
    if let Command::Dedup(_, _, inner) = cmd {
        return primary_keys(inner);
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
        | Command::JSet(k, _, _)
        | Command::JMerge(k, _) => vec![k.clone()],
        Command::Del(keys) | Command::Unlink(keys) => keys.clone(),
        Command::MSet(pairs) => pairs.iter().map(|(k, _)| k.clone()).collect(),
        Command::Rename(src, dst) | Command::SMove(src, dst, _) => {
            vec![src.clone(), dst.clone()]
        }
        _ => vec![],
    }
}

/// Keys whose state can affect a write's result.
///
/// This is deliberately broader than [`primary_keys`]: the source sets of a
/// `*STORE` command are read while computing the destination, so they must be
/// in the same ordering domain as that destination. `FLUSHDB` is represented
/// by `None` because it conflicts with every key.
pub(crate) fn ordering_keys(cmd: &Command) -> Option<Vec<String>> {
    if let Command::Dedup(_, _, inner) = cmd {
        return ordering_keys(inner);
    }
    match cmd {
        Command::FlushDb => None,
        Command::SInterStore(dst, sources)
        | Command::SUnionStore(dst, sources)
        | Command::SDiffStore(dst, sources) => {
            let mut keys = Vec::with_capacity(sources.len() + 1);
            keys.push(dst.clone());
            keys.extend(sources.iter().cloned());
            Some(keys)
        }
        _ => Some(primary_keys(cmd)),
    }
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
            let happened = opts.get || !matches!(response, Value::BulkString(None));
            if happened {
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
    tx: &broadcast::Sender<SyncMsg>,
    origin: u64,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
) -> Value {
    let claimable_members = claimable_eadd_members(cmd, state, store);
    let (response, evicted) = execute_and_record_with_evictions(store, cmd.clone());
    apply_write_effects(cmd, &response, tx, origin, state, watch_registry, store).await;
    apply_eviction_effects(&evicted, tx, origin, state, watch_registry, store).await;
    reconcile_ephemeral_state(cmd, &response, &claimable_members, origin, state);
    response
}

/// Execute one write while its key-order guards remain held through durable
/// logging and fan-out. This makes the store commit order the observable AOF,
/// replica, watcher, and browser-sync order for every pair of conflicting
/// writes.
pub(crate) async fn execute_ordered_write(
    cmd: &Command,
    tx: &broadcast::Sender<SyncMsg>,
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
        && state.dedup_is_duplicate(client, id)
    {
        record_command(command_name(effective));
        return Value::SimpleString("DUP".to_string());
    }

    // Capacity eviction may choose any key, so capped stores already serialize
    // their core writes and reserve every propagation ordering domain here.
    // Uncapped stores retain per-key concurrency.
    let _guards = if store.has_capacity_limits() {
        state.replicas.lock_all_writes().await
    } else {
        state
            .replicas
            .lock_commands(std::slice::from_ref(effective))
            .await
    };
    let response = execute_locked_write(effective, tx, origin, state, watch_registry, store).await;
    if !matches!(response, Value::Error(_))
        && let Some((client, id)) = dedup
    {
        state.commit_dedup(client, id);
    }
    response
}

/// Release one connection's ephemeral holds while keeping the affected keys
/// locked from the last-holder decision through the final DEL/SREM. This closes
/// the disconnect race where a concurrent writer could recreate a key between
/// cleanup's cardinality check and delete.
pub(crate) async fn release_ephemeral_state(
    conn_id: u64,
    tx: &broadcast::Sender<SyncMsg>,
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
pub(crate) async fn apply_eviction_effects(
    evicted: &[String],
    tx: &broadcast::Sender<SyncMsg>,
    origin: u64,
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
            origin,
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
            dedup_dirty: AtomicBool::new(false),
            dedup_order: tokio::sync::Mutex::new(()),
            save_lock: tokio::sync::Mutex::new(()),
            persistence_healthy: AtomicBool::new(true),
            persistence_failures: AtomicU64::new(0),
        };
        let store = KeyValueStore::with_config(Some(1), None, EvictionPolicy::AllKeysRandom);
        let tx = broadcast::channel::<SyncMsg>(8).0;
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
pub(crate) fn encode_notification(notif: &WatchNotif) -> Vec<u8> {
    match &notif.payload {
        NotifPayload::Full(value) => encode_keychange(&notif.key, value),
        NotifPayload::Delta(delta) => encode_keydelta(&notif.key, delta),
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
    // Fetch current values from DashMap *before* acquiring the registry lock
    // to avoid holding two locks simultaneously.
    let key_values: Vec<(String, Value, Option<KeyDelta>)> = keys
        .iter()
        .map(|k| {
            let delta = delta
                .as_ref()
                .filter(|(delta_key, _)| delta_key == k)
                .map(|(_, delta)| delta.clone());
            (k.clone(), store.get_current(k), delta)
        })
        .collect();
    fan_out(registry, &key_values).await;
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
    let key_values: Vec<(String, Value, Option<KeyDelta>)> = keys
        .iter()
        .map(|k| (k.clone(), Value::BulkString(None), None))
        .collect();
    fan_out(registry, &key_values).await;
}

/// Deliver one batch of `(key, current value)` pairs to exact-key watchers and
/// to every live query whose pattern matches.
async fn fan_out(registry: &WatchRegistry, key_values: &[(String, Value, Option<KeyDelta>)]) {
    // A connection may WATCH a key and QSUB one or more overlapping patterns.
    // Deliver each changed key once per connection: deltas such as APPEND and
    // INCR are not idempotent when clients apply them to a local replica.
    let mut delivered: HashMap<String, HashSet<u64>> = HashMap::new();
    if registry.watched_keys.load(Ordering::Relaxed) > 0 {
        let mut reg = registry.map.lock().await;
        for (key, value, delta) in key_values {
            if let Some(subs) = reg.get_mut(key) {
                subs.retain(|sub| {
                    let alive = sub.notify(key, value, delta.as_ref());
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
    // Live queries: any registered glob pattern matching a touched key gets
    // the same keychange notification.
    if registry.watched_patterns.load(Ordering::Relaxed) > 0 {
        let mut pats = registry.patterns.lock().await;
        let mut emptied = false;
        for (pattern, subs) in pats.iter_mut() {
            for (key, value, delta) in key_values {
                if core_engine::store::glob_match(pattern, key) {
                    subs.retain(|sub| {
                        if delivered
                            .get(key)
                            .is_some_and(|connections| connections.contains(&sub.conn_id))
                        {
                            return true;
                        }
                        let alive = sub.notify(key, value, delta.as_ref());
                        if alive {
                            delivered
                                .entry(key.clone())
                                .or_default()
                                .insert(sub.conn_id);
                        }
                        alive
                    });
                }
            }
            emptied |= subs.is_empty();
        }
        if emptied {
            pats.retain(|_, subs| !subs.is_empty());
        }
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
        for key in &watched_before {
            if let Some(subs) = reg.get_mut(key) {
                subs.retain(|sub| sub.notify(key, &Value::BulkString(None), None));
            }
        }
        registry.sync_len(&reg);
    }
    if registry.watched_patterns.load(Ordering::Relaxed) > 0 {
        let mut pats = registry.patterns.lock().await;
        let mut emptied = false;
        for (pattern, subs) in pats.iter_mut() {
            let sentinel = pattern.clone();
            subs.retain(|sub| sub.notify(&sentinel, &Value::BulkString(None), None));
            emptied |= subs.is_empty();
        }
        if emptied {
            pats.retain(|_, subs| !subs.is_empty());
        }
        registry.sync_patterns_len(&pats);
    }
}

/// Post-write fan-out shared by the TCP and WS command paths: WebSocket sync
/// broadcast, AOF/replication log, and watch notifications. Structured so that
/// with no WS clients, no replicas, no AOF, and no watched keys — the common
/// standalone-server case — a write costs zero locks and zero allocations here.
pub(crate) async fn apply_write_effects(
    cmd: &Command,
    response: &Value,
    tx: &broadcast::Sender<SyncMsg>,
    origin: u64,
    state: &ServerState,
    watch_registry: &WatchRegistry,
    store: &KeyValueStore,
) {
    let has_ws = tx.receiver_count() > 0;
    let needs_log = state.needs_write_log();
    let has_watch = !watch_registry.is_empty();
    if !has_ws && !needs_log && !has_watch {
        return;
    }
    // Read once, after the early-out above, so a standalone server with nothing
    // listening still pays no clock read per write.
    let Some(msg) = broadcast_for(cmd, response, now_unix_ms()) else {
        return;
    };
    if needs_log {
        state.on_write(&msg).await;
    }
    if has_watch {
        if matches!(cmd, Command::FlushDb) {
            // primary_keys() is empty for FLUSHDB, so the generic notifier has
            // nothing to announce — subscribers would silently miss the wipe.
            let watched: Vec<String> = {
                let reg = watch_registry.map.lock().await;
                reg.keys().cloned().collect()
            };
            notify_flushdb(watch_registry, watched).await;
        } else {
            notify_watchers(watch_registry, cmd, store).await;
        }
    }
    if has_ws {
        let _ = tx.send(Arc::new(SyncPush {
            origin,
            keys: primary_keys(cmd),
            resp: msg,
        }));
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
pub(crate) fn resp_push(parts: &[&[u8]]) -> Vec<u8> {
    let mut out = format!(">{}\r\n", parts.len()).into_bytes();
    for part in parts {
        out.extend_from_slice(format!("${}\r\n", part.len()).as_bytes());
        out.extend_from_slice(part);
        out.extend_from_slice(b"\r\n");
    }
    out
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
pub(crate) fn broadcast_for(cmd: &Command, response: &Value, now_ms: u64) -> Option<Vec<u8>> {
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
            // Without GET: nil response means NX/XX condition failed — don't broadcast.
            // With GET: nil means key didn't exist before, but SET still happened.
            let set_happened = opts.get || !matches!(response, Value::BulkString(None));
            if !set_happened {
                return None;
            }
            match &opts.expiry {
                None => Some(resp_push(&[b"SET", k.as_bytes(), v.as_slice()])),
                // Relative → absolute: see the note on `broadcast_for`.
                Some(SetExpiry::Ex(s)) => {
                    let pxat = now_ms.saturating_add(s.saturating_mul(1000)).to_string();
                    Some(resp_push(&[
                        b"SET",
                        k.as_bytes(),
                        v.as_slice(),
                        b"PXAT",
                        pxat.as_bytes(),
                    ]))
                }
                Some(SetExpiry::Px(ms)) => {
                    let pxat = now_ms.saturating_add(*ms).to_string();
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
            let mut parts: Vec<&[u8]> = vec![b"MSET"];
            let flat: Vec<Vec<u8>> = pairs
                .iter()
                .flat_map(|(k, v)| [k.as_bytes().to_vec(), v.clone()])
                .collect();
            let flat_refs: Vec<&[u8]> = flat.iter().map(|s| s.as_slice()).collect();
            parts.extend_from_slice(&flat_refs);
            Some(resp_push(&parts))
        }
        Command::SetNx(k, v) => match response {
            Value::Integer(1) => Some(resp_push(&[b"SET", k.as_bytes(), v.as_slice()])),
            _ => None,
        },
        Command::SetEx(k, secs, v) => {
            let pxat = now_ms.saturating_add(secs.saturating_mul(1000)).to_string();
            Some(resp_push(&[
                b"SET",
                k.as_bytes(),
                v.as_slice(),
                b"PXAT",
                pxat.as_bytes(),
            ]))
        }
        Command::PSetEx(k, ms, v) => {
            let pxat = now_ms.saturating_add(*ms).to_string();
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
                    let s = n.to_string();
                    Some(resp_push(&[b"SET", k.as_bytes(), s.as_bytes(), b"KEEPTTL"]))
                }
                _ => None,
            }
        }
        // Relative → absolute: see the note on `broadcast_for`.
        Command::Expire(k, secs) => match response {
            Value::Integer(1) => {
                let ts = now_ms.saturating_add(secs.saturating_mul(1000)).to_string();
                Some(resp_push(&[b"PEXPIREAT", k.as_bytes(), ts.as_bytes()]))
            }
            _ => None,
        },
        Command::PExpire(k, ms) => match response {
            Value::Integer(1) => {
                let ts = now_ms.saturating_add(*ms).to_string();
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
        Command::Rename(src, dst) => match response {
            Value::Error(_) => None,
            _ => Some(resp_push(&[b"RENAME", src.as_bytes(), dst.as_bytes()])),
        },

        // ── Hash ─────────────────────────────────────────────────────────────
        Command::HSet(k, pairs) => {
            let mut parts: Vec<Vec<u8>> = vec![b"HSET".to_vec(), k.as_bytes().to_vec()];
            for (f, v) in pairs {
                parts.push(f.as_bytes().to_vec());
                parts.push(v.clone());
            }
            let refs: Vec<&[u8]> = parts.iter().map(|s| s.as_slice()).collect();
            Some(resp_push(&refs))
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
            let mut parts: Vec<String> = vec!["ZADD".into(), k.clone()];
            if let Some(cond) = &opts.condition {
                parts.push(match cond {
                    ZAddCondition::Nx => "NX".into(),
                    ZAddCondition::Xx => "XX".into(),
                });
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

// ── Port configuration ────────────────────────────────────────────────────────
