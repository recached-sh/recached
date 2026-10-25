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
    /// Ephemeral (`EADD`) set memberships → the connections holding each one,
    /// keyed by `(set key, member)`. Same lifetime rule as `ephemeral`, one
    /// level down: a member survives while any connection that added it is
    /// open, and the set is deleted once it empties.
    pub(crate) ephemeral_members: std::sync::Mutex<HashMap<(String, String), HashSet<u64>>>,
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

impl ServerState {
    /// Record `conn_id` as a holder of an ephemeral key. Repeating an `ESET`
    /// from the same connection is idempotent.
    pub(crate) fn claim_ephemeral(&self, key: &str, conn_id: u64) {
        if let Ok(mut map) = self.ephemeral.lock() {
            map.entry(key.to_string()).or_default().insert(conn_id);
        }
    }

    /// Record `conn_id` as a holder of each ephemeral set membership.
    pub(crate) fn claim_ephemeral_members(&self, key: &str, members: &[String], conn_id: u64) {
        if let Ok(mut map) = self.ephemeral_members.lock() {
            for member in members {
                map.entry((key.to_string(), member.clone()))
                    .or_default()
                    .insert(conn_id);
            }
        }
    }

    /// Whether a membership is already connection-scoped. `EADD` may add a
    /// second holder to such a membership, but must never adopt a member that
    /// was created by ordinary `SADD`.
    pub(crate) fn is_ephemeral_member(&self, key: &str, member: &str) -> bool {
        self.ephemeral_members
            .lock()
            .is_ok_and(|map| map.contains_key(&(key.to_string(), member.to_string())))
    }

    /// Keys and set memberships currently held by `conn_id`, without changing
    /// the registry. Disconnect cleanup uses this to reserve all relevant key
    /// ordering guards before it decides which holds are the last ones.
    pub(crate) fn ephemeral_claims_for(
        &self,
        conn_id: u64,
    ) -> (Vec<String>, Vec<(String, Vec<String>)>) {
        let keys = self
            .ephemeral
            .lock()
            .map(|map| {
                map.iter()
                    .filter(|(_, holders)| holders.contains(&conn_id))
                    .map(|(key, _)| key.clone())
                    .collect()
            })
            .unwrap_or_default();
        let mut members: HashMap<String, Vec<String>> = HashMap::new();
        if let Ok(map) = self.ephemeral_members.lock() {
            for ((key, member), holders) in map.iter() {
                if holders.contains(&conn_id) {
                    members.entry(key.clone()).or_default().push(member.clone());
                }
            }
        }
        (keys, members.into_iter().collect())
    }

    pub(crate) fn forget_ephemeral_key(&self, key: &str) {
        if let Ok(mut map) = self.ephemeral.lock() {
            map.remove(key);
        }
    }

    pub(crate) fn forget_ephemeral_members_for_key(&self, key: &str) {
        if let Ok(mut map) = self.ephemeral_members.lock() {
            map.retain(|(member_key, _), _| member_key != key);
        }
    }

    pub(crate) fn forget_all_ephemeral(&self) {
        if let Ok(mut map) = self.ephemeral.lock() {
            map.clear();
        }
        if let Ok(mut map) = self.ephemeral_members.lock() {
            map.clear();
        }
    }

    pub(crate) fn forget_ephemeral_keys<'a>(&self, keys: impl IntoIterator<Item = &'a str>) {
        let keys: HashSet<&str> = keys.into_iter().collect();
        if let Ok(mut map) = self.ephemeral.lock() {
            map.retain(|key, _| !keys.contains(key.as_str()));
        }
        if let Ok(mut map) = self.ephemeral_members.lock() {
            map.retain(|(key, _), _| !keys.contains(key.as_str()));
        }
    }

    pub(crate) fn forget_ephemeral_members(&self, key: &str, members: &[String]) {
        if let Ok(mut map) = self.ephemeral_members.lock() {
            for member in members {
                map.remove(&(key.to_string(), member.clone()));
            }
        }
    }

    /// Ephemeral set memberships `conn_id` was the last holder of, grouped by
    /// set key. Called once when a connection closes.
    pub(crate) fn take_ephemeral_members_for(&self, conn_id: u64) -> Vec<(String, Vec<String>)> {
        let Ok(mut map) = self.ephemeral_members.lock() else {
            return Vec::new();
        };
        let mut released: HashMap<String, Vec<String>> = HashMap::new();
        map.retain(|(key, member), holders| {
            if !holders.remove(&conn_id) || !holders.is_empty() {
                return true;
            }
            released
                .entry(key.clone())
                .or_default()
                .push(member.clone());
            false
        });
        released.into_iter().collect()
    }

    /// Snapshot only durable state. Ephemeral keys and memberships are live
    /// connection state and must not be resurrected after a clean restart.
    fn durable_snapshot(&self, store: &KeyValueStore) -> Vec<SnapshotEntry> {
        let ephemeral_keys: HashSet<String> = self
            .ephemeral
            .lock()
            .expect("ephemeral mutex poisoned")
            .keys()
            .cloned()
            .collect();
        let mut ephemeral_members: HashMap<String, HashSet<String>> = HashMap::new();
        for (key, member) in self
            .ephemeral_members
            .lock()
            .expect("ephemeral member mutex poisoned")
            .keys()
        {
            ephemeral_members
                .entry(key.clone())
                .or_default()
                .insert(member.clone());
        }
        store
            .snapshot()
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
        let mut keys: Vec<String> = self
            .ephemeral
            .lock()
            .expect("ephemeral mutex poisoned")
            .keys()
            .cloned()
            .collect();
        keys.sort_unstable();
        let mut frames = Vec::new();
        for key in keys {
            if let Value::BulkString(Some(value)) = store.get_current(&key) {
                frames.push(resp_push(&[b"ESET", key.as_bytes(), value.as_slice()]));
            }
        }

        let mut by_key: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (key, member) in self
            .ephemeral_members
            .lock()
            .expect("ephemeral member mutex poisoned")
            .keys()
        {
            if matches!(
                store.execute(Command::SIsMember(key.clone(), member.clone())),
                Value::Integer(1)
            ) {
                by_key.entry(key.clone()).or_default().push(member.clone());
            }
        }
        for (key, mut members) in by_key {
            members.sort_unstable();
            let mut parts: Vec<&[u8]> = vec![b"EADD", key.as_bytes()];
            parts.extend(members.iter().map(String::as_bytes));
            frames.push(resp_push(&parts));
        }
        frames
    }

    /// Keys `conn_id` was the **last** holder of, removed from the registry.
    /// Called once when a connection closes. A key another connection still
    /// holds is left alone, which is what keeps a second tab online.
    pub(crate) fn take_ephemeral_for(&self, conn_id: u64) -> Vec<String> {
        let Ok(mut map) = self.ephemeral.lock() else {
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
    pub(crate) hwm: u64,
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
    pub(crate) fn is_replica(&self) -> bool {
        self.is_replica.load(Ordering::Relaxed)
    }

    pub(crate) fn promote_to_primary(&self) {
        self.is_replica.store(false, Ordering::Relaxed);
        info!("REPLICAOF NO ONE: promoted to primary — writes now accepted");
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
        if id > mark.hwm {
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
                mark.hwm = mark.hwm.max(id);
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
                        hwm: id,
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
        mark.refused.push_back((id, error.to_string()));
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
    /// Replicas still get the bare mutation. An older replica refuses a
    /// DEDUP frame as a failed command and drops the link, and there is no
    /// handshake to ask; so a promoted replica holds no marks for replicated
    /// writes, which the documentation states.
    pub(crate) async fn on_write_identified(&self, resp: &[u8], dedup: Option<(&str, u64)>) {
        if let Some(aof) = &self.aof {
            let result = match dedup {
                Some((client, id)) => aof.append(&dedup_wrap(client, id, resp)).await,
                None => aof.append(resp).await,
            };
            if let Err(e) = result {
                self.record_persistence_failure("aof_append", &e);
            }
        }
        if self.replicas.is_enabled() {
            self.replicas.fan_out(resp.to_vec()).await;
        }
    }

    /// Record a DEDUP outcome that changed no data — a write that did nothing
    /// (`HDEL` of a missing field), or a permanent refusal — in the AOF, so
    /// it survives a crash before the next checkpoint like an applied one.
    pub(crate) async fn log_dedup_outcome(&self, client: &str, id: u64, refusal: Option<&str>) {
        if let Some(aof) = &self.aof {
            let frame = match refusal {
                None => dedup_mark_frame(client, id),
                Some(error) => dedup_refusal_frame(client, id, error),
            };
            if let Err(e) = aof.append(&frame).await {
                self.record_persistence_failure("aof_append", &e);
            }
        }
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
                let marks: Vec<(String, u64)> = map
                    .iter()
                    .map(|(client, mark)| (client.clone(), mark.hwm))
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
        let path = self.dedup_path();
        let bytes = match tokio::fs::read(&path).await {
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
            Ok(bytes) => bytes,
        };
        let marks = rmp_serde::from_slice::<Vec<(String, u64)>>(&bytes)
            .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e.to_string()))?;
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
            mark.hwm = mark.hwm.max(hwm);
            mark.seen = now;
        }
        for (client, id, error) in refusals {
            let mark = map.entry(client).or_default();
            mark.seen = now;
            if mark.refused.len() < DEDUP_REFUSALS_PER_CLIENT {
                mark.refused.push_back((id, error));
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
        let started = std::time::Instant::now();
        let _writes = self.replicas.lock_all_writes().await;
        let checkpoint_id = self
            .snap
            .checkpoint_id
            .load(Ordering::Acquire)
            .saturating_add(1);
        let result = async {
            if let Some(aof) = &self.aof {
                // The marker must be durable before the snapshot that names it.
                // Startup can then distinguish every crash point around the
                // later AOF truncation. Its flush also makes every write the
                // marks below describe durable in the AOF.
                aof.append(&checkpoint_frame(checkpoint_id)).await?;
                aof.flush().await?;
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
            let entries = self.durable_snapshot(store);
            save_snapshot_entries(entries, &self.snap, checkpoint_id).await?;
            if let Some(aof) = &self.aof {
                aof.truncate().await?;
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
