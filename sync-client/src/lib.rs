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
use core_engine::store::KeyValueStore;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

/// Cap on outbox entries. When full, the oldest is dropped (reported via
/// [`Enqueued::dropped`] so durable storage can be cleaned up too).
pub const MAX_PENDING_WRITES: usize = 10_000;

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
    /// A mutation (server push, `keychange`, or `qstate`) was applied to the
    /// local store — notify UI subscribers.
    Applied,
    /// A pub/sub message — deliver to channel subscribers.
    /// A pub/sub message. The payload is bytes because a publisher may send
    /// binary; the adapter decides how to surface it.
    PubSub { channel: String, message: Vec<u8> },
    /// A command reply. When `retired` is set, that outbox row is now
    /// acknowledged — delete it from durable storage.
    Reply { retired: Option<u64> },
    /// A `qstate` frame: the reply to a QSUB (retire the row, if any) *and*
    /// state applied to the local store (notify UI subscribers).
    AppliedReply { retired: Option<u64> },
    /// A frame we understand but that asks for nothing — a push we do not
    /// model. It consumes no reply slot, which is correct: the server did not
    /// treat it as one either.
    Ignored,
    /// The frame could not be parsed at all.
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
    /// The outbox overflowed and this row id was evicted — delete it from
    /// durable storage.
    pub dropped: Option<u64>,
}

/// The platform-neutral sync state machine. One instance per connection
/// identity (i.e. per cache).
pub struct SyncClient {
    store: Arc<KeyValueStore>,
    /// Writes not yet acknowledged by the server, in send order.
    outbox: VecDeque<(u64, Vec<u8>)>,
    outbox_seq: u64,
    /// One entry per frame sent on the current socket: `Some(outbox id)` for
    /// data writes, `None` for session commands. Replies arrive in send
    /// order; each acknowledges the front entry.
    inflight: VecDeque<Option<u64>>,
    client_id: String,
    /// Upper 32 bits of every dedup wire id; bump per session (persisted by
    /// the adapter) so a new session's ids clear the server's high-water mark.
    epoch: u32,
    password: Option<String>,
    sync_token: Option<String>,
    sync_scopes_csv: Option<String>,
    live_queries: Vec<String>,
    attempts: u32,
    /// Cap on queued-but-unacknowledged writes. Beyond it the oldest is
    /// evicted. Configurable because the right depth depends on how long a
    /// client is expected to be offline and how large its writes are.
    max_pending: usize,
    /// Jitter source for reconnect backoff. Seeded from `client_id` rather than
    /// a system RNG so this crate stays dependency-free and I/O-free — and so
    /// the sequence is reproducible in tests. Different clients get different
    /// sequences, which is the only property that matters here.
    jitter: u64,
}

impl SyncClient {
    pub fn new(store: Arc<KeyValueStore>, client_id: String) -> Self {
        let jitter = seed_from(&client_id);
        Self {
            store,
            outbox: VecDeque::new(),
            outbox_seq: 0,
            inflight: VecDeque::new(),
            client_id,
            epoch: 0,
            password: None,
            sync_token: None,
            sync_scopes_csv: None,
            live_queries: Vec::new(),
            attempts: 0,
            max_pending: MAX_PENDING_WRITES,
            jitter,
        }
    }

    pub fn store(&self) -> &Arc<KeyValueStore> {
        &self.store
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Set the outbox cap (minimum 1). Writes past it evict the oldest, which
    /// is reported through the adapter's overflow callback.
    pub fn set_max_pending(&mut self, max: usize) {
        self.max_pending = max.max(1);
    }

    /// Current outbox cap.
    pub fn max_pending(&self) -> usize {
        self.max_pending
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

    // ── session state ─────────────────────────────────────────────────────
    // Setters record state for re-establishment on every reconnect and, when
    // the socket is open, return the frame to send now (recording it as
    // inflight — session replies must occupy a reply slot or they would
    // falsely acknowledge a data write).

    pub fn set_password(&mut self, password: &str, connected: bool) -> Option<Vec<u8>> {
        self.password = Some(password.to_string());
        self.session_frame(to_resp(&["AUTH", password]), connected)
    }

    pub fn set_sync_token(&mut self, token: &str, connected: bool) -> Option<Vec<u8>> {
        self.sync_token = Some(token.to_string());
        self.session_frame(to_resp(&["SYNC", "TOKEN", token]), connected)
    }

    /// Comma-separated glob patterns. Returns `None` (and records nothing)
    /// when the list is empty.
    pub fn set_sync_scopes(&mut self, patterns_csv: &str, connected: bool) -> Option<Vec<u8>> {
        let frame = sync_scopes_frame(patterns_csv)?;
        self.sync_scopes_csv = Some(patterns_csv.to_string());
        self.session_frame(frame, connected)
    }

    /// Register a live query (idempotent). The returned frame re-hydrates
    /// matching keys via the `qstate` reply.
    pub fn add_live_query(&mut self, pattern: &str, connected: bool) -> Option<Vec<u8>> {
        if !self.live_queries.iter().any(|p| p == pattern) {
            self.live_queries.push(pattern.to_string());
        }
        self.session_frame(to_resp(&["QSUB", pattern]), connected)
    }

    pub fn remove_live_query(&mut self, pattern: Option<&str>, connected: bool) -> Option<Vec<u8>> {
        let frame = match pattern {
            Some(p) => {
                self.live_queries.retain(|q| q != p);
                to_resp(&["QUNSUB", p])
            }
            None => {
                self.live_queries.clear();
                to_resp(&["QUNSUB"])
            }
        };
        self.session_frame(frame, connected)
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
        self.session_frame(frame, connected)
    }

    fn session_frame(&mut self, frame: Vec<u8>, connected: bool) -> Option<Vec<u8>> {
        if connected {
            self.inflight.push_back(None);
            Some(frame)
        } else {
            None
        }
    }

    // ── connection lifecycle ──────────────────────────────────────────────

    /// The socket opened: returns every frame to send, in order — session
    /// state first (AUTH → SYNC → live queries), then the full outbox replay.
    /// Live-query re-subscription re-hydrates local state; outbox entries
    /// stay queued until their replies acknowledge them.
    pub fn on_open(&mut self) -> Vec<Vec<u8>> {
        self.attempts = 0;
        self.inflight.clear();
        let mut frames = Vec::new();
        if let Some(pwd) = &self.password {
            frames.push(to_resp(&["AUTH", pwd]));
            self.inflight.push_back(None);
        }
        // This client understands `keydelta`, so say so. Sent every time the
        // socket opens because it is connection state, not session state, and
        // sent before QSUB so the initial live-query traffic already benefits.
        // An older server answers with an error, which `ack_reply` retires
        // like any other reply and which changes nothing: it simply keeps
        // sending whole values.
        frames.push(to_resp(&["CLIENT", "DELTA", "ON"]));
        self.inflight.push_back(None);
        if let Some(tok) = &self.sync_token {
            frames.push(to_resp(&["SYNC", "TOKEN", tok]));
            self.inflight.push_back(None);
        } else if let Some(csv) = &self.sync_scopes_csv
            && let Some(frame) = sync_scopes_frame(csv)
        {
            frames.push(frame);
            self.inflight.push_back(None);
        }
        for pat in &self.live_queries {
            frames.push(to_resp(&["QSUB", pat]));
            self.inflight.push_back(None);
        }
        for (id, frame) in &self.outbox {
            frames.push(frame.clone());
            self.inflight.push_back(Some(*id));
        }
        frames
    }

    /// The socket closed: returns the delay before the next reconnect
    /// attempt (exponential backoff, capped).
    /// Delay before the next reconnect attempt, in milliseconds.
    ///
    /// Exponential with a cap, then **jittered into `[delay/2, delay]`**. Without
    /// jitter every client disconnected by the same event computes an identical
    /// schedule and reconnects in lockstep — a thundering herd that can keep a
    /// recovering server down. Halving the floor keeps the growth shape intact
    /// while spreading arrivals across the window.
    pub fn on_close(&mut self) -> u32 {
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

    // ── writes ────────────────────────────────────────────────────────────

    /// Queue a write for the server. `dedup` wraps it in a duplicate-suppression
    /// envelope — store writes want this; connection-scoped commands
    /// (pub/sub) must not be wrapped, or a legitimately replayed SUBSCRIBE
    /// would be skipped as a duplicate.
    pub fn enqueue_write(&mut self, encoded: &[u8], dedup: bool, connected: bool) -> Enqueued {
        let id = self.outbox_seq;
        self.outbox_seq += 1;
        let frame = if dedup {
            self.wrap_dedup(encoded, id)
        } else {
            encoded.to_vec()
        };
        let dropped = if self.outbox.len() >= self.max_pending {
            self.outbox.pop_front().map(|(old, _)| old)
        } else {
            None
        };
        self.outbox.push_back((id, frame.clone()));
        if connected {
            self.inflight.push_back(Some(id));
        }
        Enqueued {
            id,
            frame,
            send_now: connected,
            dropped,
        }
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
        let wire_id = ((self.epoch as u64) << 32) | (outbox_id & 0xFFFF_FFFF);
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
    pub fn restore_outbox(&mut self, mut rows: Vec<(u64, Vec<u8>)>) -> Vec<(u64, u64, Vec<u8>)> {
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
        rewrites
    }

    /// Discard all queued writes and reply bookkeeping (sign-out semantics).
    pub fn clear_outbox(&mut self) {
        self.outbox.clear();
        self.inflight.clear();
    }

    pub fn outbox_len(&self) -> usize {
        self.outbox.len()
    }

    // ── incoming frames ───────────────────────────────────────────────────

    /// Classify and apply one incoming text frame.
    ///
    /// Pushes (RESP3 Push mutations, pub/sub, `keychange` arrays) are applied
    /// or surfaced; everything else is a command reply that acknowledges the
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
                if let Ok(cmd) = Command::from_value(Value::Array(Some(arr)))
                    && is_replayable_mutation(&cmd)
                {
                    self.store.execute(cmd);
                    return Incoming::Applied;
                }
                Incoming::Ignored
            }
            Ok((Value::Array(Some(items)), _)) => {
                let tag: &[u8] = match items.first() {
                    Some(Value::BulkString(Some(t))) => t,
                    _ => &[],
                };
                if tag == b"keychange" {
                    self.apply_keychange(&items);
                    Incoming::Applied
                } else if tag == b"keydelta" {
                    if self.apply_keydelta(&items) {
                        Incoming::Applied
                    } else {
                        Incoming::Ignored
                    }
                } else if tag == b"qstate" {
                    let retired = self.ack_reply();
                    self.apply_qstate(&items);
                    Incoming::AppliedReply { retired }
                } else {
                    Incoming::Reply {
                        retired: self.ack_reply(),
                    }
                }
            }
            Ok(_) => Incoming::Reply {
                retired: self.ack_reply(),
            },
            Err(_) => Incoming::Malformed,
        }
    }

    /// A reply arrived: acknowledge the oldest inflight command. Returns the
    /// retired outbox row id for durable deletion, if it was a data write.
    fn ack_reply(&mut self) -> Option<u64> {
        let id = self.inflight.pop_front()??;
        self.outbox.retain(|(i, _)| *i != id);
        Some(id)
    }

    /// Apply a `["keydelta", key, op, arg...]` frame.
    ///
    /// The delta is a command, so applying it is executing that command
    /// against the local store — the same engine the server ran it on. That
    /// is the whole reason deltas are safe to replay: there is no diff to
    /// misinterpret.
    ///
    /// Returns false for a frame this client cannot rebuild, which it then
    /// reports as ignored rather than applied. That should be unreachable —
    /// a server only sends deltas after `CLIENT DELTA ON`, and only for ops
    /// defined here — but silently dropping one would leave the local copy
    /// wrong with no signal, so it is surfaced rather than swallowed.
    fn apply_keydelta(&self, items: &[Value]) -> bool {
        if items.len() < 4 {
            return false;
        }
        let Value::BulkString(Some(key)) = &items[1] else {
            return false;
        };
        let Value::BulkString(Some(op)) = &items[2] else {
            return false;
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
                _ => return false,
            }
        }
        let Ok(cmd) = Command::from_value(Value::Array(Some(parts))) else {
            return false;
        };
        if !is_replayable_mutation(&cmd) {
            return false;
        }
        self.store.execute(cmd);
        true
    }

    fn apply_keychange(&self, items: &[Value]) {
        if items.len() != 3 {
            return;
        }
        let Value::BulkString(Some(key)) = &items[1] else {
            return;
        };
        let key = String::from_utf8_lossy(key).into_owned();
        match &items[2] {
            Value::BulkString(Some(v)) => {
                self.store
                    .execute(Command::Set(key, v.clone(), Default::default()));
            }
            Value::BulkString(None) => {
                // A nil value whose "key" is one of our registered live-query
                // patterns is the FLUSHDB sentinel: every matching key is gone.
                // Sending one frame per deleted key would mean millions of
                // frames for a single command, so the server announces it once
                // per pattern and the client expands it locally.
                if self.live_queries.contains(&key) {
                    let matched: Vec<String> = self
                        .store
                        .matching_key_values(&key, usize::MAX)
                        .into_iter()
                        .map(|(k, _)| k)
                        .collect();
                    if !matched.is_empty() {
                        self.store.execute(Command::Del(matched));
                    }
                } else {
                    self.store.execute(Command::Del(vec![key]));
                }
            }
            // Type-tagged collection: ["hash"|"list"|"set"|"zset"|"json", ...].
            Value::Array(Some(items)) => self.apply_tagged(key, items),
            // Anything else (e.g. the "ratelimit" marker) carries no client-
            // usable value.
            _ => {}
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
    /// is complete and absence is safe to interpret as deletion.
    fn apply_qstate(&self, items: &[Value]) {
        let pattern = match items.get(1) {
            Some(Value::BulkString(Some(p))) => String::from_utf8_lossy(p).into_owned(),
            _ => String::new(),
        };

        let mut present: HashSet<String> = HashSet::new();
        for pair in items.get(2..).unwrap_or(&[]).as_chunks::<2>().0 {
            let Value::BulkString(Some(k)) = &pair[0] else {
                continue;
            };
            let key = String::from_utf8_lossy(k).into_owned();
            present.insert(key.clone());
            match &pair[1] {
                Value::BulkString(Some(v)) => {
                    self.store
                        .execute(Command::Set(key, v.clone(), Default::default()));
                }
                // Collections arrive type-tagged, so the initial state of a
                // live query is complete — no follow-up typed read needed.
                Value::Array(Some(inner)) => self.apply_tagged(key, inner),
                _ => {}
            }
        }

        if pattern.is_empty() {
            return;
        }
        let stale: Vec<String> = self
            .store
            .matching_keys(&pattern)
            .into_iter()
            .filter(|k| !present.contains(k))
            .collect();
        if !stale.is_empty() {
            self.store.execute(Command::Del(stale));
        }
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
