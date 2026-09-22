//! Durability: RDB-style snapshots and the append-only file, plus the
//! private-permission file helpers both rely on.

use crate::*;

/// Write `bytes` to `path`, creating it readable only by this user.
///
/// Snapshots, the AOF, and the dedup sidecar are plaintext MessagePack dumps of
/// the keyspace. `fs::write` creates with the process umask — `0644` on a
/// typical host — so any local user could read the entire cache. The
/// documentation told operators to protect these files with filesystem
/// permissions; the server should never have been relying on that.
///
/// Permissions are also set explicitly after opening, so a file left behind
/// `0644` by an earlier version is tightened on the next write rather than
/// keeping its old mode forever.
/// Writes are fsynced before returning. Every caller is writing state that has
/// to survive a crash — a snapshot about to be renamed into place, or the dedup
/// high-water marks that stop a replayed write being applied twice — so the
/// barrier belongs here rather than at each call site.
pub(crate) async fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut opts = tokio::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    opts.mode(0o600);
    let mut f = opts.open(path).await?;
    #[cfg(unix)]
    restrict_permissions(&f).await;
    f.write_all(bytes).await?;
    f.flush().await?;
    // `sync_all`, not `sync_data`: this file was just created, so its metadata
    // is part of what has to reach the device.
    f.sync_all().await?;
    Ok(())
}

/// fsync the directory holding `path`, making a `rename` into it durable.
///
/// Renaming a fsynced temp file over the target is atomic with respect to
/// readers, but the *directory entry* is itself just a write: without this, a
/// crash can leave the old file, or no file, despite the new contents being
/// safely on disk. Only meaningful on unix — Windows has no directory handle to
/// sync — so the call is compiled out elsewhere.
#[cfg(unix)]
pub(crate) async fn sync_parent_dir(path: &std::path::Path) -> std::io::Result<()> {
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    let dir = if dir.as_os_str().is_empty() {
        std::path::Path::new(".")
    } else {
        dir
    };
    let file = tokio::fs::File::open(dir).await?;
    file.sync_all().await
}

#[cfg(not(unix))]
pub(crate) async fn sync_parent_dir(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

/// Tighten an already-open file to `0600`, ignoring failure.
///
/// Best-effort by design: on a filesystem that cannot represent unix modes this
/// is not something to fail a write over, and the caller has already created the
/// file with the right mode where the platform allows it.
#[cfg(unix)]
pub(crate) async fn restrict_permissions(f: &tokio::fs::File) {
    use std::os::unix::fs::PermissionsExt;
    let _ = f
        .set_permissions(std::fs::Permissions::from_mode(0o600))
        .await;
}

/// Path for a temp file alongside `path`, distinct per operation.
///
/// The previous fixed `.tmp` name meant two servers sharing a directory would
/// clobber each other's half-written snapshot, and made the target predictable
/// to anyone who could already write to that directory. Residual: this is not
/// unguessable, so it is a defence against collision rather than against an
/// attacker who already controls the data directory.
pub(crate) fn temp_sibling(path: &std::path::Path, tag: &str) -> PathBuf {
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    path.with_extension(format!("{tag}.{}.{id}.tmp", std::process::id()))
}

// ── snapshot persistence ──────────────────────────────────────────────────────

pub(crate) struct SnapshotConfig {
    pub(crate) path: PathBuf,
    pub(crate) last_save: AtomicI64,
    pub(crate) checkpoint_id: AtomicU64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SnapshotFile {
    version: u8,
    aof_checkpoint: u64,
    entries: Vec<SnapshotEntry>,
}

pub(crate) struct LoadedSnapshot {
    #[allow(dead_code)]
    pub(crate) found: bool,
    pub(crate) aof_checkpoint: u64,
}

#[cfg(test)]
pub(crate) async fn save_snapshot(
    store: &KeyValueStore,
    cfg: &SnapshotConfig,
    aof_checkpoint: u64,
) -> std::io::Result<()> {
    let entries = store.snapshot();
    save_snapshot_entries(entries, cfg, aof_checkpoint).await
}

pub(crate) async fn save_snapshot_entries(
    entries: Vec<SnapshotEntry>,
    cfg: &SnapshotConfig,
    aof_checkpoint: u64,
) -> std::io::Result<()> {
    let count = entries.len();
    let tmp = temp_sibling(&cfg.path, "snap");
    let bytes = rmp_serde::to_vec(&SnapshotFile {
        version: 1,
        aof_checkpoint,
        entries,
    })
    .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e.to_string()))?;
    if let Err(error) = write_private(&tmp, &bytes).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(error);
    }
    if let Err(error) = tokio::fs::rename(&tmp, &cfg.path).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(error);
    }
    sync_parent_dir(&cfg.path).await?;
    info!("Snapshot saved: {} entries → {:?}", count, cfg.path);
    Ok(())
}

pub(crate) async fn load_snapshot_checkpoint(
    store: &KeyValueStore,
    path: &std::path::Path,
) -> std::io::Result<LoadedSnapshot> {
    let bytes = match tokio::fs::read(path).await {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            info!("No snapshot at {:?}, starting fresh", path);
            return Ok(LoadedSnapshot {
                found: false,
                aof_checkpoint: 0,
            });
        }
        Err(e) => return Err(e),
        Ok(bytes) => bytes,
    };
    let (entries, aof_checkpoint) =
        if let Ok(snapshot) = rmp_serde::from_slice::<SnapshotFile>(&bytes) {
            if snapshot.version != 1 {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("unsupported snapshot format version {}", snapshot.version),
                ));
            }
            (snapshot.entries, snapshot.aof_checkpoint)
        } else {
            (
                rmp_serde::from_slice::<Vec<SnapshotEntry>>(&bytes)
                    .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e.to_string()))?,
                0,
            )
        };
    let count = entries.len();
    store.restore(entries);
    info!("Snapshot loaded: {} entries ← {:?}", count, path);
    Ok(LoadedSnapshot {
        found: true,
        aof_checkpoint,
    })
}

#[cfg(test)]
pub(crate) async fn load_snapshot(
    store: &KeyValueStore,
    path: &std::path::Path,
) -> std::io::Result<bool> {
    Ok(load_snapshot_checkpoint(store, path).await?.found)
}

// ── AOF ───────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum AofSync {
    Always,
    EverySec,
    No,
}

pub(crate) struct AofWriter {
    #[allow(dead_code)]
    pub(crate) path: PathBuf,
    pub(crate) file: tokio::sync::Mutex<tokio::fs::File>,
    pub(crate) sync: AofSync,
}

impl AofWriter {
    pub(crate) async fn open(path: PathBuf, sync: AofSync) -> std::io::Result<Self> {
        let mut opts = tokio::fs::OpenOptions::new();
        opts.create(true).append(true);
        #[cfg(unix)]
        opts.mode(0o600);
        let file = opts.open(&path).await?;
        // An AOF written by an earlier version is likely to be 0644 — tighten it
        // on open, since `mode()` only applies to files this call creates.
        #[cfg(unix)]
        restrict_permissions(&file).await;
        Ok(Self {
            path,
            file: tokio::sync::Mutex::new(file),
            sync,
        })
    }

    pub(crate) async fn append(&self, resp: &[u8]) -> std::io::Result<()> {
        let mut f = self.file.lock().await;
        f.write_all(resp).await?;
        if self.sync == AofSync::Always {
            f.flush().await?;
            f.sync_data().await?;
        }
        Ok(())
    }

    /// Flush and fsync. Called on the `everysec` ticker and before shutdown.
    ///
    /// `sync_data` rather than `sync_all`: the AOF is append-only, so its
    /// metadata beyond the length carries nothing worth an extra barrier.
    pub(crate) async fn flush(&self) -> std::io::Result<()> {
        let mut f = self.file.lock().await;
        f.flush().await?;
        f.sync_data().await
    }

    pub(crate) async fn truncate(&self) -> std::io::Result<()> {
        let f = self.file.lock().await;
        f.set_len(0).await?;
        f.sync_all().await?;
        info!("AOF truncated after snapshot save");
        Ok(())
    }
}

#[derive(Default)]
struct ReplayEphemeral {
    keys: HashSet<String>,
    members: HashSet<(String, String)>,
}

impl ReplayEphemeral {
    fn claimable_members(&self, cmd: &Command, store: &KeyValueStore) -> Vec<String> {
        let Command::EAdd(key, members) = cmd else {
            return Vec::new();
        };
        members
            .iter()
            .filter(|member| {
                self.members.contains(&(key.clone(), (*member).clone()))
                    || matches!(
                        store.execute(Command::SIsMember(key.clone(), (*member).clone())),
                        Value::Integer(0)
                    )
            })
            .cloned()
            .collect()
    }

    fn forget_keys<'a>(&mut self, keys: impl IntoIterator<Item = &'a str>) {
        let keys: HashSet<&str> = keys.into_iter().collect();
        self.keys.retain(|key| !keys.contains(key.as_str()));
        self.members.retain(|(key, _)| !keys.contains(key.as_str()));
    }

    fn observe(&mut self, cmd: &Command, response: &Value, claimable: Vec<String>) {
        if matches!(response, Value::Error(_)) {
            return;
        }
        match cmd {
            Command::ESet(key, _) => {
                self.members.retain(|(member_key, _)| member_key != key);
                self.keys.insert(key.clone());
            }
            Command::EAdd(key, _) => {
                self.keys.remove(key);
                self.members
                    .extend(claimable.into_iter().map(|member| (key.clone(), member)));
            }
            Command::Set(key, _, _) => self.forget_keys(std::iter::once(key.as_str())),
            Command::MSet(pairs) => {
                self.forget_keys(pairs.iter().map(|(key, _)| key.as_str()));
            }
            Command::Del(keys) | Command::Unlink(keys) => {
                self.forget_keys(keys.iter().map(String::as_str));
            }
            Command::FlushDb => {
                self.keys.clear();
                self.members.clear();
            }
            Command::SRem(key, members) => {
                for member in members {
                    self.members.remove(&(key.clone(), member.clone()));
                }
            }
            Command::SMove(source, destination, member)
                if matches!(response, Value::Integer(1)) =>
            {
                self.members.remove(&(source.clone(), member.clone()));
                self.members.remove(&(destination.clone(), member.clone()));
            }
            Command::Rename(source, destination) if matches!(response, Value::SimpleString(_)) => {
                self.forget_keys([source.as_str(), destination.as_str()]);
            }
            Command::SInterStore(destination, _)
            | Command::SUnionStore(destination, _)
            | Command::SDiffStore(destination, _) => {
                self.forget_keys(std::iter::once(destination.as_str()));
            }
            _ => {}
        }
    }

    fn discard_abandoned(self, store: &KeyValueStore) -> std::io::Result<()> {
        if !self.keys.is_empty()
            && let Value::Error(message) =
                store.execute(Command::Del(self.keys.into_iter().collect()))
        {
            return Err(std::io::Error::new(ErrorKind::InvalidData, message));
        }
        let mut by_key: HashMap<String, Vec<String>> = HashMap::new();
        for (key, member) in self.members {
            by_key.entry(key).or_default().push(member);
        }
        for (key, members) in by_key {
            if let Value::Error(message) = store.execute(Command::SRem(key.clone(), members)) {
                return Err(std::io::Error::new(ErrorKind::InvalidData, message));
            }
            if matches!(
                store.execute(Command::SCard(key.clone())),
                Value::Integer(0)
            ) {
                let _ = store.execute(Command::Del(vec![key]));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) async fn replay_aof(
    store: &KeyValueStore,
    path: &std::path::Path,
) -> std::io::Result<usize> {
    replay_aof_from_checkpoint(store, path, 0).await
}

pub(crate) async fn replay_aof_from_checkpoint(
    store: &KeyValueStore,
    path: &std::path::Path,
    snapshot_checkpoint: u64,
) -> std::io::Result<usize> {
    let bytes = match tokio::fs::read(path).await {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
        Ok(b) => b,
    };

    let mut replay_start = 0usize;
    let mut offset = 0;
    while offset < bytes.len() {
        match Value::parse(&bytes[offset..]) {
            Ok((value, consumed)) => {
                offset += consumed;
                if checkpoint_marker(&value) == Some(snapshot_checkpoint) {
                    replay_start = offset;
                }
            }
            Err(e) if e.is_incomplete() => break,
            Err(e) => {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("AOF corrupted at offset {offset}: {e}"),
                ));
            }
        }
    }

    let mut replayed = 0usize;
    let mut ephemeral = ReplayEphemeral::default();
    let mut offset = replay_start;
    while offset < bytes.len() {
        match Value::parse(&bytes[offset..]) {
            Ok((value, consumed)) => {
                offset += consumed;
                if checkpoint_marker(&value).is_some() {
                    continue;
                }
                // Writes are recorded via `on_write` in RESP3 Push form (`>N`);
                // normalise to Array so Command::from_value can parse them.
                let normalised = match value {
                    Value::Push(inner) => Value::Array(Some(inner)),
                    other => other,
                };
                let cmd = Command::from_value(normalised).map_err(|e| {
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("invalid AOF command at offset {}: {e}", offset - consumed),
                    )
                })?;
                let claimable = ephemeral.claimable_members(&cmd, store);
                let response = store.execute(cmd.clone());
                if let Value::Error(message) = &response {
                    return Err(std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "AOF command failed at offset {}: {message}",
                            offset - consumed
                        ),
                    ));
                }
                ephemeral.observe(&cmd, &response, claimable);
                replayed += 1;
            }
            Err(e) if e.is_incomplete() => {
                warn!("Ignoring incomplete AOF tail at offset {}", offset);
                break;
            }
            Err(e) => {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("AOF corrupted at offset {offset}: {e}"),
                ));
            }
        }
    }
    ephemeral.discard_abandoned(store)?;
    if replayed > 0 {
        info!("AOF replayed: {} commands ← {:?}", replayed, path);
    }
    Ok(replayed)
}

fn checkpoint_marker(value: &Value) -> Option<u64> {
    let items = match value {
        Value::Push(items) | Value::Array(Some(items)) => items,
        _ => return None,
    };
    match items.as_slice() {
        [Value::BulkString(Some(name)), Value::BulkString(Some(id))]
            if name == b"RECACHED-CHECKPOINT" =>
        {
            std::str::from_utf8(id).ok()?.parse().ok()
        }
        _ => None,
    }
}

pub(crate) fn checkpoint_frame(id: u64) -> Vec<u8> {
    let id = id.to_string();
    resp_push(&[b"RECACHED-CHECKPOINT", id.as_bytes()])
}

// ── Replication ───────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────────
// Durability: the guarantees `RECACHED_AOF_SYNC` and snapshot saving advertise.
//
// These assert reachability and effect rather than device-level durability — a
// unit test cannot pull the power. What they pin is that the fsync path is
// actually taken, that it does not corrupt or lose data, and that the pieces a
// crash-consistency argument depends on (temp file synced before rename, parent
// directory synced after) are wired up.
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod durability_tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "recached_dur_{}_{}_{}",
            name,
            std::process::id(),
            next_conn_id()
        ));
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[tokio::test]
    async fn aof_always_survives_a_reopen_with_every_byte_intact() {
        // `always` previously called flush(), which reaches the page cache and
        // not the device. The observable part of the fix is that the fsync path
        // runs and is still byte-exact.
        let dir = scratch("aof_always");
        let path = dir.join("a.aof");
        let w = AofWriter::open(path.clone(), AofSync::Always)
            .await
            .unwrap();
        for i in 0..64 {
            w.append(format!("*1\r\n${}\r\n{}\r\n", i.to_string().len(), i).as_bytes())
                .await
                .unwrap();
        }
        drop(w);

        let on_disk = tokio::fs::read(&path).await.unwrap();
        let expected: Vec<u8> = (0..64)
            .flat_map(|i| format!("*1\r\n${}\r\n{}\r\n", i.to_string().len(), i).into_bytes())
            .collect();
        assert_eq!(on_disk, expected, "fsync must not disturb the byte stream");

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn aof_everysec_flush_is_idempotent_and_lossless() {
        // The everysec ticker calls flush() on a cadence, including when nothing
        // has been appended since the last tick.
        let dir = scratch("aof_everysec");
        let path = dir.join("b.aof");
        let w = AofWriter::open(path.clone(), AofSync::EverySec)
            .await
            .unwrap();
        w.append(b"*1\r\n$4\r\nPING\r\n").await.unwrap();
        w.flush().await.unwrap();
        w.flush().await.unwrap(); // nothing new to sync
        assert_eq!(
            tokio::fs::read(&path).await.unwrap(),
            b"*1\r\n$4\r\nPING\r\n"
        );
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn truncating_the_aof_leaves_it_empty_and_reusable() {
        // Truncation follows a snapshot, and is now fsynced so a crash cannot
        // resurrect a log the snapshot already subsumed. It must also leave the
        // handle writable — the server keeps appending to it afterwards.
        let dir = scratch("aof_trunc");
        let path = dir.join("c.aof");
        let w = AofWriter::open(path.clone(), AofSync::No).await.unwrap();
        w.append(b"*1\r\n$4\r\nPING\r\n").await.unwrap();
        w.truncate().await.unwrap();
        assert_eq!(tokio::fs::metadata(&path).await.unwrap().len(), 0);

        w.append(b"*1\r\n$4\r\nECHO\r\n").await.unwrap();
        w.flush().await.unwrap();
        assert_eq!(
            tokio::fs::read(&path).await.unwrap(),
            b"*1\r\n$4\r\nECHO\r\n",
            "the writer must still be usable after truncation"
        );
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn a_snapshot_lands_atomically_and_leaves_no_temp_file() {
        // The temp file is fsynced, renamed over the target, and then the
        // directory is fsynced. A leftover temp file would mean the rename never
        // happened, which is the failure this sequence exists to prevent.
        let dir = scratch("snap");
        let path = dir.join("dump.rdb");
        let store = KeyValueStore::new();
        for i in 0..32 {
            store.execute(Command::Set(
                format!("k{i}"),
                format!("v{i}").into_bytes(),
                Default::default(),
            ));
        }
        let cfg = SnapshotConfig {
            path: path.clone(),
            last_save: AtomicI64::new(0),
            checkpoint_id: AtomicU64::new(0),
        };
        save_snapshot(&store, &cfg, 7).await.unwrap();

        assert!(path.exists(), "snapshot must exist after save");
        let loaded = load_snapshot_checkpoint(&KeyValueStore::new(), &path)
            .await
            .unwrap();
        assert_eq!(loaded.aof_checkpoint, 7);

        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );

        // And the bytes are a snapshot we can actually read back.
        let restored = KeyValueStore::new();
        assert!(load_snapshot(&restored, &path).await.unwrap());
        assert_eq!(
            restored.execute(Command::Get("k7".into())),
            Value::BulkString(Some(b"v7".to_vec()))
        );

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn matching_checkpoint_marker_skips_the_aof_prefix_in_the_snapshot() {
        let dir = scratch("checkpoint_replay");
        let snapshot_path = dir.join("dump.rdb");
        let aof_path = dir.join("append.aof");
        let snapshot_store = KeyValueStore::new();
        snapshot_store.execute(Command::RPush("items".into(), vec![b"a".to_vec()]));
        let cfg = SnapshotConfig {
            path: snapshot_path.clone(),
            last_save: AtomicI64::new(0),
            checkpoint_id: AtomicU64::new(0),
        };
        save_snapshot(&snapshot_store, &cfg, 42).await.unwrap();

        let aof = AofWriter::open(aof_path.clone(), AofSync::No)
            .await
            .unwrap();
        aof.append(&resp_push(&[b"RPUSH", b"items", b"a"]))
            .await
            .unwrap();
        aof.append(&checkpoint_frame(42)).await.unwrap();
        aof.append(&resp_push(&[b"RPUSH", b"items", b"b"]))
            .await
            .unwrap();
        aof.flush().await.unwrap();

        let restored = KeyValueStore::new();
        let loaded = load_snapshot_checkpoint(&restored, &snapshot_path)
            .await
            .unwrap();
        assert_eq!(loaded.aof_checkpoint, 42);
        assert_eq!(
            replay_aof_from_checkpoint(&restored, &aof_path, loaded.aof_checkpoint)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            restored.execute(Command::LRange("items".into(), 0, -1)),
            Value::Array(Some(vec![
                Value::BulkString(Some(b"a".to_vec())),
                Value::BulkString(Some(b"b".to_vec())),
            ]))
        );

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn unmatched_checkpoint_marker_replays_the_whole_aof() {
        let dir = scratch("unmatched_checkpoint");
        let aof_path = dir.join("append.aof");
        let aof = AofWriter::open(aof_path.clone(), AofSync::No)
            .await
            .unwrap();
        aof.append(&resp_push(&[b"RPUSH", b"items", b"a"]))
            .await
            .unwrap();
        aof.append(&checkpoint_frame(9)).await.unwrap();
        aof.append(&resp_push(&[b"RPUSH", b"items", b"b"]))
            .await
            .unwrap();
        aof.flush().await.unwrap();

        let restored = KeyValueStore::new();
        assert_eq!(
            replay_aof_from_checkpoint(&restored, &aof_path, 8)
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            restored.execute(Command::LRange("items".into(), 0, -1)),
            Value::Array(Some(vec![
                Value::BulkString(Some(b"a".to_vec())),
                Value::BulkString(Some(b"b".to_vec())),
            ]))
        );
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn legacy_bare_entry_snapshot_still_loads() {
        let dir = scratch("legacy_snapshot");
        let path = dir.join("dump.rdb");
        let source = KeyValueStore::new();
        source.execute(Command::Set(
            "legacy".into(),
            b"value".to_vec(),
            Default::default(),
        ));
        write_private(&path, &rmp_serde::to_vec(&source.snapshot()).unwrap())
            .await
            .unwrap();

        let restored = KeyValueStore::new();
        let loaded = load_snapshot_checkpoint(&restored, &path).await.unwrap();
        assert!(loaded.found);
        assert_eq!(loaded.aof_checkpoint, 0);
        assert_eq!(
            restored.execute(Command::Get("legacy".into())),
            Value::BulkString(Some(b"value".to_vec()))
        );
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn syncing_a_parent_directory_reports_invalid_paths() {
        // Relative snapshot paths are supported, while an invalid parent must
        // be surfaced so a successful save is never reported prematurely.
        sync_parent_dir(std::path::Path::new("recached.rdb"))
            .await
            .unwrap();
        sync_parent_dir(std::path::Path::new("/")).await.unwrap();
        assert!(
            sync_parent_dir(std::path::Path::new("/nonexistent-recached-dir/x.rdb"))
                .await
                .is_err()
        );
    }

    #[test]
    fn a_sync_token_cannot_grant_an_over_long_pattern() {
        // Token patterns reach glob_match without passing through the command
        // parser, so the cap has to be repeated there. These are matched once
        // per key per write — the most expensive place a pattern can sit.
        use base64::Engine as _;
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let secret = "s3cret";
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let mint = |payload: &str| {
            let p = engine.encode(payload);
            let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
            mac.update(p.as_bytes());
            format!("{}.{}", p, engine.encode(mac.finalize().into_bytes()))
        };

        let long = "a".repeat(core_engine::store::MAX_PATTERN_BYTES + 1);
        let err = verify_sync_token(secret, &mint(&long))
            .expect_err("an over-long granted pattern must be refused");
        assert_eq!(err, "token grants an over-long pattern");

        // One over-long pattern in an otherwise fine list is still refused.
        assert!(verify_sync_token(secret, &mint(&format!("cart:*,{long}"))).is_err());

        // A pattern exactly at the cap is still honoured.
        let at_cap = "a".repeat(core_engine::store::MAX_PATTERN_BYTES);
        assert_eq!(
            verify_sync_token(secret, &mint(&at_cap)),
            Ok(vec![Grant::rw(&at_cap)])
        );
        assert_eq!(
            verify_sync_token(secret, &mint("cart:42:*,user:1:*")),
            Ok(vec![Grant::rw("cart:42:*"), Grant::rw("user:1:*")])
        );
        // The cap applies to the pattern, not the entry: an access prefix
        // does not eat into a grant's pattern budget.
        assert_eq!(
            verify_sync_token(secret, &mint(&format!("r={at_cap}"))),
            Ok(vec![Grant::ro(&at_cap)])
        );
    }
}

// ── Expiry propagation ────────────────────────────────────────────────────────
