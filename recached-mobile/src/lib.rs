//! The Recached client core for Kotlin and Swift, exported through UniFFI.
//!
//! One [`RecachedClient`] holds a local copy of the cache in memory, persists
//! it and the outbox of unacknowledged writes to an on-device SQLite file, and
//! runs the platform-neutral sync state machine from `sync-client`.
//!
//! It opens no sockets and runs no async runtime. The app owns the WebSocket —
//! OkHttp on Android, `URLSessionWebSocketTask` on Apple platforms — and drives
//! the client through three calls:
//!
//! - [`connection_opened`](RecachedClient::connection_opened) when the socket
//!   opens, handing over a [`FrameSink`] to send through;
//! - [`frame_received`](RecachedClient::frame_received) for every incoming
//!   frame, which returns the keys that changed;
//! - [`connection_closed`](RecachedClient::connection_closed) when it closes,
//!   which returns how long to wait before reconnecting.
//!
//! Reads never touch the network or the database: they read local memory.
//! Writes apply locally, commit to disk together with their outbox row, and
//! go out on the socket when it is open — or on the next reconnect when not.
//! A write that returns `Ok` is in the database; one that returns an error
//! left nothing behind — not in memory, not on disk, not on the socket.
//!
//! An open socket is not a synchronized one. [`sync_status`](RecachedClient::sync_status)
//! moves through authentication, the replay of queued writes and the
//! snapshots of watched patterns before it reads `Ready`.

use core_engine::cmd::{Command, SetExpiry, SetOptions};
use core_engine::resp::Value;
use core_engine::store::{KeyValueStore, SnapshotEntry, glob_match};
use std::sync::{Arc, Mutex, MutexGuard};
use sync_client::{
    Incoming, OverflowPolicy, SyncClient, mutation_keys, next_epoch, to_resp, to_resp_bytes,
};

mod error;
mod persist;
mod status;

pub use error::RecachedError;
use persist::Db;
pub use status::{
    SubscriptionFailure, SubscriptionState, SyncCounters, SyncError, SyncPhase, SyncStatus,
};

uniffi::setup_scaffolding!();

/// Options for [`RecachedClient::open`].
#[derive(Debug, Default, uniffi::Record)]
pub struct ClientConfig {
    /// Sent as `AUTH` each time the socket opens.
    #[uniffi(default = None)]
    pub password: Option<String>,
    /// Sent as `SYNC TOKEN` each time the socket opens.
    #[uniffi(default = None)]
    pub sync_token: Option<String>,
    /// Cap on queued, unacknowledged writes; past it a write fails with
    /// `OutboxFull` and changes nothing. Defaults to 10,000.
    #[uniffi(default = None)]
    pub max_pending_writes: Option<u32>,
    /// Past the cap, evict the oldest queued write instead of refusing the
    /// new one — the behaviour before 0.5. An evicted write never reaches
    /// the server, though the app may already have shown it as saved.
    #[uniffi(default = None)]
    pub evict_oldest_when_full: Option<bool>,
}

/// Where the client sends frames while the socket is open.
///
/// Implemented by the app's socket wrapper. `send` is called while the client
/// holds its lock — that is what guarantees frames reach the socket in the
/// order the client recorded them, which reply matching depends on — so it
/// must queue the frame and return. It must not block, and must not call back
/// into the client. OkHttp's `WebSocket.send` and
/// `URLSessionWebSocketTask.send(_:completionHandler:)` both qualify.
///
/// A frame whose bytes are valid UTF-8 should go out as a text message, and
/// anything else as a binary message.
#[uniffi::export(with_foreign)]
pub trait FrameSink: Send + Sync {
    fn send(&self, frame: Vec<u8>);
}

/// What an incoming frame did.
#[derive(Debug, PartialEq, uniffi::Record)]
pub struct FrameOutcome {
    /// Keys the frame may have changed, deletions included. Notify their
    /// observers. A key can appear whose value came out the same.
    pub changed_keys: Vec<String>,
    /// Close the socket and reconnect after the delay
    /// [`connection_closed`](RecachedClient::connection_closed) returns.
    /// Nothing queued is lost. Either reply matching can no longer be trusted
    /// (the frame could not be parsed), or the server deferred a queued write
    /// for a reason that can clear, such as a sync token it has not accepted
    /// yet, and the reconnect replays it.
    pub reconnect: bool,
    /// A queued write the server refused for good, now removed from the
    /// outbox. Tell the application: it never reached the server.
    pub refused: Option<RefusedWrite>,
    /// The synchronization status, when this frame changed it. Notify status
    /// observers.
    pub status: Option<SyncStatus>,
}

/// A write the server will never apply, from [`FrameOutcome::refused`].
///
/// Its local effect stays, since the client cannot undo it. Keys a watched
/// pattern covers get the server's values back from a fresh snapshot shortly
/// after; any other key keeps the refused value until the app changes it.
#[derive(Debug, PartialEq, uniffi::Record)]
pub struct RefusedWrite {
    /// The keys the write would have changed.
    pub keys: Vec<String>,
    /// Those of `keys` no watched pattern covers. Their local value is the
    /// refused one, and nothing will correct it: show the write as failed,
    /// and rewrite or re-read them.
    pub unreconciled: Vec<String>,
    /// The server's error, such as `NOSCOPE key 'k' is read-only on this
    /// connection`.
    pub reason: String,
}

/// A watched pattern whose [`SubscriptionState`] changed, from
/// [`RecachedClient::take_subscription_changes`]. `state` is `None` when the
/// pattern was unwatched.
#[derive(Debug, PartialEq, uniffi::Record)]
pub struct SubscriptionChange {
    pub pattern: String,
    pub state: Option<SubscriptionState>,
}

/// One key and its value, from [`RecachedClient::get_matching`].
#[derive(Debug, PartialEq, uniffi::Record)]
pub struct Entry {
    pub key: String,
    /// The bytes of a string value; `None` for a collection, which has no
    /// single-value form.
    pub value: Option<Vec<u8>>,
}

/// True when any of `keys` matches the glob `pattern`, by the engine's own
/// rules — the ones `watch` patterns follow on the server — so a wrapper
/// routing `changed_keys` to pattern observers cannot disagree with it.
///
/// Takes the whole change set in one call: a reconnect snapshot can name
/// thousands of keys, and crossing the FFI once per key would cost more than
/// the re-read it saves.
#[uniffi::export]
pub fn any_key_matches(pattern: String, keys: Vec<String>) -> bool {
    keys.iter().any(|key| glob_match(&pattern, key))
}

/// A Recached cache on the device. See the crate documentation.
#[derive(uniffi::Object)]
pub struct RecachedClient {
    store: Arc<KeyValueStore>,
    inner: Mutex<Inner>,
}

struct Inner {
    sync: SyncClient,
    db: Db,
    /// Present while the socket is open.
    sink: Option<Arc<dyn FrameSink>>,
}

#[uniffi::export]
impl RecachedClient {
    /// Open the cache stored at `path`, creating it if needed.
    ///
    /// Restores the local copy and every write still waiting for the server,
    /// so reads work at once with no network, and the queued writes go out on
    /// the first connection.
    #[uniffi::constructor]
    pub fn open(path: String, config: ClientConfig) -> Result<Arc<Self>, RecachedError> {
        let mut db = Db::open(&path)?;
        let store = Arc::new(KeyValueStore::new());
        store.restore(db.load_entries()?);

        // The id must persist so the server's duplicate suppression spans
        // restarts, and must be unguessable, or another client could poison
        // this one's high-water mark.
        let new_id = || format!("{:032x}", rand::random::<u128>());
        let mut client_id = match db.meta_get("client_id")? {
            Some(id) => id,
            None => {
                let id = new_id();
                db.meta_put("client_id", &id)?;
                id
            }
        };

        // A new epoch per open puts this session's write ids above every id
        // the last one sent. Durable before any write can use it.
        let previous = db
            .meta_get("epoch")?
            .and_then(|e| e.parse::<u32>().ok())
            .unwrap_or(0);
        let epoch = match next_epoch(previous) {
            Some(epoch) => epoch,
            // Every epoch of this identity is used. Saturating would give
            // this session the last one's ids, and the server would skip its
            // writes as duplicates; a fresh identity starts clean. Rows still
            // queued keep the old identity inside their frames.
            None => {
                client_id = new_id();
                db.meta_put("client_id", &client_id)?;
                1
            }
        };
        db.meta_put("epoch", &epoch.to_string())?;
        let mut sync = SyncClient::new(Arc::clone(&store), client_id);
        sync.set_epoch(epoch);

        if let Some(max) = config.max_pending_writes {
            sync.set_max_pending(max as usize);
        }
        if config.evict_oldest_when_full == Some(true) {
            sync.set_overflow_policy(OverflowPolicy::EvictOldest);
        }
        // Restored frames keep their original bytes, wire ids included, so a
        // write the server applied before the app died is recognised and
        // skipped on replay rather than applied twice. Every row is kept,
        // even past the cap.
        let restored = sync.restore_outbox(db.load_outbox()?, false);
        db.replace_outbox(&restored.rewrites)?;

        if let Some(password) = &config.password {
            sync.set_password(password, false);
        }
        if let Some(token) = &config.sync_token {
            sync.set_sync_token(token, false);
        }

        Ok(Arc::new(Self {
            store,
            inner: Mutex::new(Inner {
                sync,
                db,
                sink: None,
            }),
        }))
    }

    // ── reads: local memory only ─────────────────────────────────────────

    /// The value's bytes, or `None` when the key is absent, expired, or holds
    /// a collection.
    pub fn get(&self, key: String) -> Option<Vec<u8>> {
        match self.store.execute(Command::Get(key)) {
            Value::BulkString(Some(bytes)) => Some(bytes),
            _ => None,
        }
    }

    /// The value as text. Fails with `NotUtf8` rather than returning a lossy
    /// rendering of binary data.
    pub fn get_string(&self, key: String) -> Result<Option<String>, RecachedError> {
        match self.get(key.clone()) {
            Some(bytes) => String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| RecachedError::NotUtf8 { key }),
            None => Ok(None),
        }
    }

    /// JSON at `path` (`$` or `None` for the whole document), serialised.
    pub fn get_json(&self, key: String, path: Option<String>) -> Option<String> {
        match self.store.execute(Command::JGet(key, path)) {
            Value::BulkString(Some(bytes)) => String::from_utf8(bytes).ok(),
            _ => None,
        }
    }

    pub fn exists(&self, key: String) -> bool {
        matches!(self.store.execute(Command::Exists(vec![key])), Value::Integer(n) if n > 0)
    }

    /// Remaining time to live in seconds: `-1` without an expiry, `-2` when
    /// the key does not exist.
    pub fn ttl(&self, key: String) -> i64 {
        match self.store.execute(Command::Ttl(key)) {
            Value::Integer(n) => n,
            _ => -2,
        }
    }

    /// Every local key matching a glob pattern, sorted by key.
    pub fn get_matching(&self, pattern: String) -> Vec<Entry> {
        let mut found = self.store.matching_key_values(&pattern, usize::MAX);
        found.sort_by(|(a, _), (b, _)| a.cmp(b));
        found
            .into_iter()
            .map(|(key, value)| Entry {
                key,
                value: match value {
                    Value::BulkString(Some(bytes)) => Some(bytes),
                    _ => None,
                },
            })
            .collect()
    }

    // ── writes: local, durable, then synced ──────────────────────────────
    //
    // Each changes exactly the key it names, so the caller notifies that key's
    // observers itself; frames from the server report theirs in
    // `FrameOutcome::changed_keys`.

    pub fn set(&self, key: String, value: Vec<u8>) -> Result<(), RecachedError> {
        let frame = to_resp_bytes(&[b"SET", key.as_bytes(), &value]);
        self.write(Command::Set(key, value, SetOptions::default()), frame)?;
        Ok(())
    }

    /// Set a value that expires after `seconds`, which must be at least 1.
    pub fn set_ex(&self, key: String, value: Vec<u8>, seconds: u64) -> Result<(), RecachedError> {
        if seconds == 0 {
            // The server refuses `EX 0`, so the write could never sync; the
            // local store would take it and expire the key at once.
            return Err(RecachedError::Command {
                reason: "ERR invalid expire time in 'set' command".to_string(),
            });
        }
        let secs = seconds.to_string();
        let frame = to_resp_bytes(&[b"SET", key.as_bytes(), &value, b"EX", secs.as_bytes()]);
        let options = SetOptions {
            expiry: Some(SetExpiry::Ex(seconds)),
            ..Default::default()
        };
        self.write(Command::Set(key, value, options), frame)?;
        Ok(())
    }

    /// Delete a key. Returns whether it existed locally.
    pub fn del(&self, key: String) -> Result<bool, RecachedError> {
        let frame = to_resp(&["DEL", &key]);
        let reply = self.write(Command::Del(vec![key]), frame)?;
        Ok(matches!(reply, Value::Integer(n) if n > 0))
    }

    /// Add `delta` to an integer counter and return the new local value.
    ///
    /// Queued offline as a delta, so it merges with other clients' increments
    /// on reconnect instead of overwriting them.
    pub fn incr_by(&self, key: String, delta: i64) -> Result<i64, RecachedError> {
        let frame = to_resp(&["INCRBY", &key, &delta.to_string()]);
        match self.write(Command::IncrBy(key, delta), frame)? {
            Value::Integer(n) => Ok(n),
            other => Err(RecachedError::Command {
                reason: format!("unexpected INCRBY reply {other:?}"),
            }),
        }
    }

    /// Set JSON at `path` (`$` for the whole document). `json` must be valid
    /// JSON text.
    pub fn jset(&self, key: String, path: String, json: String) -> Result<(), RecachedError> {
        let frame = to_resp(&["JSET", &key, &path, &json]);
        self.write(Command::JSet(key, path, json), frame)?;
        Ok(())
    }

    /// Apply an RFC 7386 JSON Merge Patch to the whole document.
    pub fn jmerge(&self, key: String, patch: String) -> Result<(), RecachedError> {
        let frame = to_resp(&["JMERGE", &key, &patch]);
        self.write(Command::JMerge(key, patch), frame)?;
        Ok(())
    }

    /// Number of writes not yet acknowledged by the server.
    pub fn pending_writes(&self) -> u64 {
        self.lock().sync.outbox_len() as u64
    }

    // ── synchronization status ───────────────────────────────────────────

    /// Where synchronization stands. `Ready` needs more than an open socket:
    /// authentication, the replay of every queued write, and the snapshot of
    /// every pattern watched when the socket opened.
    pub fn sync_status(&self) -> SyncStatus {
        self.lock().sync.status().into()
    }

    /// The status, when it changed since this (or a
    /// [`FrameOutcome::status`]) last reported it. Call after
    /// `connection_opening`, `connection_opened`, `connection_closed`,
    /// `watch` and `unwatch` to push changes to observers without polling.
    pub fn take_status_change(&self) -> Option<SyncStatus> {
        self.lock().sync.take_status_change().map(Into::into)
    }

    /// One watched pattern's progress, or `None` when it is not watched.
    pub fn subscription_state(&self, pattern: String) -> Option<SubscriptionState> {
        self.lock()
            .sync
            .subscription_state(&pattern)
            .map(Into::into)
    }

    /// Patterns whose progress changed since the last call, each once.
    pub fn take_subscription_changes(&self) -> Vec<SubscriptionChange> {
        self.lock()
            .sync
            .take_subscription_changes()
            .into_iter()
            .map(|(pattern, state)| SubscriptionChange {
                pattern,
                state: state.map(Into::into),
            })
            .collect()
    }

    /// Diagnostic counters.
    pub fn sync_counters(&self) -> SyncCounters {
        self.lock().sync.counters().into()
    }

    // ── session ──────────────────────────────────────────────────────────

    /// Keep every key matching `pattern` in sync: the server sends their
    /// current state, then every change. Re-established on each reconnect,
    /// where the fresh state also removes keys deleted while offline.
    ///
    /// Not persisted: call it again after `open`.
    pub fn watch(&self, pattern: String) {
        let mut inner = self.lock();
        let connected = inner.sink.is_some();
        let frame = inner.sync.add_live_query(&pattern, connected);
        inner.send(frame);
    }

    /// Stop keeping `pattern` in sync. Local copies of its keys stay readable.
    pub fn unwatch(&self, pattern: String) {
        let mut inner = self.lock();
        let connected = inner.sink.is_some();
        let frame = inner.sync.remove_live_query(Some(&pattern), connected);
        inner.send(frame);
    }

    /// Replace the sync token, e.g. after the app refreshes it. Sent now when
    /// connected, and on every later reconnect.
    pub fn set_sync_token(&self, token: String) {
        let mut inner = self.lock();
        let connected = inner.sink.is_some();
        let frame = inner.sync.set_sync_token(&token, connected);
        inner.send(frame);
    }

    // ── transport ────────────────────────────────────────────────────────

    /// The app is about to open a socket. Optional: it only makes the status
    /// read `Connecting` until the socket opens or fails.
    pub fn connection_opening(&self) {
        self.lock().sync.on_connecting();
    }

    /// The socket opened. Sends the session (auth, sync scope, live queries)
    /// and then every queued write through `sink`, and uses `sink` for
    /// everything sent until [`connection_closed`](Self::connection_closed).
    pub fn connection_opened(&self, sink: Arc<dyn FrameSink>) {
        let mut inner = self.lock();
        for frame in inner.sync.on_open() {
            sink.send(frame);
        }
        inner.sink = Some(sink);
    }

    /// Handle one incoming frame: apply it, persist what it changed, and
    /// retire the queued write its reply acknowledges or refuses for good.
    ///
    /// An `Err` means the frame was applied in memory but could not be saved;
    /// the next snapshot from the server repairs the saved copy.
    pub fn frame_received(&self, frame: Vec<u8>) -> Result<FrameOutcome, RecachedError> {
        let mut inner = self.lock();
        let connected = inner.sink.is_some();
        let mut refused = None;
        let (changed_keys, retired, reconnect) = match inner.sync.handle_frame(&frame) {
            Incoming::Applied { keys } => (keys, None, false),
            Incoming::AppliedReply { retired, keys } => (keys, retired, false),
            Incoming::Reply { retired } => (Vec::new(), retired, false),
            Incoming::Refused {
                retired,
                keys,
                unreconciled,
                reason,
            } => {
                for frame in inner.sync.resubscribe_covering(&keys, connected) {
                    inner.send(Some(frame));
                }
                refused = Some(RefusedWrite {
                    keys,
                    unreconciled,
                    reason,
                });
                (Vec::new(), Some(retired), false)
            }
            // The write stays queued, on disk too; the reconnect replays it.
            Incoming::Deferred { .. } | Incoming::Malformed => (Vec::new(), None, true),
            Incoming::PubSub { .. } | Incoming::Ignored => (Vec::new(), None, false),
        };
        // Frames the client recorded reply slots for must go out whatever
        // happens to the commit below, or every later reply is mismatched.
        for frame in inner.sync.take_frames_to_send(connected) {
            inner.send(Some(frame));
        }
        let saved = if !changed_keys.is_empty() || retired.is_some() {
            inner
                .db
                .commit(&self.store, &changed_keys, None, retired.as_slice())
        } else {
            Ok(())
        };
        let status = inner.sync.take_status_change().map(Into::into);
        saved?;
        Ok(FrameOutcome {
            changed_keys,
            reconnect,
            refused,
            status,
        })
    }

    /// The socket closed. Returns the delay in milliseconds before the next
    /// connection attempt: exponential backoff with jitter, reset by the next
    /// successful open. When the network comes back or the app returns to the
    /// foreground, reconnecting at once instead is fine.
    pub fn connection_closed(&self) -> u32 {
        let mut inner = self.lock();
        inner.sink = None;
        inner.sync.on_close()
    }
}

impl RecachedClient {
    /// A panic in an earlier call poisons the lock. Carry on rather than
    /// failing every later call: the durable state is transactional, and the
    /// in-memory copy is re-hydrated by the next live-query snapshot.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Apply a write locally, commit it with its outbox row, and send it when
    /// the socket is open — all or nothing.
    ///
    /// The capacity check comes first, so a write the outbox cannot take
    /// never shows locally. The row is queued unsent, then committed together
    /// with the keys it changed; only a committed row goes out on the socket,
    /// so its reply can never retire a row that is not on disk. If the commit
    /// fails, the row is withdrawn and the keys restored: the caller gets an
    /// error, and an error means nothing happened.
    fn write(&self, cmd: Command, frame: Vec<u8>) -> Result<Value, RecachedError> {
        let mut inner = self.lock();
        inner.sync.check_capacity(true)?;
        let keys = mutation_keys(&cmd, &self.store);
        let before: Vec<SnapshotEntry> = keys
            .iter()
            .filter_map(|key| self.store.snapshot_key(key))
            .collect();
        let reply = self.store.execute(cmd);
        if let Value::Error(reason) = reply {
            return Err(RecachedError::Command { reason });
        }
        let queued = match inner.sync.try_enqueue_write(&frame, true, false) {
            Ok(queued) => queued,
            Err(e) => {
                self.restore_keys(&keys, before);
                return Err(e.into());
            }
        };
        if let Err(e) = inner.db.commit(
            &self.store,
            &keys,
            Some((queued.id, &queued.frame)),
            queued.dropped.as_slice(),
        ) {
            // Under `evict_oldest_when_full` the evicted row is gone from
            // memory but, the transaction having rolled back, still on disk:
            // it replays after the next restart rather than being lost.
            inner.sync.cancel_enqueued(queued.id);
            self.restore_keys(&keys, before);
            return Err(e);
        }
        let connected = inner.sink.is_some();
        if let Some(frame) = inner.sync.send_enqueued(queued.id, connected) {
            inner.send(Some(frame));
        }
        Ok(reply)
    }

    /// Put `keys` back as `before` held them, after a write that could not
    /// be kept.
    fn restore_keys(&self, keys: &[String], before: Vec<SnapshotEntry>) {
        if !keys.is_empty() {
            self.store.execute(Command::Del(keys.to_vec()));
        }
        if !before.is_empty() {
            self.store.restore(before);
        }
    }
}

impl Inner {
    fn send(&self, frame: Option<Vec<u8>>) {
        if let (Some(frame), Some(sink)) = (frame, &self.sink) {
            sink.send(frame);
        }
    }
}

#[cfg(test)]
mod tests;
