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

#[derive(Clone, Copy, PartialEq, Debug)]
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
/// - Under `everysec` and `no`, an I/O failure is reported to exactly the
///   appends of the batch it hit, and is not sticky: the next batch tries
///   the file again, so a later `SAVE` can recover persistence as before.
///   Under `always`, a batch with appends that fails is a fail-stop (see
///   [`FailStop`]): the process ends before any of them is acknowledged.
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
    coalesce: Coalescer,
    stats: &'static AofStats,
    /// Where a failed write's torn fragment starts, while it is still in
    /// the file: [`NOT_TORN`], [`TORN_AT_UNKNOWN`], or the offset. Nothing
    /// is appended until the fragment is gone, since a frame written after
    /// a torn one makes the whole file unreadable on replay (or, with an
    /// unlucky byte alignment, replays as a different command). Only
    /// whoever performs a batch touches it, and performing is exclusive
    /// (`busy`, handed over under the queue lock), so relaxed accesses are
    /// ordered by that lock.
    torn_at: std::sync::atomic::AtomicU64,
    /// `always` only: a batch with appends failed to become durable, so the
    /// process is stopping (see [`AofShared::fail_stop`]). Once set,
    /// nothing more is written and every batch fails.
    stopped: std::sync::atomic::AtomicBool,
    /// What [`AofShared::fail_stop`] does after latching `stopped`.
    on_fail_stop: FailStop,
    /// Failures to inject into the next batches' writes, oldest first.
    #[cfg(test)]
    faults: std::sync::Mutex<VecDeque<AofFault>>,
    /// A failure to inject into the first batch whose bytes contain the
    /// marker, for the subprocess harness (see [`AofWriter::open`]).
    #[cfg(test)]
    armed: std::sync::Mutex<Option<(Vec<u8>, AofFault)>>,
    /// Truncations still to fail, for tests.
    #[cfg(test)]
    failing_repairs: std::sync::atomic::AtomicU32,
    /// `fdatasync`s still to fail, for tests.
    #[cfg(test)]
    failing_syncs: std::sync::atomic::AtomicU32,
    /// Holds the performer before a marked batch, for tests (see [`AofHold`]).
    #[cfg(test)]
    hold: AofHold,
}

/// Test hook: hold whoever performs a batch whose bytes contain a marker,
/// before it writes anything, until the test releases it (or a time limit
/// passes). One-shot: the first marked batch is held, later ones are not.
///
/// This is how a test lands a second connection's command inside the window
/// between a write's place in the AOF queue and its `write(2)`/`fdatasync`,
/// deterministically. In-process tests arm it with
/// [`AofWriter::hold_batch_containing`]; the subprocess harness's child arms
/// it from `RECACHED_TEST_AOF_HOLD=<ms>@<marker>`, which releases after
/// `<ms>` (an injected fault on the same batch then strikes after the hold).
///
/// It blocks the *thread* performing the batch. Under `no` and `everysec`
/// that is a tokio worker (`lead` performs inline): on a multi-thread
/// runtime the hold uses `block_in_place`, so the runtime goes on; on a
/// current-thread runtime it stalls everything until its time limit.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct AofHold {
    state: std::sync::Mutex<AofHoldState>,
    released: std::sync::Condvar,
}

#[cfg(test)]
#[derive(Default)]
struct AofHoldState {
    marker: Option<Vec<u8>>,
    limit: Option<std::time::Duration>,
    /// Batches held so far.
    held: u32,
}

#[cfg(test)]
impl AofHold {
    fn from_env() -> Self {
        let hold = AofHold::default();
        if std::env::var_os("RECACHED_FAIL_STOP_CHILD").is_some()
            && let Ok(spec) = std::env::var("RECACHED_TEST_AOF_HOLD")
        {
            let (ms, marker) = spec
                .split_once('@')
                .unwrap_or_else(|| panic!("bad test hold {spec:?}"));
            let ms: u64 = ms
                .parse()
                .unwrap_or_else(|_| panic!("bad test hold {spec:?}"));
            hold.arm(marker.as_bytes(), std::time::Duration::from_millis(ms));
        }
        hold
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, AofHoldState> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn arm(&self, marker: &[u8], limit: std::time::Duration) {
        let mut state = self.lock();
        state.marker = Some(marker.to_vec());
        state.limit = Some(limit);
        // Counted per arming, so `wait_until_held` waits for this one.
        state.held = 0;
    }

    fn release(&self) {
        self.lock().marker = None;
        self.released.notify_all();
    }

    fn held(&self) -> u32 {
        self.lock().held
    }

    /// Block the performing thread while `bytes` holds the armed marker.
    ///
    /// On a multi-thread runtime's worker the wait runs in `block_in_place`,
    /// which hands the worker's other duties — driving timers and sockets —
    /// to another thread. A worker blocked plainly could be the one that
    /// last polled the driver, and with no other worker woken nothing would
    /// fire the test's own timers or socket reads until the hold timed out.
    fn wait_if_marked(&self, bytes: &[u8]) {
        // Unarmed or unmarked — every batch of every other test — costs one
        // lock and nothing else: no hand-off of the worker, which would
        // change the scheduling the coalescing tests measure.
        if !self.is_marked(bytes) {
            return;
        }
        let worker = tokio::runtime::Handle::try_current()
            .is_ok_and(|h| h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread);
        if worker {
            tokio::task::block_in_place(|| self.wait_if_marked_here(bytes));
        } else {
            self.wait_if_marked_here(bytes);
        }
    }

    fn is_marked(&self, bytes: &[u8]) -> bool {
        self.lock()
            .marker
            .as_ref()
            .is_some_and(|m| !m.is_empty() && bytes.windows(m.len()).any(|w| w == m.as_slice()))
    }

    fn wait_if_marked_here(&self, bytes: &[u8]) {
        let mut state = self.lock();
        let hit = state
            .marker
            .as_ref()
            .is_some_and(|m| !m.is_empty() && bytes.windows(m.len()).any(|w| w == m.as_slice()));
        if !hit {
            return;
        }
        state.held = state.held.saturating_add(1);
        let deadline =
            std::time::Instant::now() + state.limit.unwrap_or(std::time::Duration::from_secs(10));
        while state.marker.is_some() {
            let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) else {
                break;
            };
            state = self
                .released
                .wait_timeout(state, left)
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
        // One-shot: whatever ended the hold, later batches pass.
        state.marker = None;
    }
}

/// What an `always` writer does when a batch it had to make durable fails.
///
/// The contract of `always` is that a write is acknowledged only once its
/// AOF frame is `fdatasync`ed. Its command has already changed memory by
/// then, so once the frame cannot be made durable the process holds state
/// ahead of its log, and every waiter in the batch would otherwise resume
/// and answer as before. Redis stops for the same reason ("Can't recover
/// from AOF write error when the AOF fsync policy is 'always'. Exiting").
/// See "AOF failures under `always`" in `docs/server/configuration.md`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FailStop {
    /// Production: log, then end the process from the thread that saw the
    /// failure, before the batch is completed — so no waiter of it, or of
    /// any batch after it, ever resumes, on any transport.
    Exit,
    /// In-process tests: latch `stopped` and complete the batch, and
    /// every later one, with an error.
    #[cfg(test)]
    Latch,
}

/// The exit status of a fail-stop: `EX_IOERR` from `sysexits.h`, distinct
/// from the status 1 of a startup error, so a supervisor or a test can tell
/// the two apart.
pub(crate) const AOF_FAIL_STOP_EXIT_CODE: i32 = 74;

/// [`AofShared::torn_at`]: the file ends on a whole frame.
const NOT_TORN: u64 = u64::MAX;
/// [`AofShared::torn_at`]: a fragment is in the file and where it starts
/// could not be worked out; appends wait for a truncation (the next
/// snapshot) to remove it.
const TORN_AT_UNKNOWN: u64 = u64::MAX - 1;

/// A write failure a test makes the next batch suffer.
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) enum AofFault {
    /// The write fails before any byte reaches the file.
    Fail(ErrorKind),
    /// The first `write(2)` takes only this many bytes; the rest follow.
    Short(usize),
    /// This many bytes reach the file, then the write fails: a torn frame.
    TornThenFail(usize, ErrorKind),
    /// As `TornThenFail`, and the next two repairs (truncations) fail too:
    /// the one right after the failure and the next batch's retry.
    TornUnrepairable(usize, ErrorKind),
    /// The write succeeds; the next `fdatasync` fails.
    FsyncFails(ErrorKind),
}

#[cfg(test)]
impl AofFault {
    /// Parse the subprocess harness's spelling: `fail:<kind>`, `short:<n>`,
    /// `torn:<n>:<kind>`, `fsync:<kind>`, with `<kind>` `enospc` or `eio`
    /// (an `Other` error).
    pub(crate) fn parse(spec: &str) -> Option<Self> {
        fn kind(s: &str) -> Option<ErrorKind> {
            match s {
                "enospc" => Some(ErrorKind::StorageFull),
                "eio" => Some(ErrorKind::Other),
                _ => None,
            }
        }
        let parts: Vec<&str> = spec.split(':').collect();
        match parts.as_slice() {
            ["fail", k] => Some(AofFault::Fail(kind(k)?)),
            ["short", n] => Some(AofFault::Short(n.parse().ok()?)),
            ["torn", n, k] => Some(AofFault::TornThenFail(n.parse().ok()?, kind(k)?)),
            ["fsync", k] => Some(AofFault::FsyncFails(kind(k)?)),
            _ => None,
        }
    }
}

/// Whether a connection gate defers its lead to the end of the scheduler
/// pass, so that other connections' appends join the same `write(2)`.
///
/// Redis writes the AOF once per event-loop iteration, after every ready
/// client has run. A tokio worker has no such hook, so a gate that led as
/// soon as its connection flushed wrote that one connection's frames alone:
/// on one worker with 50 clients every command was its own `write(2)`
/// (6M-A: 100,000 `SET`s → 100,000 batches of one frame). A gate now
/// `yield_now`s first. Tokio wakes yielded tasks only once the worker has
/// run every other ready task in its queue (and whatever it then steals
/// from other workers) and polled the I/O driver — or at its periodic
/// maintenance tick, every `event_interval` (61) polls, which is the hard
/// bound on the wait even for a worker that never idles — and by then the
/// other ready connections have queued their frames into the same open
/// batch. The first
/// gate to resume writes it for all of them; the rest find it written.
///
/// The yield costs an extra scheduler pass (an `epoll_wait(0)` when nothing
/// else is ready), which is pure loss with one client. So it is adaptive:
/// after [`COALESCE_MISSES`] yields in a row that no other append joined,
/// the next [`COALESCE_BACKOFF`] gates lead at once, then it probes again.
/// The heuristic only decides *when* a batch is written, never what a
/// write covers or what a reply waits for, so a racy decision is harmless.
struct Coalescer {
    enabled: std::sync::atomic::AtomicBool,
    /// Gates still to lead at once before the next probe.
    skip: std::sync::atomic::AtomicU32,
    /// Consecutive yields that no other append joined.
    misses: std::sync::atomic::AtomicU32,
}

/// Yields in a row that gained nothing before coalescing backs off.
const COALESCE_MISSES: u32 = 2;
/// Gates that lead at once after a back-off, before the next probe.
const COALESCE_BACKOFF: u32 = 32;

impl Coalescer {
    fn new(enabled: bool) -> Self {
        Coalescer {
            enabled: std::sync::atomic::AtomicBool::new(enabled),
            skip: std::sync::atomic::AtomicU32::new(0),
            misses: std::sync::atomic::AtomicU32::new(0),
        }
    }

    /// Whether this gate should yield before leading.
    fn should_yield(&self) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        if !self.enabled.load(Relaxed) {
            return false;
        }
        // Decrement while positive; at zero, probe.
        self.skip
            .fetch_update(Relaxed, Relaxed, |skip| skip.checked_sub(1))
            .is_err()
    }

    /// Record whether a yield let another append join (or another task
    /// write the batch).
    fn observe(&self, gained: bool, stats: &AofStats) {
        use std::sync::atomic::Ordering::Relaxed;
        if gained {
            bump(&stats.coalesce_gained, 1);
            self.misses.store(0, Relaxed);
            return;
        }
        bump(&stats.coalesce_missed, 1);
        if self.misses.fetch_add(1, Relaxed).saturating_add(1) >= COALESCE_MISSES {
            self.misses.store(0, Relaxed);
            self.skip.store(COALESCE_BACKOFF, Relaxed);
        }
    }
}

#[derive(Default)]
struct AofQueue {
    /// Batches not yet taken, oldest first. Only the last can still be
    /// joined, and only while not `sealed`.
    batches: std::collections::VecDeque<AofBatch>,
    /// A leader is performing a batch: everyone else only queues.
    busy: bool,
    /// The completion of the newest batch ever queued. Batches are performed
    /// in queue order, so once it completes every append queued before it
    /// was taken has been performed too — even one whose batch has already
    /// left `batches` and is being written. See [`AofWriter::tail`].
    newest: Option<Arc<AofDone>>,
}

struct AofBatch {
    bytes: Vec<u8>,
    /// Logical appends in `bytes`, for [`AOF_STATS`].
    frames: u64,
    /// When the first append joined, for [`AOF_STATS`].
    opened: Option<std::time::Instant>,
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
    fn is_complete(&self) -> bool {
        self.result.get().is_some()
    }

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
            frames: 0,
            opened: None,
            truncate: false,
            sealed: false,
            done: Arc::new(AofDone::default()),
        }
    }

    fn push_frame(&mut self, frame: &[u8]) {
        if self.opened.is_none() {
            self.opened = Some(std::time::Instant::now());
        }
        self.bytes.extend_from_slice(frame);
        self.frames = self.frames.saturating_add(1);
    }

    /// Whether performing this batch waits on the disk, and so must not run
    /// on an async worker.
    fn needs_disk(&self, policy: AofSync) -> bool {
        self.truncate || (policy == AofSync::Always && !self.bytes.is_empty())
    }
}

impl AofShared {
    fn lock(&self) -> std::sync::MutexGuard<'_, AofQueue> {
        match self.queue.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                bump(&self.stats.lock_contended, 1);
                self.queue
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
            }
        }
    }

    fn perform(&self, batch: &AofBatch) -> std::io::Result<()> {
        use std::sync::atomic::Ordering::Relaxed;
        if self.is_stopped() {
            return Err(stopped_error());
        }
        #[cfg(test)]
        self.hold.wait_if_marked(&batch.bytes);
        let file = &self.file;
        if batch.truncate {
            // A failed ftruncate leaves the file as it was, so a fragment
            // it would have removed is still tracked.
            self.truncate_to(file, 0)?;
            self.torn_at.store(NOT_TORN, Relaxed);
            file.sync_all()?;
        }
        if !batch.bytes.is_empty() {
            self.remove_torn_fragment(file)?;
            let queued = batch
                .opened
                .map(|opened| opened.elapsed())
                .unwrap_or_default();
            // Arc count minus the batch's own: the tasks sharing this write.
            let waiters =
                u64::try_from(Arc::strong_count(&batch.done).saturating_sub(1)).unwrap_or(u64::MAX);
            #[cfg(test)]
            let (written, result) = self.write_with_fault(file, &batch.bytes);
            #[cfg(not(test))]
            let (written, result) = write_counted(file, &batch.bytes, self.stats);
            if let Err(e) = result {
                if written > 0 {
                    self.cut_back(file, &batch.bytes, written);
                }
                return Err(e);
            }
            self.stats.record_batch(
                batch.frames,
                u64::try_from(batch.bytes.len()).unwrap_or(u64::MAX),
                waiters,
                queued,
            );
        }
        // `always`: the appends of this batch are acknowledged only once
        // durable, so the fsync belongs here — one per batch.
        if self.policy == AofSync::Always && !batch.bytes.is_empty() {
            self.sync()?;
        }
        Ok(())
    }

    /// [`sync_counted`] on the AOF, after applying an injected failure.
    fn sync(&self) -> std::io::Result<()> {
        #[cfg(test)]
        if self
            .failing_syncs
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |n| n.checked_sub(1),
            )
            .is_ok()
        {
            bump(&self.stats.fsyncs, 1);
            return Err(std::io::Error::other("injected fdatasync failure"));
        }
        sync_counted(&self.file, self.stats)
    }

    fn is_stopped(&self) -> bool {
        self.stopped.load(std::sync::atomic::Ordering::Acquire)
    }

    /// The outcome to complete `batch` with. Under `always`, a batch that
    /// carried appends and failed is a fail-stop: none of its appends may
    /// be acknowledged, and in production this does not return. A failed
    /// batch with no appends (a truncation, a flush anchor) only reports
    /// its error: the file still holds every acknowledged frame, and no
    /// write waits on it. `everysec` and `no` are unchanged: the error goes
    /// to the batch's waiters, which latch persistence unhealthy.
    fn settle(&self, batch: &AofBatch, result: std::io::Result<()>) -> std::io::Result<()> {
        let result = match result {
            Err(e) if self.policy == AofSync::Always && batch.frames > 0 => {
                self.fail_stop("aof_write", &e);
                Err(e)
            }
            other => other,
        };
        // A fail-stop that began elsewhere (`flush`'s fsync runs outside
        // `busy`) may still be ending the process: a batch completed now,
        // even with an error, would resume waiters whose callers treat an
        // error as "record it" and still fan out and reply. Never complete.
        if self.is_stopped() && self.on_fail_stop == FailStop::Exit {
            park_until_exit();
        }
        result
    }

    /// An `always` write or `fdatasync` failed: latch `stopped`, so no
    /// later batch is written or completed with success, then stop.
    ///
    /// In production this ends the process here, on the thread performing
    /// the batch and before completing it, so no waiter of the batch — on
    /// any transport, deferred or not — resumes to send a reply, fan the
    /// write out to replicas or watchers, or run another command. It holds
    /// no lock (performing is exclusive through `busy`, not a held guard)
    /// and needs none: `process::exit` runs no destructors, and std's exit
    /// hook only `try_lock`s stdout. Later batches cannot start, because
    /// this one never releases `busy`. Replies of batches already durable
    /// may or may not reach their clients before the process ends; those
    /// clients see a connection reset for a write that *is* durable, which
    /// is the unknown outcome any crash gives and a DEDUP retry resolves.
    #[cold]
    fn fail_stop(&self, operation: &'static str, error: &std::io::Error) {
        if self.stopped.swap(true, std::sync::atomic::Ordering::AcqRel) {
            // Already stopping; this batch failed because of it.
            return;
        }
        match self.on_fail_stop {
            FailStop::Exit => exit_after_aof_failure(operation, error),
            #[cfg(test)]
            FailStop::Latch => {
                error!(operation, error = %error, "AOF fail-stop (latched for a test)");
            }
        }
    }

    /// Before writing: if an earlier failure left a fragment the cut could
    /// not remove, remove it now — or fail this batch too, rather than
    /// append a frame after a fragment.
    fn remove_torn_fragment(&self, file: &std::fs::File) -> std::io::Result<()> {
        use std::sync::atomic::Ordering::Relaxed;
        let at = self.torn_at.load(Relaxed);
        if at == NOT_TORN {
            return Ok(());
        }
        if at == TORN_AT_UNKNOWN {
            return Err(std::io::Error::other(
                "AOF holds a torn frame at an unknown offset; refusing to append after it \
                 until a snapshot truncates the file",
            ));
        }
        // Never extend: a file already shorter than the fragment's start
        // (truncated from outside) has no fragment left to remove.
        if file.metadata()?.len() > at {
            self.truncate_to(file, at).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!(
                        "AOF holds a torn frame past offset {at} that cannot be removed ({e}); \
                         refusing to append after it"
                    ),
                )
            })?;
            warn!("AOF: removed the torn frame an earlier failed write left past offset {at}");
        }
        self.torn_at.store(NOT_TORN, Relaxed);
        Ok(())
    }

    /// A write failed after `written` bytes of `bytes` reached the file:
    /// cut off the torn frame among them, keeping the frames that landed
    /// whole, so the file ends on a whole frame again, as Redis does on a
    /// partial AOF write. If the cut fails, remember where it should go:
    /// the next batch retries it before writing anything.
    ///
    /// Where the fragment starts is read from the file, not tracked: the
    /// AOF is opened `O_APPEND` and appended to only by the batch being
    /// performed (`busy`), so this write's bytes are the file's last
    /// `written` bytes. That stays right when something outside the server
    /// truncated or appended to the file earlier, and never extends it.
    fn cut_back(&self, file: &std::fs::File, bytes: &[u8], written: usize) {
        use std::sync::atomic::Ordering::Relaxed;
        bump(&self.stats.torn_writes, 1);
        let whole = whole_frames_len(bytes, written);
        let torn = u64::try_from(written.saturating_sub(whole)).unwrap_or(u64::MAX);
        if torn == 0 {
            // Every byte that landed belongs to a whole frame.
            warn!("AOF write failed after {written} byte(s), on a frame boundary");
            return;
        }
        let at = match file.metadata() {
            Ok(meta) => meta.len().checked_sub(torn),
            Err(e) => {
                error!("AOF write failed part-way and the file cannot be measured ({e})");
                None
            }
        };
        let Some(at) = at.filter(|&at| at < TORN_AT_UNKNOWN) else {
            self.torn_at.store(TORN_AT_UNKNOWN, Relaxed);
            error!(
                "AOF write failed after {written} byte(s) and the torn frame cannot be located; \
                 no further appends until a snapshot truncates the file"
            );
            return;
        };
        match self.truncate_to(file, at) {
            Ok(()) => warn!(
                "AOF write failed after {written} byte(s); cut the torn frame back to offset {at}"
            ),
            Err(e) => {
                self.torn_at.store(at, Relaxed);
                error!(
                    "AOF write failed after {written} byte(s) and cutting the torn frame back to \
                     offset {at} failed too ({e}); no further appends until it succeeds"
                );
            }
        }
    }

    fn truncate_to(&self, file: &std::fs::File, len: u64) -> std::io::Result<()> {
        #[cfg(test)]
        if self
            .failing_repairs
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |n| n.checked_sub(1),
            )
            .is_ok()
        {
            return Err(std::io::Error::other("injected repair failure"));
        }
        file.set_len(len)
    }

    /// [`write_counted`], after applying the next injected fault, if any.
    #[cfg(test)]
    fn write_with_fault(&self, file: &std::fs::File, bytes: &[u8]) -> (usize, std::io::Result<()>) {
        use std::io::Write as _;
        let fault = self
            .faults
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pop_front()
            .or_else(|| {
                let mut armed = self
                    .armed
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let hit = armed.as_ref().is_some_and(|(marker, _)| {
                    bytes.windows(marker.len()).any(|w| w == marker.as_slice())
                });
                if hit {
                    armed.take().map(|(_, f)| f)
                } else {
                    None
                }
            });
        let torn = |n: usize| -> (usize, std::io::Result<()>) {
            let head = bytes.get(..n.min(bytes.len())).unwrap_or_default();
            let mut f = file;
            match f.write_all(head) {
                Ok(()) => (head.len(), Ok(())),
                Err(e) => (0, Err(e)),
            }
        };
        match fault {
            None => write_counted(file, bytes, self.stats),
            Some(AofFault::Fail(kind)) => (0, Err(kind.into())),
            Some(AofFault::Short(n)) => {
                let (head, Ok(())) = torn(n) else {
                    return (0, Err(std::io::Error::other("short-write setup failed")));
                };
                let tail = bytes.get(head..).unwrap_or_default();
                let (rest, result) = write_counted(file, tail, self.stats);
                (head + rest, result)
            }
            Some(AofFault::TornThenFail(n, kind)) => (torn(n).0, Err(kind.into())),
            Some(AofFault::TornUnrepairable(n, kind)) => {
                // The cut right after the failure, and the next batch's
                // retry of it.
                self.failing_repairs
                    .fetch_add(2, std::sync::atomic::Ordering::Relaxed);
                (torn(n).0, Err(kind.into()))
            }
            Some(AofFault::FsyncFails(_)) => {
                self.failing_syncs
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                write_counted(file, bytes, self.stats)
            }
        }
    }

    /// Lead: perform queued batches until none is left. Inline batches run
    /// here; the first that needs the disk hands the rest of the drain to
    /// the blocking pool.
    fn lead(self: &Arc<Self>) {
        loop {
            let batch = {
                let mut queue = self.lock();
                if queue.busy {
                    bump(&self.stats.lead_busy, 1);
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
            if !batch.bytes.is_empty() {
                bump(&self.stats.inline_batches, 1);
            }
            // Before releasing `busy`: a fail-stop must not let another
            // leader start. (Inline batches never carry `always` appends,
            // which need the disk, so this is belt and braces.)
            let result = self.settle(&batch, result);
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
            if !batch.bytes.is_empty() {
                bump(&self.stats.blocking_batches, 1);
            }
            let result = self.settle(&batch, result);
            batch.done.complete(result);
        }
    }
}

/// How many leading bytes of `bytes[..written]` are whole RESP frames.
/// Only on the failure path: a batch is a run of frames, so parsing them
/// one after another stops at the first one that did not land in full.
fn whole_frames_len(bytes: &[u8], written: usize) -> usize {
    let landed = bytes.get(..written).unwrap_or(bytes);
    let mut end = 0usize;
    while let Some(rest) = landed.get(end..).filter(|rest| !rest.is_empty()) {
        match Value::parse(rest) {
            Ok((_, used)) if used > 0 => end = end.saturating_add(used),
            _ => break,
        }
    }
    end
}

/// `write_all`, counting each `write(2)` and its time. Returns how many
/// bytes reached the file alongside the outcome, so a failure part-way can
/// be cut back.
fn write_counted(
    mut file: &std::fs::File,
    bytes: &[u8],
    stats: &AofStats,
) -> (usize, std::io::Result<()>) {
    use std::io::Write as _;
    let mut written = 0usize;
    while let Some(rest) = bytes.get(written..).filter(|rest| !rest.is_empty()) {
        let started = std::time::Instant::now();
        let result = file.write(rest);
        bump(&stats.write_calls, 1);
        bump(&stats.write_ns, nanos(started.elapsed()));
        match result {
            Ok(0) => return (written, Err(ErrorKind::WriteZero.into())),
            Ok(n) => written = written.saturating_add(n),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return (written, Err(e)),
        }
    }
    (written, Ok(()))
}

/// In-process tests latch a fail-stop rather than end the test runner; the
/// subprocess harness (`tests::aof_fail_stop`) runs the real exit, by
/// setting `RECACHED_TEST_AOF_FAIL_STOP=exit` for its child. Test builds
/// only: a release binary has no such variable.
#[cfg(test)]
fn test_fail_stop() -> FailStop {
    // Only in the harness's child: a stray export in a developer's or CI's
    // environment must not make the test runner itself exit.
    if std::env::var_os("RECACHED_FAIL_STOP_CHILD").is_none() {
        return FailStop::Latch;
    }
    match std::env::var("RECACHED_TEST_AOF_FAIL_STOP").as_deref() {
        Ok("exit") => FailStop::Exit,
        _ => FailStop::Latch,
    }
}

/// `RECACHED_TEST_AOF_FAULT=<fault>@<marker>`: the first batch whose bytes
/// contain `<marker>` suffers `<fault>` ([`AofFault::parse`]). Test builds
/// only, for the subprocess harness's child.
#[cfg(test)]
fn test_armed_fault() -> Option<(Vec<u8>, AofFault)> {
    std::env::var_os("RECACHED_FAIL_STOP_CHILD")?;
    let spec = std::env::var("RECACHED_TEST_AOF_FAULT").ok()?;
    let (fault, marker) = spec.split_once('@')?;
    let fault = AofFault::parse(fault).unwrap_or_else(|| panic!("bad test fault {spec:?}"));
    Some((marker.as_bytes().to_vec(), fault))
}

/// A thread that finds the writer stopped under [`FailStop::Exit`] while
/// another thread is ending the process: block here, completing nothing,
/// until the process is gone.
#[cold]
fn park_until_exit() -> ! {
    loop {
        std::thread::park();
    }
}

/// The error every batch gets once an `always` writer has stopped.
fn stopped_error() -> std::io::Error {
    std::io::Error::other(
        "the AOF stopped after a failed write or fsync under appendfsync always; \
         nothing more is written",
    )
}

/// The production [`FailStop::Exit`]: report, then end the process.
///
/// Logged through `tracing` (the operator's log) and written straight to
/// stderr as well, so the reason survives a log pipeline that drops the
/// last lines of a dying process. A failed stderr write is ignored: there
/// is nothing left to report it to, and panicking here instead would
/// unwind the drain thread and leave its batch's waiters parked rather
/// than stop the process.
#[cold]
fn exit_after_aof_failure(operation: &'static str, error: &std::io::Error) -> ! {
    use std::io::Write as _;
    // `error!` writes stdout synchronously; if stdout is a stalled pipe it
    // could block here and the process would never exit (or be restarted).
    // The watchdog exits regardless. `process::exit` itself only
    // `try_lock`s stdout. A failed spawn just leaves the direct path.
    let _watchdog = std::thread::Builder::new()
        .name("aof-fail-stop".into())
        .spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(5));
            std::process::exit(AOF_FAIL_STOP_EXIT_CODE)
        });
    let _ignored = writeln!(
        std::io::stderr(),
        "recached-server: fatal: AOF {operation} failed under appendfsync always ({error}); \
         exiting with status {AOF_FAIL_STOP_EXIT_CODE} before acknowledging the write"
    );
    error!(
        operation,
        error = %error,
        exit_code = AOF_FAIL_STOP_EXIT_CODE,
        "Can't recover from an AOF write error when appendfsync is 'always': exiting \
         before any affected write is acknowledged. Fix the disk, then restart; the AOF \
         replays every acknowledged write"
    );
    std::process::exit(AOF_FAIL_STOP_EXIT_CODE)
}

/// `sync_data`, counted.
fn sync_counted(file: &std::fs::File, stats: &AofStats) -> std::io::Result<()> {
    let started = std::time::Instant::now();
    let result = file.sync_data();
    bump(&stats.fsyncs, 1);
    bump(&stats.fsync_ns, nanos(started.elapsed()));
    result
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
///
/// Leading is deferred once per settle (see [`Coalescer`]), so that the
/// other connections ready on the worker queue their frames first and one
/// `write(2)` covers all of them. Deferring moves only *when* the batch is
/// written: the reply still waits for it.
pub(crate) struct AofGatedWriter<W> {
    inner: W,
    deferred: Arc<DeferredAof>,
    state: Arc<ServerState>,
    waiting:
        Option<std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + Send>>>,
    /// The writer `waiting` is for: a gate dropped mid-yield has not led yet,
    /// so `Drop` leads it.
    waiting_on: Option<Arc<AofShared>>,
    /// This settle already yielded once; later tickets lead at once.
    yielded: bool,
    /// A batch of this connection failed under `always`: every later write
    /// fails too, so a caller that ignores one error and flushes again
    /// (QUIT, a protocol error) cannot release the replies held behind it.
    poisoned: bool,
}

impl<W> AofGatedWriter<W> {
    pub(crate) fn new(inner: W, state: Arc<ServerState>) -> Self {
        AofGatedWriter {
            inner,
            deferred: Arc::new(DeferredAof::default()),
            state,
            waiting: None,
            waiting_on: None,
            yielded: false,
            poisoned: false,
        }
    }

    /// The handle [`with_deferred_aof`] needs to route this connection's
    /// client writes here.
    pub(crate) fn deferral(&self) -> Arc<DeferredAof> {
        Arc::clone(&self.deferred)
    }

    /// Wait until every batch this connection queued is written. An error
    /// latches persistence unhealthy; under `always` it is a fail-stop
    /// (see [`FailStop`]), and the socket write fails instead of passing
    /// on replies that would claim durability.
    fn poll_settled(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        use std::task::Poll;
        if self.poisoned {
            return Poll::Ready(Err(withheld()));
        }
        loop {
            if let Some(waiting) = &mut self.waiting {
                match waiting.as_mut().poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(result) => {
                        self.waiting = None;
                        let shared = self.waiting_on.take();
                        if let Err(e) = result {
                            self.failed(shared.as_deref(), e)?;
                        }
                    }
                }
            }
            let Some((shared, done)) = self.deferred.pop() else {
                self.yielded = false;
                return Poll::Ready(Ok(()));
            };
            if let Some(result) = done.result.get() {
                // Another task already wrote it.
                if let Err((kind, message)) = result {
                    let e = std::io::Error::new(*kind, message.clone());
                    self.failed(Some(&shared), e)?;
                }
                continue;
            }
            self.waiting_on = Some(Arc::clone(&shared));
            if !self.yielded && shared.coalesce.should_yield() {
                self.yielded = true;
                // Everyone holding the batch's completion now: anyone more
                // after the yield joined this write.
                let holders = Arc::strong_count(&done);
                self.waiting = Some(Box::pin(async move {
                    tokio::task::yield_now().await;
                    let gained = done.is_complete() || Arc::strong_count(&done) > holders;
                    shared.coalesce.observe(gained, shared.stats);
                    shared.lead();
                    gate_wait(&shared, &done).await
                }));
            } else {
                // Nobody may be writing yet: queued appends do not lead.
                shared.lead();
                self.waiting = Some(Box::pin(async move { gate_wait(&shared, &done).await }));
            }
        }
    }
}

impl<W> AofGatedWriter<W> {
    /// One of this connection's batches failed. Latch persistence
    /// unhealthy, as for any AOF failure; if its writer stopped (`always`),
    /// fail the socket write too, so the replies behind it are never sent.
    fn failed(&mut self, shared: Option<&AofShared>, e: std::io::Error) -> std::io::Result<()> {
        self.state.record_persistence_failure("aof_append", &e);
        if shared.is_some_and(AofShared::is_stopped) {
            self.poisoned = true;
            return Err(withheld());
        }
        Ok(())
    }
}

fn withheld() -> std::io::Error {
    std::io::Error::new(
        ErrorKind::BrokenPipe,
        "reply withheld: an AOF write of this connection failed under appendfsync always",
    )
}

/// Wait for a gate's batch, counting the waits another task's write made park.
async fn gate_wait(shared: &AofShared, done: &AofDone) -> std::io::Result<()> {
    if !done.is_complete() {
        bump(&shared.stats.gate_parked, 1);
    }
    done.wait().await
}

impl<W> Drop for AofGatedWriter<W> {
    /// A connection that closes with batches queued still gets them
    /// written: lead them, without waiting — including one it was still
    /// yielding before leading.
    fn drop(&mut self) {
        if let Some(shared) = self.waiting_on.take() {
            shared.lead();
        }
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
        std::task::ready!(self.poll_settled(cx))?;
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::ready!(self.poll_settled(cx))?;
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::ready!(self.poll_settled(cx))?;
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::task::ready!(self.poll_settled(cx))?;
        std::pin::Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}

/// A point in the AOF queue, from [`AofWriter::tail`].
///
/// Holding one keeps a reference to its batch's completion, which the
/// batch statistics (`waiters`) and the coalescer's "did another append
/// join" check count; a waiting QSUB can therefore read as one more joiner.
/// Both are heuristics and metrics only: they decide when a batch is
/// written, never what it covers or what a reply waits for.
pub(crate) struct AofTail {
    shared: Arc<AofShared>,
    done: Arc<AofDone>,
}

impl AofTail {
    /// Wait until every append queued before the tail was taken has been
    /// performed: written under `no` and `everysec`, written and
    /// `fdatasync`ed under `always`. Leads first, so the wait never depends
    /// on another task leading the batch. Holds nothing while it waits;
    /// cancelling it strands nothing (the lead already ran or handed the
    /// batch to the blocking pool).
    ///
    /// The error is the tail batch's own: under `everysec`/`no` a failure
    /// is non-fatal (persistence latches unhealthy, as for any write);
    /// under `always` the writer has stopped, see [`Self::writer_stopped`].
    pub(crate) async fn settled(&self) -> std::io::Result<()> {
        self.shared.lead();
        gate_wait(&self.shared, &self.done).await
    }

    /// Whether the AOF stopped after a failure under `always`. Nothing a
    /// failed batch held may be shown to a client then.
    pub(crate) fn writer_stopped(&self) -> bool {
        self.shared.is_stopped()
    }
}

impl AofWriter {
    pub(crate) async fn open(path: PathBuf, sync: AofSync) -> std::io::Result<Self> {
        let mut opts = tokio::fs::OpenOptions::new();
        // `append`: every write lands at the end of the file, wherever
        // anything else left it. `cut_back` relies on this to find a torn
        // frame (the failed write's bytes are the file's last ones).
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
                coalesce: Coalescer::new(true),
                stats: &AOF_STATS,
                torn_at: std::sync::atomic::AtomicU64::new(NOT_TORN),
                stopped: std::sync::atomic::AtomicBool::new(false),
                #[cfg(not(test))]
                on_fail_stop: FailStop::Exit,
                #[cfg(test)]
                on_fail_stop: test_fail_stop(),
                #[cfg(test)]
                faults: std::sync::Mutex::new(VecDeque::new()),
                #[cfg(test)]
                armed: std::sync::Mutex::new(test_armed_fault()),
                #[cfg(test)]
                failing_repairs: std::sync::atomic::AtomicU32::new(0),
                #[cfg(test)]
                failing_syncs: std::sync::atomic::AtomicU32::new(0),
                #[cfg(test)]
                hold: AofHold::from_env(),
            }),
        })
    }

    /// Hold the next batch containing `marker` before its write, until
    /// [`release_hold`](Self::release_hold) (or ten seconds). See [`AofHold`].
    #[cfg(test)]
    pub(crate) fn hold_batch_containing(&self, marker: &[u8]) {
        self.shared
            .hold
            .arm(marker, std::time::Duration::from_secs(10));
    }

    /// Let a held batch go on.
    #[cfg(test)]
    pub(crate) fn release_hold(&self) {
        self.shared.hold.release();
    }

    /// Wait until a performer is held, failing the test after five seconds.
    #[cfg(test)]
    pub(crate) async fn wait_until_held(&self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while self.shared.hold.held() == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "no AOF batch was held"
            );
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
    }

    /// Turn cross-connection coalescing (see [`Coalescer`]) on or off.
    /// `RECACHED_AOF_COALESCE=off` sets it off, for A/B measurement.
    pub(crate) fn set_coalescing(&self, enabled: bool) {
        self.shared
            .coalesce
            .enabled
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    /// Count this writer's batches in `stats` instead of the process-wide
    /// [`AOF_STATS`], so a test can read its own numbers.
    #[cfg(test)]
    fn with_private_stats(mut self) -> (Self, &'static AofStats) {
        let stats: &'static AofStats = Box::leak(Box::new(AofStats::new()));
        let shared =
            Arc::get_mut(&mut self.shared).expect("a freshly opened writer has no other handle");
        shared.stats = stats;
        (self, stats)
    }

    /// Make the next batch's write suffer `fault`.
    #[cfg(test)]
    pub(crate) fn inject_fault(&self, fault: AofFault) {
        self.shared
            .faults
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push_back(fault);
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
                    let batch = AofBatch::new();
                    queue.newest = Some(Arc::clone(&batch.done));
                    queue.batches.push_back(batch);
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

    /// Where the AOF queue ends now: the newest batch, unless it has
    /// already completed (then `None` — everything queued so far is
    /// performed).
    ///
    /// Taken under the all-write barrier, this covers every client write
    /// committed before the barrier, deferred ones included: a deferred
    /// write queues its frame under its stripes before the store change
    /// becomes reachable, so its frame is in this batch or an earlier one.
    /// QSUB uses it to send `qstate` only once what it shows has reached
    /// the AOF stage a reply would wait for (6N-C PR 0c, RV-8).
    pub(crate) fn tail(&self) -> Option<AofTail> {
        let queue = self.shared.lock();
        let done = queue.newest.as_ref().filter(|done| !done.is_complete())?;
        Some(AofTail {
            shared: Arc::clone(&self.shared),
            done: Arc::clone(done),
        })
    }

    pub(crate) async fn append(&self, resp: &[u8]) -> std::io::Result<()> {
        self.enqueue(false, |batch| batch.push_frame(resp))
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
    /// connection when it settles. Under `everysec` and `no` the client's
    /// reply is the same either way: the failure only latches persistence
    /// unhealthy for later writes. Under `always` the process stops before
    /// the reply is sent (see [`FailStop`]).
    pub(crate) async fn append_client_write(&self, resp: &[u8]) -> std::io::Result<()> {
        let deferred = AOF_DEFERRED.try_with(Arc::clone).ok();
        match deferred {
            Some(deferred) => {
                let done = self.queue_only(false, |batch| batch.push_frame(resp));
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
        tokio::task::spawn_blocking(move || {
            if shared.is_stopped() {
                return Err(stopped_error());
            }
            if shared.is_stopped() && shared.on_fail_stop == FailStop::Exit {
                park_until_exit();
            }
            let result = shared.sync();
            // `always`: every acknowledged frame was synced by its own
            // batch, but after a failed fdatasync the kernel may have
            // dropped dirty pages and a later one can report success
            // regardless, so no later sync could be trusted to vouch for
            // a write either. Stop, as for a batch's own fsync.
            if let Err(e) = &result
                && shared.policy == AofSync::Always
            {
                shared.fail_stop("aof_fsync", e);
            }
            result
        })
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

/// The connection gate under concurrency: coalescing across connections,
/// acknowledgement order, cross-connection file order, cancellation and
/// failure. Each test reads its writer's private [`AofStats`].
#[cfg(test)]
mod gate_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::task::Poll;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "recached_gate_{}_{}_{}",
            name,
            std::process::id(),
            next_conn_id()
        ));
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    fn state() -> Arc<ServerState> {
        ServerState::for_tests(
            Arc::new(SnapshotConfig {
                path: std::env::temp_dir().join("recached_gate_unused.rdb"),
                last_save: std::sync::atomic::AtomicI64::new(0),
                checkpoint_id: AtomicU64::new(0),
            }),
            ReplHub::new(),
            false,
        )
    }

    fn frame(tag: &str) -> Vec<u8> {
        format!("*2\r\n$3\r\nTAG\r\n${}\r\n{}\r\n", tag.len(), tag).into_bytes()
    }

    /// A socket that checks, on every byte it is handed, that the AOF file
    /// already holds the frame of the command it answers.
    struct ProbeSocket {
        aof: PathBuf,
        frame: Vec<u8>,
        early: Arc<AtomicU64>,
    }

    impl tokio::io::AsyncWrite for ProbeSocket {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            let on_disk = std::fs::read(&self.aof).unwrap_or_default();
            if !on_disk.windows(self.frame.len()).any(|w| w == self.frame) {
                self.early.fetch_add(1, Ordering::SeqCst);
            }
            Poll::Ready(Ok(buf.len()))
        }
        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    struct Run {
        batches: u64,
        fsyncs: u64,
        frames: u64,
        early_replies: u64,
        on_disk: Vec<u8>,
        healthy: bool,
    }

    /// `clients` connections become ready together; each appends one frame
    /// through its gate (`frames_each` with a pipeline) and replies.
    async fn connections(
        name: &str,
        policy: AofSync,
        clients: usize,
        frames_each: usize,
        coalesce: bool,
        fault: Option<AofFault>,
    ) -> Run {
        let dir = scratch(name);
        let path = dir.join("f.aof");
        let (w, stats) = AofWriter::open(path.clone(), policy)
            .await
            .unwrap()
            .with_private_stats();
        w.set_coalescing(coalesce);
        if let Some(fault) = fault {
            w.inject_fault(fault);
        }
        let w = Arc::new(w);
        let state = state();
        let early = Arc::new(AtomicU64::new(0));
        let barrier = Arc::new(tokio::sync::Barrier::new(clients));
        let mut tasks = Vec::new();
        for c in 0..clients {
            let (w, state, early, barrier, path) = (
                Arc::clone(&w),
                Arc::clone(&state),
                Arc::clone(&early),
                Arc::clone(&barrier),
                path.clone(),
            );
            tasks.push(tokio::spawn(async move {
                let last = frame(&format!("c{c}f{}", frames_each - 1));
                let gate = AofGatedWriter::new(
                    ProbeSocket {
                        aof: path,
                        frame: last,
                        early,
                    },
                    Arc::clone(&state),
                );
                let mut socket = tokio::io::BufWriter::new(gate);
                let deferral = socket.get_ref().deferral();
                barrier.wait().await;
                with_deferred_aof(deferral, async {
                    for f in 0..frames_each {
                        w.append_client_write(&frame(&format!("c{c}f{f}")))
                            .await
                            .unwrap();
                        tokio::io::AsyncWriteExt::write_all(&mut socket, b"+OK\r\n")
                            .await
                            .unwrap();
                    }
                    tokio::io::AsyncWriteExt::flush(&mut socket).await.unwrap();
                })
                .await;
            }));
        }
        for task in tasks {
            tokio::time::timeout(std::time::Duration::from_secs(10), task)
                .await
                .expect("no gate may hang")
                .unwrap();
        }
        let on_disk = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        Run {
            batches: stats.batches.load(Ordering::Relaxed),
            fsyncs: stats.fsyncs.load(Ordering::Relaxed),
            frames: stats.frames.load(Ordering::Relaxed),
            early_replies: early.load(Ordering::SeqCst),
            on_disk,
            healthy: state.persistence_is_healthy(),
        }
    }

    fn holds_every_frame(on_disk: &[u8], clients: usize, frames_each: usize) -> bool {
        (0..clients).all(|c| {
            (0..frames_each).all(|f| {
                let fr = frame(&format!("c{c}f{f}"));
                on_disk.windows(fr.len()).filter(|w| *w == fr).count() == 1
            })
        })
    }

    /// The 6M-A finding, as a test: fifty connections ready on one worker
    /// used to cost fifty `write(2)`s. With coalescing they share a few; with
    /// it off — the old behaviour, kept as the control — each is its own.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn fifty_connections_on_one_worker_share_their_aof_write() {
        let on = connections("coalesce_on", AofSync::EverySec, 50, 1, true, None).await;
        assert_eq!(on.frames, 50);
        assert!(
            on.batches <= 3,
            "50 ready connections took {} writes with coalescing",
            on.batches
        );
        assert_eq!(on.early_replies, 0, "a reply preceded its AOF write");
        assert!(holds_every_frame(&on.on_disk, 50, 1));

        let off = connections("coalesce_off", AofSync::EverySec, 50, 1, false, None).await;
        assert_eq!(off.batches, 50, "the control: one write per connection");
        assert_eq!(off.early_replies, 0);
        assert!(holds_every_frame(&off.on_disk, 50, 1));
    }

    /// No reply byte reaches a socket before the frame of the command it
    /// answers is in the file — across policies, pipelines and workers.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn no_reply_precedes_its_aof_write_under_any_policy() {
        for (name, policy) in [
            ("no", AofSync::No),
            ("everysec", AofSync::EverySec),
            ("always", AofSync::Always),
        ] {
            for frames_each in [1, 16] {
                let run = connections(name, policy, 32, frames_each, true, None).await;
                assert_eq!(run.early_replies, 0, "{name} P={frames_each}");
                assert!(holds_every_frame(&run.on_disk, 32, frames_each), "{name}");
                assert_eq!(run.frames, 32 * frames_each as u64, "{name}");
                // `always`: every batch that carried frames was fdatasynced
                // before it completed (`perform`), and replies wait for
                // completion — the file check above cannot see an fsync.
                if policy == AofSync::Always {
                    assert_eq!(run.fsyncs, run.batches, "always: one fdatasync per batch");
                }
            }
        }
    }

    /// Frames keep the order they were queued in, across connections and
    /// workers: appends are numbered under a lock, as the store's ordering
    /// stripes number conflicting writes, and the file must count up.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn frames_from_many_connections_keep_their_queue_order() {
        let dir = scratch("order");
        let path = dir.join("f.aof");
        let w = Arc::new(
            AofWriter::open(path.clone(), AofSync::EverySec)
                .await
                .unwrap(),
        );
        let state = state();
        let order = Arc::new(tokio::sync::Mutex::new(0u64));
        let mut tasks = Vec::new();
        for _ in 0..50 {
            let (w, state, order) = (Arc::clone(&w), Arc::clone(&state), Arc::clone(&order));
            tasks.push(tokio::spawn(async move {
                let gate = AofGatedWriter::new(tokio::io::sink(), state);
                let deferral = gate.deferral();
                let mut socket = tokio::io::BufWriter::new(gate);
                with_deferred_aof(deferral, async {
                    for _ in 0..40 {
                        for _ in 0..4 {
                            let mut next = order.lock().await;
                            w.append_client_write(&frame(&format!("{:08}", *next)))
                                .await
                                .unwrap();
                            *next += 1;
                        }
                        tokio::io::AsyncWriteExt::write_all(&mut socket, b"+OK\r\n")
                            .await
                            .unwrap();
                        tokio::io::AsyncWriteExt::flush(&mut socket).await.unwrap();
                    }
                })
                .await;
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        w.flush().await.unwrap();
        let on_disk = std::fs::read(&path).unwrap();
        let mut offset = 0;
        let mut expected = 0u64;
        while offset < on_disk.len() {
            let (value, used) = Value::parse(&on_disk[offset..]).expect("whole frames");
            offset += used;
            let Value::Array(Some(items)) = value else {
                panic!("not a frame")
            };
            let Some(Value::BulkString(Some(tag))) = items.get(1) else {
                panic!("no tag")
            };
            assert_eq!(
                std::str::from_utf8(tag).unwrap(),
                format!("{expected:08}"),
                "frame out of queue order"
            );
            expected += 1;
        }
        assert_eq!(expected, 50 * 40 * 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A connection dropped while its gate is yielding — before it led —
    /// still gets its frame written, by the drop.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn a_gate_dropped_mid_yield_still_writes_its_frame() {
        let dir = scratch("drop_mid_yield");
        let path = dir.join("f.aof");
        let (w, stats) = AofWriter::open(path.clone(), AofSync::EverySec)
            .await
            .unwrap()
            .with_private_stats();
        let w = Arc::new(w);
        let mut gate = AofGatedWriter::new(tokio::io::sink(), state());
        let deferral = gate.deferral();
        with_deferred_aof(deferral, w.append_client_write(&frame("orphan")))
            .await
            .unwrap();
        // One poll: the gate yields (coalescing probes on a fresh writer)
        // and has not led yet.
        let pending = std::future::poll_fn(|cx| {
            Poll::Ready(std::pin::Pin::new(&mut gate).poll_flush(cx).is_pending())
        })
        .await;
        assert!(pending, "the gate should be yielding before it leads");
        assert_eq!(stats.batches.load(Ordering::Relaxed), 0, "nothing led yet");
        drop(gate);
        assert_eq!(stats.batches.load(Ordering::Relaxed), 1, "the drop led it");
        let on_disk = std::fs::read(&path).unwrap();
        assert_eq!(on_disk, frame("orphan"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A failed shared write reaches every connection in it: none hangs,
    /// and persistence is latched unhealthy.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn a_failed_shared_write_reaches_every_waiter() {
        for fault in [
            AofFault::Fail(ErrorKind::StorageFull),
            AofFault::TornThenFail(7, ErrorKind::Other),
        ] {
            let run = connections("fail_shared", AofSync::EverySec, 20, 1, true, Some(fault)).await;
            assert!(!run.healthy, "{fault:?}: the failure must latch unhealthy");
        }
    }

    /// A short `write(2)` is finished, not mistaken for success or failure.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn a_short_write_is_completed() {
        let run = connections(
            "short",
            AofSync::EverySec,
            20,
            2,
            true,
            Some(AofFault::Short(5)),
        )
        .await;
        assert!(run.healthy);
        assert_eq!(run.early_replies, 0);
        assert!(holds_every_frame(&run.on_disk, 20, 2));
    }

    /// One client: coalescing finds nobody to wait for and backs off, so
    /// most flushes lead at once instead of paying a scheduler pass.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn a_lone_client_backs_coalescing_off() {
        let dir = scratch("lone");
        let path = dir.join("f.aof");
        let (w, stats) = AofWriter::open(path.clone(), AofSync::EverySec)
            .await
            .unwrap()
            .with_private_stats();
        let gate = AofGatedWriter::new(tokio::io::sink(), state());
        let deferral = gate.deferral();
        let mut socket = tokio::io::BufWriter::new(gate);
        with_deferred_aof(deferral, async {
            for i in 0..340 {
                w.append_client_write(&frame(&i.to_string())).await.unwrap();
                tokio::io::AsyncWriteExt::write_all(&mut socket, b"+OK\r\n")
                    .await
                    .unwrap();
                tokio::io::AsyncWriteExt::flush(&mut socket).await.unwrap();
            }
        })
        .await;
        let yields = stats.coalesce_gained.load(Ordering::Relaxed)
            + stats.coalesce_missed.load(Ordering::Relaxed);
        // Two misses, then 32 straight leads: 340 flushes → 20 probes.
        assert_eq!(yields, 20, "probes with one client");
        assert_eq!(stats.batches.load(Ordering::Relaxed), 340);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn set(key: &str, value: &str) -> Vec<u8> {
        format!(
            "*3\r\n$3\r\nSET\r\n${}\r\n{}\r\n${}\r\n{}\r\n",
            key.len(),
            key,
            value.len(),
            value
        )
        .into_bytes()
    }

    /// A write that fails part-way is cut back to the last whole frame, so
    /// the next batch lands right after it and the file still replays to
    /// exactly the writes that succeeded. Before, the fragment stayed and
    /// every later frame sat behind it: replay refused the file.
    #[tokio::test]
    async fn a_torn_write_is_cut_back_and_the_file_stays_replayable() {
        for policy in [AofSync::EverySec, AofSync::Always] {
            let dir = scratch("torn_cut");
            let path = dir.join("f.aof");
            let (w, stats) = AofWriter::open(path.clone(), policy)
                .await
                .unwrap()
                .with_private_stats();
            w.append(&set("a", "1")).await.unwrap();
            w.inject_fault(AofFault::TornThenFail(9, ErrorKind::StorageFull));
            assert!(w.append(&set("b", "2")).await.is_err());
            if policy == AofSync::Always {
                // A fail-stop (latched in tests): nothing more is written,
                // and the file was still cut back for the restart.
                assert!(w.append(&set("c", "3")).await.is_err());
                assert_eq!(std::fs::read(&path).unwrap(), set("a", "1"));
                let _ = std::fs::remove_dir_all(&dir);
                continue;
            }
            w.append(&set("c", "3")).await.unwrap();
            let on_disk = std::fs::read(&path).unwrap();
            assert_eq!(on_disk, [set("a", "1"), set("c", "3")].concat());
            assert_eq!(stats.torn_writes.load(Ordering::Relaxed), 1);

            let store = KeyValueStore::new();
            replay_aof_recovering(&store, &path, 0)
                .await
                .expect("the AOF replays after a torn write");
            assert_eq!(
                store.execute(Command::Get("c".into())),
                Value::BulkString(Some(b"3".to_vec()))
            );
            assert_eq!(
                store.execute(Command::Get("b".into())),
                Value::BulkString(None)
            );
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// If the cut fails too, nothing is appended after the fragment: the
    /// next batch retries the cut and fails without writing, and the first
    /// batch whose cut succeeds writes after the last whole frame.
    #[tokio::test]
    async fn an_uncut_torn_frame_blocks_appends_until_it_is_removed() {
        let dir = scratch("torn_uncut");
        let path = dir.join("f.aof");
        let w = AofWriter::open(path.clone(), AofSync::EverySec)
            .await
            .unwrap();
        w.append(&set("a", "1")).await.unwrap();
        w.inject_fault(AofFault::TornUnrepairable(9, ErrorKind::Other));
        assert!(w.append(&set("b", "2")).await.is_err());
        let fragment = set("b", "2")[..9].to_vec();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            [set("a", "1"), fragment.clone()].concat()
        );
        let err = w.append(&set("c", "3")).await.unwrap_err();
        assert!(err.to_string().contains("torn frame"), "{err}");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            [set("a", "1"), fragment].concat(),
            "nothing may be appended after a torn frame"
        );
        w.append(&set("d", "4")).await.unwrap();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            [set("a", "1"), set("d", "4")].concat()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A truncation (snapshot checkpoint) resets the tracked length: the
    /// next torn write is cut back to the truncated file, not to the old
    /// length (which would have extended the file with zeros).
    #[tokio::test]
    async fn a_torn_write_after_truncate_cuts_back_to_the_truncated_file() {
        let dir = scratch("torn_after_truncate");
        let path = dir.join("f.aof");
        let w = AofWriter::open(path.clone(), AofSync::EverySec)
            .await
            .unwrap();
        w.append(&set("a", "1")).await.unwrap();
        w.append(&set("b", "2")).await.unwrap();
        w.truncate().await.unwrap();
        w.append(&set("c", "3")).await.unwrap();
        w.inject_fault(AofFault::TornThenFail(5, ErrorKind::Other));
        assert!(w.append(&set("d", "4")).await.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), set("c", "3"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The tracked length starts from the file as found: appends to an AOF
    /// that already holds frames are cut back to its real end.
    #[tokio::test]
    async fn a_torn_write_on_a_reopened_file_keeps_its_old_frames() {
        let dir = scratch("torn_reopen");
        let path = dir.join("f.aof");
        std::fs::write(&path, set("old", "0")).unwrap();
        let w = AofWriter::open(path.clone(), AofSync::EverySec)
            .await
            .unwrap();
        w.inject_fault(AofFault::TornThenFail(4, ErrorKind::Other));
        assert!(w.append(&set("x", "1")).await.is_err());
        w.append(&set("y", "2")).await.unwrap();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            [set("old", "0"), set("y", "2")].concat()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Something outside the server truncated the AOF (a `copytruncate`
    /// log rotation, `: > file`), then a write tore: the cut must not
    /// extend the file back to where it used to end, filling it with NULs
    /// that make startup refuse it. Found in the 6M review (F2).
    #[tokio::test]
    async fn a_torn_write_after_an_outside_truncation_never_extends_the_file() {
        let dir = scratch("torn_outside_truncate");
        let path = dir.join("f.aof");
        let w = AofWriter::open(path.clone(), AofSync::EverySec)
            .await
            .unwrap();
        w.append(&set("a", "1")).await.unwrap();
        w.append(&set("b", "2")).await.unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(0)
            .unwrap();
        w.inject_fault(AofFault::TornThenFail(5, ErrorKind::Other));
        assert!(w.append(&set("c", "3")).await.is_err());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"",
            "cut back to empty, not extended"
        );
        w.append(&set("d", "4")).await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), set("d", "4"));
        let store = KeyValueStore::new();
        replay_aof_recovering(&store, &path, 0).await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A torn frame whose cut failed, then a truncation whose ftruncate
    /// fails too: the fragment must still be known, so the next append
    /// removes it instead of landing after it. Found in the 6M review (F1).
    #[tokio::test]
    async fn a_failed_truncation_keeps_the_torn_fragment_tracked() {
        let dir = scratch("torn_failed_truncate");
        let path = dir.join("f.aof");
        let w = AofWriter::open(path.clone(), AofSync::EverySec)
            .await
            .unwrap();
        w.append(&set("a", "1")).await.unwrap();
        // Two failing truncations: the cut, then the truncate's ftruncate(0).
        w.inject_fault(AofFault::TornUnrepairable(9, ErrorKind::Other));
        assert!(w.append(&set("b", "2")).await.is_err());
        assert!(w.truncate().await.is_err(), "injected ftruncate(0) failure");
        w.append(&set("d", "4")).await.unwrap();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            [set("a", "1"), set("d", "4")].concat()
        );
        let store = KeyValueStore::new();
        replay_aof_recovering(&store, &path, 0).await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Frames appended to the file by something else (a second server
    /// misconfigured onto the same path) are not cut with a torn write:
    /// the cut point comes from the file, not from what this writer last
    /// wrote. Found in the 6M review (F3).
    #[tokio::test]
    async fn a_torn_write_keeps_frames_appended_from_outside() {
        let dir = scratch("torn_outside_append");
        let path = dir.join("f.aof");
        let w = AofWriter::open(path.clone(), AofSync::EverySec)
            .await
            .unwrap();
        w.append(&set("a", "1")).await.unwrap();
        {
            use std::io::Write as _;
            let mut other = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            other.write_all(&set("other", "x")).unwrap();
        }
        w.inject_fault(AofFault::TornThenFail(5, ErrorKind::Other));
        assert!(w.append(&set("c", "3")).await.is_err());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            [set("a", "1"), set("other", "x")].concat()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The cut removes only the torn frame: frames of the failed batch that
    /// reached the file whole stay, as their commands were applied. Found
    /// in the 6M review (F4).
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn a_torn_write_keeps_the_batch_frames_that_landed_whole() {
        let dir = scratch("torn_keep_whole");
        let path = dir.join("f.aof");
        let w = Arc::new(
            AofWriter::open(path.clone(), AofSync::EverySec)
                .await
                .unwrap(),
        );
        let (a, b) = (set("a", "1"), set("b", "2"));
        // One batch [a, b]: all of a and 3 bytes of b land.
        w.inject_fault(AofFault::TornThenFail(a.len() + 3, ErrorKind::Other));
        let gate = AofGatedWriter::new(tokio::io::sink(), state());
        let deferral = gate.deferral();
        let mut socket = tokio::io::BufWriter::new(gate);
        with_deferred_aof(deferral, async {
            w.append_client_write(&a).await.unwrap();
            w.append_client_write(&b).await.unwrap();
            tokio::io::AsyncWriteExt::flush(&mut socket).await.unwrap();
        })
        .await;
        assert_eq!(std::fs::read(&path).unwrap(), a);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn whole_frames_stop_at_the_first_partial_one() {
        let (a, b) = (set("a", "1"), set("bb", "22"));
        let both = [a.clone(), b.clone()].concat();
        assert_eq!(whole_frames_len(&both, 0), 0);
        assert_eq!(whole_frames_len(&both, 3), 0);
        assert_eq!(whole_frames_len(&both, a.len()), a.len());
        assert_eq!(whole_frames_len(&both, a.len() + 1), a.len());
        assert_eq!(whole_frames_len(&both, both.len()), both.len());
        // A written count past the buffer (cannot happen) is clamped.
        assert_eq!(whole_frames_len(&both, both.len() + 10), both.len());
    }

    // ── 6N-0B: `always` fail-stop (latched in-process; the real exit is
    // covered end to end by `tests::aof_fail_stop`) ───────────────────────

    /// Every append sharing a failed `always` batch fails, and so does
    /// every later batch: nothing is written once the writer stopped.
    #[tokio::test(flavor = "current_thread")]
    async fn an_always_failure_fails_the_whole_batch_and_every_later_one() {
        for fault in [
            AofFault::Fail(ErrorKind::StorageFull),
            AofFault::TornThenFail(5, ErrorKind::Other),
            AofFault::FsyncFails(ErrorKind::Other),
        ] {
            let dir = scratch("always_stop");
            let path = dir.join("f.aof");
            let w = Arc::new(
                AofWriter::open(path.clone(), AofSync::Always)
                    .await
                    .unwrap(),
            );
            w.append(&set("a", "1")).await.unwrap();
            w.inject_fault(fault);
            // Queued together before anyone leads: one batch of three.
            let (d1, d2, d3) = (
                w.queue_only(false, |b| b.push_frame(&set("b", "2"))),
                w.queue_only(false, |b| b.push_frame(&set("c", "3"))),
                w.queue_only(false, |b| b.push_frame(&set("d", "4"))),
            );
            assert!(Arc::ptr_eq(&d1, &d3), "one shared batch");
            w.shared.lead();
            for done in [d1, d2, d3] {
                assert!(
                    done.wait().await.is_err(),
                    "{fault:?}: a waiter resumed with success"
                );
            }
            assert!(w.shared.is_stopped(), "{fault:?}");
            let before = std::fs::read(&path).unwrap();
            let err = w.append(&set("e", "5")).await.unwrap_err();
            assert!(err.to_string().contains("stopped"), "{err}");
            assert!(w.flush().await.is_err(), "{fault:?}: flush after a stop");
            assert_eq!(
                std::fs::read(&path).unwrap(),
                before,
                "{fault:?}: written after the stop"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// Under `always` the gate refuses to pass replies once its batch
    /// failed, so the connection's buffered `+OK`s never reach the client.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn the_gate_withholds_replies_behind_a_failed_always_batch() {
        let dir = scratch("gate_withhold");
        let w = Arc::new(
            AofWriter::open(dir.join("f.aof"), AofSync::Always)
                .await
                .unwrap(),
        );
        w.inject_fault(AofFault::Fail(ErrorKind::StorageFull));
        let gate = AofGatedWriter::new(Vec::<u8>::new(), state());
        let deferral = gate.deferral();
        let mut socket = tokio::io::BufWriter::new(gate);
        let flushed = with_deferred_aof(deferral, async {
            w.append_client_write(&set("a", "1")).await.unwrap();
            tokio::io::AsyncWriteExt::write_all(&mut socket, b"+OK\r\n")
                .await
                .unwrap();
            tokio::io::AsyncWriteExt::flush(&mut socket).await
        })
        .await;
        assert!(flushed.is_err(), "the reply was released");
        assert!(
            socket.get_ref().inner.is_empty(),
            "bytes reached the socket"
        );
        // Sticky (6N-0 review F2): a caller that ignores the error and
        // flushes again (QUIT, a protocol error) finds no ticket left to
        // fail on, and must still not release the buffered reply.
        assert!(tokio::io::AsyncWriteExt::flush(&mut socket).await.is_err());
        assert!(
            tokio::io::AsyncWriteExt::write_all(socket.get_mut(), b"+OK\r\n")
                .await
                .is_err()
        );
        assert!(
            socket.get_ref().inner.is_empty(),
            "bytes reached the socket on a retry"
        );
        assert!(!socket.get_ref().state.persistence_is_healthy());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `everysec` keeps its contract: the reply passes, persistence latches
    /// unhealthy (later writes fail closed), and the writer keeps working.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn everysec_failures_still_pass_replies_and_do_not_stop() {
        let dir = scratch("everysec_pass");
        let w = Arc::new(
            AofWriter::open(dir.join("f.aof"), AofSync::EverySec)
                .await
                .unwrap(),
        );
        w.inject_fault(AofFault::Fail(ErrorKind::StorageFull));
        let gate = AofGatedWriter::new(Vec::<u8>::new(), state());
        let deferral = gate.deferral();
        let mut socket = tokio::io::BufWriter::new(gate);
        with_deferred_aof(deferral, async {
            w.append_client_write(&set("a", "1")).await.unwrap();
            tokio::io::AsyncWriteExt::write_all(&mut socket, b"+OK\r\n")
                .await
                .unwrap();
            tokio::io::AsyncWriteExt::flush(&mut socket).await.unwrap();
        })
        .await;
        assert_eq!(socket.get_ref().inner, b"+OK\r\n");
        assert!(!socket.get_ref().state.persistence_is_healthy());
        assert!(!w.shared.is_stopped());
        w.inject_fault(AofFault::FsyncFails(ErrorKind::Other));
        w.append(&set("b", "2")).await.unwrap();
        assert!(
            w.flush().await.is_err(),
            "the ticker's fsync failure is reported"
        );
        assert!(!w.shared.is_stopped(), "everysec never stops");
        w.append(&set("c", "3")).await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A failed `fdatasync` from `flush` (checkpoint, shutdown) is a
    /// fail-stop under `always`.
    #[tokio::test]
    async fn an_always_flush_fsync_failure_stops() {
        let dir = scratch("always_flush");
        let w = AofWriter::open(dir.join("f.aof"), AofSync::Always)
            .await
            .unwrap();
        w.append(&set("a", "1")).await.unwrap();
        w.shared
            .failing_syncs
            .store(1, std::sync::atomic::Ordering::Relaxed);
        assert!(w.flush().await.is_err());
        assert!(w.shared.is_stopped());
        assert!(w.append(&set("b", "2")).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A truncation that fails with no append in its batch acknowledged
    /// nothing, so under `always` it is reported (SAVE fails, persistence
    /// latches unhealthy) but is not a fail-stop: the file still holds
    /// every acknowledged frame.
    #[tokio::test]
    async fn an_always_truncation_failure_alone_does_not_stop() {
        let dir = scratch("always_truncate");
        let path = dir.join("f.aof");
        let w = AofWriter::open(path.clone(), AofSync::Always)
            .await
            .unwrap();
        w.append(&set("a", "1")).await.unwrap();
        w.shared
            .failing_repairs
            .store(1, std::sync::atomic::Ordering::Relaxed);
        assert!(w.truncate().await.is_err());
        assert!(!w.shared.is_stopped());
        w.append(&set("b", "2")).await.unwrap();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            [set("a", "1"), set("b", "2")].concat()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An append that joins a truncation's batch is a frame of it: if the
    /// truncation fails, that append was not written, so it stops.
    #[tokio::test(flavor = "current_thread")]
    async fn an_always_truncation_failure_with_appends_in_its_batch_stops() {
        let dir = scratch("always_truncate_frames");
        let w = AofWriter::open(dir.join("f.aof"), AofSync::Always)
            .await
            .unwrap();
        w.shared
            .failing_repairs
            .store(1, std::sync::atomic::Ordering::Relaxed);
        let truncated = w.queue_only(true, |b| b.truncate = true);
        let joined = w.queue_only(false, |b| b.push_frame(&set("a", "1")));
        assert!(Arc::ptr_eq(&truncated, &joined));
        w.shared.lead();
        assert!(joined.wait().await.is_err());
        assert!(w.shared.is_stopped());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn harness_fault_specs_parse() {
        assert!(matches!(
            AofFault::parse("fail:enospc"),
            Some(AofFault::Fail(ErrorKind::StorageFull))
        ));
        assert!(matches!(
            AofFault::parse("short:3"),
            Some(AofFault::Short(3))
        ));
        assert!(matches!(
            AofFault::parse("torn:9:eio"),
            Some(AofFault::TornThenFail(9, ErrorKind::Other))
        ));
        assert!(matches!(
            AofFault::parse("fsync:eio"),
            Some(AofFault::FsyncFails(ErrorKind::Other))
        ));
        assert!(AofFault::parse("fail:bogus").is_none());
        assert!(AofFault::parse("explode").is_none());
    }
}
