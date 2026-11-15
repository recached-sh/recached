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

/// The append-only file, written in batches by whichever appender finds no
/// write in progress.
///
/// Every write the server acknowledges is first appended here, so this is on
/// the path of every write of an AOF deployment. It used to hold a
/// `tokio::fs::File` behind an async mutex: tokio runs each file operation on
/// its blocking pool and completes it before accepting the next, so every
/// append paid a thread-pool round trip while holding the process-wide lock,
/// and `always` paid one `fdatasync` per write. Measured on the 6I production
/// benchmark (1 worker, pipelined `SET`): ~5,000 writes/s with `everysec`
/// against Redis's ~220,000, and ~350–700/s with `always`.
///
/// Appends join an open *batch* — bytes in append order plus one shared
/// completion. A batch is performed by the *leader*: the appender that finds
/// no write in progress takes the oldest batch and writes it, then the next,
/// until none is queued; appends arriving meanwhile join the next batch and
/// wait for it. Under `everysec` and `no` the leader writes inline, in its own
/// task, as Redis's event loop does: a page-cache `write(2)`, no thread
/// hand-off (a first version handed every batch to a writer thread, and on
/// one worker the futex wake-ups each way cost more than the write — 26% of
/// the worker's samples). A batch that must reach the disk — every batch
/// under `always`, and a truncation — is drained on the blocking pool
/// instead, where an `fdatasync` covers every append in it (group commit, as
/// Redis does once per event-loop iteration).
///
/// What each call guarantees is unchanged:
///
/// - `append` returns once its bytes were handed to the OS (`write(2)`), and
///   under `always` once they were `fdatasync`ed. A process crash after an
///   acknowledged write therefore loses nothing; `everysec` loses at most the
///   last second to a power cut.
/// - The file holds appends in the order they were made. Callers append
///   while holding the replication-order stripes of the write, so for
///   conflicting writes the AOF order is still the store order.
/// - `flush` returns once everything appended before it is durable: it waits
///   for those appends to be written, then `fdatasync`s on the blocking pool,
///   so — as with Redis's background fsync — appends keep being written while
///   it runs instead of queueing behind a disk barrier once a second.
/// - `truncate` empties the file after everything appended before it was
///   written, and before anything appended after it.
/// - An I/O failure is reported to exactly the appends of the batch it hit,
///   and is not sticky: the next batch tries the file again, so a later
///   `SAVE` can recover persistence as before.
/// - Cancellation-safe: the inline leader performs a batch with no `.await`
///   in between, and a blocking-pool drain completes and hands over on its
///   own, so a dropped append (a client disconnecting) never strands the
///   batches queued behind it.
///
/// Memory is bounded by the callers: every append waits for its batch, so at
/// most one frame per in-flight write is ever buffered.
pub(crate) struct AofWriter {
    #[allow(dead_code)]
    pub(crate) path: PathBuf,
    pub(crate) sync: AofSync,
    shared: Arc<AofShared>,
}

/// State shared between appenders and whoever is writing.
struct AofShared {
    queue: std::sync::Mutex<AofQueue>,
    file: std::fs::File,
    policy: AofSync,
}

#[derive(Default)]
struct AofQueue {
    /// Batches not yet taken, oldest first. Only the last can still be
    /// joined, and only while not `sealed`.
    batches: std::collections::VecDeque<AofBatch>,
    /// A leader is performing a batch: everyone else only queues.
    busy: bool,
}

struct AofBatch {
    bytes: Vec<u8>,
    /// Empty the file before writing this batch's bytes.
    truncate: bool,
    /// No further appends may join (a flush or truncate is anchored to it).
    sealed: bool,
    done: Arc<AofDone>,
}

/// One batch's outcome, shared by every append in it.
#[derive(Default)]
struct AofDone {
    result: std::sync::OnceLock<Result<(), (ErrorKind, String)>>,
    notify: tokio::sync::Notify,
}

impl AofDone {
    async fn wait(&self) -> std::io::Result<()> {
        loop {
            let notified = self.notify.notified();
            tokio::pin!(notified);
            // Registered before the check, so a completion between the check
            // and the await is not missed.
            notified.as_mut().enable();
            if let Some(result) = self.result.get() {
                return result
                    .clone()
                    .map_err(|(kind, message)| std::io::Error::new(kind, message));
            }
            notified.await;
        }
    }

    fn complete(&self, result: std::io::Result<()>) {
        let _ = self
            .result
            .set(result.map_err(|e| (e.kind(), e.to_string())));
        self.notify.notify_waiters();
    }
}

impl AofBatch {
    fn new() -> Self {
        AofBatch {
            bytes: Vec::new(),
            truncate: false,
            sealed: false,
            done: Arc::new(AofDone::default()),
        }
    }

    /// Whether performing this batch waits on the disk, and so must not run
    /// on an async worker.
    fn needs_disk(&self, policy: AofSync) -> bool {
        self.truncate || (policy == AofSync::Always && !self.bytes.is_empty())
    }
}

impl AofShared {
    fn lock(&self) -> std::sync::MutexGuard<'_, AofQueue> {
        self.queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn perform(&self, batch: &AofBatch) -> std::io::Result<()> {
        use std::io::Write as _;
        let mut file = &self.file;
        if batch.truncate {
            file.set_len(0)?;
            file.sync_all()?;
        }
        if !batch.bytes.is_empty() {
            file.write_all(&batch.bytes)?;
        }
        // `always`: the appends of this batch are acknowledged only once
        // durable, so the fsync belongs here — one per batch.
        if self.policy == AofSync::Always && !batch.bytes.is_empty() {
            file.sync_data()?;
        }
        Ok(())
    }

    /// Lead: perform queued batches until none is left. Inline batches run
    /// here; the first that needs the disk hands the rest of the drain to
    /// the blocking pool.
    fn lead(self: &Arc<Self>) {
        loop {
            let batch = {
                let mut queue = self.lock();
                if queue.busy {
                    return;
                }
                let Some(front) = queue.batches.front() else {
                    return;
                };
                if front.needs_disk(self.policy) {
                    // Claimed before the task exists, so no inline leader
                    // can start meanwhile; released by the drain itself.
                    queue.busy = true;
                    drop(queue);
                    let shared = Arc::clone(self);
                    match tokio::runtime::Handle::try_current() {
                        Ok(runtime) => {
                            runtime.spawn_blocking(move || shared.drain_blocking());
                        }
                        // No runtime to hand it to (a connection's writer
                        // dropped during shutdown): drain here rather than
                        // panic in `spawn_blocking` or leave it unwritten.
                        Err(_) => shared.drain_blocking(),
                    }
                    return;
                }
                queue.busy = true;
                queue
                    .batches
                    .pop_front()
                    .expect("the front batch was seen two lines above")
            };
            let result = self.perform(&batch);
            self.lock().busy = false;
            batch.done.complete(result);
        }
    }

    /// The blocking-pool drain: holds the writer role from its spawn until
    /// the queue is empty, completing each batch itself.
    fn drain_blocking(&self) {
        loop {
            let batch = {
                let mut queue = self.lock();
                match queue.batches.pop_front() {
                    Some(batch) => batch,
                    None => {
                        queue.busy = false;
                        return;
                    }
                }
            };
            let result = self.perform(&batch);
            batch.done.complete(result);
        }
    }
}

tokio::task_local! {
    /// The AOF batches a TCP connection has queued and not yet waited for;
    /// set for the connection's task by [`AofGatedWriter::scope`].
    static AOF_DEFERRED: Arc<DeferredAof>;
}

/// Run a connection's future with its client writes deferring their AOF
/// waits to the [`AofGatedWriter`] that `deferral` came from.
pub(crate) async fn with_deferred_aof<F: std::future::Future>(
    deferral: Arc<DeferredAof>,
    fut: F,
) -> F::Output {
    AOF_DEFERRED.scope(deferral, fut).await
}

/// A connection's queued-but-unwaited AOF batches, oldest first.
#[derive(Default)]
pub(crate) struct DeferredAof {
    tickets: std::sync::Mutex<VecDeque<(Arc<AofShared>, Arc<AofDone>)>>,
}

impl DeferredAof {
    fn push(&self, shared: &Arc<AofShared>, done: Arc<AofDone>) {
        let mut tickets = self
            .tickets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Consecutive appends usually share a batch: wait for it once.
        if tickets
            .back()
            .is_some_and(|(_, last)| Arc::ptr_eq(last, &done))
        {
            return;
        }
        tickets.push_back((Arc::clone(shared), done));
    }

    fn pop(&self) -> Option<(Arc<AofShared>, Arc<AofDone>)> {
        self.tickets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pop_front()
    }
}

/// A connection's socket, holding back every write until the AOF batches its
/// commands queued are written.
///
/// Redis writes the AOF once per event-loop iteration and sends replies
/// after it; a TCP connection here used to wait for each command's own
/// `write(2)` before running the next, so a pipeline of 16 `SET`s was 16
/// writes. With this gate under the connection's buffered writer, the
/// pipeline's appends queue as they run and the first byte headed for the
/// socket — a buffered flush, a large reply written through, anything —
/// first leads and waits for them. No reply can therefore precede the AOF
/// write of any command before it on the connection, which is what an
/// acknowledgement promised before.
pub(crate) struct AofGatedWriter<W> {
    inner: W,
    deferred: Arc<DeferredAof>,
    state: Arc<ServerState>,
    waiting:
        Option<std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + Send>>>,
}

impl<W> AofGatedWriter<W> {
    pub(crate) fn new(inner: W, state: Arc<ServerState>) -> Self {
        AofGatedWriter {
            inner,
            deferred: Arc::new(DeferredAof::default()),
            state,
            waiting: None,
        }
    }

    /// The handle [`with_deferred_aof`] needs to route this connection's
    /// client writes here.
    pub(crate) fn deferral(&self) -> Arc<DeferredAof> {
        Arc::clone(&self.deferred)
    }

    fn poll_settled(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        use std::task::Poll;
        loop {
            if let Some(waiting) = &mut self.waiting {
                match waiting.as_mut().poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(result) => {
                        self.waiting = None;
                        if let Err(e) = result {
                            self.state.record_persistence_failure("aof_append", &e);
                        }
                    }
                }
            }
            let Some((shared, done)) = self.deferred.pop() else {
                return Poll::Ready(());
            };
            // Nobody may be writing yet: queued appends do not lead.
            shared.lead();
            self.waiting = Some(Box::pin(async move { done.wait().await }));
        }
    }
}

impl<W> Drop for AofGatedWriter<W> {
    /// A connection that closes with batches queued still gets them
    /// written: lead them, without waiting.
    fn drop(&mut self) {
        while let Some((shared, _)) = self.deferred.pop() {
            shared.lead();
        }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for AofGatedWriter<W> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::task::ready!(self.poll_settled(cx));
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::ready!(self.poll_settled(cx));
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::ready!(self.poll_settled(cx));
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::task::ready!(self.poll_settled(cx));
        std::pin::Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
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
        let file = file.into_std().await;
        Ok(Self {
            path,
            sync,
            shared: Arc::new(AofShared {
                queue: std::sync::Mutex::new(AofQueue::default()),
                file,
                policy: sync,
            }),
        })
    }

    /// Queue work onto the open batch — or, when `fresh`, onto a new batch
    /// after it — lead if no one is writing, and return the batch's
    /// completion. Queueing is one lock, so nothing can be appended between
    /// sealing the open batch and starting the fresh one.
    fn enqueue(&self, fresh: bool, edit: impl FnOnce(&mut AofBatch)) -> Arc<AofDone> {
        let done = self.queue_only(fresh, edit);
        self.shared.lead();
        done
    }

    /// [`enqueue`](Self::enqueue) without leading.
    fn queue_only(&self, fresh: bool, edit: impl FnOnce(&mut AofBatch)) -> Arc<AofDone> {
        {
            let mut queue = self.shared.lock();
            let batch = match queue.batches.back_mut() {
                Some(open) if !open.sealed && !fresh => open,
                open => {
                    if let Some(open) = open {
                        open.sealed = true;
                    }
                    queue.batches.push_back(AofBatch::new());
                    queue
                        .batches
                        .back_mut()
                        .expect("a batch was pushed on the line above")
                }
            };
            edit(batch);
            Arc::clone(&batch.done)
        }
    }

    pub(crate) async fn append(&self, resp: &[u8]) -> std::io::Result<()> {
        self.enqueue(false, |batch| batch.bytes.extend_from_slice(resp))
            .wait()
            .await
    }

    /// [`append`](Self::append) for a client write, which may *defer* the
    /// wait: inside a connection that gates its socket on the AOF
    /// ([`AofGatedWriter`]), the frame is queued — its place in the file
    /// fixed now, under the caller's ordering stripes — and the connection
    /// waits for it, once for its whole pipeline, before any reply reaches
    /// the socket. Elsewhere it is `append`.
    ///
    /// A deferred append reports `Ok`; a write failure is recorded by the
    /// connection when it settles. Either way the client's reply is the
    /// same: an AOF failure never changed it, only latched persistence
    /// unhealthy for later writes.
    pub(crate) async fn append_client_write(&self, resp: &[u8]) -> std::io::Result<()> {
        let deferred = AOF_DEFERRED.try_with(Arc::clone).ok();
        match deferred {
            Some(deferred) => {
                let done = self.queue_only(false, |batch| batch.bytes.extend_from_slice(resp));
                deferred.push(&self.shared, done);
                Ok(())
            }
            None => self.append(resp).await,
        }
    }

    /// Make everything appended so far durable. Called on the `everysec`
    /// ticker, by checkpoints and before shutdown.
    ///
    /// `sync_data` rather than `sync_all`: the AOF is append-only, so its
    /// metadata beyond the length carries nothing worth an extra barrier.
    pub(crate) async fn flush(&self) -> std::io::Result<()> {
        // Everything appended so far is written once this sealed batch is.
        self.enqueue(false, |batch| batch.sealed = true)
            .wait()
            .await?;
        let shared = Arc::clone(&self.shared);
        tokio::task::spawn_blocking(move || shared.file.sync_data())
            .await
            .map_err(|e| std::io::Error::other(format!("AOF fsync task failed: {e}")))?
    }

    /// Empty the file after everything appended so far, before anything
    /// appended later.
    pub(crate) async fn truncate(&self) -> std::io::Result<()> {
        // A batch of its own, so it cannot erase appends made before it;
        // appends made after it join it and are written after the truncation.
        self.enqueue(true, |batch| batch.truncate = true)
            .wait()
            .await?;
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

#[cfg(test)]
pub(crate) async fn replay_aof_from_checkpoint(
    store: &KeyValueStore,
    path: &std::path::Path,
    snapshot_checkpoint: u64,
) -> std::io::Result<usize> {
    Ok(replay_aof_recovering(store, path, snapshot_checkpoint)
        .await?
        .0)
}

/// Replay the AOF after the snapshot's checkpoint, and recover the
/// duplicate-suppression state its records carry.
///
/// Marks are collected from the *whole* file, the part before the matching
/// checkpoint included: those writes are already in the snapshot, but a
/// crash between the snapshot and its dedup sidecar (as servers before 0.5
/// ordered them) leaves their marks nowhere else. Marks only rise, so taking
/// them from writes the snapshot holds is always safe.
pub(crate) async fn replay_aof_recovering(
    store: &KeyValueStore,
    path: &std::path::Path,
    snapshot_checkpoint: u64,
) -> std::io::Result<(usize, RecoveredDedup)> {
    let mut recovered = RecoveredDedup::default();
    let bytes = match tokio::fs::read(path).await {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok((0, recovered)),
        Err(e) => return Err(e),
        Ok(b) => b,
    };

    let mut replay_start = 0usize;
    let mut offset = 0;
    // Refusals in file order, and where each (client, id)'s sit: a later mark
    // for the same write — a refused write retried above the mark that then
    // succeeded — cancels all of them, as `commit_dedup` does live. A write
    // refused on each of several attempts leaves one record per attempt.
    // Merged after the marks, an uncancelled one would be answered on replay
    // of a write that was applied.
    let mut refusals: Vec<Option<(String, u64, String)>> = Vec::new();
    let mut refusal_at: HashMap<(String, u64), Vec<usize>> = HashMap::new();
    let mut torn_at = None;
    while offset < bytes.len() {
        match Value::parse(&bytes[offset..]) {
            Ok((value, consumed)) => {
                offset += consumed;
                if checkpoint_marker(&value) == Some(snapshot_checkpoint) {
                    replay_start = offset;
                }
                match dedup_record(&value) {
                    Some(DedupRecord::Mark(client, id)) => {
                        for at in refusal_at.remove(&(client.clone(), id)).unwrap_or_default() {
                            if let Some(slot) = refusals.get_mut(at) {
                                *slot = None;
                            }
                        }
                        let hwm = recovered.marks.entry(client).or_default();
                        *hwm = (*hwm).max(id);
                    }
                    Some(DedupRecord::Refused(client, id, error)) => {
                        refusal_at
                            .entry((client.clone(), id))
                            .or_default()
                            .push(refusals.len());
                        refusals.push(Some((client, id, error)));
                    }
                    None => {}
                }
            }
            Err(e) if e.is_incomplete() => {
                torn_at = Some(offset);
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
    recovered.refusals = refusals.into_iter().flatten().collect();

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
                // Marks and refusals were collected above; these records carry
                // no data.
                if matches!(dedup_record(&value), Some(DedupRecord::Refused(..)))
                    || is_dedup_mark_only(&value)
                {
                    continue;
                }
                // Writes are recorded via `on_write` in RESP3 Push form (`>N`);
                // normalise to Array so Command::from_value can parse them.
                let normalised = match value {
                    Value::Push(inner) => Value::Array(Some(inner)),
                    other => other,
                };
                let cmd = match Command::from_value(normalised) {
                    // The write inside its envelope; the mark was taken above.
                    Ok(Command::Dedup(_, _, inner)) => *inner,
                    Ok(cmd) => cmd,
                    Err(e) => {
                        return Err(std::io::Error::new(
                            ErrorKind::InvalidData,
                            format!("invalid AOF command at offset {}: {e}", offset - consumed),
                        ));
                    }
                };
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
    // The torn record is cut off, not just skipped. The AOF is appended to
    // from here on, so leaving the fragment in place put every later write
    // behind it, and the next startup refused the file as corrupted. The
    // fragment never completed, so no write it held was acknowledged.
    if let Some(valid) = torn_at {
        let file = tokio::fs::OpenOptions::new().write(true).open(path).await?;
        file.set_len(valid as u64).await?;
        file.sync_all().await?;
        warn!(
            "Truncated an incomplete AOF tail of {} byte(s) at offset {}",
            bytes.len().saturating_sub(valid),
            valid
        );
    }
    Ok((replayed, recovered))
}

/// The dedup information an AOF record carries.
pub(crate) enum DedupRecord {
    /// `DEDUP <client> <id> <write…>` or `RECACHED-DEDUP <client> <id>`.
    Mark(String, u64),
    /// `RECACHED-DEDUP-REFUSED <client> <id> <error>`.
    Refused(String, u64, String),
}

pub(crate) fn dedup_record(value: &Value) -> Option<DedupRecord> {
    let items = match value {
        Value::Push(items) | Value::Array(Some(items)) => items,
        _ => return None,
    };
    let bulk = |i: usize| match items.get(i) {
        Some(Value::BulkString(Some(b))) => Some(b.as_slice()),
        _ => None,
    };
    let name = bulk(0)?;
    let client = String::from_utf8(bulk(1)?.to_vec()).ok()?;
    let id: u64 = std::str::from_utf8(bulk(2)?).ok()?.parse().ok()?;
    let wrapped_write = name.eq_ignore_ascii_case(b"DEDUP") && items.len() > 3;
    let mark_only = name == b"RECACHED-DEDUP" && items.len() == 3;
    if wrapped_write || mark_only {
        Some(DedupRecord::Mark(client, id))
    } else if name == b"RECACHED-DEDUP-REFUSED" && items.len() == 4 {
        let error = String::from_utf8_lossy(bulk(3)?).into_owned();
        Some(DedupRecord::Refused(client, id, error))
    } else {
        None
    }
}

pub(crate) fn is_dedup_mark_only(value: &Value) -> bool {
    matches!(
        value,
        Value::Push(items) | Value::Array(Some(items))
            if matches!(items.first(), Some(Value::BulkString(Some(n))) if n == b"RECACHED-DEDUP")
    )
}

/// Wrap a write frame (`>N …`, as `broadcast_for` builds them) in the
/// `DEDUP <client> <id>` envelope it arrived in, for the AOF: one frame, so
/// the write and its mark survive or are lost together.
pub(crate) fn dedup_wrap(client: &str, id: u64, frame: &[u8]) -> Vec<u8> {
    let Some(split) = frame.windows(2).position(|w| w == b"\r\n") else {
        return frame.to_vec();
    };
    let count: Option<usize> = std::str::from_utf8(&frame[1..split])
        .ok()
        .and_then(|n| n.parse().ok());
    let Some(count) = count.filter(|_| frame.first() == Some(&b'>')) else {
        return frame.to_vec();
    };
    let id = id.to_string();
    let mut out = format!(
        ">{}\r\n$5\r\nDEDUP\r\n${}\r\n{}\r\n${}\r\n{}\r\n",
        count + 3,
        client.len(),
        client,
        id.len(),
        id
    )
    .into_bytes();
    out.extend_from_slice(&frame[split + 2..]);
    out
}

/// The AOF record for a DEDUP write that was applied but changed nothing, so
/// there is no mutation to wrap.
pub(crate) fn dedup_mark_frame(client: &str, id: u64) -> Vec<u8> {
    let id = id.to_string();
    resp_push(&[b"RECACHED-DEDUP", client.as_bytes(), id.as_bytes()])
}

/// The AOF record for a DEDUP write refused for good.
pub(crate) fn dedup_refusal_frame(client: &str, id: u64, error: &str) -> Vec<u8> {
    let id = id.to_string();
    resp_push(&[
        b"RECACHED-DEDUP-REFUSED",
        client.as_bytes(),
        id.as_bytes(),
        error.as_bytes(),
    ])
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

    /// Concurrent appenders share batches. Whatever the interleaving, every
    /// byte must be in the file, each appender's frames in the order it made
    /// them, and no frame torn by another — under every policy.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_appends_are_all_written_whole_and_in_each_writers_order() {
        for (name, policy) in [
            ("no", AofSync::No),
            ("everysec", AofSync::EverySec),
            ("always", AofSync::Always),
        ] {
            let dir = scratch(&format!("aof_concurrent_{name}"));
            let path = dir.join("d.aof");
            let w = Arc::new(AofWriter::open(path.clone(), policy).await.unwrap());
            let mut tasks = Vec::new();
            for writer in 0..8u32 {
                let w = Arc::clone(&w);
                tasks.push(tokio::spawn(async move {
                    for seq in 0..200u32 {
                        let body = format!("{writer}:{seq}");
                        let frame = format!("*1\r\n${}\r\n{body}\r\n", body.len());
                        w.append(frame.as_bytes()).await.unwrap();
                    }
                }));
            }
            for task in tasks {
                task.await.unwrap();
            }
            // Every append already returned, so its bytes are in the file
            // before any flush.
            let on_disk = String::from_utf8(tokio::fs::read(&path).await.unwrap()).unwrap();
            let mut next = [0u32; 8];
            let mut frames = 0;
            let mut rest = on_disk.as_str();
            while !rest.is_empty() {
                let (value, used) = Value::parse(rest.as_bytes()).expect("whole frames only");
                let Value::Array(Some(items)) = value else {
                    panic!("unexpected frame {value:?}")
                };
                let Some(Value::BulkString(Some(body))) = items.first() else {
                    panic!("unexpected frame body")
                };
                let body = String::from_utf8(body.clone()).unwrap();
                let (writer, seq) = body.split_once(':').unwrap();
                let (writer, seq): (usize, u32) = (writer.parse().unwrap(), seq.parse().unwrap());
                assert_eq!(seq, next[writer], "{name}: writer {writer} out of order");
                next[writer] += 1;
                frames += 1;
                rest = &rest[used..];
            }
            assert_eq!(frames, 8 * 200, "{name}: every append is in the file");
            drop(w);
            let _ = tokio::fs::remove_dir_all(&dir).await;
        }
    }

    /// An append whose caller goes away mid-write (a client disconnecting)
    /// must not strand the appends queued behind it: the batch still
    /// completes and the next appends are written.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_cancelled_append_strands_nothing() {
        for (name, policy) in [("always", AofSync::Always), ("everysec", AofSync::EverySec)] {
            let dir = scratch(&format!("aof_cancel_{name}"));
            let path = dir.join("f.aof");
            let w = Arc::new(AofWriter::open(path.clone(), policy).await.unwrap());
            for _ in 0..50 {
                let w2 = Arc::clone(&w);
                let task = tokio::spawn(async move { w2.append(b"*1\r\n$1\r\nX\r\n").await });
                task.abort();
                let _ = task.await;
            }
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                w.append(b"*1\r\n$4\r\nLAST\r\n"),
            )
            .await
            .expect("an append after cancelled ones completes")
            .unwrap();
            w.flush().await.unwrap();
            let on_disk = tokio::fs::read(&path).await.unwrap();
            assert!(on_disk.ends_with(b"*1\r\n$4\r\nLAST\r\n"), "{name}");
            // Whatever the aborted appends got to queue is whole frames.
            let before_last = &on_disk[..on_disk.len() - b"*1\r\n$4\r\nLAST\r\n".len()];
            assert!(
                before_last.chunks(11).all(|f| f == b"*1\r\n$1\r\nX\r\n"),
                "{name}: torn frames"
            );
            drop(w);
            let _ = tokio::fs::remove_dir_all(&dir).await;
        }
    }

    /// A truncate erases exactly what was appended before it: an append that
    /// completed first is gone, one made after it survives, and a flush
    /// after both makes the survivor durable.
    #[tokio::test]
    async fn truncate_orders_against_appends_on_both_sides() {
        let dir = scratch("aof_trunc_order");
        let path = dir.join("e.aof");
        let w = AofWriter::open(path.clone(), AofSync::EverySec)
            .await
            .unwrap();
        for _ in 0..50 {
            w.append(b"*1\r\n$6\r\nBEFORE\r\n").await.unwrap();
        }
        let (truncated, after) = tokio::join!(w.truncate(), async {
            // Starts once the truncate is queued: join polls in order.
            w.append(b"*1\r\n$5\r\nAFTER\r\n").await
        });
        truncated.unwrap();
        after.unwrap();
        w.flush().await.unwrap();
        assert_eq!(
            tokio::fs::read(&path).await.unwrap(),
            b"*1\r\n$5\r\nAFTER\r\n"
        );
        drop(w);
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    /// A failed write is reported to its appender, as before; Linux only,
    /// where `/dev/full` fails every write with ENOSPC.
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn a_failed_write_is_reported_to_the_append() {
        let w = AofWriter::open(PathBuf::from("/dev/full"), AofSync::No)
            .await
            .unwrap();
        let err = w.append(b"*1\r\n$4\r\nPING\r\n").await.unwrap_err();
        // Rebuilt from kind and message for every appender of the batch, so
        // the OS code survives only in the text the failure log prints.
        assert_eq!(err.kind(), ErrorKind::StorageFull, "ENOSPC, got {err:?}");
        assert!(err.to_string().contains("os error 28"), "{err}");
        // Not sticky: the next batch tries the file again (and fails the
        // same way here) rather than replaying a remembered error.
        assert!(w.append(b"*1\r\n$4\r\nPING\r\n").await.is_err());
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
