//! The shared server state every connection writes through: AOF handle,
//! replica registry, replica/primary role, and write de-duplication.

use crate::*;

pub(crate) struct ServerState {
    pub(crate) snap: Arc<SnapshotConfig>,
    pub(crate) aof: Option<Arc<AofWriter>>,
    pub(crate) replicas: ReplRegistry,
    /// true = currently acting as a read-only replica
    pub(crate) is_replica: std::sync::atomic::AtomicBool,
    /// Duplicate-suppression bookkeeping for DEDUP-wrapped writes, per client
    /// id: the highest id applied, when the client was last seen, and its
    /// recent permanent refusals. Clients send monotonically increasing ids
    /// and replay in order, so one high-water mark per client suffices for
    /// writes that were applied; the refusals are what a mark alone cannot
    /// say (see [`DedupMark`]). Marks are committed only after the wrapped
    /// write succeeds, recorded in the AOF with the write itself, and
    /// persisted with snapshot checkpoints.
    pub(crate) dedup: std::sync::Mutex<HashMap<String, DedupMark>>,
    /// Ephemeral (`ESET`) keys → every connection currently holding them.
    ///
    /// A *set* of holders rather than one owner, because presence is the case
    /// this exists for and a user has more than one tab. With a single owner,
    /// each `ESET` transferred it, so closing the **most recent** tab deleted
    /// the key while every earlier tab was still open — the user went offline
    /// while still looking at the page. The key now outlives every holder and
    /// is removed when the last one goes.
    pub(crate) ephemeral: std::sync::Mutex<HashMap<String, HashSet<u64>>>,
    /// Ephemeral (`EADD`) set memberships: set key → member → the connections
    /// holding it. Same lifetime rule as `ephemeral`, one level down: a member
    /// survives while any connection that added it is open, and the set is
    /// deleted once it empties.
    ///
    /// Nested by set key so a write to a key drops its memberships with one
    /// `remove`. Keyed by `(key, member)` it had to be scanned whole, under the
    /// mutex, on every write: with 20k members held, `SET` on unrelated keys
    /// ran at ~1,200/s (6G, `benchmarks/v0.6.0/6g-cpu-c4`).
    pub(crate) ephemeral_members: std::sync::Mutex<EphemeralMembers>,
    /// Set the first time a connection claims an ephemeral key or member;
    /// never cleared. Every write calls [`Self::forget_ephemeral_keys`], which
    /// took both registry mutexes — process-wide, so every worker's every
    /// write contended on them — and scanned both maps, even on a server that
    /// has never seen `ESET` or `EADD`. With this unset there is nothing to
    /// forget, and the write path touches neither lock.
    pub(crate) ephemeral_in_use: AtomicBool,
    /// Set when a dedup high-water mark advances; cleared once persisted.
    pub(crate) dedup_dirty: std::sync::atomic::AtomicBool,
    /// Serializes DEDUP checks through successful execution and mark commit.
    pub(crate) dedup_order: tokio::sync::Mutex<()>,
    /// Serializes SAVE, BGSAVE, autosave, and shutdown checkpoints.
    pub(crate) save_lock: tokio::sync::Mutex<()>,
    /// False after a persistence failure. Client writes remain disabled until
    /// an operator completes a successful SAVE or restarts with healthy files.
    pub(crate) persistence_healthy: AtomicBool,
    pub(crate) persistence_failures: AtomicU64,
}

/// Set key → member → connections holding that membership.
pub(crate) type EphemeralMembers = HashMap<String, HashMap<String, HashSet<u64>>>;

/// A connection-scoped claim map's lock, recovered if a panic poisoned it
/// (6O-G). It used to be skipped on poison — `if let Ok(..) = lock()` —
/// so one panic while it was held ended ephemeral tracking for good: keys
/// were never deleted on disconnect again. The maps hold plain claims,
/// consistent after any single insert or remove, so the data is usable.
/// Always `Ok`, so the call sites keep their shape.
#[allow(clippy::result_unit_err)]
fn ephemeral_lock<T>(map: &std::sync::Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, ()> {
    Ok(ephemeral_guard(map))
}

/// [`ephemeral_lock`]'s guard itself; clears the poison too, so a recovered
/// map reads as healthy (review C-3: snapshots `.expect`ed it unpoisoned).
fn ephemeral_guard<T>(map: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    map.lock().unwrap_or_else(|poisoned| {
        map.clear_poison();
        poisoned.into_inner()
    })
}

impl ServerState {
    /// Record `conn_id` as a holder of an ephemeral key. Repeating an `ESET`
    /// from the same connection is idempotent.
    pub(crate) fn claim_ephemeral(&self, key: &str, conn_id: u64) {
        // Before the claim exists, so any forget ordered after it sees the
        // flag (see `forget_ephemeral_keys`).
        self.ephemeral_in_use.store(true, Ordering::Release);
        if let Ok(mut map) = ephemeral_lock(&self.ephemeral) {
            map.entry(key.to_string()).or_default().insert(conn_id);
        }
    }

    /// Record `conn_id` as a holder of each ephemeral set membership.
    pub(crate) fn claim_ephemeral_members(&self, key: &str, members: &[String], conn_id: u64) {
        self.ephemeral_in_use.store(true, Ordering::Release);
        if let Ok(mut map) = ephemeral_lock(&self.ephemeral_members) {
            let held = map.entry(key.to_string()).or_default();
            for member in members {
                held.entry(member.clone()).or_default().insert(conn_id);
            }
        }
    }

    /// Whether a membership is already connection-scoped. `EADD` may add a
    /// second holder to such a membership, but must never adopt a member that
    /// was created by ordinary `SADD`.
    pub(crate) fn is_ephemeral_member(&self, key: &str, member: &str) -> bool {
        ephemeral_lock(&self.ephemeral_members)
            .is_ok_and(|map| map.get(key).is_some_and(|held| held.contains_key(member)))
    }

    /// Keys and set memberships currently held by `conn_id`, without changing
    /// the registry. Disconnect cleanup uses this to reserve all relevant key
    /// ordering guards before it decides which holds are the last ones.
    pub(crate) fn ephemeral_claims_for(
        &self,
        conn_id: u64,
    ) -> (Vec<String>, Vec<(String, Vec<String>)>) {
        let keys = ephemeral_lock(&self.ephemeral)
            .map(|map| {
                map.iter()
                    .filter(|(_, holders)| holders.contains(&conn_id))
                    .map(|(key, _)| key.clone())
                    .collect()
            })
            .unwrap_or_default();
        let mut members: Vec<(String, Vec<String>)> = Vec::new();
        if let Ok(map) = ephemeral_lock(&self.ephemeral_members) {
            for (key, held) in map.iter() {
                let mine: Vec<String> = held
                    .iter()
                    .filter(|(_, holders)| holders.contains(&conn_id))
                    .map(|(member, _)| member.clone())
                    .collect();
                if !mine.is_empty() {
                    members.push((key.clone(), mine));
                }
            }
        }
        (keys, members)
    }

    pub(crate) fn forget_ephemeral_key(&self, key: &str) {
        if let Ok(mut map) = ephemeral_lock(&self.ephemeral) {
            map.remove(key);
        }
    }

    pub(crate) fn forget_ephemeral_members_for_key(&self, key: &str) {
        if let Ok(mut map) = ephemeral_lock(&self.ephemeral_members) {
            map.remove(key);
        }
    }

    pub(crate) fn forget_all_ephemeral(&self) {
        if let Ok(mut map) = ephemeral_lock(&self.ephemeral) {
            map.clear();
        }
        if let Ok(mut map) = ephemeral_lock(&self.ephemeral_members) {
            map.clear();
        }
    }

    /// A regular write to these keys ends their ephemeral status.
    ///
    /// Called on every write. Skipped without locking until the first claim:
    /// a claim sets `ephemeral_in_use` before it is recorded, and a forget
    /// that must see it is ordered after it by the key's write-order stripe,
    /// which makes the flag visible. A forget racing a claim on no common
    /// lock may miss it — exactly as if it had run first, which the locked
    /// version could also do.
    pub(crate) fn forget_ephemeral_keys<'a>(&self, keys: impl IntoIterator<Item = &'a str>) {
        if !self.ephemeral_in_use.load(Ordering::Acquire) {
            return;
        }
        let keys: Vec<&str> = keys.into_iter().collect();
        if let Ok(mut map) = ephemeral_lock(&self.ephemeral)
            && !map.is_empty()
        {
            // By key, not a scan of every claim: O(keys written).
            for key in &keys {
                map.remove(*key);
            }
        }
        if let Ok(mut map) = ephemeral_lock(&self.ephemeral_members)
            && !map.is_empty()
        {
            for key in &keys {
                map.remove(*key);
            }
        }
    }

    pub(crate) fn forget_ephemeral_members(&self, key: &str, members: &[String]) {
        if let Ok(mut map) = ephemeral_lock(&self.ephemeral_members)
            && let Some(held) = map.get_mut(key)
        {
            for member in members {
                held.remove(member);
            }
            if held.is_empty() {
                map.remove(key);
            }
        }
    }

    /// Ephemeral set memberships `conn_id` was the last holder of, grouped by
    /// set key. Called once when a connection closes.
    pub(crate) fn take_ephemeral_members_for(&self, conn_id: u64) -> Vec<(String, Vec<String>)> {
        let Ok(mut map) = ephemeral_lock(&self.ephemeral_members) else {
            return Vec::new();
        };
        let mut released: Vec<(String, Vec<String>)> = Vec::new();
        map.retain(|key, held| {
            let mut last = Vec::new();
            held.retain(|member, holders| {
                if !holders.remove(&conn_id) || !holders.is_empty() {
                    return true;
                }
                last.push(member.clone());
                false
            });
            if !last.is_empty() {
                released.push((key.clone(), last));
            }
            !held.is_empty()
        });
        released
    }

    /// Snapshot only durable state. Ephemeral keys and memberships are live
    /// connection state and must not be resurrected after a clean restart.
    fn durable_snapshot(&self, store: &KeyValueStore) -> Vec<SnapshotEntry> {
        let ephemeral_keys: HashSet<String> =
            ephemeral_guard(&self.ephemeral).keys().cloned().collect();
        let ephemeral_members: HashMap<String, HashSet<String>> =
            ephemeral_guard(&self.ephemeral_members)
                .iter()
                .map(|(key, held)| (key.clone(), held.keys().cloned().collect()))
                .collect();
        // In the expiry-log format every entry still in the map is kept,
        // expired or not: only the primary decides expiry, and logs a `DEL`
        // when it removes one. Filtering by this node's clock was the one
        // place that clock still decided (6O-H review C, R2-2). Older
        // binaries drop such entries when they load the file.
        let entries = if self.writes_expiry_log() {
            core_engine::store::without_expiry(|| store.snapshot())
        } else {
            store.snapshot()
        };
        entries
            .into_iter()
            .filter_map(|mut entry| {
                if ephemeral_keys.contains(&entry.key) {
                    return None;
                }
                if let SnapshotValue::Set(members) = &mut entry.value {
                    if let Some(ephemeral) = ephemeral_members.get(&entry.key) {
                        members.retain(|member| !ephemeral.contains(member));
                    }
                    if members.is_empty() {
                        return None;
                    }
                }
                Some(entry)
            })
            .collect()
    }

    /// Re-seed the freshly truncated AOF with live ephemeral ownership. The
    /// snapshot deliberately omits this state, but later relative mutations
    /// (notably APPEND) still need replay to know that their key must be
    /// discarded after a crash.
    fn ephemeral_aof_seed(&self, store: &KeyValueStore) -> Vec<Vec<u8>> {
        let mut keys: Vec<String> = ephemeral_guard(&self.ephemeral).keys().cloned().collect();
        keys.sort_unstable();
        let mut frames = Vec::new();
        // A key's deadline goes with it: `EXPIRE` does not end ephemeral
        // status, and a seed without it brought a volatile key back
        // permanent (6O-H review C, M3; pre-existing).
        let deadline = |key: &str| match store.execute(Command::PExpireTime(key.to_string())) {
            Value::Integer(at) if at > 0 => Some(at.to_string()),
            _ => None,
        };
        for key in keys {
            if let Value::BulkString(Some(value)) = store.get_current(&key) {
                frames.push(resp_push(&[b"ESET", key.as_bytes(), value.as_slice()]));
                if let Some(at) = deadline(&key) {
                    frames.push(resp_push(&[b"PEXPIREAT", key.as_bytes(), at.as_bytes()]));
                }
            }
        }

        let mut by_key: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let held: Vec<(String, String)> = ephemeral_guard(&self.ephemeral_members)
            .iter()
            .flat_map(|(key, held)| held.keys().map(move |member| (key.clone(), member.clone())))
            .collect();
        for (key, member) in held {
            if matches!(
                store.execute(Command::SIsMember(key.clone(), member.clone())),
                Value::Integer(1)
            ) {
                by_key.entry(key).or_default().push(member);
            }
        }
        for (key, mut members) in by_key {
            members.sort_unstable();
            let mut parts: Vec<&[u8]> = vec![b"EADD", key.as_bytes()];
            parts.extend(members.iter().map(String::as_bytes));
            frames.push(resp_push(&parts));
            if let Some(at) = deadline(&key) {
                frames.push(resp_push(&[b"PEXPIREAT", key.as_bytes(), at.as_bytes()]));
            }
        }
        frames
    }

    /// Keys `conn_id` was the **last** holder of, removed from the registry.
    /// Called once when a connection closes. A key another connection still
    /// holds is left alone, which is what keeps a second tab online.
    pub(crate) fn take_ephemeral_for(&self, conn_id: u64) -> Vec<String> {
        let Ok(mut map) = ephemeral_lock(&self.ephemeral) else {
            return Vec::new();
        };
        let mut released = Vec::new();
        map.retain(|key, holders| {
            if !holders.remove(&conn_id) || !holders.is_empty() {
                return true;
            }
            released.push(key.clone());
            false
        });
        released
    }
}

/// Sweep dedup client entries idle longer than this once the map is large.
pub(crate) const DEDUP_IDLE_MS: u64 = 24 * 60 * 60 * 1000;

pub(crate) const DEDUP_SWEEP_THRESHOLD: usize = 10_000;

/// Permanent refusals remembered per client. Past it the oldest is forgotten,
/// and a replay of that write is answered `+DUP` as before 0.5.
pub(crate) const DEDUP_REFUSALS_PER_CLIENT: usize = 64;

/// Longest error text kept for one remembered refusal. An out-of-scope
/// refusal echoes the key, which may be 512 KB; the class a client acts on is
/// the error's first word, so the tail is not worth holding outside
/// `maxmemory` and rewriting into the sidecar at every checkpoint.
pub(crate) const DEDUP_REFUSAL_ERROR_BYTES: usize = 512;

/// One client's duplicate-suppression state.
///
/// The high-water mark answers "was this write applied?" only for writes that
/// were. A write refused for good never raises it, but a later write that is
/// accepted does — so after a lost reply the refused write's replay sits below
/// the mark and would be answered `+DUP`, and the client would retire it as
/// applied without ever hearing it was refused. Remembering recent refusals
/// lets the replay be refused again instead: the first outcome is final.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct DedupMark {
    /// Highest id applied; `None` while only refusals are known. Not `0`:
    /// wire id 0 is a real first write, and an entry created by a refusal
    /// must not claim it was applied.
    pub(crate) hwm: Option<u64>,
    pub(crate) seen: u64,
    /// `(id, error)` of recent permanent refusals, oldest first.
    pub(crate) refused: VecDeque<(u64, String)>,
}

/// How a DEDUP write that has been seen before must be answered.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DedupReplay {
    /// Applied before: answer `+DUP` and do nothing.
    Duplicate,
    /// Refused for good before: answer the same error and do nothing.
    Refused(String),
}

/// Dedup state recovered from the AOF: marks of the writes it carries, and
/// refusals it recorded.
#[derive(Debug, Default)]
pub(crate) struct RecoveredDedup {
    pub(crate) marks: HashMap<String, u64>,
    pub(crate) refusals: Vec<(String, u64, String)>,
}

/// `error` cut to at most [`DEDUP_REFUSAL_ERROR_BYTES`], on a character
/// boundary. The leading error code — what a client classifies by — is kept.
pub(crate) fn bounded_refusal(error: &str) -> &str {
    if error.len() <= DEDUP_REFUSAL_ERROR_BYTES {
        return error;
    }
    let mut end = DEDUP_REFUSAL_ERROR_BYTES;
    while !error.is_char_boundary(end) {
        end -= 1;
    }
    error.get(..end).unwrap_or_default()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Write `value` as MessagePack to `path` atomically: temp file, fsync,
/// rename, directory fsync.
async fn write_atomically<T: serde::Serialize>(
    path: &std::path::Path,
    tag: &str,
    value: &T,
) -> std::io::Result<()> {
    let tmp = temp_sibling(path, tag);
    let bytes = rmp_serde::to_vec(value)
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e.to_string()))?;
    if let Err(error) = write_private(&tmp, &bytes).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(error);
    }
    if let Err(error) = tokio::fs::rename(&tmp, path).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(error);
    }
    sync_parent_dir(path).await
}

impl ServerState {
    /// A state with no AOF, for tests that only need the shared tables.
    #[cfg(test)]
    pub(crate) fn for_tests(
        snap: Arc<SnapshotConfig>,
        replicas: ReplRegistry,
        is_replica: bool,
    ) -> Arc<Self> {
        Self::for_tests_with_aof(snap, replicas, is_replica, None)
    }

    /// [`for_tests`](Self::for_tests) with an AOF.
    #[cfg(test)]
    pub(crate) fn for_tests_with_aof(
        snap: Arc<SnapshotConfig>,
        replicas: ReplRegistry,
        is_replica: bool,
        aof: Option<Arc<AofWriter>>,
    ) -> Arc<Self> {
        Arc::new(ServerState {
            snap,
            aof,
            replicas,
            is_replica: AtomicBool::new(is_replica),
            dedup: std::sync::Mutex::new(HashMap::new()),
            ephemeral: std::sync::Mutex::new(HashMap::new()),
            ephemeral_members: std::sync::Mutex::new(HashMap::new()),
            ephemeral_in_use: std::sync::atomic::AtomicBool::new(false),
            dedup_dirty: std::sync::atomic::AtomicBool::new(false),
            dedup_order: tokio::sync::Mutex::new(()),
            save_lock: tokio::sync::Mutex::new(()),
            persistence_healthy: AtomicBool::new(true),
            persistence_failures: AtomicU64::new(0),
        })
    }

    pub(crate) fn is_replica(&self) -> bool {
        self.is_replica.load(Ordering::Relaxed)
    }

    /// Stop following a primary: accept writes, and enforce `store`'s caps,
    /// which a replica leaves to its primary (see `KeyValueStore::set_follower`).
    ///
    /// Behind the all-write barrier. A replicated frame holds its stripes
    /// while it checks [`Self::is_replica`] and applies (`apply_repl_frame`),
    /// so each frame is either applied as a follower's before the promotion,
    /// or refused after it, which ends the link (6L review, R2). Before, a
    /// link still up kept applying the old primary's frames with the caps on,
    /// and what they evicted reached nobody: not this node's AOF, replicas,
    /// WebSocket clients or watchers.
    pub(crate) async fn promote_to_primary(&self, store: &KeyValueStore) {
        let _all_writes = self.replicas.lock_all_writes().await;
        store.set_follower(false);
        self.is_replica.store(false, Ordering::Relaxed);
        info!("REPLICAOF NO ONE: promoted to primary — writes now accepted");
        // A follower does not enforce its caps, so it may be far above them.
        // The first write that needs room evicts down to them in one go (6L
        // review, R3): say so here rather than leave an unexplained pause.
        let keys = store.key_count();
        let memory = store.approximate_memory_bytes();
        let over_keys = store.max_keys().filter(|&cap| keys > cap);
        let over_memory = store.max_memory_bytes().filter(|&cap| memory > cap);
        if over_keys.is_some() || over_memory.is_some() {
            warn!(
                keys,
                max_keys = ?over_keys,
                memory_bytes = memory,
                max_memory_bytes = ?over_memory,
                "promoted above this node's caps: the next write that needs room evicts the excess at once"
            );
        }
    }

    /// True when a write must be RESP-encoded for the durability/replication
    /// path even if no other consumer needs it.
    pub(crate) fn needs_write_log(&self) -> bool {
        self.aof.is_some() || self.replicas.is_enabled()
    }

    /// Whether a DEDUP write has been seen before, and how to answer it.
    /// `None` means it must be executed.
    ///
    /// Only an id at or below the mark has been seen. Above it — a refused
    /// write retried before anything later was accepted — the write runs
    /// again, as it always has, and may now succeed; its success clears the
    /// remembered refusal. At or below it, the write cannot run again, and a
    /// remembered refusal is what says it was refused rather than applied.
    pub(crate) fn dedup_replay(&self, client: &str, id: u64) -> Option<DedupReplay> {
        let now = now_ms();
        let mut map = self.dedup.lock().expect("dedup mutex poisoned");
        if map.len() > DEDUP_SWEEP_THRESHOLD {
            let before = map.len();
            map.retain(|_, mark| now.saturating_sub(mark.seen) < DEDUP_IDLE_MS);
            if map.len() != before {
                self.dedup_dirty.store(true, Ordering::Release);
            }
        }
        let mark = map.get_mut(client)?;
        mark.seen = now;
        if mark.hwm.is_none_or(|hwm| id > hwm) {
            return None;
        }
        Some(
            match mark.refused.iter().find(|(refused, _)| *refused == id) {
                Some((_, error)) => DedupReplay::Refused(error.clone()),
                None => DedupReplay::Duplicate,
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn dedup_is_duplicate(&self, client: &str, id: u64) -> bool {
        self.dedup_replay(client, id).is_some()
    }

    pub(crate) fn commit_dedup(&self, client: &str, id: u64) {
        let now = now_ms();
        let mut map = self.dedup.lock().expect("dedup mutex poisoned");
        // Looked up before inserting: `entry` would allocate the client id on
        // every write, under the one lock every DEDUP write takes.
        match map.get_mut(client) {
            Some(mark) => {
                mark.hwm = Some(mark.hwm.map_or(id, |hwm| hwm.max(id)));
                mark.seen = now;
                // Applied on a retry after an earlier refusal: the refusal no
                // longer describes it.
                if !mark.refused.is_empty() {
                    mark.refused.retain(|(refused, _)| *refused != id);
                }
            }
            None => {
                map.insert(
                    client.to_string(),
                    DedupMark {
                        hwm: Some(id),
                        seen: now,
                        refused: VecDeque::new(),
                    },
                );
            }
        }
        self.dedup_dirty.store(true, Ordering::Release);
    }

    /// Remember that a DEDUP write was refused for good, so its replay is
    /// refused again rather than answered as a duplicate. Transient refusals
    /// are not remembered: the write is meant to be retried.
    pub(crate) fn remember_dedup_refusal(&self, client: &str, id: u64, error: &str) {
        let mut map = self.dedup.lock().expect("dedup mutex poisoned");
        let mark = map.entry(client.to_string()).or_default();
        mark.seen = now_ms();
        if mark.refused.iter().any(|(refused, _)| *refused == id) {
            return;
        }
        if mark.refused.len() >= DEDUP_REFUSALS_PER_CLIENT {
            mark.refused.pop_front();
        }
        mark.refused
            .push_back((id, bounded_refusal(error).to_string()));
        self.dedup_dirty.store(true, Ordering::Release);
    }

    /// Merge what AOF replay recovered: marks only ever rise, and recovered
    /// refusals join the remembered ones. Startup only.
    pub(crate) fn merge_recovered_dedup(&self, recovered: RecoveredDedup) {
        if recovered.marks.is_empty() && recovered.refusals.is_empty() {
            return;
        }
        let count = recovered.marks.len();
        for (client, hwm) in recovered.marks {
            self.commit_dedup(&client, hwm);
        }
        for (client, id, error) in recovered.refusals {
            self.remember_dedup_refusal(&client, id, &error);
        }
        info!("Recovered {} dedup high-water mark(s) from the AOF", count);
    }

    /// Test/support convenience for the check-then-commit operation.
    #[cfg(test)]
    pub(crate) fn dedup_seen(&self, client: &str, id: u64) -> bool {
        // Spelled out rather than `dedup_is_duplicate`, so a test reads it as
        // the check-then-commit the write path performs.
        if self.dedup_is_duplicate(client, id) {
            true
        } else {
            self.commit_dedup(client, id);
            false
        }
    }

    pub(crate) fn persistence_is_healthy(&self) -> bool {
        self.persistence_healthy.load(Ordering::Acquire)
    }

    pub(crate) fn record_persistence_failure(
        &self,
        operation: &'static str,
        error: &std::io::Error,
    ) {
        self.persistence_healthy.store(false, Ordering::Release);
        self.persistence_failures.fetch_add(1, Ordering::Relaxed);
        gauge!("recached_persistence_healthy").set(0.0);
        counter!("recached_persistence_errors_total", "operation" => operation).increment(1);
        error!(operation, error = %error, "persistence failure; client writes are disabled");
    }

    /// Latch persistence unhealthy for an internal failure that left the
    /// keyspace different from what the AOF and replicas were sent (6O-G: a
    /// write that panicked inside a transaction). Writes then fail closed
    /// with `MISCONF` until a `SAVE` snapshots the actual state.
    pub(crate) fn record_internal_failure(&self, operation: &'static str) {
        self.persistence_healthy.store(false, Ordering::Release);
        self.persistence_failures.fetch_add(1, Ordering::Relaxed);
        gauge!("recached_persistence_healthy").set(0.0);
        counter!("recached_persistence_errors_total", "operation" => operation).increment(1);
    }

    /// Called after every successful in-memory write. A runtime AOF error
    /// latches the server unhealthy so later client writes fail closed.
    pub(crate) async fn on_write(&self, resp: &[u8]) {
        self.on_write_identified(resp, None).await;
    }

    /// [`on_write`](Self::on_write) for a write that arrived DEDUP-wrapped.
    ///
    /// The AOF records the write *inside* its DEDUP envelope — one frame, so
    /// a torn tail drops both or neither — and replay restores the mark with
    /// the mutation. Logging the bare mutation, as before 0.5, let a crash
    /// between checkpoints bring back the write without its mark: the
    /// client's retry of an `INCR` whose reply was lost then ran it twice.
    ///
    /// Replicas get the same envelope when DEDUP replication is on (the 0.6
    /// default; every replica then negotiated it at the handshake), so a
    /// promoted replica answers a retry of a write it applied with `+DUP`.
    /// One frame carries both, so a replica can never hold the write without
    /// its mark. With it off they get the bare mutation, as before 0.6.
    pub(crate) async fn on_write_identified(&self, resp: &[u8], dedup: Option<(&str, u64)>) {
        self.on_write_logged(resp, dedup, false).await;
    }

    /// [`on_write_identified`](Self::on_write_identified); `may_defer` lets
    /// a client write leave its AOF wait to its connection (see
    /// `AofGatedWriter`), which is only allowed when nothing else will see the
    /// write before that wait: no replica stream (fanned out right after
    /// this), and — the caller's part — no push receiver or watcher. Those
    /// keep seeing a write only once it is in the AOF, as before.
    ///
    /// A DEDUP write never defers (6N-C, finding O1). Its mark is committed
    /// as soon as this returns, and a replay on *another* connection is
    /// answered `+DUP` from that mark — through a gate that holds no ticket
    /// for the original batch. Deferred, the `+DUP` could leave before the
    /// original's `write(2)` (or its `fdatasync` under `always`), and a
    /// failure of that batch then lost a write a client was told was
    /// applied. Waiting here commits every mark after its frame's AOF stage,
    /// which also covers the WebSocket early-replay path, which reads marks
    /// without `dedup_order`.
    pub(crate) async fn on_write_logged(
        &self,
        resp: &[u8],
        dedup: Option<(&str, u64)>,
        may_defer: bool,
    ) {
        let wrapped = dedup.map(|(client, id)| dedup_wrap(client, id, resp));
        if let Some(aof) = &self.aof {
            let frame = wrapped.as_deref().unwrap_or(resp);
            let result = if may_defer && dedup.is_none() && !self.replicas.is_enabled() {
                aof.append_client_write(frame).await
            } else if aof.fanout() == AofFanout::Coalesced && dedup.is_none() {
                // Still waited for here, under the caller's stripes, before
                // the fan-out below: only when the batch is led changes.
                // Not for a DEDUP write: it holds the process-wide
                // `dedup_order`, so no other DEDUP write can join its batch
                // and its yield would only hold everyone else's back.
                aof.append_coalesced(frame).await
            } else {
                aof.append(frame).await
            };
            if let Err(e) = result {
                self.record_persistence_failure("aof_append", &e);
            }
        }
        if self.replicas.is_enabled() {
            let frame = match wrapped {
                Some(envelope) if self.replicas.streams_dedup() => envelope,
                _ => resp.to_vec(),
            };
            self.replicas.fan_out(frame).await;
        }
    }

    /// [`on_write_logged`](Self::on_write_logged) for a write that found
    /// expired entries and removed them first (6O-H, Expiry Option E1):
    /// `expired` is their `DEL`, logged **before** the write's frame.
    ///
    /// The AOF takes both in one batch, queued in one step, so they are
    /// contiguous and in order: one `write(2)`, and under `always` one
    /// `fdatasync` before either is acknowledged. A torn tail can keep the
    /// `DEL` without the frame, never the frame without the `DEL`; the `DEL`
    /// alone is harmless, since the primary had already treated the entry as
    /// gone. Replicas get them as two frames, `DEL` first. A DEDUP write's
    /// `DEL` stays outside its envelope: it is not part of the write a retry
    /// would repeat, only the state the write found.
    pub(crate) async fn on_write_logged_after(
        &self,
        expired: &[u8],
        resp: &[u8],
        dedup: Option<(&str, u64)>,
        may_defer: bool,
    ) {
        let wrapped = dedup.map(|(client, id)| dedup_wrap(client, id, resp));
        if let Some(aof) = &self.aof {
            let frames = [expired, wrapped.as_deref().unwrap_or(resp)];
            let result = if may_defer && dedup.is_none() && !self.replicas.is_enabled() {
                aof.append_client_writes(&frames).await
            } else if aof.fanout() == AofFanout::Coalesced && dedup.is_none() {
                aof.append_coalesced_frames(&frames).await
            } else {
                aof.append_frames(&frames).await
            };
            if let Err(e) = result {
                self.record_persistence_failure("aof_append", &e);
            }
        }
        if self.replicas.is_enabled() {
            self.replicas.fan_out(expired.to_vec()).await;
            let frame = match wrapped {
                Some(envelope) if self.replicas.streams_dedup() => envelope,
                _ => resp.to_vec(),
            };
            self.replicas.fan_out(frame).await;
        }
    }

    /// The sweeper's `DEL` frames for one chunk (6O-H): one AOF batch, so
    /// one `write(2)` and, under `always`, one `fdatasync` for the chunk,
    /// without the coalescer's yield (every writer is held by the sweep's
    /// barrier, so nothing could join), then replicated in order (review B,
    /// B-3).
    /// Queue the sweeper's `DEL` frames, under the all-write barrier its
    /// caller holds: in the AOF (their place in the file fixed, nothing
    /// written yet) and on the replication stream, in that order, as a
    /// write's frames are. The caller releases the barrier, then waits on
    /// the result: the `write(2)`, and under `always` the `fdatasync`, no
    /// longer stop every writer (6O-H review E, F1).
    ///
    /// The replicas may receive these `DEL`s before the AOF holds them,
    /// unlike a client write's frames. A crash in between leaves the
    /// primary's file without a `DEL` of a key that had already expired:
    /// on restart that key is masked by reads and swept (and logged) again,
    /// and the replicas resynchronize in full from the new run id.
    pub(crate) async fn on_sweep_queued(&self, frames: &[&[u8]]) -> Option<QueuedFrames<'_>> {
        if frames.is_empty() {
            return None;
        }
        let queued = self.aof.as_ref().map(|aof| aof.queue_frames(frames));
        if self.replicas.is_enabled() {
            for frame in frames {
                self.replicas.fan_out(frame.to_vec()).await;
            }
        }
        queued
    }

    /// The outcome of [`on_sweep_queued`](Self::on_sweep_queued)'s AOF
    /// frames, waited on outside the barrier.
    pub(crate) async fn on_sweep_written(&self, queued: Option<QueuedFrames<'_>>) {
        if let Some(queued) = queued
            && let Err(e) = queued.wait().await
        {
            self.record_persistence_failure("aof_append", &e);
        }
    }

    /// [`on_write_logged`](Self::on_write_logged) for a transaction's
    /// frames, in command order (6O-G): the AOF takes them as one batch
    /// (never split by the coalescer: they are queued in one step), as a
    /// group between `TXN-BEGIN` and `TXN-COMMIT` in a `txn` file (6O-I),
    /// then each is replicated in order. Transactions carry no DEDUP
    /// identity.
    pub(crate) async fn on_transaction_logged(&self, frames: &[&[u8]], may_defer: bool) {
        if frames.is_empty() {
            return;
        }
        if let Some(aof) = &self.aof {
            // A group in a `txn` file (6O-I): replayed whole or not at all.
            let result = if may_defer && !self.replicas.is_enabled() {
                aof.append_client_transaction(frames).await
            } else if aof.fanout() == AofFanout::Coalesced {
                aof.append_coalesced_transaction(frames).await
            } else {
                aof.append_transaction(frames).await
            };
            if let Err(e) = result {
                self.record_persistence_failure("aof_append", &e);
            }
        }
        if self.replicas.is_enabled() {
            for frame in frames {
                self.replicas.fan_out(frame.to_vec()).await;
            }
        }
    }

    /// Record a DEDUP outcome that changed no data — a write that did nothing
    /// (`HDEL` of a missing field), or a permanent refusal — in the AOF, so
    /// it survives a crash before the next checkpoint like an applied one.
    pub(crate) async fn log_dedup_outcome(&self, client: &str, id: u64, refusal: Option<&str>) {
        let replicate = self.replicas.is_enabled() && self.replicas.streams_dedup();
        if self.aof.is_none() && !replicate {
            return;
        }
        let frame = match refusal {
            None => dedup_mark_frame(client, id),
            Some(error) => dedup_refusal_frame(client, id, bounded_refusal(error)),
        };
        if let Some(aof) = &self.aof
            && let Err(e) = aof.append(&frame).await
        {
            self.record_persistence_failure("aof_append", &e);
        }
        // An outcome that changed no data still has to reach a replica, or a
        // promoted one would run the retry the primary answered `+DUP` — or
        // answer `+DUP` to a write the primary refused.
        if replicate {
            self.replicas.fan_out(frame).await;
        }
    }

    /// The duplicate-suppression table, for a full replica synchronization.
    pub(crate) fn dedup_table(&self) -> Vec<DedupTableEntry> {
        let map = self
            .dedup
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        map.iter()
            .map(|(client, mark)| {
                (
                    client.clone(),
                    mark.hwm,
                    mark.refused.iter().cloned().collect(),
                )
            })
            .collect()
    }

    /// Replace the duplicate-suppression table with a primary's, after a full
    /// synchronization replaced the data. Marks this replica held for an
    /// earlier primary describe writes that no longer exist here.
    pub(crate) fn replace_dedup_table(&self, table: Vec<DedupTableEntry>) {
        let now = now_ms();
        let mut map = self
            .dedup
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        map.clear();
        for (client, hwm, refused) in table {
            let refused: VecDeque<(u64, String)> = refused
                .into_iter()
                .rev()
                .take(DEDUP_REFUSALS_PER_CLIENT)
                .rev()
                .map(|(id, error)| (id, bounded_refusal(&error).to_string()))
                .collect();
            map.insert(
                client,
                DedupMark {
                    hwm,
                    seen: now,
                    refused,
                },
            );
        }
        self.dedup_dirty.store(true, Ordering::Release);
    }

    /// Path of the dedup sidecar, alongside the snapshot.
    pub(crate) fn dedup_path(&self) -> std::path::PathBuf {
        self.snap.path.with_extension("dedup")
    }

    /// Path of the remembered-refusals sidecar. A separate file, so the
    /// marks sidecar keeps the format every earlier server reads.
    pub(crate) fn dedup_refusals_path(&self) -> std::path::PathBuf {
        self.snap.path.with_extension("dedup-refusals")
    }

    /// Persist dedup high-water marks in the same checkpoint as store data.
    /// Written atomically (temp + rename) and only when a mark has advanced.
    /// The map stores one `u64` per client.
    pub(crate) async fn persist_dedup(&self) -> std::io::Result<()> {
        if !self.dedup_dirty.swap(false, Ordering::Relaxed) {
            return Ok(());
        }
        let result = async {
            let (marks, refusals) = {
                let map = self
                    .dedup
                    .lock()
                    .map_err(|_| std::io::Error::other("dedup mutex poisoned"))?;
                // A client known only by its refusals has no mark to write.
                let marks: Vec<(String, u64)> = map
                    .iter()
                    .filter_map(|(client, mark)| mark.hwm.map(|hwm| (client.clone(), hwm)))
                    .collect();
                let refusals: Vec<(String, u64, String)> = map
                    .iter()
                    .flat_map(|(client, mark)| {
                        mark.refused
                            .iter()
                            .map(move |(id, error)| (client.clone(), *id, error.clone()))
                    })
                    .collect();
                (marks, refusals)
            };
            // Refusals first: a crash between the two files then leaves
            // extra refusals at worst, each for an id that was refused.
            write_atomically(&self.dedup_refusals_path(), "dedupr", &refusals).await?;
            write_atomically(&self.dedup_path(), "dedup", &marks).await
        }
        .await;
        if result.is_err() {
            self.dedup_dirty.store(true, Ordering::Release);
        }
        result
    }

    /// Restore dedup marks at boot. `seen` timestamps are not persisted — they
    /// only drive idle sweeping, so restored entries start their idle clock now.
    pub(crate) async fn load_dedup(&self) -> std::io::Result<()> {
        // Each file on its own: the refusals sidecar is written first, so a
        // crash during the first checkpoint can leave it without the marks.
        let marks = match tokio::fs::read(self.dedup_path()).await {
            Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e),
            Ok(bytes) => rmp_serde::from_slice::<Vec<(String, u64)>>(&bytes)
                .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e.to_string()))?,
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let refusals = match tokio::fs::read(self.dedup_refusals_path()).await {
            Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e),
            Ok(bytes) => rmp_serde::from_slice::<Vec<(String, u64, String)>>(&bytes)
                .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e.to_string()))?,
        };
        let mut map = self
            .dedup
            .lock()
            .map_err(|_| std::io::Error::other("dedup mutex poisoned"))?;
        let count = marks.len();
        for (client, hwm) in marks {
            let mark = map.entry(client).or_default();
            mark.hwm = Some(mark.hwm.map_or(hwm, |old| old.max(hwm)));
            mark.seen = now;
        }
        for (client, id, error) in refusals {
            let mark = map.entry(client).or_default();
            mark.seen = now;
            if mark.refused.len() < DEDUP_REFUSALS_PER_CLIENT {
                mark.refused
                    .push_back((id, bounded_refusal(&error).to_string()));
            }
        }
        info!("Restored {} dedup high-water mark(s)", count);
        Ok(())
    }

    /// Establish one exact snapshot/AOF checkpoint. Client writes remain
    /// blocked until the durable snapshot is installed and the old AOF is
    /// truncated, so no post-snapshot write can be discarded.
    pub(crate) async fn save(&self, store: &KeyValueStore) -> std::io::Result<()> {
        let _save = self.save_lock.lock().await;
        let _writes = self.replicas.lock_all_writes().await;
        self.checkpoint_locked(store).await
    }

    /// Whether this node writes its AOF in the expiry-log format (6O-H):
    /// it logs every expiration it acts on. A primary always does; a
    /// replica only while it follows a primary that negotiated `expirylog`.
    pub(crate) fn writes_expiry_log(&self) -> bool {
        !self.is_replica() || self.replicas.upstream_expirylog.load(Ordering::Acquire)
    }

    /// [`save`](Self::save) for a caller that already holds `save_lock`
    /// and the all-write barrier (a replica's full synchronization, 6O-H).
    pub(crate) async fn checkpoint_locked(&self, store: &KeyValueStore) -> std::io::Result<()> {
        self.checkpoint_locked_with(store, None).await
    }

    /// [`checkpoint_locked`](Self::checkpoint_locked) writing `payload`, a
    /// serialized `Vec<SnapshotEntry>`, as the snapshot instead of
    /// serializing the store: a replica's full synchronization writes what
    /// its primary sent, as received (6O-H review C, R2-2/R2-4).
    pub(crate) async fn checkpoint_locked_with(
        &self,
        store: &KeyValueStore,
        payload: Option<&[u8]>,
    ) -> std::io::Result<()> {
        let started = std::time::Instant::now();
        let checkpoint_id = self
            .snap
            .checkpoint_id
            .load(Ordering::Acquire)
            .saturating_add(1);
        if is_control_id(checkpoint_id) {
            return Err(std::io::Error::other(format!(
                "checkpoint id {checkpoint_id} is reserved for AOF control records"
            )));
        }
        let result = async {
            if let Some(aof) = &self.aof {
                // The marker must be durable before the snapshot that names it.
                // Startup can then distinguish every crash point around the
                // later AOF truncation. Its flush also makes every write the
                // marks below describe durable in the AOF.
                aof.append(&checkpoint_frame(checkpoint_id)).await?;
                aof.flush().await?;
                // Taken now that the marker is durable: should anything
                // below fail, the next save uses a new id. Reusing this one
                // after a renamed snapshot and a failed truncation made a
                // crashed later save skip every write in between (6O-H
                // review C, L2; pre-existing).
                self.snap
                    .checkpoint_id
                    .store(checkpoint_id, Ordering::Release);
            }
            // The marks before the snapshot, never after. Writes are blocked,
            // so they describe exactly the state being saved. A crash after
            // the marks but before the snapshot rename leaves marks *ahead*
            // of the old snapshot: with an AOF, every write they cover is in
            // it and replays; without one, those writes are lost with the
            // crash anyway — the snapshot-only contract — and a retry of one
            // is answered as applied. Written after, as before 0.5, a crash
            // in the same window left marks *behind* a snapshot that held
            // their writes, and a retried `INCR` ran twice.
            self.persist_dedup().await?;
            match payload {
                Some(payload) => save_snapshot_payload(payload, &self.snap, checkpoint_id).await?,
                None => {
                    let entries = self.durable_snapshot(store);
                    save_snapshot_entries(entries, &self.snap, checkpoint_id).await?;
                }
            }
            if let Some(aof) = &self.aof {
                // The new file begins in the expiry-log format, before the
                // seed, when this node logs its expirations: `FORMAT` is
                // queued in the truncation's own batch (6O-H, R2-7b).
                aof.truncate_then(self.writes_expiry_log()).await?;
                for frame in self.ephemeral_aof_seed(store) {
                    aof.append(&frame).await?;
                }
                aof.flush().await?;
            }
            Ok(())
        }
        .await;
        histogram!("recached_snapshot_duration_seconds").record(started.elapsed().as_secs_f64());
        match result {
            Ok(()) => {
                store.reset_dirty();
                self.snap
                    .checkpoint_id
                    .store(checkpoint_id, Ordering::Release);
                self.snap
                    .last_save
                    .store(now_unix_secs(), Ordering::Release);
                self.persistence_healthy.store(true, Ordering::Release);
                gauge!("recached_persistence_healthy").set(1.0);
                gauge!("recached_last_successful_save_timestamp_seconds")
                    .set(now_unix_secs() as f64);
                counter!("recached_snapshot_saves_total", "status" => "success").increment(1);
                Ok(())
            }
            Err(e) => {
                counter!("recached_snapshot_saves_total", "status" => "error").increment(1);
                self.record_persistence_failure("snapshot", &e);
                Err(e)
            }
        }
    }
}
