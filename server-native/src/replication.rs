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
}

#[derive(Default)]
struct ReplBacklog {
    frames: VecDeque<ReplFrame>,
    bytes: usize,
}

/// Connected-replica registry. `count` mirrors `senders.len()` (updated by
/// every writer while holding the lock) so the per-write hot path can skip
/// the mutex entirely when no replica is connected.
pub(crate) struct ReplHub {
    pub(crate) senders: tokio::sync::Mutex<Vec<ReplicaHandle>>,
    pub(crate) count: AtomicUsize,
    run_id: String,
    offset: AtomicU64,
    backlog: tokio::sync::Mutex<ReplBacklog>,
    backlog_bytes: AtomicUsize,
    queue_bytes: AtomicUsize,
    /// Per-key ordering domains shared by client writes and full-sync
    /// snapshots. Conflicting writes hold their guards until propagation has
    /// completed; independent keys remain concurrent.
    write_order: Vec<tokio::sync::Mutex<()>>,
    /// Stream DEDUP identities with the writes they belong to
    /// (`RECACHED_REPL_DEDUP`, on by default). Fixed at startup: every
    /// replica of this primary must then accept them, see [`REPL_CAP_DEDUP`].
    dedup: AtomicBool,
    /// On a replica: whether the primary it follows streams DEDUP identities.
    /// False while following a pre-0.6 primary, or one with
    /// `RECACHED_REPL_DEDUP=off` — a promoted replica then holds no marks.
    pub(crate) upstream_dedup: AtomicBool,
}

const WRITE_ORDER_STRIPES: usize = 256;

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
    /// Deepest send queue across connected replicas, in frames.
    ///
    /// Queue depth measures frames waiting to be written to a replica. Applied
    /// offset acknowledgements provide the separate end-to-end lag metric.
    pub(crate) async fn max_queue_depth(&self) -> usize {
        let senders = self.senders.lock().await;
        senders
            .iter()
            .map(|r| r.tx.max_capacity().saturating_sub(r.tx.capacity()))
            .max()
            .unwrap_or(0)
    }

    pub(crate) async fn max_queue_bytes(&self) -> usize {
        let senders = self.senders.lock().await;
        senders
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
        let mut backlog = self.backlog.lock().await;
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
        gauge!("recached_replication_backlog_bytes").set(backlog.bytes as f64);

        let mut reg = self.senders.lock().await;
        reg.retain(|r| {
            let charged_bytes = bytes.len();
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
                counter!("recached_replication_disconnects_total", "reason" => "queue_items")
                    .increment(1);
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => false,
            }
        });
        self.count.store(reg.len(), Ordering::Relaxed);
    }

    /// Frames the furthest-behind replica has yet to acknowledge.
    ///
    /// This is true lag: how much of what the primary sent has actually been
    /// applied downstream. Queue depth only shows what is stuck locally, and
    /// reads zero for a replica that has received frames but cannot apply them.
    pub(crate) async fn max_lag_frames(&self) -> u64 {
        let senders = self.senders.lock().await;
        senders
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
            senders: tokio::sync::Mutex::new(Vec::new()),
            count: AtomicUsize::new(0),
            run_id: generate_run_id(),
            offset: AtomicU64::new(0),
            backlog: tokio::sync::Mutex::new(ReplBacklog::default()),
            backlog_bytes: AtomicUsize::new(0),
            queue_bytes: AtomicUsize::new(DEFAULT_REPL_QUEUE_BYTES),
            write_order: (0..WRITE_ORDER_STRIPES)
                .map(|_| tokio::sync::Mutex::new(()))
                .collect(),
            dedup: AtomicBool::new(true),
            upstream_dedup: AtomicBool::new(false),
        })
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

    /// Lock every ordering stripe touched by the supplied commands. Guards are
    /// acquired in numeric order, which also makes multi-key writes deadlock
    /// free. A global command such as FLUSHDB locks every stripe.
    pub(crate) async fn lock_commands<'a>(
        &'a self,
        commands: &[Command],
    ) -> Vec<tokio::sync::MutexGuard<'a, ()>> {
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
    ) -> Vec<tokio::sync::MutexGuard<'a, ()>>
    where
        I: IntoIterator<Item = &'k str>,
    {
        let mut stripes = Vec::new();
        let mut lock_all = false;
        for command in commands {
            if !is_write_command(command) {
                continue;
            }
            match ordering_keys(command) {
                None => {
                    lock_all = true;
                    break;
                }
                Some(keys) => stripes.extend(keys.iter().map(|key| Self::stripe_for(key))),
            }
        }
        if lock_all {
            stripes = (0..WRITE_ORDER_STRIPES).collect();
        } else {
            stripes.extend(extra_keys.into_iter().map(Self::stripe_for));
            stripes.sort_unstable();
            stripes.dedup();
        }

        let mut guards = Vec::with_capacity(stripes.len());
        for stripe in stripes {
            guards.push(self.write_order[stripe].lock().await);
        }
        guards
    }

    pub(crate) async fn lock_all_writes(&self) -> Vec<tokio::sync::MutexGuard<'_, ()>> {
        let mut guards = Vec::with_capacity(WRITE_ORDER_STRIPES);
        for lock in &self.write_order {
            guards.push(lock.lock().await);
        }
        guards
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
        let backlog = self.backlog.lock().await;
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

/// Default per-replica channel capacity (number of pending write frames).
/// When a replica falls this many writes behind the primary it is disconnected
/// so it can reconnect and request the retained backlog, falling back to a full
/// snapshot if necessary. The primary write path is never blocked.
pub(crate) const DEFAULT_REPL_CHANNEL_CAPACITY: usize = 4096;
pub(crate) const DEFAULT_REPL_QUEUE_BYTES: usize = 8 * 1024 * 1024;
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
    {
        let mut reg = replicas.senders.lock().await;
        reg.push(ReplicaHandle {
            tx,
            sent: Arc::clone(&sent),
            acked: Arc::clone(&acked),
            pending_bytes: Arc::clone(&pending_bytes),
            max_queue_bytes: replicas.queue_bytes.load(Ordering::Acquire),
        });
        replicas.count.store(reg.len(), Ordering::Relaxed);
    }
    drop(write_barrier);

    let mode = match (&snapshot, &dedup_table) {
        (Some(_), Some(_)) => SYNC_MODE_FULL_DEDUP,
        (Some(_), None) => b'F',
        (None, _) => b'P',
    };
    socket.write_all(&[mode]).await?;
    socket.write_all(&current_offset.to_le_bytes()).await?;
    if let Some(snapshot) = snapshot {
        counter!("recached_replication_syncs_total", "type" => "full").increment(1);
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
    let (mut rd, mut wr) = tokio::io::split(socket);
    let mut ack_buf = [0u8; 8];
    loop {
        tokio::select! {
            frame = rx.recv() => {
                let Some(queued) = frame else { break };
                write_repl_frame(&mut wr, &queued.frame).await?;
                wr.flush().await?;
            }
            res = rd.read_exact(&mut ack_buf) => {
                // A replica that closes its read side, or one running a build
                // that predates acks, simply stops updating the gauge — it is
                // not an error, so the stream continues either way.
                if res.is_err() {
                    break;
                }
                let applied = u64::from_le_bytes(ack_buf);
                // Monotonic: a reordered or replayed ack must never walk the
                // high-water mark backwards and report negative lag.
                acked.fetch_max(applied, Ordering::Relaxed);
            }
        }
    }
    Ok(())
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

pub(crate) async fn run_repl_client(
    primary_addr: String,
    store: Arc<KeyValueStore>,
    state: Arc<ServerState>,
    repl_password: Option<String>,
    failover_timeout_secs: Option<u64>,
    tx: broadcast::Sender<SyncMsg>,
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
    tx: &broadcast::Sender<SyncMsg>,
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
        store.replace(entries);
        match table {
            Some(table) => {
                let clients = table.len();
                state.replace_dedup_table(table);
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
            apply_repl_frame(store, state, tx, &bytes).await?;
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

    // 2. Stream write commands from primary, acknowledging each applied offset.
    loop {
        let (offset, cmd_bytes) = read_repl_frame(socket).await?;
        let expected = resume.offset.saturating_add(1);
        if offset != expected {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!("replication offset gap: expected {expected}, received {offset}"),
            ));
        }
        apply_repl_frame(store, state, tx, &cmd_bytes).await?;
        resume.offset = offset;

        // Acknowledge on the same socket. TcpStream is unbuffered, so this is a
        // single 8-byte write with no flush.
        socket.write_all(&offset.to_le_bytes()).await?;
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

async fn apply_repl_frame(
    store: &KeyValueStore,
    state: &ServerState,
    tx: &broadcast::Sender<SyncMsg>,
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
    match dedup_record(&value) {
        Some(DedupRecord::Refused(client, id, error)) => {
            state.remember_dedup_refusal(&client, id, &error);
            state.log_dedup_outcome(&client, id, Some(&error)).await;
            return Ok(());
        }
        Some(DedupRecord::Mark(client, id)) if is_dedup_mark_only(&value) => {
            state.commit_dedup(&client, id);
            state.log_dedup_outcome(&client, id, None).await;
            return Ok(());
        }
        Some(DedupRecord::Mark(client, id)) => {
            return apply_dedup_envelope(store, state, tx, value, &client, id).await;
        }
        None => {}
    }
    let normalised = match value {
        Value::Push(inner) => Value::Array(Some(inner)),
        other => other,
    };
    let cmd = Command::from_value(normalised)
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))?;
    let push = SyncPush::for_command(0, &cmd, primary_keys(&cmd), bytes.to_vec());
    if let Value::Error(error) = store.execute(cmd) {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("replicated command failed: {error}"),
        ));
    }
    let _ = tx.send(Arc::new(push));
    state.on_write(bytes).await;
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
    tx: &broadcast::Sender<SyncMsg>,
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
    let push = SyncPush::for_command(0, &cmd, primary_keys(&cmd), bare.clone());
    if let Value::Error(error) = store.execute(cmd) {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("replicated command failed: {error}"),
        ));
    }
    state.commit_dedup(client, id);
    let _ = tx.send(Arc::new(push));
    state.on_write_identified(&bare, Some((client, id))).await;
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

    #[tokio::test]
    async fn replica_queue_enforces_bytes_even_below_its_item_limit() {
        let hub = ReplHub::new();
        hub.configure_backlog(1024);
        hub.configure_queue_bytes(3);
        let (tx, _rx) = mpsc::channel(16);
        hub.senders.lock().await.push(ReplicaHandle {
            tx,
            sent: Arc::new(AtomicU64::new(0)),
            acked: Arc::new(AtomicU64::new(0)),
            pending_bytes: Arc::new(AtomicUsize::new(0)),
            max_queue_bytes: 3,
        });
        hub.count.store(1, Ordering::Relaxed);

        hub.fan_out(vec![0; 4]).await;
        assert_eq!(hub.count.load(Ordering::Relaxed), 0);
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
