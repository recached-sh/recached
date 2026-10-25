//! Platform-neutral Recached sync client.
//!
//! This crate is the *brain* of every Recached client SDK: the durable
//! outbox, duplicate-suppression `DEDUP` envelopes, ordered-reply acknowledgment
//! correlation, session re-establishment after reconnect, and backoff
//! policy. It performs no I/O and knows nothing about WebSockets, IndexedDB,
//! or timers — methods take the current connection state and return
//! *effects* (frames to send, outbox rows to persist or delete, events to
//! surface) for a platform adapter to execute.
//!
//! Adapters: `wasm-edge` (browser: WebSocket + IndexedDB + `setTimeout`);
//! planned uniffi bindings for Kotlin/Swift and `flutter_rust_bridge`
//! (roadmap #6) reuse this crate unchanged, so merge semantics can never
//! drift between platforms.
//!
//! The wire protocol this implements is specified in `docs/server/protocol.md`.

use core_engine::cmd::Command;
use core_engine::resp::Value;
use core_engine::store::{KeyValueStore, glob_match};
use std::collections::{HashSet, VecDeque};
use std::fmt;
use std::sync::Arc;

/// Default cap on outbox entries. What happens to a write past it is the
/// [`OverflowPolicy`]: by default the write is refused, never an older one
/// dropped to make room.
pub const MAX_PENDING_WRITES: usize = 10_000;

/// Highest outbox id a duplicate-suppressed write can carry within one epoch.
/// The low 32 bits of a wire id are the outbox id, so one more would wrap onto
/// an id the server has already seen and the write would be skipped as a
/// duplicate. [`SyncClient::try_enqueue_write`] refuses it instead.
pub const MAX_WIRE_SEQ: u64 = u32::MAX as u64;

/// What [`SyncClient::try_enqueue_write`] does with a write that arrives while
/// the outbox is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverflowPolicy {
    /// Refuse the new write with [`EnqueueError::OutboxFull`] and leave every
    /// queued write where it is. The default: the caller learns at once that
    /// the write was not accepted, before it has shown it as done.
    #[default]
    Reject,
    /// The behaviour up to 0.4: evict the oldest queued write to make room,
    /// reported through [`Enqueued::dropped`]. The evicted write never reaches
    /// the server, though the application may already have shown it as saved.
    /// Only for callers that would rather lose old writes than refuse new ones.
    EvictOldest,
}

/// Why a write could not be queued. Nothing was queued, and nothing already
/// queued was changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnqueueError {
    /// The outbox holds `pending` writes and its cap is `capacity`. Writes
    /// drain as the server acknowledges them.
    OutboxFull { pending: usize, capacity: usize },
    /// This session has used every duplicate-suppression id its epoch can
    /// hold ([`MAX_WIRE_SEQ`]). A new session — a new epoch, see
    /// [`next_epoch`] — is needed before another write can be queued.
    WireIdsExhausted,
}

impl fmt::Display for EnqueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutboxFull { pending, capacity } => write!(
                f,
                "outbox full: {pending} writes pending, capacity {capacity}"
            ),
            Self::WireIdsExhausted => {
                f.write_str("this session has used every write id; start a new session")
            }
        }
    }
}

impl std::error::Error for EnqueueError {}

/// The epoch a new session should use after `previous`, or `None` when the
/// epoch space is used up.
///
/// `None` means the client must take a new identity — a fresh client id,
/// starting again at epoch 1 — because every epoch of this one has been used.
/// Saturating at `u32::MAX` instead, as adapters used to, makes two sessions
/// share an epoch: the second reuses the first's wire ids, and the server
/// skips its writes as duplicates.
pub fn next_epoch(previous: u32) -> Option<u32> {
    previous.checked_add(1)
}

/// Where the connection is in establishing synchronization.
///
/// An open socket is only the start. The client is [`Ready`](Self::Ready)
/// once the session is authenticated and scoped, every write queued before
/// the socket opened has been answered, and every live query registered at
/// that point has received its snapshot. Live queries added later report their
/// own progress through [`SubscriptionState`] without taking the connection
/// out of `Ready`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncPhase {
    /// No socket, or the last one closed.
    Disconnected,
    /// The adapter is opening a socket ([`SyncClient::on_connecting`]).
    Connecting,
    /// Session setup — `AUTH`, the `CLIENT` opt-ins, `SYNC TOKEN` or `SYNC`
    /// scopes — sent and not yet answered. Never skipped: every open sends
    /// the opt-ins, so `Ready` always waits for at least one reply.
    Authenticating,
    /// Writes queued before the socket opened have been sent and not all
    /// answered yet.
    Replaying,
    /// Live-query snapshots requested on open have not all arrived.
    Hydrating,
    /// Everything above has completed.
    Ready,
    /// The socket is open but synchronization is impaired: authentication or
    /// the sync scope was refused, a live query was refused, or the socket is
    /// being abandoned after a deferred write or an unparseable frame. The
    /// status's error says which.
    Degraded,
}

/// A connection-level synchronization problem, from [`SyncStatus::error`].
///
/// Reasons are the server's error text. The server never echoes a password
/// or token into one, and the client adds nothing that could carry one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    /// `AUTH` was refused.
    AuthFailed { reason: String },
    /// `SYNC TOKEN` or `SYNC` scopes were refused.
    ScopeRefused { reason: String },
    /// A queued write was refused for a reason that can clear. It stays
    /// queued and replays on the next connection.
    WriteDeferred { reason: String },
    /// A live query was refused; its keys are not being kept in sync.
    SubscriptionFailed {
        pattern: String,
        failure: SubscriptionFailure,
        reason: String,
    },
    /// An incoming frame could not be parsed, so reply matching on this
    /// socket can no longer be trusted.
    MalformedFrame,
}

/// Why a live query was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubscriptionFailure {
    /// More keys match than the server will send in one snapshot
    /// (`RECACHED_MAX_QSUB_INITIAL_KEYS`). The server sends no partial
    /// snapshot, so nothing local was reconciled against one.
    SnapshotTooLarge,
    /// The pattern is outside this connection's sync scope.
    OutOfScope,
    /// Any other refusal, such as the per-connection live-query limit.
    Other,
}

/// The connection's synchronization status: see [`SyncClient::status`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncStatus {
    pub phase: SyncPhase,
    /// What is wrong, when something is. While
    /// [`Disconnected`](SyncPhase::Disconnected) or
    /// [`Connecting`](SyncPhase::Connecting) this is the problem that ended
    /// the last connection, if any.
    pub error: Option<SyncError>,
}

/// One live query's progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionState {
    /// Registered, waiting for a connection to request its snapshot. Local
    /// copies of its keys are whatever the last session left.
    Pending,
    /// Snapshot requested and not yet received.
    Hydrating,
    /// The snapshot has been applied; changes stream as they happen.
    Current,
    /// The server refused it. Retried on the next connection.
    Failed {
        failure: SubscriptionFailure,
        reason: String,
    },
}

/// Counters for diagnostics. Monotonic over the client's lifetime.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncCounters {
    /// Sockets opened ([`SyncClient::on_open`] calls).
    pub connections: u64,
    /// Queued writes re-sent by `on_open`.
    pub replayed_writes: u64,
    /// Writes the server refused for a reason that can clear.
    pub deferred_writes: u64,
    /// Writes the server refused for good.
    pub refused_writes: u64,
    /// Writes [`SyncClient::try_enqueue_write`] refused (full outbox,
    /// exhausted ids).
    pub rejected_writes: u64,
    /// Writes evicted under [`OverflowPolicy::EvictOldest`].
    pub evicted_writes: u64,
    /// Live-query snapshots applied.
    pub snapshots_applied: u64,
    /// Frames that could not be parsed.
    pub malformed_frames: u64,
}

/// One reply slot: what the next reply in send order answers.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Slot {
    /// A queued write. `replay`: re-sent by `on_open`, so part of the replay
    /// that [`SyncPhase::Replaying`] waits for.
    Write {
        id: u64,
        replay: bool,
    },
    Auth,
    /// `SYNC TOKEN` or `SYNC <patterns>`.
    Scope,
    Capability(Capability),
    /// `QSUB`. `initial`: sent by `on_open`, so part of the hydration that
    /// [`SyncPhase::Hydrating`] waits for.
    LiveQuery {
        pattern: String,
        initial: bool,
    },
    /// Anything else: `QUNSUB`, a one-shot command.
    Session,
}

impl Slot {
    /// Part of session setup, which [`SyncPhase::Authenticating`] waits for.
    fn is_setup(&self) -> bool {
        matches!(self, Slot::Auth | Slot::Scope | Slot::Capability(_))
    }
}

/// The `CLIENT` opt-ins sent on every open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Capability {
    Delta,
    Expiry,
    /// `CLIENT PUSHFRAMES ON`: `keychange` and `keydelta` arrive as RESP3
    /// push frames, so no command reply can be mistaken for one.
    PushFrames,
    /// `CLIENT DEDUPBARRIER ON`: after a write is deferred, the server defers
    /// every later write from this client on the same socket too, so none of
    /// them can raise the duplicate-suppression mark past the deferred one.
    DedupBarrier,
}

#[derive(Debug, Clone)]
struct LiveQuery {
    pattern: String,
    state: SubscriptionState,
}

/// Reconnect backoff: `500ms * 2^attempts`, capped.
pub const BACKOFF_BASE_MS: u32 = 500;
pub const BACKOFF_CAP_MS: u32 = 30_000;

/// FNV-1a over the client id — a stable, non-zero seed per client.
fn seed_from(client_id: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in client_id.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h | 1
}

/// What an incoming frame turned out to be, and what the adapter must do.
#[derive(Debug, PartialEq)]
pub enum Incoming {
    /// A mutation (server push, `keychange`, or `keydelta`) was applied to the
    /// local store — notify the subscribers of `keys`.
    ///
    /// `keys` names every key the frame may have changed, deleted ones
    /// included. It can name a key whose value came out the same — a re-sent
    /// value is applied regardless — so a subscriber that cares compares before
    /// re-rendering. It never leaves out a key that did change. Adapters with
    /// one coarse "something changed" signal can ignore it.
    Applied { keys: Vec<String> },
    /// A pub/sub message. The payload is bytes because a publisher may send
    /// binary; the adapter decides how to surface it.
    PubSub { channel: String, message: Vec<u8> },
    /// A command reply. When `retired` is set, that outbox row is now
    /// acknowledged — delete it from durable storage.
    Reply { retired: Option<u64> },
    /// The server refused a queued write for good: the key is out of this
    /// connection's scope or read-only, the value has the wrong type, the
    /// command is invalid. Replaying it would be refused again, so the row is
    /// retired — delete it from durable storage — and the application should
    /// hear about it.
    ///
    /// The write still shows in the local store; the client has no way to undo
    /// it. For keys a live query covers,
    /// [`resubscribe_covering`](SyncClient::resubscribe_covering) puts back the
    /// server's values. `unreconciled` lists the keys no live query covers:
    /// their local value is the refused one, and nothing will correct it until
    /// the application writes them again or watches them.
    Refused {
        retired: u64,
        keys: Vec<String>,
        unreconciled: Vec<String>,
        reason: String,
    },
    /// The server refused a queued write for a reason that can clear without
    /// the write changing: no sync token or password accepted yet, a read-only
    /// replica, persistence or memory trouble. The row stays queued. Drop the
    /// socket; the reconnect replays it, in order, after a backoff that keeps
    /// growing until a write is accepted.
    ///
    /// Until then nothing more is sent on this socket: a later write accepted
    /// here would raise the server's duplicate-suppression mark past this one,
    /// and its replay would be skipped as already applied.
    Deferred { reason: String },
    /// A `qstate` frame: the reply to a QSUB *and* state applied to the local
    /// store — notify the subscribers of `keys`, which covers every key in the
    /// snapshot and every local key the snapshot reconciled away.
    ///
    /// `retired` is always `None`: a snapshot answers a `QSUB`, never a write.
    /// It is kept so adapters can treat both reply kinds alike.
    AppliedReply {
        retired: Option<u64>,
        keys: Vec<String>,
    },
    /// A frame we understand but that asks for nothing — a push we do not
    /// model. It consumes no reply slot, which is correct: the server did not
    /// treat it as one either.
    Ignored,
    /// The frame could not be parsed at all. Nothing more is sent on this
    /// socket; the adapter must drop it.
    ///
    /// This is *not* [`Ignored`](Self::Ignored), and the difference matters.
    /// If the frame was a command reply, the server has consumed a command
    /// that this client will never account for, so the inflight FIFO is now
    /// one ahead of reality: every later reply acknowledges the wrong command,
    /// resolves the wrong caller and retires the wrong outbox row. There is no
    /// way to resynchronise in place — the adapter must drop the socket, which
    /// [`on_open`](SyncClient::on_open) rebuilds from a cleared FIFO.
    ///
    /// It is reachable without an attacker: a `keychange` for a watched
    /// collection larger than `resp::MAX_ARRAY_ELEMENTS` parses on the server
    /// and fails here.
    Malformed,
}

/// Result of queueing a write.
#[derive(Debug, PartialEq)]
pub struct Enqueued {
    /// Outbox row id — persist the row under this key.
    pub id: u64,
    /// The wire frame (dedup-wrapped when requested). These exact bytes are
    /// stored in the outbox and replayed on reconnect.
    ///
    /// Bytes rather than `String`: a frame carries a stored value, which may be
    /// arbitrary binary. Holding it as text would corrupt the queued write and,
    /// on reconnect, replay the corrupted version to the server.
    pub frame: Vec<u8>,
    /// Send `frame` now (the caller reported the socket open, and the write
    /// was recorded as inflight).
    pub send_now: bool,
    /// Only under [`OverflowPolicy::EvictOldest`]: the outbox was full and
    /// this row id was evicted — delete it from durable storage. That write
    /// will never reach the server.
    pub dropped: Option<u64>,
}

/// Result of [`SyncClient::restore_outbox`].
#[derive(Debug, PartialEq)]
pub struct Restored {
    /// `(old id, new id, frame)` for every restored row, in replay order.
    /// Replace durable storage with these rows atomically.
    pub rewrites: Vec<(u64, u64, Vec<u8>)>,
    /// Frames to send now, in order: the restored rows, when the socket was
    /// already open. Each holds a reply slot.
    pub send_now: Vec<Vec<u8>>,
}

/// The platform-neutral sync state machine. One instance per connection
/// identity (i.e. per cache).
pub struct SyncClient {
    store: Arc<KeyValueStore>,
    /// Writes not yet acknowledged by the server, in send order.
    outbox: VecDeque<(u64, Vec<u8>)>,
    outbox_seq: u64,
    /// One slot per frame sent on the current socket, in send order. Replies
    /// arrive in send order; each answers the front slot.
    inflight: VecDeque<Slot>,
    /// Ids overflow evicted after they were sent on the current socket. Their
    /// replies still arrive, in their slots, and must retire nothing; knowing
    /// them here answers that in O(1) instead of searching the surviving
    /// queue for a row that is not in it — which made draining a connection
    /// that had overflowed O(n²). Reset with `inflight`.
    sent_then_dropped: HashSet<u64>,
    client_id: String,
    /// Upper 32 bits of every dedup wire id; bump per session (persisted by
    /// the adapter) so a new session's ids clear the server's high-water mark.
    epoch: u32,
    password: Option<String>,
    sync_token: Option<String>,
    sync_scopes_csv: Option<String>,
    live_queries: Vec<LiveQuery>,
    attempts: u32,
    /// Cap on queued-but-unacknowledged writes. Configurable because the
    /// right depth depends on how long a client is expected to be offline and
    /// how large its writes are.
    max_pending: usize,
    overflow: OverflowPolicy,
    /// Jitter source for reconnect backoff. Seeded from `client_id` rather than
    /// a system RNG so this crate stays dependency-free and I/O-free — and so
    /// the sequence is reproducible in tests. Different clients get different
    /// sequences, which is the only property that matters here.
    jitter: u64,
    /// This socket is being abandoned ([`Incoming::Deferred`] or
    /// [`Incoming::Malformed`]): treat it as closed until the next open.
    paused: bool,
    /// The last session ended with a write deferred. The next open then keeps
    /// the backoff growing instead of resetting it, so a client whose token
    /// stays missing does not reconnect twice a second forever.
    deferring: bool,
    /// The server accepted `CLIENT PUSHFRAMES ON` on this socket: every
    /// `keychange` and `keydelta` now arrives as a push, so a plain array is
    /// always a reply. Until then a reply shaped like one is ambiguous.
    push_frames: bool,
    // ── lifecycle ────────────────────────────────────────────────────────
    open: bool,
    connecting: bool,
    /// Session-setup slots (`AUTH`, `CLIENT` opt-ins, `SYNC`) not yet
    /// answered on this socket.
    setup_pending: usize,
    /// Writes `on_open` replayed that are not yet answered.
    replay_pending: usize,
    /// `QSUB`s `on_open` sent that are not yet answered.
    hydrate_pending: usize,
    auth_failed: Option<String>,
    scope_refused: Option<String>,
    /// Why this socket is being abandoned, while `paused`.
    pause_reason: Option<SyncError>,
    /// The problem that ended the last connection, shown while disconnected.
    disconnect_reason: Option<SyncError>,
    reported_status: Option<SyncStatus>,
    /// Patterns whose [`SubscriptionState`] changed since the last
    /// [`take_subscription_changes`](Self::take_subscription_changes).
    subscription_changes: Vec<String>,
    /// Keys a snapshot on this socket left alone because a queued write
    /// covered them (see `apply_qstate`).
    protected: HashSet<String>,
    /// Protected keys whose write then came back `+DUP` — applied in an
    /// earlier session, so no `keychange` will bring the server's value.
    /// Their live queries are re-subscribed by
    /// [`take_frames_to_send`](SyncClient::take_frames_to_send).
    resync_keys: Vec<String>,
    counters: SyncCounters,
}

impl SyncClient {
    pub fn new(store: Arc<KeyValueStore>, client_id: String) -> Self {
        let jitter = seed_from(&client_id);
        Self {
            store,
            outbox: VecDeque::new(),
            outbox_seq: 0,
            inflight: VecDeque::new(),
            sent_then_dropped: HashSet::new(),
            client_id,
            epoch: 0,
            password: None,
            sync_token: None,
            sync_scopes_csv: None,
            live_queries: Vec::new(),
            attempts: 0,
            max_pending: MAX_PENDING_WRITES,
            overflow: OverflowPolicy::default(),
            jitter,
            paused: false,
            deferring: false,
            push_frames: false,
            open: false,
            connecting: false,
            setup_pending: 0,
            replay_pending: 0,
            hydrate_pending: 0,
            auth_failed: None,
            scope_refused: None,
            pause_reason: None,
            disconnect_reason: None,
            reported_status: None,
            subscription_changes: Vec::new(),
            protected: HashSet::new(),
            resync_keys: Vec::new(),
            counters: SyncCounters::default(),
        }
    }

    pub fn store(&self) -> &Arc<KeyValueStore> {
        &self.store
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Set the outbox cap (minimum 1). A lower cap than the rows already
    /// queued drops none of them; new writes are refused until it drains.
    pub fn set_max_pending(&mut self, max: usize) {
        self.max_pending = max.max(1);
    }

    /// Current outbox cap.
    pub fn max_pending(&self) -> usize {
        self.max_pending
    }

    /// Choose what a write past the cap does. [`OverflowPolicy::Reject`]
    /// unless changed.
    pub fn set_overflow_policy(&mut self, policy: OverflowPolicy) {
        self.overflow = policy;
    }

    pub fn overflow_policy(&self) -> OverflowPolicy {
        self.overflow
    }

    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    pub fn set_epoch(&mut self, epoch: u32) {
        self.epoch = epoch;
    }

    /// True while nothing has been written this session — the window in which
    /// adopting a persisted client identity is safe.
    pub fn no_writes_yet(&self) -> bool {
        self.outbox_seq == 0 && self.outbox.is_empty()
    }

    /// Adopt a persisted client id. Refused (returns false) once writes have
    /// been issued — switching identity mid-stream would fragment the
    /// server's high-water mark.
    pub fn adopt_client_id(&mut self, client_id: String) -> bool {
        if self.no_writes_yet() {
            self.client_id = client_id;
            true
        } else {
            false
        }
    }

    pub fn counters(&self) -> SyncCounters {
        self.counters
    }

    // ── session state ─────────────────────────────────────────────────────
    // Setters record state for re-establishment on every reconnect and, when
    // the socket is open, return the frame to send now (recording it as
    // inflight — session replies must occupy a reply slot or they would
    // falsely acknowledge a data write).

    pub fn set_password(&mut self, password: &str, connected: bool) -> Option<Vec<u8>> {
        self.password = Some(password.to_string());
        self.session_frame(to_resp(&["AUTH", password]), Slot::Auth, connected)
    }

    pub fn set_sync_token(&mut self, token: &str, connected: bool) -> Option<Vec<u8>> {
        self.sync_token = Some(token.to_string());
        self.session_frame(to_resp(&["SYNC", "TOKEN", token]), Slot::Scope, connected)
    }

    /// Comma-separated glob patterns. Returns `None` (and records nothing)
    /// when the list is empty.
    pub fn set_sync_scopes(&mut self, patterns_csv: &str, connected: bool) -> Option<Vec<u8>> {
        let frame = sync_scopes_frame(patterns_csv)?;
        self.sync_scopes_csv = Some(patterns_csv.to_string());
        self.session_frame(frame, Slot::Scope, connected)
    }

    /// Register a live query (idempotent). The returned frame re-hydrates
    /// matching keys via the `qstate` reply.
    pub fn add_live_query(&mut self, pattern: &str, connected: bool) -> Option<Vec<u8>> {
        if !self.live_queries.iter().any(|q| q.pattern == pattern) {
            self.live_queries.push(LiveQuery {
                pattern: pattern.to_string(),
                state: SubscriptionState::Pending,
            });
            self.subscription_changes.push(pattern.to_string());
        }
        self.qsub_frame(pattern, false, connected)
    }

    pub fn remove_live_query(&mut self, pattern: Option<&str>, connected: bool) -> Option<Vec<u8>> {
        let frame = match pattern {
            Some(p) => {
                if self.live_queries.iter().any(|q| q.pattern == p) {
                    self.subscription_changes.push(p.to_string());
                }
                self.live_queries.retain(|q| q.pattern != p);
                to_resp(&["QUNSUB", p])
            }
            None => {
                self.subscription_changes
                    .extend(self.live_queries.drain(..).map(|q| q.pattern));
                to_resp(&["QUNSUB"])
            }
        };
        self.session_frame(frame, Slot::Session, connected)
    }

    /// The live queries currently registered, with their progress.
    pub fn subscriptions(&self) -> Vec<(String, SubscriptionState)> {
        self.live_queries
            .iter()
            .map(|q| (q.pattern.clone(), q.state.clone()))
            .collect()
    }

    /// One live query's progress, or `None` when `pattern` is not registered.
    pub fn subscription_state(&self, pattern: &str) -> Option<SubscriptionState> {
        self.live_queries
            .iter()
            .find(|q| q.pattern == pattern)
            .map(|q| q.state.clone())
    }

    /// True when a registered live query covers `key`, so a fresh snapshot
    /// can correct its local copy.
    pub fn is_covered(&self, key: &str) -> bool {
        self.live_queries
            .iter()
            .any(|q| glob_match(&q.pattern, key))
    }

    /// Register a one-shot frame the adapter is about to send whose reply must
    /// occupy a reply slot but which is neither durable nor replayed on
    /// reconnect — a read-through `GET`, a `SUBSCRIBE`, an introspection
    /// command. Returns the frame unchanged when connected, `None` otherwise.
    ///
    /// Browser SDKs never needed this: they read only from the local store. A
    /// server-side adapter that offers read-through must still take a slot, or
    /// its reply would falsely acknowledge the oldest queued write.
    pub fn session_command(&mut self, frame: Vec<u8>, connected: bool) -> Option<Vec<u8>> {
        self.session_frame(frame, Slot::Session, connected)
    }

    fn session_frame(&mut self, frame: Vec<u8>, slot: Slot, connected: bool) -> Option<Vec<u8>> {
        if connected && !self.paused {
            if slot.is_setup() {
                self.setup_pending += 1;
            }
            self.inflight.push_back(slot);
            Some(frame)
        } else {
            None
        }
    }

    fn qsub_frame(&mut self, pattern: &str, initial: bool, connected: bool) -> Option<Vec<u8>> {
        let frame = self.session_frame(
            to_resp(&["QSUB", pattern]),
            Slot::LiveQuery {
                pattern: pattern.to_string(),
                initial,
            },
            connected,
        )?;
        if initial {
            self.hydrate_pending += 1;
        }
        self.set_subscription(pattern, SubscriptionState::Hydrating);
        Some(frame)
    }

    fn set_subscription(&mut self, pattern: &str, state: SubscriptionState) {
        if let Some(q) = self.live_queries.iter_mut().find(|q| q.pattern == pattern)
            && q.state != state
        {
            q.state = state;
            self.subscription_changes.push(pattern.to_string());
        }
    }

    // ── connection lifecycle ──────────────────────────────────────────────

    /// The adapter is opening a socket. Optional: it only makes the status
    /// read [`SyncPhase::Connecting`] until the socket opens or fails.
    pub fn on_connecting(&mut self) {
        if !self.open {
            self.connecting = true;
        }
    }

    /// The socket opened: returns every frame to send, in order — the session
    /// (AUTH → CLIENT opt-ins → SYNC), then the full outbox replay, then the
    /// live queries. Live-query re-subscription re-hydrates local state;
    /// outbox entries stay queued until their replies acknowledge them.
    ///
    /// The outbox goes **before** the live queries. A `qstate` snapshot is
    /// complete, so reconciling against it removes local keys it lacks and
    /// overwrites the rest. Taken before the replay, it lacks this client's
    /// own queued writes: an item created offline vanished and an offline
    /// edit reverted to the server's old value, until each write's own
    /// keychange restored it a moment later — a flicker on every reconnect,
    /// written to durable storage too. Replayed first, the snapshot already
    /// contains them. The session still leads: writes need its auth and scope.
    pub fn on_open(&mut self) -> Vec<Vec<u8>> {
        if !self.deferring {
            self.attempts = 0;
        }
        self.paused = false;
        self.push_frames = false;
        self.open = true;
        self.connecting = false;
        self.setup_pending = 0;
        self.replay_pending = 0;
        self.hydrate_pending = 0;
        self.auth_failed = None;
        self.scope_refused = None;
        self.pause_reason = None;
        self.disconnect_reason = None;
        // Every live query re-snapshots below, so nothing protected on the
        // last socket still needs a resubscribe of its own.
        self.protected.clear();
        self.resync_keys.clear();
        self.counters.connections += 1;
        self.inflight.clear();
        self.sent_then_dropped.clear();
        let mut frames = Vec::new();
        if let Some(pwd) = &self.password {
            frames.push(to_resp(&["AUTH", pwd]));
            self.inflight.push_back(Slot::Auth);
            self.setup_pending += 1;
        }
        // This client understands `keydelta`, so say so. Sent every time the
        // socket opens because it is connection state, not session state, and
        // sent before QSUB so the initial live-query traffic already benefits.
        // An older server answers with an error, which changes nothing: it
        // simply keeps sending whole values.
        frames.push(to_resp(&["CLIENT", "DELTA", "ON"]));
        self.inflight.push_back(Slot::Capability(Capability::Delta));
        self.setup_pending += 1;
        // Likewise for expiry: keychange and qstate values then carry the
        // key's time to live, so an expiring key stays expiring here. An older
        // server refuses it and sends bare values, as before.
        frames.push(to_resp(&["CLIENT", "EXPIRY", "ON"]));
        self.inflight
            .push_back(Slot::Capability(Capability::Expiry));
        self.setup_pending += 1;
        // Push framing: keychange/keydelta as RESP3 pushes, so a reply can
        // never be taken for one. Until a server says yes, a reply shaped like
        // a keychange is still read as one — the 0.4 behaviour.
        frames.push(to_resp(&["CLIENT", "PUSHFRAMES", "ON"]));
        self.inflight
            .push_back(Slot::Capability(Capability::PushFrames));
        self.setup_pending += 1;
        // Ordered deferral: the server then refuses every write after a
        // deferred one on this socket, so none can raise the duplicate mark
        // past it. Sent before the outbox, which it protects.
        frames.push(to_resp(&["CLIENT", "DEDUPBARRIER", "ON"]));
        self.inflight
            .push_back(Slot::Capability(Capability::DedupBarrier));
        self.setup_pending += 1;
        if let Some(tok) = &self.sync_token {
            frames.push(to_resp(&["SYNC", "TOKEN", tok]));
            self.inflight.push_back(Slot::Scope);
            self.setup_pending += 1;
        } else if let Some(csv) = &self.sync_scopes_csv
            && let Some(frame) = sync_scopes_frame(csv)
        {
            frames.push(frame);
            self.inflight.push_back(Slot::Scope);
            self.setup_pending += 1;
        }
        for (id, frame) in &self.outbox {
            frames.push(frame.clone());
            self.inflight.push_back(Slot::Write {
                id: *id,
                replay: true,
            });
        }
        self.replay_pending = self.outbox.len();
        self.counters.replayed_writes += self.outbox.len() as u64;
        let patterns: Vec<String> = self
            .live_queries
            .iter()
            .map(|q| q.pattern.clone())
            .collect();
        for pat in patterns {
            if let Some(frame) = self.qsub_frame(&pat, true, true) {
                frames.push(frame);
            }
        }
        frames
    }

    /// The socket closed: returns the delay before the next reconnect
    /// attempt, in milliseconds. Call it for every close, whether or not the
    /// adapter will reconnect — it is also what moves the status to
    /// [`SyncPhase::Disconnected`].
    ///
    /// Exponential with a cap, then **jittered into `[delay/2, delay]`**. Without
    /// jitter every client disconnected by the same event computes an identical
    /// schedule and reconnects in lockstep — a thundering herd that can keep a
    /// recovering server down. Halving the floor keeps the growth shape intact
    /// while spreading arrivals across the window.
    pub fn on_close(&mut self) -> u32 {
        if self.open {
            self.disconnect_reason = self.current_error();
        }
        self.paused = false;
        self.open = false;
        self.connecting = false;
        self.push_frames = false;
        self.setup_pending = 0;
        self.replay_pending = 0;
        self.hydrate_pending = 0;
        self.pause_reason = None;
        let stale: Vec<String> = self
            .live_queries
            .iter()
            .filter(|q| !matches!(q.state, SubscriptionState::Failed { .. }))
            .map(|q| q.pattern.clone())
            .collect();
        for pattern in stale {
            self.set_subscription(&pattern, SubscriptionState::Pending);
        }
        let delay = BACKOFF_BASE_MS
            .saturating_mul(1u32 << self.attempts.min(6))
            .min(BACKOFF_CAP_MS);
        self.attempts = self.attempts.saturating_add(1);

        let half = delay / 2;
        if half == 0 {
            return delay;
        }
        half + (self.next_jitter() % (half as u64 + 1)) as u32
    }

    /// xorshift64* — small, dependency-free, and good enough for spreading
    /// reconnects. Not for anything security-sensitive.
    fn next_jitter(&mut self) -> u64 {
        let mut x = self.jitter;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.jitter = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    // ── status ────────────────────────────────────────────────────────────

    /// The connection's synchronization status, derived from protocol events
    /// alone — never from a timer, and never from the socket merely being
    /// open.
    pub fn status(&self) -> SyncStatus {
        SyncStatus {
            phase: self.phase(),
            error: if self.open {
                self.current_error()
            } else {
                self.disconnect_reason.clone()
            },
        }
    }

    /// The status, when it differs from the one this last returned. Adapters
    /// call it after each `on_open`, `on_close` and `handle_frame` (and after
    /// session setters) and notify their listeners on `Some` — so status
    /// changes are pushed, never polled for.
    pub fn take_status_change(&mut self) -> Option<SyncStatus> {
        let status = self.status();
        if self.reported_status.as_ref() == Some(&status) {
            return None;
        }
        self.reported_status = Some(status.clone());
        Some(status)
    }

    /// Patterns whose [`SubscriptionState`] changed since the last call, each
    /// once, with its current state (`None` when the pattern was removed).
    pub fn take_subscription_changes(&mut self) -> Vec<(String, Option<SubscriptionState>)> {
        let mut seen = HashSet::new();
        let changed: Vec<String> = self
            .subscription_changes
            .drain(..)
            .filter(|p| seen.insert(p.clone()))
            .collect();
        changed
            .into_iter()
            .map(|p| {
                let state = self.subscription_state(&p);
                (p, state)
            })
            .collect()
    }

    fn phase(&self) -> SyncPhase {
        if !self.open {
            return if self.connecting {
                SyncPhase::Connecting
            } else {
                SyncPhase::Disconnected
            };
        }
        if self.paused {
            SyncPhase::Degraded
        } else if self.setup_pending > 0 {
            SyncPhase::Authenticating
        } else if self.auth_failed.is_some() || self.scope_refused.is_some() {
            SyncPhase::Degraded
        } else if self.replay_pending > 0 {
            SyncPhase::Replaying
        } else if self.hydrate_pending > 0 {
            SyncPhase::Hydrating
        } else if self
            .live_queries
            .iter()
            .any(|q| matches!(q.state, SubscriptionState::Failed { .. }))
        {
            SyncPhase::Degraded
        } else {
            SyncPhase::Ready
        }
    }

    fn current_error(&self) -> Option<SyncError> {
        if let Some(reason) = &self.pause_reason {
            return Some(reason.clone());
        }
        if let Some(reason) = &self.auth_failed {
            return Some(SyncError::AuthFailed {
                reason: reason.clone(),
            });
        }
        if let Some(reason) = &self.scope_refused {
            return Some(SyncError::ScopeRefused {
                reason: reason.clone(),
            });
        }
        self.live_queries.iter().find_map(|q| match &q.state {
            SubscriptionState::Failed { failure, reason } => Some(SyncError::SubscriptionFailed {
                pattern: q.pattern.clone(),
                failure: *failure,
                reason: reason.clone(),
            }),
            _ => None,
        })
    }

    // ── writes ────────────────────────────────────────────────────────────

    /// Whether a write could be queued now. Adapters that apply a write to
    /// the local store before queueing it call this first, so a write the
    /// outbox would refuse never changes local state. A refusal counts in
    /// [`SyncCounters::rejected_writes`].
    pub fn check_capacity(&mut self, dedup: bool) -> Result<(), EnqueueError> {
        let refusal = if dedup && self.outbox_seq > MAX_WIRE_SEQ {
            Some(EnqueueError::WireIdsExhausted)
        } else if self.overflow == OverflowPolicy::Reject && self.outbox.len() >= self.max_pending {
            Some(EnqueueError::OutboxFull {
                pending: self.outbox.len(),
                capacity: self.max_pending,
            })
        } else {
            None
        };
        match refusal {
            Some(e) => {
                self.counters.rejected_writes += 1;
                Err(e)
            }
            None => Ok(()),
        }
    }

    /// Queue a write for the server. `dedup` wraps it in a duplicate-suppression
    /// envelope — store writes want this; connection-scoped commands
    /// (pub/sub) must not be wrapped, or a legitimately replayed SUBSCRIBE
    /// would be skipped as a duplicate.
    ///
    /// Fails, queueing nothing and changing nothing already queued, when the
    /// outbox is full (under the default [`OverflowPolicy::Reject`]) or the
    /// session's wire ids are used up.
    pub fn try_enqueue_write(
        &mut self,
        encoded: &[u8],
        dedup: bool,
        connected: bool,
    ) -> Result<Enqueued, EnqueueError> {
        self.check_capacity(dedup)?;
        let connected = connected && !self.paused;
        let id = self.outbox_seq;
        self.outbox_seq += 1;
        let frame = if dedup {
            self.wrap_dedup(encoded, id)
        } else {
            encoded.to_vec()
        };
        let dropped = if self.outbox.len() >= self.max_pending {
            // Only reachable under `EvictOldest`: `check_capacity` refused it
            // otherwise.
            self.counters.evicted_writes += 1;
            self.outbox.pop_front().map(|(old, _)| old)
        } else {
            None
        };
        // While connected every queued row has been sent — `on_open` replays
        // the outbox and connected writes go out at once — so the dropped row
        // still has a reply on its way.
        if connected && let Some(old) = dropped {
            self.sent_then_dropped.insert(old);
        }
        self.outbox.push_back((id, frame.clone()));
        if connected {
            self.inflight.push_back(Slot::Write { id, replay: false });
        }
        Ok(Enqueued {
            id,
            frame,
            send_now: connected,
            dropped,
        })
    }

    /// Take back the write just queued with `connected = false`, before it was
    /// sent — for an adapter whose durable commit of that write failed, so
    /// the write is neither kept nor sent. Returns whether it was removed:
    /// only the newest row, and only when it is not in flight, can be.
    pub fn cancel_enqueued(&mut self, id: u64) -> bool {
        let in_flight = self
            .inflight
            .iter()
            .any(|s| matches!(s, Slot::Write { id: i, .. } if *i == id));
        if in_flight || self.outbox.back().map(|(i, _)| *i) != Some(id) {
            return false;
        }
        self.outbox.pop_back();
        true
    }

    /// Send a row queued with `connected = false`, now that it is safely
    /// stored: records its reply slot and returns its frame, or `None` when
    /// the socket is closed or being abandoned (the next open replays it).
    pub fn send_enqueued(&mut self, id: u64, connected: bool) -> Option<Vec<u8>> {
        if !connected || self.paused {
            return None;
        }
        let in_flight = self
            .inflight
            .iter()
            .rev()
            .any(|s| matches!(s, Slot::Write { id: i, .. } if *i == id));
        if in_flight {
            return None;
        }
        let frame = self
            .outbox
            .iter()
            .rev()
            .find(|(i, _)| *i == id)
            .map(|(_, f)| f.clone())?;
        self.inflight.push_back(Slot::Write { id, replay: false });
        Some(frame)
    }

    /// Splice a `DEDUP <client> <wire-id>` envelope into an already-encoded
    /// RESP array: bump the element count and prepend three elements.
    fn wrap_dedup(&self, plain: &[u8], outbox_id: u64) -> Vec<u8> {
        // Only the header is text; `rest` is copied through untouched so a
        // binary value inside the frame is never reinterpreted.
        let Some(split) = plain.windows(2).position(|w| w == b"\r\n") else {
            return plain.to_vec();
        };
        let head = &plain[..split];
        let rest = &plain[split + 2..];
        let n: usize = std::str::from_utf8(head)
            .ok()
            .map(|h| h.trim_start_matches('*'))
            .and_then(|h| h.parse().ok())
            .unwrap_or(0);
        if n == 0 {
            return plain.to_vec();
        }
        // `check_capacity` keeps `outbox_id` within 32 bits, so this never
        // wraps onto an id the server has already seen.
        debug_assert!(outbox_id <= MAX_WIRE_SEQ);
        let wire_id = ((self.epoch as u64) << 32) | (outbox_id & MAX_WIRE_SEQ);
        let id_s = wire_id.to_string();
        let mut out = format!(
            "*{}\r\n$5\r\nDEDUP\r\n${}\r\n{}\r\n${}\r\n{}\r\n",
            n + 3,
            self.client_id.len(),
            self.client_id,
            id_s.len(),
            id_s,
        )
        .into_bytes();
        out.extend_from_slice(rest);
        out
    }

    /// Restore outbox rows persisted by a previous session. Rows receive fresh
    /// ids after any writes already queued this session, while preserving their
    /// original order. Adapters must replace durable storage atomically with
    /// the returned rows: an in-place delete/put loop can overwrite a row that
    /// has not been visited yet.
    ///
    /// Every row is kept, even past the outbox cap: they are writes the
    /// application was told were accepted. New writes are refused until the
    /// outbox drains below the cap.
    ///
    /// `connected` says whether the socket is already open. If it is, the
    /// restored rows are returned in [`Restored::send_now`] and must be sent
    /// at once. Holding them for the next reconnect is unsafe: a write this
    /// session sends under the same identity carries a higher wire id, and
    /// once the server has accepted it every restored row would be skipped as
    /// a duplicate — and retired as though applied.
    pub fn restore_outbox(&mut self, mut rows: Vec<(u64, Vec<u8>)>, connected: bool) -> Restored {
        rows.sort_by_key(|(id, _)| *id);
        let mut rewrites = Vec::with_capacity(rows.len());
        for (old_id, frame) in rows {
            let new_id = self.outbox_seq;
            self.outbox_seq = self.outbox_seq.saturating_add(1);
            rewrites.push((old_id, new_id, frame));
        }
        for (_, new_id, frame) in rewrites.iter().rev() {
            self.outbox.push_front((*new_id, frame.clone()));
        }
        let mut send_now = Vec::new();
        if connected && self.open && !self.paused {
            for (_, new_id, frame) in &rewrites {
                send_now.push(frame.clone());
                self.inflight.push_back(Slot::Write {
                    id: *new_id,
                    replay: true,
                });
                self.replay_pending += 1;
            }
            self.counters.replayed_writes += rewrites.len() as u64;
        }
        Restored { rewrites, send_now }
    }

    /// Discard all queued writes and reply bookkeeping (sign-out semantics).
    pub fn clear_outbox(&mut self) {
        self.deferring = false;
        self.outbox.clear();
        // Session slots keep their meaning; only write slots are forgotten,
        // and their replies then retire nothing.
        for slot in self.inflight.iter_mut() {
            if matches!(slot, Slot::Write { .. }) {
                *slot = Slot::Session;
            }
        }
        self.replay_pending = 0;
        self.sent_then_dropped.clear();
    }

    pub fn outbox_len(&self) -> usize {
        self.outbox.len()
    }

    /// The queued rows, oldest first: `(id, frame)`. For an adapter that must
    /// persist rows queued before its storage was available.
    pub fn outbox_rows(&self) -> impl Iterator<Item = (u64, &[u8])> {
        self.outbox
            .iter()
            .map(|(id, frame)| (*id, frame.as_slice()))
    }

    // ── incoming frames ───────────────────────────────────────────────────

    /// Classify and apply one incoming frame.
    ///
    /// Pushes (RESP3 Push mutations, pub/sub, `keychange` arrays) are applied
    /// or surfaced; everything else is a command reply that answers the
    /// oldest inflight command — `qstate` arrays are both the reply to a
    /// QSUB *and* state to apply. See `docs/server/protocol.md`.
    pub fn handle_frame(&mut self, raw: &[u8]) -> Incoming {
        match Value::parse(raw) {
            Ok((Value::Push(arr), _)) => {
                if arr.len() == 3
                    && let (
                        Value::BulkString(Some(kind)),
                        Value::BulkString(Some(channel)),
                        Value::BulkString(Some(payload)),
                    ) = (&arr[0], &arr[1], &arr[2])
                    && kind.eq_ignore_ascii_case(b"message")
                {
                    return Incoming::PubSub {
                        channel: String::from_utf8_lossy(channel).into_owned(),
                        message: payload.clone(),
                    };
                }
                // `CLIENT PUSHFRAMES ON` sends live-query changes as pushes.
                match push_tag(&arr) {
                    b"keychange" => {
                        return Incoming::Applied {
                            keys: self.apply_keychange(&arr),
                        };
                    }
                    b"keydelta" => {
                        return match self.apply_keydelta(&arr) {
                            Some(key) => Incoming::Applied { keys: vec![key] },
                            None => Incoming::Ignored,
                        };
                    }
                    _ => {}
                }
                if let Ok(cmd) = Command::from_value(Value::Array(Some(arr)))
                    && is_replayable_mutation(&cmd)
                {
                    let keys = mutation_keys(&cmd, &self.store);
                    self.store.execute(cmd);
                    return Incoming::Applied { keys };
                }
                Incoming::Ignored
            }
            Ok((Value::Array(Some(items)), _)) => {
                let tag = push_tag(&items);
                // Without push framing a keychange is a plain array, and the
                // only way to tell it from a reply is its first element.
                if !self.push_frames && tag == b"keychange" {
                    return Incoming::Applied {
                        keys: self.apply_keychange(&items),
                    };
                }
                if !self.push_frames && tag == b"keydelta" {
                    return match self.apply_keydelta(&items) {
                        Some(key) => Incoming::Applied { keys: vec![key] },
                        None => Incoming::Ignored,
                    };
                }
                // A snapshot is only ever the reply to a QSUB. An array that
                // merely looks like one — an `LRANGE` of a list whose first
                // element is "qstate" — answering any other command is that
                // command's reply, and must not be applied: reconciling
                // against it would delete local keys.
                if tag == b"qstate"
                    && let Some(Slot::LiveQuery { pattern, .. }) = self.inflight.front()
                    && items.get(1).is_some_and(
                        |p| matches!(p, Value::BulkString(Some(b)) if b == pattern.as_bytes()),
                    )
                {
                    let Some(Slot::LiveQuery { pattern, initial }) = self.inflight.pop_front()
                    else {
                        unreachable!("front slot checked above");
                    };
                    let keys = self.apply_qstate(&items);
                    self.counters.snapshots_applied += 1;
                    self.live_query_answered(&pattern, initial, SubscriptionState::Current);
                    return Incoming::AppliedReply {
                        retired: None,
                        keys,
                    };
                }
                Incoming::Reply {
                    retired: self.ack_reply(false),
                }
            }
            Ok((Value::Error(reason), _)) => self.error_reply(reason),
            Ok((value, _)) => Incoming::Reply {
                retired: self.ack_reply(matches!(&value, Value::SimpleString(s) if s == "DUP")),
            },
            Err(_) => {
                self.paused = true;
                self.pause_reason = Some(SyncError::MalformedFrame);
                self.counters.malformed_frames += 1;
                Incoming::Malformed
            }
        }
    }

    /// A `QSUB` was answered, by a snapshot or a refusal.
    fn live_query_answered(&mut self, pattern: &str, initial: bool, state: SubscriptionState) {
        if initial {
            self.hydrate_pending = self.hydrate_pending.saturating_sub(1);
        }
        // Another QSUB for the same pattern still in flight will answer
        // again; until it does the query is still hydrating.
        let again = self
            .inflight
            .iter()
            .any(|s| matches!(s, Slot::LiveQuery { pattern: p, .. } if p == pattern));
        if !again {
            self.set_subscription(pattern, state);
        }
    }

    /// An error reply. A session command's error changes nothing — an older
    /// server refusing `CLIENT DELTA ON`, say — beyond what it says about the
    /// session: a refused `AUTH`, sync scope or live query degrades it. A
    /// queued write's error means the server did not apply it, which is not
    /// an acknowledgment: the row is kept when the refusal can clear and
    /// retired, and reported, when it cannot.
    fn error_reply(&mut self, reason: String) -> Incoming {
        let id = match self.inflight.pop_front() {
            Some(Slot::Write { id, replay }) => {
                if replay {
                    self.replay_pending = self.replay_pending.saturating_sub(1);
                }
                id
            }
            Some(Slot::Auth) => {
                self.setup_pending = self.setup_pending.saturating_sub(1);
                self.auth_failed = Some(reason);
                return Incoming::Reply { retired: None };
            }
            Some(Slot::Scope) => {
                self.setup_pending = self.setup_pending.saturating_sub(1);
                self.scope_refused = Some(reason);
                return Incoming::Reply { retired: None };
            }
            Some(Slot::LiveQuery { pattern, initial }) => {
                let failure = if reason.starts_with("NOSCOPE") {
                    SubscriptionFailure::OutOfScope
                } else if reason.contains("initial state exceeds") {
                    SubscriptionFailure::SnapshotTooLarge
                } else {
                    SubscriptionFailure::Other
                };
                self.live_query_answered(
                    &pattern,
                    initial,
                    SubscriptionState::Failed { failure, reason },
                );
                return Incoming::Reply { retired: None };
            }
            Some(Slot::Capability(_)) => {
                self.setup_pending = self.setup_pending.saturating_sub(1);
                return Incoming::Reply { retired: None };
            }
            Some(Slot::Session) | None => return Incoming::Reply { retired: None },
        };
        if self.sent_then_dropped.remove(&id) {
            // Overflow already evicted it; there is nothing left to keep.
            return Incoming::Reply { retired: Some(id) };
        }
        if is_transient_refusal(&reason) {
            self.paused = true;
            self.deferring = true;
            self.counters.deferred_writes += 1;
            self.pause_reason = Some(SyncError::WriteDeferred {
                reason: reason.clone(),
            });
            return Incoming::Deferred { reason };
        }
        self.counters.refused_writes += 1;
        let keys = match self.outbox.iter().position(|(i, _)| *i == id) {
            Some(position) => self
                .outbox
                .remove(position)
                .map(|(_, frame)| frame_keys(&frame, &self.store))
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let unreconciled = keys
            .iter()
            .filter(|k| !self.is_covered(k))
            .cloned()
            .collect();
        Incoming::Refused {
            retired: id,
            keys,
            unreconciled,
            reason,
        }
    }

    /// Re-subscribe every live query covering one of `keys`, so its fresh
    /// snapshot replaces the local copies — after an [`Incoming::Refused`]
    /// write, whose local effect the server never applied. Returns the frames
    /// to send now, each holding a reply slot. Nothing while disconnected: the
    /// next open re-subscribes every live query anyway.
    pub fn resubscribe_covering(&mut self, keys: &[String], connected: bool) -> Vec<Vec<u8>> {
        let patterns: Vec<String> = self
            .live_queries
            .iter()
            .filter(|q| keys.iter().any(|k| glob_match(&q.pattern, k)))
            .map(|q| q.pattern.clone())
            .collect();
        patterns
            .iter()
            .filter_map(|p| self.qsub_frame(p, false, connected))
            .collect()
    }

    /// Frames the client needs sent that no other call returned: today, the
    /// re-subscriptions that repair keys a snapshot protected for a write
    /// which then turned out to be a duplicate. Adapters call this after each
    /// [`handle_frame`](Self::handle_frame) and send what it returns, in
    /// order; each frame holds a reply slot. Empty while disconnected.
    pub fn take_frames_to_send(&mut self, connected: bool) -> Vec<Vec<u8>> {
        if self.resync_keys.is_empty() || !connected || self.paused {
            return Vec::new();
        }
        let keys = std::mem::take(&mut self.resync_keys);
        self.resubscribe_covering(&keys, connected)
    }

    /// A non-error reply arrived: it answers the oldest inflight command.
    /// Returns the retired outbox row id for durable deletion, if that was a
    /// data write.
    fn ack_reply(&mut self, duplicate: bool) -> Option<u64> {
        let (id, replay) = match self.inflight.pop_front()? {
            Slot::Write { id, replay } => (id, replay),
            Slot::Auth => {
                self.setup_pending = self.setup_pending.saturating_sub(1);
                return None;
            }
            Slot::Scope => {
                self.setup_pending = self.setup_pending.saturating_sub(1);
                return None;
            }
            Slot::Capability(capability) => {
                self.setup_pending = self.setup_pending.saturating_sub(1);
                if capability == Capability::PushFrames {
                    self.push_frames = true;
                }
                return None;
            }
            // A QSUB answered by something other than a snapshot — not a
            // reply any server sends — leaves the query unhydrated rather
            // than claiming it current.
            Slot::LiveQuery { pattern, initial } => {
                self.live_query_answered(&pattern, initial, SubscriptionState::Pending);
                return None;
            }
            Slot::Session => return None,
        };
        if replay {
            self.replay_pending = self.replay_pending.saturating_sub(1);
        }
        if self.deferring {
            // A write got through: whatever deferred the last one has
            // cleared, so the next drop starts the backoff over.
            self.deferring = false;
            self.attempts = 0;
        }
        // Replies arrive in send order and the outbox is sent in queue order,
        // so the acknowledged row is almost always the front one. Scanning the
        // whole queue on every reply made draining n writes O(n²). A row that
        // overflow already evicted is known by id and retires nothing. The
        // search remains only for a row a restore placed out of send order.
        let row = if self.outbox.front().is_some_and(|(front, _)| *front == id) {
            self.outbox.pop_front()
        } else if self.sent_then_dropped.remove(&id) {
            // Evicted after it was sent: nothing left to remove.
            None
        } else if let Some(position) = self.outbox.iter().position(|(i, _)| *i == id) {
            self.outbox.remove(position)
        } else {
            None
        };
        if !self.protected.is_empty()
            && let Some((_, frame)) = row
        {
            for key in frame_keys(&frame, &self.store) {
                // An applied write's own keychange brings the server's value;
                // a duplicate produces none, so its live query must re-send.
                if self.protected.remove(&key) && duplicate {
                    self.resync_keys.push(key);
                }
            }
        }
        Some(id)
    }

    /// Apply a `["keydelta", key, op, arg...]` frame.
    ///
    /// The delta is a command, so applying it is executing that command
    /// against the local store — the same engine the server ran it on. That
    /// is the whole reason deltas are safe to replay: there is no diff to
    /// misinterpret.
    ///
    /// Returns the changed key, or `None` for a frame this client cannot
    /// rebuild, which it then reports as ignored rather than applied. That
    /// should be unreachable — a server only sends deltas after
    /// `CLIENT DELTA ON`, and only for ops defined here — but silently dropping
    /// one would leave the local copy wrong with no signal, so it is surfaced
    /// rather than swallowed.
    fn apply_keydelta(&self, items: &[Value]) -> Option<String> {
        if items.len() < 4 {
            return None;
        }
        let Value::BulkString(Some(key)) = &items[1] else {
            return None;
        };
        let Value::BulkString(Some(op)) = &items[2] else {
            return None;
        };
        // Rebuild the original command text and let the ordinary parser
        // produce it, so the delta path cannot drift from the command path.
        let mut parts: Vec<Value> = Vec::with_capacity(items.len() - 1);
        parts.push(Value::BulkString(Some(op.clone())));
        parts.push(Value::BulkString(Some(key.clone())));
        for arg in &items[3..] {
            match arg {
                Value::BulkString(Some(bytes)) => {
                    parts.push(Value::BulkString(Some(bytes.clone())))
                }
                _ => return None,
            }
        }
        let Ok(cmd) = Command::from_value(Value::Array(Some(parts))) else {
            return None;
        };
        if !is_replayable_mutation(&cmd) {
            return None;
        }
        self.store.execute(cmd);
        Some(String::from_utf8_lossy(key).into_owned())
    }

    /// Apply a `["keychange", key, value]` frame. Returns the keys it changed:
    /// the key itself, or for the FLUSHDB sentinel every local key the
    /// pattern matched.
    fn apply_keychange(&self, items: &[Value]) -> Vec<String> {
        if items.len() != 3 {
            return Vec::new();
        }
        let Value::BulkString(Some(key)) = &items[1] else {
            return Vec::new();
        };
        let key = String::from_utf8_lossy(key).into_owned();
        let (value, ttl_ms) = split_expiry(&items[2]);
        match value {
            Value::BulkString(Some(v)) => {
                self.store
                    .execute(Command::Set(key.clone(), v.clone(), Default::default()));
                self.expire_in(&key, ttl_ms);
                vec![key]
            }
            Value::BulkString(None) => {
                // A nil value whose "key" is one of our registered live-query
                // patterns is the FLUSHDB sentinel: every matching key is gone.
                // Sending one frame per deleted key would mean millions of
                // frames for a single command, so the server announces it once
                // per pattern and the client expands it locally.
                if self.live_queries.iter().any(|q| q.pattern == key) {
                    let matched: Vec<String> = self
                        .store
                        .matching_key_values(&key, usize::MAX)
                        .into_iter()
                        .map(|(k, _)| k)
                        .collect();
                    if !matched.is_empty() {
                        self.store.execute(Command::Del(matched.clone()));
                    }
                    matched
                } else {
                    self.store.execute(Command::Del(vec![key.clone()]));
                    vec![key]
                }
            }
            // Type-tagged collection: ["hash"|"list"|"set"|"zset"|"json", ...].
            Value::Array(Some(items)) => {
                self.apply_tagged(key.clone(), items);
                self.expire_in(&key, ttl_ms);
                vec![key]
            }
            // Anything else (e.g. the "ratelimit" marker) carries no client-
            // usable value.
            _ => Vec::new(),
        }
    }

    /// Give a key just written from the server the time to live that came with
    /// it. Writing the value cleared any earlier one, so a key without one
    /// stays permanent, as it is on the server.
    fn expire_in(&self, key: &str, ttl_ms: Option<u64>) {
        if let Some(ms) = ttl_ms {
            self.store.execute(Command::PExpire(key.to_string(), ms));
        }
    }

    /// Apply a type-tagged collection value from a keychange or qstate frame.
    ///
    /// The key is replaced wholesale rather than merged: the frame carries the
    /// complete current value, so rebuilding is both simpler and correct when a
    /// member was removed. Anything unrecognised is ignored rather than guessed
    /// at, so a newer server cannot corrupt an older client's local copy.
    fn apply_tagged(&self, key: String, items: &[Value]) {
        fn text(v: &Value) -> Option<String> {
            match v {
                Value::BulkString(Some(b)) => Some(String::from_utf8_lossy(b).into_owned()),
                Value::SimpleString(s) => Some(s.clone()),
                _ => None,
            }
        }
        fn bytes(v: &Value) -> Option<Vec<u8>> {
            match v {
                Value::BulkString(Some(b)) => Some(b.clone()),
                Value::SimpleString(s) => Some(s.as_bytes().to_vec()),
                _ => None,
            }
        }
        let Some(tag) = items.first().and_then(text) else {
            return;
        };
        // Values may be arbitrary bytes; hash fields, set and sorted-set
        // members are identifiers and stay text. `raw` keeps the bytes, `as_text`
        // reinterprets a slot when the position calls for an identifier.
        let raw: Vec<Vec<u8>> = items[1..].iter().filter_map(bytes).collect();
        let as_text = |b: &Vec<u8>| String::from_utf8_lossy(b).into_owned();

        // Clear first so removed members do not linger.
        self.store.execute(Command::Del(vec![key.clone()]));
        match tag.as_str() {
            "hash" => {
                let pairs: Vec<(String, Vec<u8>)> = raw
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| (as_text(&c[0]), c[1].clone()))
                    .collect();
                if !pairs.is_empty() {
                    self.store.execute(Command::HSet(key, pairs));
                }
            }
            "list" => {
                if !raw.is_empty() {
                    self.store.execute(Command::RPush(key, raw));
                }
            }
            "set" => {
                if !raw.is_empty() {
                    self.store
                        .execute(Command::SAdd(key, raw.iter().map(as_text).collect()));
                }
            }
            "zset" => {
                let members: Vec<(f64, String)> = raw
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .filter_map(|c| {
                        as_text(&c[1])
                            .parse::<f64>()
                            .ok()
                            .map(|sc| (sc, as_text(&c[0])))
                    })
                    .collect();
                if !members.is_empty() {
                    self.store
                        .execute(Command::ZAdd(key, Default::default(), members));
                }
            }
            "json" => {
                // JSON is UTF-8 by definition, so this position is text.
                if let Some(doc) = raw.first() {
                    self.store
                        .execute(Command::JSet(key, "$".to_string(), as_text(doc)));
                }
            }
            _ => {}
        }
    }

    /// Apply a `["qstate", pattern, k, v, ...]` snapshot, and reconcile the
    /// local copy of `pattern` against it.
    ///
    /// Applying the snapshot alone is not enough. A snapshot is the complete
    /// truth for its pattern, so a key held locally that the snapshot does not
    /// carry has been deleted or has expired — and nothing else will ever say
    /// so. `keychange` only reports changes that happened while the socket was
    /// up, so a removal during a disconnect used to leave the key in local
    /// memory, with its last value, forever. A browser tab hid this by
    /// starting empty on reload; a server process holding the same cache for
    /// months does not.
    ///
    /// The server refuses snapshots above `RECACHED_MAX_QSUB_INITIAL_KEYS`
    /// rather than sending a partial one, so every qstate reaching this method
    /// is complete and absence is safe to interpret as deletion. A refusal is
    /// an error reply, which never reaches here.
    ///
    /// **Keys with a queued write are left alone.** The server answers in
    /// send order, so by the time a snapshot arrives every write sent before
    /// its `QSUB` has been answered, and any row still queued is one the
    /// snapshot does not reflect: sent after the `QSUB`, deferred, or not sent
    /// yet. Applying the snapshot to its keys would roll back a write the
    /// application has already seen succeed — or, for a key it created
    /// offline, delete it — until the write's own change came back. Those
    /// keys keep their local value; the write's acknowledgment and the
    /// `keychange` it produces bring the server's value, and a refusal
    /// re-subscribes so a fresh snapshot does.
    ///
    /// Returns every key the snapshot carried followed by every stale key it
    /// removed, leaving out the protected keys, which did not change.
    fn apply_qstate(&mut self, items: &[Value]) -> Vec<String> {
        let pattern = match items.get(1) {
            Some(Value::BulkString(Some(p))) => String::from_utf8_lossy(p).into_owned(),
            _ => String::new(),
        };
        let protected: HashSet<String> = self
            .outbox
            .iter()
            .flat_map(|(_, frame)| frame_keys(frame, &self.store))
            .collect();

        let mut present: HashSet<String> = HashSet::new();
        let mut changed: Vec<String> = Vec::new();
        for pair in items.get(2..).unwrap_or(&[]).as_chunks::<2>().0 {
            let Value::BulkString(Some(k)) = &pair[0] else {
                continue;
            };
            let key = String::from_utf8_lossy(k).into_owned();
            if !present.insert(key.clone()) {
                continue;
            }
            if protected.contains(&key) {
                self.protected.insert(key);
                continue;
            }
            changed.push(key.clone());
            let (value, ttl_ms) = split_expiry(&pair[1]);
            match value {
                Value::BulkString(Some(v)) => {
                    self.store
                        .execute(Command::Set(key.clone(), v.clone(), Default::default()));
                    self.expire_in(&key, ttl_ms);
                }
                // Collections arrive type-tagged, so the initial state of a
                // live query is complete — no follow-up typed read needed.
                Value::Array(Some(inner)) => {
                    self.apply_tagged(key.clone(), inner);
                    self.expire_in(&key, ttl_ms);
                }
                _ => {}
            }
        }

        if pattern.is_empty() {
            return changed;
        }
        let stale: Vec<String> = self
            .store
            .matching_keys(&pattern)
            .into_iter()
            .filter(|k| !present.contains(k))
            .collect();
        let (kept, stale): (Vec<String>, Vec<String>) =
            stale.into_iter().partition(|k| protected.contains(k));
        self.protected.extend(kept);
        if !stale.is_empty() {
            self.store.execute(Command::Del(stale.clone()));
            changed.extend(stale);
        }
        changed
    }
}

/// Split a synced value from the `["px", <ms>, value]` wrapper a server sends
/// after `CLIENT EXPIRY ON` while the key has a time to live.
fn split_expiry(value: &Value) -> (&Value, Option<u64>) {
    if let Value::Array(Some(items)) = value
        && let [Value::BulkString(Some(tag)), Value::Integer(ms), inner] = items.as_slice()
        && tag == b"px"
    {
        return (inner, Some((*ms).max(0) as u64));
    }
    (value, None)
}

/// The first element of an array frame as bytes, or empty: the tag that
/// names `keychange`, `keydelta` and `qstate` frames.
fn push_tag(items: &[Value]) -> &[u8] {
    match items.first() {
        Some(Value::BulkString(Some(t))) => t,
        _ => &[],
    }
}

/// Whether a refused write may be accepted later unchanged; shared with the
/// server so both sides classify a refusal alike.
pub use core_engine::resp::is_transient_refusal;

/// The keys a queued write frame changes, looking through its `DEDUP` envelope.
fn frame_keys(frame: &[u8], store: &KeyValueStore) -> Vec<String> {
    let Ok((value, _)) = Value::parse(frame) else {
        return Vec::new();
    };
    match Command::from_value(value) {
        Ok(Command::Dedup(_, _, inner)) => mutation_keys(&inner, store),
        Ok(cmd) => mutation_keys(&cmd, store),
        Err(_) => Vec::new(),
    }
}

fn sync_scopes_frame(patterns_csv: &str) -> Option<Vec<u8>> {
    let pats: Vec<&str> = patterns_csv
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if pats.is_empty() {
        return None;
    }
    let mut parts = vec!["SYNC"];
    parts.extend(pats);
    Some(to_resp(&parts))
}

/// Mutations another peer may push at us that are safe to replay into the
/// local store. Everything else (command replies, admin, unknown) is ignored.
pub fn is_replayable_mutation(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::Set(_, _, _)
            | Command::ESet(_, _)
            | Command::EAdd(_, _)
            // `broadcast_for` emits APPEND verbatim, so leaving it out here
            // meant a client syncing on the propagation path applied every
            // mutation except this one and held a stale value with no signal.
            // It is also the op `keydelta` exists for.
            | Command::Append(_, _)
            | Command::Del(_)
            | Command::Unlink(_)
            | Command::MSet(_)
            | Command::Incr(_)
            | Command::Decr(_)
            | Command::IncrBy(_, _)
            | Command::DecrBy(_, _)
            | Command::Expire(_, _)
            | Command::PExpire(_, _)
            | Command::ExpireAt(_, _)
            | Command::PExpireAt(_, _)
            | Command::Persist(_)
            | Command::FlushDb
            | Command::Rename(_, _)
            | Command::HSet(_, _)
            | Command::HDel(_, _)
            | Command::HSetNx(_, _, _)
            | Command::LPush(_, _)
            | Command::RPush(_, _)
            | Command::LPop(_, _)
            | Command::RPop(_, _)
            | Command::LSet(_, _, _)
            | Command::LRem(_, _, _)
            | Command::LTrim(_, _, _)
            | Command::SAdd(_, _)
            | Command::SRem(_, _)
            | Command::SMove(_, _, _)
            | Command::SInterStore(_, _)
            | Command::SUnionStore(_, _)
            | Command::SDiffStore(_, _)
            | Command::ZAdd(_, _, _)
            | Command::ZRem(_, _)
            | Command::ZIncrBy(_, _, _)
            | Command::JSet(_, _, _)
            | Command::JMerge(_, _)
    )
}

/// The keys a replayable mutation may change, for change notification.
///
/// Call it *before* executing the command: `FLUSHDB` names every key the store
/// holds at that moment, which afterwards is no longer knowable. A command that
/// is not a replayable mutation changes nothing a client tracks and yields no
/// keys. Adapters use this for their own local writes too, so a write the
/// application makes notifies the same subscribers a pushed one would.
pub fn mutation_keys(cmd: &Command, store: &KeyValueStore) -> Vec<String> {
    match cmd {
        Command::Set(k, _, _)
        | Command::ESet(k, _)
        | Command::EAdd(k, _)
        | Command::Append(k, _)
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
        | Command::LPush(k, _)
        | Command::RPush(k, _)
        | Command::LPop(k, _)
        | Command::RPop(k, _)
        | Command::LSet(k, _, _)
        | Command::LRem(k, _, _)
        | Command::LTrim(k, _, _)
        | Command::SAdd(k, _)
        | Command::SRem(k, _)
        | Command::SInterStore(k, _)
        | Command::SUnionStore(k, _)
        | Command::SDiffStore(k, _)
        | Command::ZAdd(k, _, _)
        | Command::ZRem(k, _)
        | Command::ZIncrBy(k, _, _)
        | Command::JSet(k, _, _)
        | Command::JMerge(k, _) => vec![k.clone()],
        Command::Del(keys) | Command::Unlink(keys) => keys.clone(),
        Command::MSet(pairs) => pairs.iter().map(|(k, _)| k.clone()).collect(),
        Command::Rename(from, to) | Command::SMove(from, to, _) => {
            vec![from.clone(), to.clone()]
        }
        Command::FlushDb => store.matching_keys("*"),
        _ => Vec::new(),
    }
}

/// RESP array encoding, shared by adapters.
/// Encode a RESP array frame from text arguments.
///
/// Returns bytes, not a `String`: frames are concatenated with, and replayed
/// alongside, frames carrying binary values, so the whole pipeline is byte-based.
pub fn to_resp(parts: &[&str]) -> Vec<u8> {
    let byte_parts: Vec<&[u8]> = parts.iter().map(|p| p.as_bytes()).collect();
    to_resp_bytes(&byte_parts)
}

/// Encode a RESP array frame from raw byte arguments, for commands carrying a
/// binary value.
pub fn to_resp_bytes(parts: &[&[u8]]) -> Vec<u8> {
    let mut out = format!("*{}\r\n", parts.len()).into_bytes();
    for part in parts {
        out.extend_from_slice(format!("${}\r\n", part.len()).as_bytes());
        out.extend_from_slice(part);
        out.extend_from_slice(b"\r\n");
    }
    out
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod reliability_tests;

#[cfg(test)]
mod model_tests;
