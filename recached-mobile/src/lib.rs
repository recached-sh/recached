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

use core_engine::cmd::{Command, SetExpiry, SetOptions};
use core_engine::resp::Value;
use core_engine::store::KeyValueStore;
use std::sync::{Arc, Mutex, MutexGuard};
use sync_client::{Incoming, SyncClient, mutation_keys, to_resp, to_resp_bytes};

mod error;
mod persist;

pub use error::RecachedError;
use persist::Db;

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
    /// Cap on queued, unacknowledged writes; past it the oldest is dropped.
    /// Defaults to 10,000.
    #[uniffi(default = None)]
    pub max_pending_writes: Option<u32>,
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
    /// The frame could not be parsed, so reply matching can no longer be
    /// trusted. Close the socket; the reconnect rebuilds it, and nothing queued
    /// is lost.
    pub reconnect: bool,
}

/// One key and its value, from [`RecachedClient::get_matching`].
#[derive(Debug, PartialEq, uniffi::Record)]
pub struct Entry {
    pub key: String,
    /// The bytes of a string value; `None` for a collection, which has no
    /// single-value form.
    pub value: Option<Vec<u8>>,
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
        let client_id = match db.meta_get("client_id")? {
            Some(id) => id,
            None => {
                let id = format!("{:032x}", rand::random::<u128>());
                db.meta_put("client_id", &id)?;
                id
            }
        };
        let mut sync = SyncClient::new(Arc::clone(&store), client_id);

        // A new epoch per open puts this session's write ids above every id
        // the last one sent. Durable before any write can use it.
        let epoch = db
            .meta_get("epoch")?
            .and_then(|e| e.parse::<u32>().ok())
            .unwrap_or(0)
            .saturating_add(1);
        db.meta_put("epoch", &epoch.to_string())?;
        sync.set_epoch(epoch);

        if let Some(max) = config.max_pending_writes {
            sync.set_max_pending(max as usize);
        }
        // Restored frames keep their original bytes, wire ids included, so a
        // write the server applied before the app died is recognised and
        // skipped on replay rather than applied twice.
        let restored = sync.restore_outbox(db.load_outbox()?);
        db.replace_outbox(&restored)?;

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

    /// Set a value that expires after `seconds`.
    pub fn set_ex(&self, key: String, value: Vec<u8>, seconds: u64) -> Result<(), RecachedError> {
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
                message: format!("unexpected INCRBY reply {other:?}"),
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
    /// retire the queued write its reply acknowledges.
    ///
    /// An `Err` means the frame was applied in memory but could not be saved;
    /// the next snapshot from the server repairs the saved copy.
    pub fn frame_received(&self, frame: Vec<u8>) -> Result<FrameOutcome, RecachedError> {
        let mut inner = self.lock();
        let (changed_keys, retired, reconnect) = match inner.sync.handle_frame(&frame) {
            Incoming::Applied { keys } => (keys, None, false),
            Incoming::AppliedReply { retired, keys } => (keys, retired, false),
            Incoming::Reply { retired } => (Vec::new(), retired, false),
            Incoming::Malformed => (Vec::new(), None, true),
            Incoming::PubSub { .. } | Incoming::Ignored => (Vec::new(), None, false),
        };
        if !changed_keys.is_empty() || retired.is_some() {
            inner
                .db
                .commit(&self.store, &changed_keys, None, retired.as_slice())?;
        }
        Ok(FrameOutcome {
            changed_keys,
            reconnect,
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
    /// the socket is open.
    fn write(&self, cmd: Command, frame: Vec<u8>) -> Result<Value, RecachedError> {
        let mut inner = self.lock();
        let keys = mutation_keys(&cmd, &self.store);
        let reply = self.store.execute(cmd);
        if let Value::Error(message) = reply {
            return Err(RecachedError::Command { message });
        }
        let connected = inner.sink.is_some();
        let queued = inner.sync.enqueue_write(&frame, true, connected);
        // Committed before the frame is sent, so its reply can never retire a
        // row that is not on disk yet.
        let saved = inner.db.commit(
            &self.store,
            &keys,
            Some((queued.id, &queued.frame)),
            queued.dropped.as_slice(),
        );
        // Sent even when saving failed: the client has recorded the frame as
        // in flight, and a reply to a frame that never went out would
        // acknowledge the wrong write.
        if queued.send_now {
            inner.send(Some(queued.frame));
        }
        saved.map(|()| reply)
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
