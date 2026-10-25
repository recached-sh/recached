use core_engine::cmd::{Command, SetOptions};
use core_engine::resp::Value;
use core_engine::store::{KeyValueStore, SnapshotEntry, SnapshotValue, format_score, glob_match};
use js_sys::Promise;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use sync_client::{
    EnqueueError, Enqueued, Incoming, OverflowPolicy, SubscriptionState, SyncClient, SyncError,
    SyncPhase, SyncStatus, is_replayable_mutation, mutation_keys, to_resp, to_resp_bytes,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{BroadcastChannel, Event, MessageEvent, WebSocket};

// ── IndexedDB JS glue ─────────────────────────────────────────────────────────
//
// Two object stores matter: "wal" (the local copy, as commands to replay) and
// "outbox" (writes awaiting a server acknowledgment, as wire frames).
//
// Keys. Every tab sharing the origin shares one database, so a key must not
// collide across tabs. Since 0.5:
//
//   wal    [t, tag, seq]  t: the writing tab's monotonic wall clock, so tabs
//                         interleave roughly in time; tag: the tab's session;
//                         seq: strictly increasing within the session
//   outbox [tag, id]      id: the session's outbox row id
//
// Rows written before 0.5 have numeric keys. IndexedDB orders numbers before
// arrays, so they replay first, as they should: they are older.
//
// Durability. Every write resolves on the *transaction's* `complete` event
// and rejects on `error` or `abort` — a quota failure arrives as an abort. A
// request's `success` is not a commit: the transaction can still abort after
// it, and before 0.5 a write was reported done at that point.

#[wasm_bindgen(inline_js = r#"
export function openRecachedDb() {
    return new Promise((resolve, reject) => {
        // v2 added 'outbox' (writes awaiting server acknowledgment);
        // v3 adds 'meta' (client identity + session epoch for replay deduplication).
        const req = indexedDB.open('recached', 3);
        req.onupgradeneeded = (e) => {
            const db = e.target.result;
            if (!db.objectStoreNames.contains('wal')) db.createObjectStore('wal');
            if (!db.objectStoreNames.contains('outbox')) db.createObjectStore('outbox');
            if (!db.objectStoreNames.contains('meta')) db.createObjectStore('meta');
        };
        req.onsuccess = (e) => resolve(e.target.result);
        req.onerror   = (e) => reject(e.target.error);
        req.onblocked = () => reject(new Error('IndexedDB upgrade blocked by another tab'));
    });
}
function readAll(db, name) {
    return new Promise((resolve, reject) => {
        const tx    = db.transaction(name, 'readonly');
        const store = tx.objectStore(name);
        let done = 0, keys, vals;
        const finish = () => { if (++done === 2) resolve([keys, vals]); };
        store.getAllKeys().onsuccess = (e) => { keys = e.target.result; finish(); };
        store.getAll().onsuccess    = (e) => { vals = e.target.result; finish(); };
        tx.onerror = (e) => reject(e.target.error);
        tx.onabort = (e) => reject(e.target.error || new Error('transaction aborted'));
    });
}
function committed(tx) {
    return new Promise((resolve, reject) => {
        tx.oncomplete = () => resolve(undefined);
        tx.onerror    = (e) => reject(e.target.error);
        tx.onabort    = (e) => reject(e.target.error || new Error('transaction aborted'));
    });
}
// Run `body(tx)` in a readwrite transaction over `names`, resolving when it
// commits. A synchronous throw — a closed database after the user cleared
// site data, an invalid key — becomes a rejection, and aborts the
// transaction, rather than an exception unwinding into wasm.
function write(db, names, body) {
    let tx, done;
    try {
        tx = db.transaction(names, 'readwrite');
        done = committed(tx);
        body(tx);
        return done;
    } catch (e) {
        // The abort rejects `done` too; the caller hears `e` instead.
        if (done) done.catch(() => {});
        try { if (tx) tx.abort(); } catch (_) {}
        return Promise.reject(e);
    }
}
export function idbReadAll(db) { return readAll(db, 'wal'); }
export function idbOutboxReadAll(db) { return readAll(db, 'outbox'); }
// One local write: its WAL record and its outbox row commit together or not
// at all, so a reload never shows a write that will not sync, nor syncs one
// it does not show. Either part may be absent (null).
export function idbWriteLocal(db, walKey, walCmd, outboxKey, outboxFrame) {
    const names = [];
    if (walKey !== null) names.push('wal');
    if (outboxKey !== null) names.push('outbox');
    if (names.length === 0) return Promise.resolve(undefined);
    return write(db, names, (tx) => {
        if (walKey !== null) tx.objectStore('wal').put(walCmd, walKey);
        if (outboxKey !== null) tx.objectStore('outbox').put(outboxFrame, outboxKey);
    });
}
export function idbWalAppendMany(db, keys, cmds) {
    return write(db, 'wal', (tx) => {
        const store = tx.objectStore('wal');
        for (let i = 0; i < keys.length; i++) store.put(cmds[i], keys[i]);
    });
}
export function idbOutboxDelete(db, key) {
    return write(db, 'outbox', (tx) => tx.objectStore('outbox').delete(key));
}
// Replace the rows this tab read with their renumbered copies, in one
// transaction. Only the keys read are deleted: clearing the store, as
// before 0.5, also deleted rows another open tab wrote meanwhile.
export function idbOutboxReplace(db, oldKeys, newKeys, cmds) {
    return write(db, 'outbox', (tx) => {
        const store = tx.objectStore('outbox');
        for (let i = 0; i < oldKeys.length; i++) store.delete(oldKeys[i]);
        for (let i = 0; i < newKeys.length; i++) store.put(cmds[i], newKeys[i]);
    });
}
// Compaction: the records read are replaced by a snapshot, in one
// transaction, so an interruption leaves the old log intact. Records another
// tab appended after the read are kept.
export function idbWalReplace(db, oldKeys, newKeys, cmds) {
    return write(db, 'wal', (tx) => {
        const store = tx.objectStore('wal');
        for (let i = 0; i < oldKeys.length; i++) store.delete(oldKeys[i]);
        for (let i = 0; i < newKeys.length; i++) store.put(cmds[i], newKeys[i]);
    });
}
export function idbWalClear(db) {
    return write(db, 'wal', (tx) => tx.objectStore('wal').clear());
}
export function idbClear(db) {
    return write(db, ['wal', 'outbox'], (tx) => {
        tx.objectStore('wal').clear();
        tx.objectStore('outbox').clear();
    });
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = "openRecachedDb")]
    fn open_recached_db() -> Promise;
    #[wasm_bindgen(js_name = "idbReadAll")]
    fn idb_read_all(db: &JsValue) -> Promise;
    #[wasm_bindgen(js_name = "idbOutboxReadAll")]
    fn idb_outbox_read_all(db: &JsValue) -> Promise;
    #[wasm_bindgen(js_name = "idbWriteLocal")]
    fn idb_write_local_js(
        db: &JsValue,
        wal_key: &JsValue,
        wal_cmd: &JsValue,
        outbox_key: &JsValue,
        outbox_frame: &JsValue,
    ) -> Promise;
    #[wasm_bindgen(js_name = "idbWalAppendMany")]
    fn idb_wal_append_many_js(db: &JsValue, keys: &JsValue, cmds: &JsValue) -> Promise;
    #[wasm_bindgen(js_name = "idbOutboxDelete")]
    fn idb_outbox_delete_js(db: &JsValue, key: &JsValue) -> Promise;
    #[wasm_bindgen(js_name = "idbOutboxReplace")]
    fn idb_outbox_replace_js(
        db: &JsValue,
        old_keys: &JsValue,
        new_keys: &JsValue,
        cmds: &JsValue,
    ) -> Promise;
    #[wasm_bindgen(js_name = "idbWalClear")]
    fn idb_wal_clear_js(db: &JsValue) -> Promise;
    #[wasm_bindgen(js_name = "idbWalReplace")]
    fn idb_wal_replace_js(
        db: &JsValue,
        old_keys: &JsValue,
        new_keys: &JsValue,
        cmds: &JsValue,
    ) -> Promise;
    #[wasm_bindgen(js_name = "idbClear")]
    fn idb_clear_js(db: &JsValue) -> Promise;
}

// ── RESP helper ───────────────────────────────────────────────────────────────

/// Build a RESP array frame from owned byte arguments.
///
/// Bytes rather than `String`: WAL compaction re-encodes stored values, and a
/// value pushed from the server may be arbitrary binary. Going through a
/// `String` here would corrupt exactly the values the store holds correctly.
fn to_resp_owned(parts: &[Vec<u8>]) -> Vec<u8> {
    let mut out = format!("*{}\r\n", parts.len()).into_bytes();
    for part in parts {
        out.extend_from_slice(format!("${}\r\n", part.len()).as_bytes());
        out.extend_from_slice(part);
        out.extend_from_slice(b"\r\n");
    }
    out
}

// ── WAL compaction ────────────────────────────────────────────────────────────

/// Compact after this many replayed WAL entries on load — rewrite as minimal
/// snapshot commands so the next replay is fast regardless of write history.
const WAL_COMPACT_THRESHOLD: u32 = 1000;

/// How long server-applied changes gather before their keys are written to
/// the WAL in one transaction.
const SYNCED_STATE_FLUSH_MS: i32 = 250;

/// Milliseconds since the Unix epoch, from whichever clock the target has.
#[cfg(target_arch = "wasm32")]
fn wall_clock_ms() -> u64 {
    js_sys::Date::now() as u64
}

#[cfg(not(target_arch = "wasm32"))]
fn wall_clock_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Convert snapshot entries into minimal RESP command strings suitable for
/// storing in the WAL. Each entry produces one command; entries with a TTL on
/// collection types produce an extra PEXPIREAT command.
///
/// Expiries are written as absolute deadlines (`PXAT`, `PEXPIREAT`). A
/// relative `PX`, as before 0.5, restarted the clock on every reload: a key
/// with a minute left, reloaded an hour later, lived another minute.
fn snapshot_to_resp_cmds(entries: &[SnapshotEntry]) -> Vec<Vec<u8>> {
    // `SystemTime::now()` is unsupported on wasm32-unknown-unknown and panics
    // outright. This runs inside WAL compaction, immediately after the WAL has
    // been cleared — a panic here therefore destroys the entire persisted cache
    // rather than merely failing. Use the platform clock the rest of this crate
    // already uses in the browser.
    let now_ms = wall_clock_ms();
    let mut out = Vec::new();
    for e in entries {
        if e.expires_at_ms.is_some_and(|exp| now_ms >= exp) {
            continue;
        }
        // Identifiers are text and values are bytes, so every part is built as
        // bytes and the text ones are converted on the way in.
        let t = |x: &str| x.as_bytes().to_vec();
        let data_parts: Vec<Vec<u8>> = match &e.value {
            SnapshotValue::Str(s) => {
                if let Some(exp) = e.expires_at_ms {
                    vec![
                        t("SET"),
                        t(&e.key),
                        s.as_slice().to_vec(),
                        t("PXAT"),
                        t(&exp.to_string()),
                    ]
                } else {
                    vec![t("SET"), t(&e.key), s.as_slice().to_vec()]
                }
            }
            SnapshotValue::Hash(map) => {
                if map.is_empty() {
                    continue;
                }
                let mut parts = vec![t("HSET"), t(&e.key)];
                for (f, v) in map {
                    parts.push(t(f));
                    parts.push(v.as_slice().to_vec());
                }
                parts
            }
            SnapshotValue::List(list) => {
                if list.is_empty() {
                    continue;
                }
                let mut parts = vec![t("RPUSH"), t(&e.key)];
                parts.extend(list.iter().map(|v| v.as_slice().to_vec()));
                parts
            }
            SnapshotValue::Set(set) => {
                if set.is_empty() {
                    continue;
                }
                let mut parts = vec![t("SADD"), t(&e.key)];
                parts.extend(set.iter().map(|m| t(m)));
                parts
            }
            SnapshotValue::ZSet(pairs) => {
                if pairs.is_empty() {
                    continue;
                }
                let mut parts = vec![t("ZADD"), t(&e.key)];
                for (member, score) in pairs {
                    parts.push(t(&format_score(*score)));
                    parts.push(t(member));
                }
                parts
            }
            // Rate-limiter attempt state is transient and server-side; it is
            // not persisted in the browser WAL.
            SnapshotValue::RateLimiter { .. } => continue,
            SnapshotValue::Json(doc) => {
                vec![t("JSET"), t(&e.key), t("$"), t(doc)]
            }
        };
        out.push(to_resp_owned(&data_parts));
        // Non-string types with a TTL need a separate PEXPIREAT command.
        if !matches!(&e.value, SnapshotValue::Str(_))
            && let Some(exp) = e.expires_at_ms
        {
            out.push(to_resp_owned(&[
                t("PEXPIREAT"),
                t(&e.key),
                t(&exp.to_string()),
            ]));
        }
    }
    out
}

/// WAL records that set `keys` to exactly their state in `store`: a `DEL`,
/// then the commands that rebuild the key, if it still exists. Replayed after
/// everything before them, they leave the key as it is now — which is what a
/// server-applied change must persist, since applying the change itself
/// again (an `HSET` onto whatever came before) would not.
fn key_state_cmds(store: &KeyValueStore, keys: &[String]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for key in keys {
        out.push(to_resp(&["DEL", key]));
        if let Some(entry) = store.snapshot_key(key) {
            out.extend(snapshot_to_resp_cmds(std::slice::from_ref(&entry)));
        }
    }
    out
}

/// Unguessable client id: `crypto.randomUUID()` when available (unguessability
/// is what stops another authenticated client from poisoning this client's
/// dedup high-water mark), with a Math.random+timestamp fallback.
fn random_client_id() -> String {
    if let Some(w) = web_sys::window()
        && let Ok(c) = js_sys::Reflect::get(&w, &JsValue::from_str("crypto"))
        && !c.is_undefined()
        && let Ok(f) = js_sys::Reflect::get(&c, &JsValue::from_str("randomUUID"))
        && f.is_function()
        && let Ok(v) = js_sys::Function::from(f).call0(&c)
        && let Some(s) = v.as_string()
    {
        return s;
    }
    format!(
        "{:08x}{:08x}{:08x}-{:x}",
        (js_sys::Math::random() * u32::MAX as f64) as u32,
        (js_sys::Math::random() * u32::MAX as f64) as u32,
        (js_sys::Math::random() * u32::MAX as f64) as u32,
        js_sys::Date::now() as u64,
    )
}

// ── WebSocket connection state ────────────────────────────────────────────────
//
// The sync logic itself — outbox, dedup envelopes, reply/ack correlation,
// session re-establishment, backoff — lives in the platform-neutral
// `sync-client` crate (shared with the mobile bindings). This layer only
// adapts it to the browser: WebSocket I/O, IndexedDB persistence, setTimeout
// reconnect timers, and JS callbacks.

/// Event closures for the *current* socket. Replaced wholesale on reconnect;
/// dropping the old ones detaches them.
#[derive(Default)]
struct WsHandlers {
    onmessage: Option<Closure<dyn FnMut(MessageEvent)>>,
    onopen: Option<Closure<dyn FnMut(Event)>>,
    onclose: Option<Closure<dyn FnMut(Event)>>,
}

/// Callbacks the SDK registers. Each is called with no borrow held.
#[derive(Default)]
struct Callbacks {
    mutation: RefCell<Option<js_sys::Function>>,
    message: RefCell<Option<js_sys::Function>>,
    /// Only under the legacy eviction policy: a queued write was dropped.
    outbox_full: RefCell<Option<js_sys::Function>>,
    status: RefCell<Option<js_sys::Function>>,
    subscription: RefCell<Option<js_sys::Function>>,
    refused: RefCell<Option<js_sys::Function>>,
    persistence_error: RefCell<Option<js_sys::Function>>,
}

/// IndexedDB writes in flight, so their failures reach the application and
/// [`RecachedCache::flush`] can wait for them.
#[derive(Default)]
struct Durability {
    next: Cell<u64>,
    pending: RefCell<BTreeMap<u64, Promise>>,
    /// The first failure since the last `flush`, which `flush` reports.
    unreported: RefCell<Option<JsValue>>,
    failures: Cell<u64>,
}

/// Everything the reconnect machinery needs, clonable into JS closures.
/// The socket itself lives behind `Rc<RefCell<…>>` so a reconnect fired from
/// an event handler can replace it.
#[derive(Clone)]
struct WsShared {
    /// The platform-neutral sync state machine.
    core: Rc<RefCell<SyncClient>>,
    store: Arc<KeyValueStore>,
    callbacks: Rc<Callbacks>,
    ws: Rc<RefCell<Option<WebSocket>>>,
    handlers: Rc<RefCell<WsHandlers>>,
    /// Shared with `RecachedCache.idb` — the open IndexedDB handle, if any.
    idb: Rc<RefCell<Option<JsValue>>>,
    /// This session's tag in IndexedDB keys: its client id.
    tag: Rc<String>,
    /// Next WAL sequence number within this session.
    seq: Rc<Cell<u64>>,
    /// The session's monotonic clock for WAL keys.
    wal_clock: Rc<Cell<f64>>,
    durability: Rc<Durability>,
    /// Write server-applied changes to the WAL, so a reload shows the last
    /// synchronized state. On by default.
    persist_synced: Rc<Cell<bool>>,
    /// Keys a server frame changed, waiting to be written to the WAL.
    synced_dirty: Rc<RefCell<HashSet<String>>>,
    synced_flush: TimerCallback,
    /// When the status last became `Ready`, in ms since the epoch.
    last_synced_at: Rc<Cell<Option<f64>>>,
    url: Rc<RefCell<Option<String>>>,
    auto_reconnect: Rc<Cell<bool>>,
    /// Socket generation: bumped by `open_socket`. Stale sockets' close events
    /// (e.g. one replaced by an explicit `connect`) must not schedule
    /// reconnects over the live socket.
    generation: Rc<Cell<u64>>,
    /// Keeps the pending reconnect timer's callback alive until it fires.
    reconnect_cb: TimerCallback,
}

/// Handle to a pending timer's callback, kept alive until it fires.
type TimerCallback = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

/// Call a registered callback with no borrow held across the call, so a
/// listener may call back into the cache.
fn call(cb: &RefCell<Option<js_sys::Function>>, args: &[JsValue]) {
    let f = cb.borrow().clone();
    if let Some(f) = f {
        let arr: js_sys::Array = args.iter().collect();
        let _ = f.apply(&JsValue::NULL, &arr);
    }
}

fn set_prop(obj: &js_sys::Object, key: &str, value: &JsValue) {
    let _ = js_sys::Reflect::set(obj, &JsValue::from_str(key), value);
}

/// The error a write throws when the outbox cannot take it. `name` is
/// `OutboxFullError`, with `pending` and `capacity`, so the SDK can rethrow
/// it as its own class.
fn enqueue_error(e: EnqueueError) -> JsValue {
    let err = js_sys::Error::new(&e.to_string());
    match e {
        EnqueueError::OutboxFull { pending, capacity } => {
            err.set_name("OutboxFullError");
            set_prop(&err, "pending", &JsValue::from_f64(pending as f64));
            set_prop(&err, "capacity", &JsValue::from_f64(capacity as f64));
        }
        EnqueueError::WireIdsExhausted => err.set_name("WriteIdsExhaustedError"),
    }
    err.into()
}

fn phase_name(phase: SyncPhase) -> &'static str {
    match phase {
        SyncPhase::Disconnected => "disconnected",
        SyncPhase::Connecting => "connecting",
        SyncPhase::Authenticating => "authenticating",
        SyncPhase::Replaying => "replaying",
        SyncPhase::Hydrating => "hydrating",
        SyncPhase::Ready => "ready",
        SyncPhase::Degraded => "degraded",
    }
}

fn failure_name(failure: sync_client::SubscriptionFailure) -> &'static str {
    match failure {
        sync_client::SubscriptionFailure::SnapshotTooLarge => "snapshot-too-large",
        sync_client::SubscriptionFailure::OutOfScope => "out-of-scope",
        sync_client::SubscriptionFailure::Other => "other",
    }
}

fn sync_error_js(e: &SyncError) -> JsValue {
    let obj = js_sys::Object::new();
    let (kind, reason) = match e {
        SyncError::AuthFailed { reason } => ("auth-failed", Some(reason)),
        SyncError::ScopeRefused { reason } => ("scope-refused", Some(reason)),
        SyncError::WriteDeferred { reason } => ("write-deferred", Some(reason)),
        SyncError::SubscriptionFailed {
            pattern,
            failure,
            reason,
        } => {
            set_prop(&obj, "pattern", &JsValue::from_str(pattern));
            set_prop(&obj, "failure", &JsValue::from_str(failure_name(*failure)));
            ("subscription-failed", Some(reason))
        }
        SyncError::MalformedFrame => ("malformed-frame", None),
    };
    set_prop(&obj, "kind", &JsValue::from_str(kind));
    if let Some(reason) = reason {
        set_prop(&obj, "reason", &JsValue::from_str(reason));
    }
    obj.into()
}

fn subscription_js(state: &SubscriptionState) -> JsValue {
    let obj = js_sys::Object::new();
    let state_name = match state {
        SubscriptionState::Pending => "pending",
        SubscriptionState::Hydrating => "hydrating",
        SubscriptionState::Current => "current",
        SubscriptionState::Failed { failure, reason } => {
            set_prop(&obj, "failure", &JsValue::from_str(failure_name(*failure)));
            set_prop(&obj, "reason", &JsValue::from_str(reason));
            "failed"
        }
    };
    set_prop(&obj, "state", &JsValue::from_str(state_name));
    obj.into()
}

/// The status object the SDK receives: `{ phase, error, pendingWrites,
/// lastSyncedAt, persistenceFailures }`.
fn status_js(sh: &WsShared, status: &SyncStatus) -> JsValue {
    let obj = js_sys::Object::new();
    set_prop(&obj, "phase", &JsValue::from_str(phase_name(status.phase)));
    set_prop(
        &obj,
        "error",
        &status.error.as_ref().map_or(JsValue::NULL, sync_error_js),
    );
    set_prop(
        &obj,
        "pendingWrites",
        &JsValue::from_f64(sh.core.borrow().outbox_len() as f64),
    );
    set_prop(
        &obj,
        "lastSyncedAt",
        &sh.last_synced_at
            .get()
            .map_or(JsValue::NULL, JsValue::from_f64),
    );
    set_prop(
        &obj,
        "persistenceFailures",
        &JsValue::from_f64(sh.durability.failures.get() as f64),
    );
    obj.into()
}

/// Tell the SDK what the last event did to the status and the live queries.
/// Called after every event that can move them, so listeners are pushed to,
/// never polled.
fn emit_status(sh: &WsShared) {
    let (status, subs) = {
        let mut core = sh.core.borrow_mut();
        (core.take_status_change(), core.take_subscription_changes())
    };
    if let Some(status) = status {
        // "Last synchronized" is the moment synchronization *completed* —
        // never merely the moment a socket opened.
        if status.phase == SyncPhase::Ready {
            sh.last_synced_at.set(Some(js_sys::Date::now()));
        }
        let js = status_js(sh, &status);
        call(&sh.callbacks.status, &[js]);
    }
    for (pattern, state) in subs {
        let js = state.as_ref().map_or(JsValue::NULL, subscription_js);
        call(
            &sh.callbacks.subscription,
            &[JsValue::from_str(&pattern), js],
        );
    }
}

// ── durable writes ────────────────────────────────────────────────────────────

/// Follow an IndexedDB write: report a failure to the application, and keep
/// the promise for `flush` until it settles.
fn track(sh: &WsShared, op: &'static str, promise: Promise) {
    let d = Rc::clone(&sh.durability);
    let id = d.next.get();
    d.next.set(id + 1);
    d.pending.borrow_mut().insert(id, promise.clone());
    let callbacks = Rc::clone(&sh.callbacks);
    spawn_local(async move {
        let result = JsFuture::from(promise).await;
        d.pending.borrow_mut().remove(&id);
        if let Err(error) = result {
            d.failures.set(d.failures.get() + 1);
            if d.unreported.borrow().is_none() {
                *d.unreported.borrow_mut() = Some(error.clone());
            }
            let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
                .ok()
                .and_then(|m| m.as_string())
                .or_else(|| error.as_string())
                .unwrap_or_else(|| "IndexedDB write failed".to_string());
            web_sys::console::error_1(&JsValue::from_str(&format!(
                "recached: IndexedDB {op} failed: {message}"
            )));
            call(
                &callbacks.persistence_error,
                &[JsValue::from_str(op), JsValue::from_str(&message)],
            );
        }
    });
}

/// The next WAL key for this session: `[t, tag, seq]`.
fn next_wal_key(sh: &WsShared) -> JsValue {
    let t = js_sys::Date::now().max(sh.wal_clock.get());
    sh.wal_clock.set(t);
    let seq = sh.seq.get();
    sh.seq.set(seq + 1);
    let key = js_sys::Array::new();
    key.push(&JsValue::from_f64(t));
    key.push(&JsValue::from_str(&sh.tag));
    key.push(&JsValue::from_f64(seq as f64));
    key.into()
}

fn outbox_key(sh: &WsShared, id: u64) -> JsValue {
    let key = js_sys::Array::new();
    key.push(&JsValue::from_str(&sh.tag));
    key.push(&JsValue::from_f64(id as f64));
    key.into()
}

/// Persist one local write: its WAL record and its outbox row, together.
fn persist_local_write(sh: &WsShared, wal_cmd: &[u8], outbox: Option<(u64, &[u8])>) {
    let Some(db) = sh.idb.borrow().clone() else {
        return;
    };
    let wal_key = next_wal_key(sh);
    let wal_cmd: JsValue = js_sys::Uint8Array::from(wal_cmd).into();
    let (ob_key, ob_frame) = match outbox {
        Some((id, frame)) => (
            outbox_key(sh, id),
            JsValue::from(js_sys::Uint8Array::from(frame)),
        ),
        None => (JsValue::NULL, JsValue::NULL),
    };
    let promise = idb_write_local_js(&db, &wal_key, &wal_cmd, &ob_key, &ob_frame);
    track(sh, "write", promise);
}

/// Persist only an outbox row (pub/sub commands, which change no local state).
fn persist_outbox_row(sh: &WsShared, id: u64, frame: &[u8]) {
    let Some(db) = sh.idb.borrow().clone() else {
        return;
    };
    let frame: JsValue = js_sys::Uint8Array::from(frame).into();
    let promise = idb_write_local_js(
        &db,
        &JsValue::NULL,
        &JsValue::NULL,
        &outbox_key(sh, id),
        &frame,
    );
    track(sh, "outbox write", promise);
}

/// Delete an acknowledged (or evicted) outbox row from durable storage. A
/// failure leaves the row to replay after a reload, where the server answers
/// it as a duplicate — reported, but not a loss.
fn outbox_delete(sh: &WsShared, id: u64) {
    if let Some(db) = sh.idb.borrow().clone() {
        let promise = idb_outbox_delete_js(&db, &outbox_key(sh, id));
        track(sh, "outbox delete", promise);
    }
}

/// Note keys a server frame changed, to be written to the WAL shortly.
fn mark_synced(sh: &WsShared, keys: &[String]) {
    if keys.is_empty() || !sh.persist_synced.get() || sh.idb.borrow().is_none() {
        return;
    }
    sh.synced_dirty.borrow_mut().extend(keys.iter().cloned());
    if sh.synced_flush.borrow().is_some() {
        return;
    }
    match web_sys::window() {
        Some(window) => {
            let sh2 = sh.clone();
            let cb = Closure::wrap(Box::new(move || {
                sh2.synced_flush.borrow_mut().take();
                flush_synced(&sh2);
            }) as Box<dyn FnMut()>);
            let scheduled = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    SYNCED_STATE_FLUSH_MS,
                )
                .is_ok();
            if scheduled {
                *sh.synced_flush.borrow_mut() = Some(cb);
            } else {
                flush_synced(sh);
            }
        }
        None => flush_synced(sh),
    }
}

/// Write the current state of every key a server frame changed since the
/// last flush, in one transaction.
fn flush_synced(sh: &WsShared) {
    let keys: Vec<String> = sh.synced_dirty.borrow_mut().drain().collect();
    let Some(db) = sh.idb.borrow().clone() else {
        return;
    };
    if keys.is_empty() {
        return;
    }
    let cmds = key_state_cmds(&sh.store, &keys);
    let js_keys = js_sys::Array::new();
    let js_cmds = js_sys::Array::new();
    for cmd in &cmds {
        js_keys.push(&next_wal_key(sh));
        js_cmds.push(&js_sys::Uint8Array::from(cmd.as_slice()));
    }
    track(
        sh,
        "synced-state write",
        idb_wal_append_many_js(&db, &js_keys, &js_cmds),
    );
}

// ── incoming frames ───────────────────────────────────────────────────────────

/// Execute the effects the core requested for an incoming frame.
fn dispatch_incoming(sh: &WsShared, incoming: Incoming) {
    match incoming {
        Incoming::Applied { keys } => {
            mark_synced(sh, &keys);
            notify_mutation(&sh.callbacks, &keys);
        }
        Incoming::PubSub { channel, message } => {
            let payload = match std::str::from_utf8(&message) {
                Ok(text) => JsValue::from_str(text),
                // A binary publish cannot be a JS string; the listener gets
                // the bytes rather than a lossy rendering of them.
                Err(_) => js_sys::Uint8Array::from(&message[..]).into(),
            };
            call(
                &sh.callbacks.message,
                &[JsValue::from_str(&channel), payload],
            );
        }
        Incoming::Reply { retired } => {
            if let Some(id) = retired {
                outbox_delete(sh, id);
            }
        }
        Incoming::AppliedReply { retired, keys } => {
            if let Some(id) = retired {
                outbox_delete(sh, id);
            }
            mark_synced(sh, &keys);
            notify_mutation(&sh.callbacks, &keys);
        }
        // Replaying a write the server refused for good would be refused
        // again. Its local effect stays; re-subscribing the live queries that
        // cover its keys puts back the server's values, and the application
        // hears about the rest.
        Incoming::Refused {
            retired,
            keys,
            unreconciled,
            reason,
        } => {
            outbox_delete(sh, retired);
            web_sys::console::warn_1(&JsValue::from_str(&format!(
                "recached: the server refused a queued write to {keys:?}: {reason}"
            )));
            if socket_is_open(sh) {
                let frames = sh.core.borrow_mut().resubscribe_covering(&keys, true);
                if let Some(ws) = sh.ws.borrow().as_ref() {
                    for frame in frames {
                        ws_send_frame(ws, &frame);
                    }
                }
            }
            let to_js = |ks: &[String]| -> JsValue {
                ks.iter()
                    .map(|k| JsValue::from_str(k))
                    .collect::<js_sys::Array>()
                    .into()
            };
            call(
                &sh.callbacks.refused,
                &[
                    to_js(&keys),
                    to_js(&unreconciled),
                    JsValue::from_str(&reason),
                ],
            );
        }
        // Refused for a reason that can clear, such as a sync token not yet
        // accepted: the write stays queued, and the reconnect replays it.
        Incoming::Deferred { reason } => {
            web_sys::console::warn_1(&JsValue::from_str(&format!(
                "recached: the server deferred a queued write ({reason}); reconnecting to replay it"
            )));
            if let Some(ws) = sh.ws.borrow().as_ref() {
                let _ = ws.close();
            }
        }
        Incoming::Ignored => {}
        // A frame we could not parse may have been a reply the server already
        // counted, which would leave the inflight FIFO permanently one ahead —
        // every later reply then retires the wrong outbox row. There is no way
        // to resynchronise in place, so drop the socket and let the reconnect
        // rebuild the FIFO from `on_open`, which clears it. The outbox is
        // durable, so nothing queued is lost by doing this.
        Incoming::Malformed => {
            if let Some(ws) = sh.ws.borrow().as_ref() {
                let _ = ws.close();
            }
        }
    }
    // Frames the client queued itself — re-subscriptions for keys a snapshot
    // protected — each hold a reply slot and must go out now.
    let open = socket_is_open(sh);
    let extra = sh.core.borrow_mut().take_frames_to_send(open);
    if let Some(ws) = sh.ws.borrow().as_ref() {
        for frame in extra {
            ws_send_frame(ws, &frame);
        }
    }
}

fn socket_is_open(sh: &WsShared) -> bool {
    matches!(
        sh.ws.borrow().as_ref(),
        Some(ws) if ws.ready_state() == WebSocket::OPEN
    )
}

/// Whether writes go through the outbox at all: not in local-only mode (no
/// `connect()` and no persistence), where there is nothing to sync to.
fn syncing(sh: &WsShared) -> bool {
    sh.url.borrow().is_some() || sh.idb.borrow().is_some()
}

/// Refuse a write the outbox cannot take, before anything changes.
fn preflight(sh: &WsShared, dedup: bool) -> Result<(), JsValue> {
    if !syncing(sh) {
        return Ok(());
    }
    sh.core
        .borrow_mut()
        .check_capacity(dedup)
        .map_err(enqueue_error)
}

/// Queue a write through the core and mirror its effects: possible legacy
/// eviction, and an immediate send when connected. Returns the queued row for
/// persistence, or `None` in local-only mode.
fn queue_write(sh: &WsShared, encoded: &[u8], dedup: bool) -> Result<Option<Enqueued>, JsValue> {
    if !syncing(sh) {
        return Ok(None);
    }
    let connected = socket_is_open(sh);
    let e = sh
        .core
        .borrow_mut()
        .try_enqueue_write(encoded, dedup, connected)
        .map_err(enqueue_error)?;
    if let Some(old) = e.dropped {
        outbox_delete(sh, old);
        web_sys::console::warn_1(&JsValue::from_str(
            "recached: offline write queue full — dropped the oldest queued write",
        ));
        let pending = sh.core.borrow().outbox_len();
        call(
            &sh.callbacks.outbox_full,
            &[
                JsValue::from_f64(old as f64),
                JsValue::from_f64(pending as f64),
            ],
        );
    }
    if e.send_now
        && let Some(ws) = sh.ws.borrow().as_ref()
    {
        ws_send_frame(ws, &e.frame);
    }
    Ok(Some(e))
}

/// Extract the RESP bytes from a message event, whether the peer sent a text
/// frame or a binary one.
fn event_frame_bytes(e: &MessageEvent) -> Option<Vec<u8>> {
    let data = e.data();
    if let Some(text) = data.as_string() {
        return Some(text.into_bytes());
    }
    if let Ok(buf) = data.clone().dyn_into::<js_sys::ArrayBuffer>() {
        return Some(js_sys::Uint8Array::new(&buf).to_vec());
    }
    if let Ok(arr) = data.dyn_into::<js_sys::Uint8Array>() {
        return Some(arr.to_vec());
    }
    None
}

/// Post a RESP frame to other tabs, as a string when the bytes are text and a
/// `Uint8Array` when they are not.
fn bc_post(bc: &BroadcastChannel, frame: &[u8]) {
    let msg = match std::str::from_utf8(frame) {
        Ok(text) => JsValue::from_str(text),
        Err(_) => js_sys::Uint8Array::from(frame).into(),
    };
    let _ = bc.post_message(&msg);
}

/// Send one RESP frame over the socket.
///
/// Text frames must be well-formed UTF-8 per the WebSocket spec, so a frame
/// carrying a binary value goes out as a binary frame — which the server
/// accepts from 0.2.2. Everything else stays text, so nothing changes for the
/// overwhelming majority of traffic.
fn ws_send_frame(ws: &WebSocket, frame: &[u8]) {
    match std::str::from_utf8(frame) {
        Ok(text) => {
            let _ = ws.send_with_str(text);
        }
        Err(_) => {
            let _ = ws.send_with_u8_array(frame);
        }
    }
}

/// Open a socket to the stored URL and wire its event handlers. Called on
/// `connect()` and again by the backoff timer after a drop.
fn open_socket(sh: &WsShared) {
    let Some(url) = sh.url.borrow().clone() else {
        return;
    };
    let generation = sh.generation.get() + 1;
    sh.generation.set(generation);
    sh.core.borrow_mut().on_connecting();
    emit_status(sh);

    let ws = match WebSocket::new(&url) {
        Ok(w) => w,
        Err(_) => {
            let delay = sh.core.borrow_mut().on_close();
            emit_status(sh);
            schedule_reconnect(sh, delay);
            return;
        }
    };

    // ── onmessage: classify + apply via the core, execute its effects ────
    //
    // ArrayBuffer rather than the default Blob: the server sends a *binary*
    // frame whenever a reply carries a value that is not valid UTF-8, and a
    // Blob would have to be read asynchronously — the handler below would drop
    // it on the floor.
    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
    let sh_msg = sh.clone();
    let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| {
        let Some(raw) = event_frame_bytes(&e) else {
            return;
        };
        let incoming = sh_msg.core.borrow_mut().handle_frame(&raw);
        dispatch_incoming(&sh_msg, incoming);
        emit_status(&sh_msg);
    }) as Box<dyn FnMut(MessageEvent)>);
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

    // ── onopen: the core decides what to send (session, then outbox) ─────
    let sh_open = sh.clone();
    let ws_for_open = ws.clone();
    let onopen = Closure::wrap(Box::new(move |_e: Event| {
        let frames = sh_open.core.borrow_mut().on_open();
        for frame in frames {
            ws_send_frame(&ws_for_open, &frame);
        }
        emit_status(&sh_open);
    }) as Box<dyn FnMut(Event)>);
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));

    // ── onclose: always tell the core; reconnect only when enabled ───────
    let sh_close = sh.clone();
    let onclose = Closure::wrap(Box::new(move |_e: Event| {
        if sh_close.generation.get() == generation {
            // Without auto-reconnect the status must still leave `Ready`:
            // an open-then-closed socket is not synchronized.
            let delay = sh_close.core.borrow_mut().on_close();
            emit_status(&sh_close);
            schedule_reconnect(&sh_close, delay);
        }
    }) as Box<dyn FnMut(Event)>);
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));

    let mut h = sh.handlers.borrow_mut();
    h.onmessage = Some(onmessage);
    h.onopen = Some(onopen);
    h.onclose = Some(onclose);
    *sh.ws.borrow_mut() = Some(ws);
}

/// Schedule an `open_socket` retry after `delay` ms. No-op when auto-reconnect
/// is off or there is no window (non-browser contexts).
fn schedule_reconnect(sh: &WsShared, delay: u32) {
    if !sh.auto_reconnect.get() {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let sh2 = sh.clone();
    let cb = Closure::wrap(Box::new(move || {
        if sh2.auto_reconnect.get() {
            open_socket(&sh2);
        }
    }) as Box<dyn FnMut()>);
    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        cb.as_ref().unchecked_ref(),
        delay as i32,
    );
    *sh.reconnect_cb.borrow_mut() = Some(cb);
}

// ── RecachedCache ─────────────────────────────────────────────────────────────

#[wasm_bindgen]
pub struct RecachedCache {
    store: Arc<KeyValueStore>,
    bc: Option<BroadcastChannel>,
    /// Handle to the open IndexedDB; None until enable_persistence() resolves.
    idb: Rc<RefCell<Option<JsValue>>>,
    callbacks: Rc<Callbacks>,
    _onbc: Option<Closure<dyn FnMut(MessageEvent)>>,
    /// Shared WebSocket connection state (socket, session, offline queue).
    shared: WsShared,
    /// True when connected via unencrypted ws:// (not wss://).
    ws_is_plaintext: bool,
}

impl Default for RecachedCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Tell the SDK which keys just changed. It routes them to per-key and
/// per-pattern listeners, so a hook reading one key re-reads only when that
/// key changes, not on every write anywhere in the store.
fn notify_mutation(callbacks: &Callbacks, keys: &[String]) {
    let arr: js_sys::Array = keys.iter().map(|k| JsValue::from_str(k)).collect();
    call(&callbacks.mutation, &[arr.into()]);
}

/// A string reply as the methods returning one report it.
fn reply_text(resp: Value) -> String {
    match resp {
        Value::SimpleString(s) => s,
        Value::Error(e) => e,
        _ => "ERR".to_string(),
    }
}

// ── public API ────────────────────────────────────────────────────────────────

#[wasm_bindgen]
impl RecachedCache {
    #[wasm_bindgen(constructor)]
    pub fn new() -> RecachedCache {
        let store = Arc::new(KeyValueStore::new());
        let callbacks = Rc::new(Callbacks::default());
        let idb = Rc::new(RefCell::new(None));
        // A fresh identity for every session — every page load, every tab.
        // Tabs share one IndexedDB, and before 0.5 each adopted the identity
        // stored there and bumped its epoch: the newer tab's writes then
        // raised the server's duplicate mark past every write the older tab
        // had yet to send, and those were skipped as duplicates — silently
        // lost. Restored outbox rows keep the identity they were written
        // under, inside their frames, so replaying them needs none.
        let client_id = random_client_id();
        let mut core = SyncClient::new(Arc::clone(&store), client_id.clone());
        core.set_epoch(1);
        RecachedCache {
            shared: WsShared {
                core: Rc::new(RefCell::new(core)),
                store: Arc::clone(&store),
                callbacks: Rc::clone(&callbacks),
                ws: Rc::new(RefCell::new(None)),
                handlers: Rc::new(RefCell::new(WsHandlers::default())),
                idb: Rc::clone(&idb),
                tag: Rc::new(client_id),
                seq: Rc::new(Cell::new(0)),
                wal_clock: Rc::new(Cell::new(0.0)),
                durability: Rc::new(Durability::default()),
                persist_synced: Rc::new(Cell::new(true)),
                synced_dirty: Rc::new(RefCell::new(HashSet::new())),
                synced_flush: Rc::new(RefCell::new(None)),
                last_synced_at: Rc::new(Cell::new(None)),
                url: Rc::new(RefCell::new(None)),
                auto_reconnect: Rc::new(Cell::new(false)),
                generation: Rc::new(Cell::new(0)),
                reconnect_cb: Rc::new(RefCell::new(None)),
            },
            store,
            bc: None,
            idb,
            callbacks,
            _onbc: None,
            ws_is_plaintext: false,
        }
    }

    /// Apply a store write locally and send it everywhere it goes: the outbox
    /// (and socket), other tabs, and IndexedDB. `wal_cmd` is the form the WAL
    /// records, when it differs from the wire form.
    ///
    /// The capacity check comes first, so a write the outbox would refuse
    /// throws `OutboxFullError` before it changes anything.
    fn local_write(
        &self,
        cmd: Command,
        encoded: &[u8],
        wal_cmd: Option<&[u8]>,
    ) -> Result<Value, JsValue> {
        preflight(&self.shared, true)?;
        let keys = mutation_keys(&cmd, &self.store);
        let resp = self.store.execute(cmd);
        if let Value::Error(_) = resp {
            return Ok(resp);
        }
        let queued = queue_write(&self.shared, encoded, true)?;
        if let Some(bc) = &self.bc {
            bc_post(bc, encoded);
        }
        persist_local_write(
            &self.shared,
            wal_cmd.unwrap_or(encoded),
            queued.as_ref().map(|e| (e.id, e.frame.as_slice())),
        );
        notify_mutation(&self.callbacks, &keys);
        emit_status(&self.shared);
        Ok(resp)
    }

    /// Queue a connection-scoped command (pub/sub) — replayed on reconnect
    /// but never dedup-wrapped: skipping a re-sent SUBSCRIBE would silently
    /// drop the subscription.
    fn ws_enqueue_nodedup(&self, encoded: &[u8]) -> Result<(), JsValue> {
        preflight(&self.shared, false)?;
        if let Some(e) = queue_write(&self.shared, encoded, false)? {
            persist_outbox_row(&self.shared, e.id, &e.frame);
        }
        Ok(())
    }

    /// Register a JS callback invoked after every mutation from any source
    /// (local write, server WebSocket push, or BroadcastChannel sync).
    /// The SDK's `onMutation()` wires this up automatically.
    pub fn set_mutation_callback(&mut self, cb: js_sys::Function) {
        *self.callbacks.mutation.borrow_mut() = Some(cb);
    }

    /// Register a JS callback invoked when the outbox evicts a queued write
    /// under the legacy policy (`set_overflow_policy("evict-oldest")`).
    /// Signature: `cb(droppedId: number, pendingWrites: number)`. Under the
    /// default policy a full outbox throws `OutboxFullError` from the write
    /// instead, and this is never called.
    pub fn set_outbox_full_callback(&mut self, cb: js_sys::Function) {
        *self.callbacks.outbox_full.borrow_mut() = Some(cb);
    }

    /// Register a callback for synchronization status changes:
    /// `cb(status: { phase, error, pendingWrites, lastSyncedAt,
    /// persistenceFailures })`.
    pub fn set_status_callback(&mut self, cb: js_sys::Function) {
        *self.callbacks.status.borrow_mut() = Some(cb);
    }

    /// Register a callback for live-query progress:
    /// `cb(pattern: string, state: { state, failure?, reason? } | null)`.
    pub fn set_subscription_callback(&mut self, cb: js_sys::Function) {
        *self.callbacks.subscription.borrow_mut() = Some(cb);
    }

    /// Register a callback for writes the server refused for good:
    /// `cb(keys: string[], unreconciled: string[], reason: string)`.
    /// `unreconciled` names the keys no live query covers, whose local value
    /// is the refused one and will not be corrected.
    pub fn set_refused_callback(&mut self, cb: js_sys::Function) {
        *self.callbacks.refused.borrow_mut() = Some(cb);
    }

    /// Register a callback for IndexedDB failures:
    /// `cb(operation: string, message: string)`. A write the callback reports
    /// is in memory, and syncs while the page stays open, but is not durable.
    pub fn set_persistence_error_callback(&mut self, cb: js_sys::Function) {
        *self.callbacks.persistence_error.borrow_mut() = Some(cb);
    }

    /// `"reject"` (the default) refuses a write past the outbox cap with
    /// `OutboxFullError`; `"evict-oldest"` drops the oldest queued write
    /// instead, as before 0.5.
    pub fn set_overflow_policy(&self, policy: &str) -> Result<(), JsValue> {
        let policy = match policy {
            "reject" => OverflowPolicy::Reject,
            "evict-oldest" => OverflowPolicy::EvictOldest,
            other => {
                return Err(JsValue::from_str(&format!(
                    "unknown overflow policy {other:?}; expected \"reject\" or \"evict-oldest\""
                )));
            }
        };
        self.shared.core.borrow_mut().set_overflow_policy(policy);
        Ok(())
    }

    /// Set the outbox cap (default 10,000; minimum 1).
    pub fn set_max_pending(&self, max: u32) {
        self.shared.core.borrow_mut().set_max_pending(max as usize);
    }

    /// Whether server-applied changes are written to IndexedDB, so a reload
    /// shows the last synchronized state rather than only this browser's own
    /// writes. On by default.
    pub fn set_persist_synced_state(&self, enabled: bool) {
        self.shared.persist_synced.set(enabled);
    }

    /// Writes queued locally and not yet acknowledged by the server. Poll this
    /// to show sync state, or to back off before the queue overflows.
    pub fn pending_writes(&self) -> usize {
        self.shared.core.borrow().outbox_len()
    }

    /// The synchronization status, as the status callback receives it.
    pub fn sync_status(&self) -> JsValue {
        let status = self.shared.core.borrow().status();
        status_js(&self.shared, &status)
    }

    /// One live query's progress, or `null` when it is not registered.
    pub fn subscription_state(&self, pattern: &str) -> JsValue {
        self.shared
            .core
            .borrow()
            .subscription_state(pattern)
            .as_ref()
            .map_or(JsValue::NULL, subscription_js)
    }

    /// Resolves once every IndexedDB write made before this call has
    /// committed. Rejects with the first failure since the last `flush`.
    ///
    /// A write method returns as soon as the change is in memory; this is
    /// how to know it is on disk. Committed is not crash-proof: browsers may
    /// still lose the last moments of writes on a power cut.
    pub fn flush(&self) -> Promise {
        flush_synced(&self.shared);
        let d = Rc::clone(&self.shared.durability);
        let pending: Vec<Promise> = d.pending.borrow().values().cloned().collect();
        wasm_bindgen_futures::future_to_promise(async move {
            let mut first: Option<JsValue> = None;
            for promise in pending {
                if let Err(e) = JsFuture::from(promise).await {
                    first.get_or_insert(e);
                }
            }
            let unreported = d.unreported.borrow_mut().take();
            match first.or(unreported) {
                Some(e) => Err(e),
                None => Ok(JsValue::UNDEFINED),
            }
        })
    }

    /// Register a JS callback invoked when a pub/sub message arrives.
    /// Signature: `cb(channel: string, message: string)`.
    /// The SDK's `onMessage()` wires this up automatically.
    pub fn set_message_callback(&mut self, cb: js_sys::Function) {
        *self.callbacks.message.borrow_mut() = Some(cb);
    }

    /// Open the IndexedDB WAL, replay all stored commands into the in-memory
    /// store, and enable persistence for future writes.
    ///
    /// Call once at startup before reading or writing if you want the cache to
    /// survive page refreshes. Safe to call with no server connection.
    /// Rejects — rather than carrying on without persistence — when
    /// IndexedDB cannot be opened or read.
    ///
    /// ```js
    /// await cache.enable_persistence();
    /// ```
    pub fn enable_persistence(&self) -> Promise {
        let store = Arc::clone(&self.store);
        let sh = self.shared.clone();

        wasm_bindgen_futures::future_to_promise(async move {
            let db = JsFuture::from(open_recached_db()).await?;
            let restored_ids: HashSet<u64>;

            // Restore the durable outbox: writes from earlier sessions — any
            // tab — that never got a server acknowledgment. They re-send
            // before writes queued in this session. Each keeps the identity
            // and wire id it was written under, inside its frame, so a write
            // the server already applied is recognised and skipped.
            {
                let result = JsFuture::from(idb_outbox_read_all(&db)).await?;
                let pair = js_sys::Array::from(&result);
                let keys = js_sys::Array::from(&pair.get(0));
                let vals = js_sys::Array::from(&pair.get(1));
                // IndexedDB returns rows in key order — per session, in the
                // order written — which is the replay order.
                let mut restored: Vec<(u64, Vec<u8>)> = Vec::with_capacity(keys.length() as usize);
                let old_keys = js_sys::Array::new();
                for i in 0..keys.length() {
                    let raw = vals.get(i);
                    let cmd = match raw.as_string() {
                        // Rows written by 0.2.1 and earlier, when frames were
                        // strings rather than Uint8Array.
                        Some(t) => t.into_bytes(),
                        None => js_sys::Uint8Array::new(&raw).to_vec(),
                    };
                    old_keys.push(&keys.get(i));
                    if !cmd.is_empty() {
                        restored.push((u64::from(i), cmd));
                    }
                }
                // Restore and send in one synchronous step. A write made
                // before this point has already gone out; one made after it
                // must go out after these rows, so they are sent here, before
                // any await gives the page a chance to write. Collected first:
                // holding the `RefCell` borrow across an await lets an
                // incoming frame re-enter and panic on a double borrow.
                let connected = socket_is_open(&sh);
                let renumbered = sh.core.borrow_mut().restore_outbox(restored, connected);
                if let Some(ws) = sh.ws.borrow().as_ref() {
                    for frame in &renumbered.send_now {
                        ws_send_frame(ws, frame);
                    }
                }
                let new_keys = js_sys::Array::new();
                let frames = js_sys::Array::new();
                for (_, new_id, frame) in &renumbered.rewrites {
                    new_keys.push(&outbox_key(&sh, *new_id));
                    frames.push(&js_sys::Uint8Array::from(frame.as_slice()));
                }
                // Created now, so it runs before any delete a reply to one of
                // these rows causes: IndexedDB runs overlapping transactions
                // in creation order.
                let replaced = idb_outbox_replace_js(&db, &old_keys, &new_keys, &frames);
                restored_ids = renumbered.rewrites.iter().map(|(_, id, _)| *id).collect();
                JsFuture::from(replaced).await?;
                emit_status(&sh);
            }

            let result = JsFuture::from(idb_read_all(&db)).await?;
            let pair = js_sys::Array::from(&result);
            let keys = js_sys::Array::from(&pair.get(0));
            let vals = js_sys::Array::from(&pair.get(1));

            let entry_count = keys.length();
            for i in 0..entry_count {
                let raw = vals.get(i);
                let cmd_bytes = match raw.as_string() {
                    // Written by 0.2.1 and earlier, when frames were strings.
                    Some(t) => t.into_bytes(),
                    None => js_sys::Uint8Array::new(&raw).to_vec(),
                };
                if let Ok((value, _)) = Value::parse(&cmd_bytes)
                    && let Ok(cmd) = Command::from_value(value)
                {
                    store.execute(cmd);
                }
            }

            *sh.idb.borrow_mut() = Some(db.clone());

            // Writes queued before persistence was enabled exist only in
            // memory; write their rows now, so closing the page cannot lose
            // them. (Their local effect is not in the WAL: a reload shows it
            // once a live query hydrates it, or the server echoes it.)
            let unsaved: Vec<(u64, Vec<u8>)> = sh
                .core
                .borrow()
                .outbox_rows()
                .filter(|(id, _)| !restored_ids.contains(id))
                .map(|(id, frame)| (id, frame.to_vec()))
                .collect();
            for (id, frame) in unsaved {
                persist_outbox_row(&sh, id, &frame);
            }

            // If the WAL grew large, compact: replace the records just read
            // with minimal snapshot commands. This keeps startup replay fast
            // regardless of how many writes accumulated between refreshes.
            // Atomic, and limited to the records read, so neither an
            // interruption nor another tab's concurrent writes are lost.
            if entry_count > WAL_COMPACT_THRESHOLD {
                let cmds = snapshot_to_resp_cmds(&store.snapshot());
                let new_keys = js_sys::Array::new();
                let arr = js_sys::Array::new();
                for cmd in &cmds {
                    new_keys.push(&next_wal_key(&sh));
                    arr.push(&js_sys::Uint8Array::from(&cmd[..]));
                }
                JsFuture::from(idb_wal_replace_js(&db, &keys, &new_keys, &arr)).await?;
            }

            Ok(JsValue::UNDEFINED)
        })
    }

    /// Erase the IndexedDB WAL. The in-memory store is not affected.
    /// Useful for sign-out flows where you want to drop all persisted state.
    ///
    /// ```js
    /// await cache.clear_persistence();
    /// ```
    pub fn clear_persistence(&self) -> Promise {
        // Sign-out semantics: unsent offline writes are discarded too.
        self.shared.core.borrow_mut().clear_outbox();
        self.shared.synced_dirty.borrow_mut().clear();
        let maybe_db = self.idb.borrow().as_ref().cloned();
        if let Some(db) = maybe_db {
            wasm_bindgen_futures::future_to_promise(async move {
                JsFuture::from(idb_clear_js(&db)).await?;
                Ok(JsValue::UNDEFINED)
            })
        } else {
            Promise::resolve(&JsValue::UNDEFINED)
        }
    }

    /// Opt-in to cross-tab sync via the BroadcastChannel API.
    /// All tabs calling `broadcast()` with the same channel name share mutations
    /// automatically — no server required. Isolated by default (never called = no sync).
    /// Calling again with a different name replaces the previous channel cleanly.
    pub fn broadcast(&mut self, channel_name: &str) -> Result<(), JsValue> {
        let bc = BroadcastChannel::new(channel_name)?;
        let store_clone = Arc::clone(&self.store);
        let callbacks = Rc::clone(&self.callbacks);

        let onbc = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Some(raw) = event_frame_bytes(&e)
                && let Ok((value, _)) = Value::parse(&raw)
                && let Ok(cmd) = Command::from_value(value)
                && is_replayable_mutation(&cmd)
            {
                let keys = mutation_keys(&cmd, &store_clone);
                store_clone.execute(cmd);
                notify_mutation(&callbacks, &keys);
            }
        }) as Box<dyn FnMut(MessageEvent)>);

        bc.set_onmessage(Some(onbc.as_ref().unchecked_ref()));

        self._onbc = Some(onbc);
        self.bc = Some(bc);
        Ok(())
    }

    /// Connect to the native Recached backend via WebSockets.
    /// Calling this a second time cleanly replaces the previous connection.
    pub fn connect(&mut self, url: &str) -> Result<(), JsValue> {
        self.ws_is_plaintext = url.starts_with("ws://");
        // Silence the old socket's close handler before replacing it — its
        // generation is now stale, so it cannot schedule a reconnect over the
        // new one, but closing it early keeps things tidy.
        self.shared.auto_reconnect.set(false);
        if let Some(old_ws) = self.shared.ws.borrow_mut().take() {
            let _ = old_ws.close();
            // Its close event will be ignored as stale, so close it in the
            // core now: the old socket's session is over.
            self.shared.core.borrow_mut().on_close();
        }
        *self.shared.url.borrow_mut() = Some(url.to_string());
        self.shared.auto_reconnect.set(true);
        open_socket(&self.shared);
        Ok(())
    }

    /// Close the connection and stop reconnecting. Local reads and writes keep
    /// working; writes queue and are replayed on the next `connect`.
    pub fn disconnect(&mut self) {
        self.shared.auto_reconnect.set(false);
        if let Some(ws) = self.shared.ws.borrow_mut().take() {
            let _ = ws.close();
        }
    }

    /// Enable or disable automatic reconnection (enabled by default once
    /// `connect` is called).
    pub fn set_auto_reconnect(&self, enabled: bool) {
        self.shared.auto_reconnect.set(enabled);
    }

    /// True when the socket exists and is OPEN.
    fn socket_open(&self) -> bool {
        socket_is_open(&self.shared)
    }

    /// Send a session frame the core produced (it has already recorded the
    /// inflight reply slot).
    fn send_session_frame(&self, frame: Option<Vec<u8>>) {
        if let Some(frame) = frame
            && let Some(ws) = self.shared.ws.borrow().as_ref()
        {
            ws_send_frame(ws, &frame);
        }
        emit_status(&self.shared);
    }

    /// Send an AUTH command to the server. The password is remembered and
    /// re-sent automatically on every reconnect. The response arrives
    /// asynchronously via onmessage.
    pub fn auth(&self, password: &str) -> String {
        if self.ws_is_plaintext {
            web_sys::console::warn_1(&JsValue::from_str(
                "recached: AUTH over unencrypted ws:// exposes the password in plaintext; use wss://",
            ));
        }
        let open = self.socket_open();
        let frame = self.shared.core.borrow_mut().set_password(password, open);
        self.send_session_frame(frame);
        "OK".to_string()
    }

    /// Present a signed sync-scope token (servers running with
    /// `RECACHED_SYNC_SECRET`). The token is remembered and re-presented
    /// automatically on every reconnect. The grant confirmation arrives
    /// asynchronously.
    pub fn sync_token(&self, token: &str) {
        let open = self.socket_open();
        let frame = self.shared.core.borrow_mut().set_sync_token(token, open);
        self.send_session_frame(frame);
    }

    /// Set sync scopes directly from comma-separated glob patterns. Only
    /// honoured by servers without a sync secret — a bandwidth filter, not an
    /// authorization boundary. Re-applied automatically on reconnect.
    ///
    /// An entry may state its access: `r=catalog:*` is read-only,
    /// `rw=cart:42:*` and bare patterns are read-write.
    pub fn sync_scopes(&self, patterns_csv: &str) {
        let open = self.socket_open();
        let frame = self
            .shared
            .core
            .borrow_mut()
            .set_sync_scopes(patterns_csv, open);
        self.send_session_frame(frame);
    }

    /// Subscribe to a live query. The server replies with the current state
    /// of every key matching the glob pattern — applied into the local store —
    /// then streams every change to matching keys. Re-subscribed automatically
    /// on reconnect, which also re-hydrates the matching keys. The mutation
    /// callback fires on the initial state and on each change; read the data
    /// with `get_matching`.
    pub fn live_query(&self, pattern: &str) {
        let open = self.socket_open();
        let frame = self.shared.core.borrow_mut().add_live_query(pattern, open);
        self.send_session_frame(frame);
    }

    /// Drop one live query, or all of them when `pattern` is omitted.
    pub fn live_unquery(&self, pattern: Option<String>) {
        let open = self.socket_open();
        let frame = self
            .shared
            .core
            .borrow_mut()
            .remove_live_query(pattern.as_deref(), open);
        self.send_session_frame(frame);
    }

    /// Increment (or with a negative delta, decrement) an integer counter —
    /// locally, on the server, and across tabs. Returns the new local value.
    ///
    /// Offline increments queue as *deltas* and merge additively with
    /// concurrent increments from other clients on reconnect (PN-counter
    /// semantics) — unlike a plain `set`, nobody's increments are lost.
    ///
    /// Throws `OutboxFullError`, changing nothing, when the outbox is full.
    pub fn incr_by(&self, key: &str, delta: i64) -> Result<i64, JsValue> {
        let delta_s = delta.to_string();
        let encoded = to_resp(&["INCRBY", key, &delta_s]);
        match self.local_write(Command::IncrBy(key.to_string(), delta), &encoded, None)? {
            Value::Integer(n) => Ok(n),
            Value::Error(e) => Err(JsValue::from_str(&e)),
            _ => Ok(0),
        }
    }

    /// Set JSON at a path (`"$"` = whole document) — locally, on the server,
    /// and across tabs. `value` must be valid JSON text. Returns "OK" or an
    /// error string; throws `OutboxFullError` when the outbox is full.
    pub fn jset(&self, key: &str, path: &str, value: &str) -> Result<String, JsValue> {
        let encoded = to_resp(&["JSET", key, path, value]);
        let cmd = Command::JSet(key.to_string(), path.to_string(), value.to_string());
        Ok(reply_text(self.local_write(cmd, &encoded, None)?))
    }

    /// Read JSON at a path from the local store, serialized. `None` when the
    /// key or path does not exist.
    pub fn jget(&self, key: &str, path: Option<String>) -> Option<String> {
        match self
            .store
            .execute(Command::JGet(key.to_string(), path.clone()))
        {
            Value::BulkString(Some(bytes)) => Some(String::from_utf8_lossy(&bytes).into_owned()),
            _ => None,
        }
    }

    /// RFC 7386 JSON Merge Patch against the whole document — locally, on the
    /// server, and across tabs. `null` fields remove keys; a `null` patch
    /// deletes the document. Returns "OK" or an error string; throws
    /// `OutboxFullError` when the outbox is full.
    pub fn jmerge(&self, key: &str, patch: &str) -> Result<String, JsValue> {
        let encoded = to_resp(&["JMERGE", key, patch]);
        let cmd = Command::JMerge(key.to_string(), patch.to_string());
        Ok(reply_text(self.local_write(cmd, &encoded, None)?))
    }

    /// True when any of `keys` matches the glob `pattern`, using the engine's
    /// own matcher — the same rules a live query uses on the server, so the
    /// SDK can never route a change to a pattern the server would not.
    ///
    /// Takes the whole change set in one call: a reconnect `qstate` can name
    /// thousands of keys, and crossing the wasm boundary once per key per
    /// pattern would cost more than the re-read it saves.
    #[wasm_bindgen(js_name = "anyKeyMatches")]
    pub fn any_key_matches(&self, pattern: &str, keys: Vec<String>) -> bool {
        keys.iter().any(|k| glob_match(pattern, k))
    }

    /// Snapshot of local keys matching a glob pattern, as an array of
    /// `[key, value]` pairs sorted by key — deterministic order for UI
    /// rendering and stable snapshot comparison. String values only; keys
    /// holding collection types come back with `null` (read them with typed
    /// accessors).
    pub fn get_matching(&self, pattern: &str) -> js_sys::Array {
        let mut kvs = self.store.matching_key_values(pattern, usize::MAX);
        kvs.sort_by(|(a, _), (b, _)| a.cmp(b));
        let out = js_sys::Array::new();
        for (k, v) in kvs {
            let pair = js_sys::Array::new();
            pair.push(&JsValue::from_str(&k));
            match v {
                Value::BulkString(Some(bytes)) => {
                    match std::str::from_utf8(&bytes) {
                        Ok(text) => pair.push(&JsValue::from_str(text)),
                        // A binary value has no string form; handing back a
                        // lossy rendering would corrupt it silently in the one
                        // API a live query reads through.
                        Err(_) => pair.push(&js_sys::Uint8Array::from(&bytes[..]).into()),
                    };
                }
                _ => {
                    pair.push(&JsValue::NULL);
                }
            }
            out.push(&pair);
        }
        out
    }

    /// Set a key-value pair locally, sync to the server, and fan out to other
    /// tabs if broadcast() was called. Throws `OutboxFullError`, changing
    /// nothing, when the outbox is full.
    pub fn set(&self, key: &str, value: &str) -> Result<String, JsValue> {
        let encoded = to_resp(&["SET", key, value]);
        let cmd = Command::Set(
            key.to_string(),
            value.as_bytes().to_vec(),
            SetOptions::default(),
        );
        Ok(reply_text(self.local_write(cmd, &encoded, None)?))
    }

    /// Set a key to raw bytes, synced to the server and other tabs.
    ///
    /// Values are byte-transparent, so this stores and replicates exactly the
    /// bytes given — compressed payloads, protobuf, images. `set()` is the
    /// right call for text; this exists for everything a JS string cannot hold.
    #[wasm_bindgen(js_name = "setBytes")]
    pub fn set_bytes(&self, key: &str, value: &[u8]) -> Result<String, JsValue> {
        let encoded = to_resp_bytes(&[b"SET", key.as_bytes(), value]);
        let cmd = Command::Set(key.to_string(), value.to_vec(), SetOptions::default());
        Ok(reply_text(self.local_write(cmd, &encoded, None)?))
    }

    /// Set a key with a TTL in seconds, synced to the server and other tabs if
    /// broadcast() was called.
    ///
    /// The WAL records the absolute deadline, so a reload does not restart
    /// the TTL; the server, which receives the relative form, rebases it when
    /// the write arrives — later than now if it was queued offline.
    pub fn set_ex(&self, key: &str, value: &str, seconds: u32) -> Result<String, JsValue> {
        let opts = SetOptions {
            expiry: Some(core_engine::cmd::SetExpiry::Ex(seconds as u64)),
            ..Default::default()
        };
        let encoded = to_resp(&["SET", key, value, "EX", &seconds.to_string()]);
        let deadline = (wall_clock_ms() + u64::from(seconds) * 1000).to_string();
        let wal = to_resp(&["SET", key, value, "PXAT", &deadline]);
        let cmd = Command::Set(key.to_string(), value.as_bytes().to_vec(), opts);
        Ok(reply_text(self.local_write(cmd, &encoded, Some(&wal))?))
    }

    /// Get a value from the local store without a network round trip.
    pub fn get(&self, key: &str) -> Result<Option<String>, JsValue> {
        match self.store.execute(Command::Get(key.to_string())) {
            Value::BulkString(Some(data)) => match String::from_utf8(data) {
                Ok(text) => Ok(Some(text)),
                // A backend can write binary that syncs into this cache.
                // Returning it lossily would hand the application text that is
                // not what was stored, so this throws instead.
                Err(_) => Err(JsValue::from_str(
                    "value is not valid UTF-8 — use getBytes() to read binary values",
                )),
            },
            _ => Ok(None),
        }
    }

    /// Read a key as raw bytes, whatever it contains.
    ///
    /// `get()` is the right call for text; this exists for values a backend
    /// wrote as binary — compressed payloads, protobuf, images — which cannot
    /// be represented as a JS string. Returns `undefined` when the key is
    /// absent.
    #[wasm_bindgen(js_name = "getBytes")]
    pub fn get_bytes(&self, key: &str) -> Option<js_sys::Uint8Array> {
        match self.store.execute(Command::Get(key.to_string())) {
            Value::BulkString(Some(data)) => Some(js_sys::Uint8Array::from(&data[..])),
            _ => None,
        }
    }

    /// Delete a key locally, sync to the server, and fan out to other tabs if
    /// broadcast() was called. Throws `OutboxFullError`, changing nothing,
    /// when the outbox is full.
    pub fn del(&self, key: &str) -> Result<i32, JsValue> {
        let encoded = to_resp(&["DEL", key]);
        match self.local_write(Command::Del(vec![key.to_string()]), &encoded, None)? {
            Value::Integer(i) => Ok(i as i32),
            _ => Ok(0),
        }
    }

    /// Get the TTL of a key in seconds (-1 = no TTL, -2 = key doesn't exist).
    pub fn ttl(&self, key: &str) -> i32 {
        match self.store.execute(Command::Ttl(key.to_string())) {
            Value::Integer(n) => n as i32,
            _ => -2,
        }
    }

    /// Check if a key exists in the local store.
    pub fn exists(&self, key: &str) -> bool {
        matches!(
            self.store.execute(Command::Exists(vec![key.to_string()])),
            Value::Integer(1)
        )
    }

    /// Publish a message to a channel on the server.
    pub fn publish(&self, channel: &str, message: &str) -> Result<(), JsValue> {
        self.ws_enqueue_nodedup(&to_resp(&["PUBLISH", channel, message]))
    }

    /// Publish raw bytes to a channel on the server.
    ///
    /// Subscribers receive a `Uint8Array` rather than a string when the payload
    /// is not valid UTF-8.
    #[wasm_bindgen(js_name = "publishBytes")]
    pub fn publish_bytes(&self, channel: &str, message: &[u8]) -> Result<(), JsValue> {
        self.ws_enqueue_nodedup(&to_resp_bytes(&[b"PUBLISH", channel.as_bytes(), message]))
    }

    /// Subscribe to a channel on the server. Push messages arrive via the `onmessage` callback.
    pub fn subscribe(&self, channel: &str) -> Result<(), JsValue> {
        self.ws_enqueue_nodedup(&to_resp(&["SUBSCRIBE", channel]))
    }

    /// Unsubscribe from a channel on the server.
    pub fn unsubscribe(&self, channel: &str) -> Result<(), JsValue> {
        self.ws_enqueue_nodedup(&to_resp(&["UNSUBSCRIBE", channel]))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
//
// Only the parts of this crate that do not touch `web_sys`/`js_sys` can run on
// a native target. That is the WAL encoding and the frame builders — the
// correctness-critical, browser-independent logic. The socket, IndexedDB, and
// `Closure` plumbing needs `wasm-bindgen-test` in a headless browser and is
// covered end-to-end by the JS SDK suite instead.
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn entry(key: &str, value: SnapshotValue, expires_at_ms: Option<u64>) -> SnapshotEntry {
        SnapshotEntry {
            key: key.to_string(),
            value,
            expires_at_ms,
        }
    }

    fn now_ms() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Join the emitted RESP frames for substring assertions.
    ///
    /// Frames are bytes now that values can be binary; these tests all use text
    /// payloads, so rendering them lossily for assertions is safe here and
    /// keeps the assertions readable.
    fn joined(entries: &[SnapshotEntry]) -> String {
        as_text(&snapshot_to_resp_cmds(entries)).join("")
    }

    fn as_text(frames: &[Vec<u8>]) -> Vec<String> {
        frames
            .iter()
            .map(|f| String::from_utf8_lossy(f).into_owned())
            .collect()
    }

    // ── WAL encoding: every value type must survive a page reload ─────────────

    #[test]
    fn strings_persist_as_set() {
        let out = as_text(&snapshot_to_resp_cmds(&[entry(
            "k",
            SnapshotValue::Str("v".into()),
            None,
        )]));
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("SET") && out[0].contains("k") && out[0].contains("v"));
    }

    #[test]
    fn hashes_persist_every_field() {
        let mut m = HashMap::new();
        m.insert("f1".to_string(), "v1".into());
        m.insert("f2".to_string(), "v2".into());
        let s = joined(&[entry("h", SnapshotValue::Hash(m), None)]);
        assert!(s.contains("HSET"));
        for part in ["f1", "v1", "f2", "v2"] {
            assert!(s.contains(part), "field {part} lost from WAL: {s}");
        }
    }

    #[test]
    fn lists_persist_in_order_with_rpush() {
        // RPUSH (not LPUSH) — replaying with LPUSH would reverse the list.
        let s = joined(&[entry(
            "l",
            SnapshotValue::List(vec!["a".into(), "b".into(), "c".into()]),
            None,
        )]);
        assert!(s.contains("RPUSH"), "{s}");
        let (ia, ib, ic) = (
            s.find("\r\na\r\n").unwrap(),
            s.find("\r\nb\r\n").unwrap(),
            s.find("\r\nc\r\n").unwrap(),
        );
        assert!(ia < ib && ib < ic, "list order not preserved: {s}");
    }

    #[test]
    fn sets_and_zsets_persist_with_their_members() {
        let s = joined(&[entry(
            "st",
            SnapshotValue::Set(vec!["m1".into(), "m2".into()]),
            None,
        )]);
        assert!(s.contains("SADD") && s.contains("m1") && s.contains("m2"));

        let z = joined(&[entry(
            "z",
            SnapshotValue::ZSet(vec![("alice".into(), 1.5), ("bob".into(), 2.0)]),
            None,
        )]);
        assert!(z.contains("ZADD"), "{z}");
        // ZADD takes score before member.
        assert!(
            z.find("1.5").unwrap() < z.find("alice").unwrap(),
            "score must precede member: {z}"
        );
        // Whole scores serialize without a trailing .0, matching the server.
        assert!(z.contains("\r\n2\r\n"), "expected bare '2' score: {z}");
    }

    #[test]
    fn json_documents_persist_as_a_root_jset() {
        let s = joined(&[entry("j", SnapshotValue::Json("{\"a\":1}".into()), None)]);
        assert!(
            s.contains("JSET") && s.contains("$") && s.contains("{\"a\":1}"),
            "{s}"
        );
    }

    // ── TTL handling ──────────────────────────────────────────────────────────

    #[test]
    fn already_expired_entries_are_dropped_not_resurrected() {
        // Replaying an expired key would bring deleted data back on reload.
        let out = as_text(&snapshot_to_resp_cmds(&[
            entry("dead", SnapshotValue::Str("v".into()), Some(1)),
            entry("live", SnapshotValue::Str("v".into()), None),
        ]));
        let s = out.join("");
        assert!(!s.contains("dead"), "expired key resurrected: {s}");
        assert!(s.contains("live"));
    }

    #[test]
    fn strings_with_a_ttl_carry_their_absolute_deadline() {
        // A relative PX, as before 0.5, restarted the clock on every reload:
        // a key with a minute left, reloaded an hour later, lived another
        // minute. PXAT keeps the deadline the server set.
        let exp = now_ms() + 60_000;
        let s = joined(&[entry("k", SnapshotValue::Str("v".into()), Some(exp))]);
        assert!(s.contains("PXAT"), "{s}");
        assert!(s.contains(&exp.to_string()), "{s}");
    }

    #[test]
    fn a_restored_ttl_does_not_restart() {
        // Replayed later than it was written, the key keeps its deadline.
        let exp = now_ms() + 2_000;
        let cmds = snapshot_to_resp_cmds(&[entry("k", SnapshotValue::Str("v".into()), Some(exp))]);
        let store = KeyValueStore::new();
        for frame in &cmds {
            let (value, _) = Value::parse(frame).unwrap();
            store.execute(Command::from_value(value).unwrap());
        }
        match store.execute(Command::PTtl("k".into())) {
            Value::Integer(ms) => assert!(ms <= 2_000, "{ms}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn key_state_records_replace_a_key_rather_than_merging_into_it() {
        // A server-applied change persists the key's whole state. Replaying
        // the raw `HSET` instead would leave fields a later snapshot removed.
        let current = KeyValueStore::new();
        current.execute(Command::HSet(
            "h".into(),
            vec![("kept".into(), b"1".to_vec())],
        ));
        let cmds = key_state_cmds(&current, &["h".to_string(), "gone".to_string()]);

        let replayed = KeyValueStore::new();
        replayed.execute(Command::HSet(
            "h".into(),
            vec![("stale".into(), b"x".to_vec())],
        ));
        replayed.execute(Command::Set(
            "gone".into(),
            b"x".to_vec(),
            Default::default(),
        ));
        for frame in &cmds {
            let (value, _) = Value::parse(frame).unwrap();
            replayed.execute(Command::from_value(value).unwrap());
        }
        assert_eq!(
            replayed.execute(Command::HGet("h".into(), "stale".into())),
            Value::BulkString(None)
        );
        assert_eq!(
            replayed.execute(Command::HGet("h".into(), "kept".into())),
            Value::BulkString(Some(b"1".to_vec()))
        );
        assert_eq!(
            replayed.execute(Command::Get("gone".into())),
            Value::BulkString(None)
        );
    }

    #[test]
    fn collections_with_a_ttl_get_a_separate_pexpireat() {
        // Collection commands cannot carry an inline TTL, so the expiry rides
        // in a second command — as an absolute timestamp this time.
        let exp = now_ms() + 60_000;
        let out = as_text(&snapshot_to_resp_cmds(&[entry(
            "l",
            SnapshotValue::List(vec!["a".into()]),
            Some(exp),
        )]));
        assert_eq!(out.len(), 2, "expected RPUSH + PEXPIREAT: {out:?}");
        assert!(out[1].contains("PEXPIREAT") && out[1].contains(&exp.to_string()));
    }

    #[test]
    fn a_string_with_a_ttl_emits_only_one_command() {
        let exp = now_ms() + 60_000;
        let out = as_text(&snapshot_to_resp_cmds(&[entry(
            "k",
            SnapshotValue::Str("v".into()),
            Some(exp),
        )]));
        assert_eq!(out.len(), 1, "SET carries PX inline: {out:?}");
    }

    // ── Entries that must not be written ──────────────────────────────────────

    #[test]
    fn empty_collections_emit_nothing() {
        // `RPUSH key` with no members is a syntax error on replay, which would
        // abort WAL hydration part-way through.
        let out = as_text(&snapshot_to_resp_cmds(&[
            entry("h", SnapshotValue::Hash(HashMap::new()), None),
            entry("l", SnapshotValue::List(vec![]), None),
            entry("s", SnapshotValue::Set(vec![]), None),
            entry("z", SnapshotValue::ZSet(vec![]), None),
        ]));
        assert!(out.is_empty(), "empty collections must be skipped: {out:?}");
    }

    #[test]
    fn rate_limiter_state_is_not_persisted_in_the_browser() {
        // Attempt state is server-side and transient; replaying it locally
        // would let a client fabricate its own rate-limit history.
        let out = as_text(&snapshot_to_resp_cmds(&[entry(
            "rl",
            SnapshotValue::RateLimiter {
                limit: 10,
                window_ms: 1000,
                events: vec![1, 2, 3],
            },
            None,
        )]));
        assert!(
            out.is_empty(),
            "rate-limiter state leaked into the WAL: {out:?}"
        );
    }

    #[test]
    fn an_empty_snapshot_produces_no_commands() {
        assert!(snapshot_to_resp_cmds(&[]).is_empty());
    }

    #[test]
    fn every_emitted_command_is_a_well_formed_resp_array() {
        let mut m = HashMap::new();
        m.insert("f".to_string(), "v".into());
        let out = as_text(&snapshot_to_resp_cmds(&[
            entry("s", SnapshotValue::Str("v".into()), None),
            entry("h", SnapshotValue::Hash(m), None),
            entry("l", SnapshotValue::List(vec!["a".into()]), None),
            entry("z", SnapshotValue::ZSet(vec![("m".into(), 1.0)]), None),
        ]));
        assert_eq!(out.len(), 4);
        for frame in &out {
            assert!(frame.starts_with('*'), "not a RESP array: {frame}");
            assert!(frame.ends_with("\r\n"), "unterminated frame: {frame}");
            // A parseable frame is the only guarantee that replay will work.
            assert!(
                core_engine::resp::Value::parse(frame.as_bytes()).is_ok(),
                "WAL frame does not parse: {frame}"
            );
        }
    }

    #[test]
    fn wal_round_trips_through_the_real_engine() {
        // The strongest check: encode a snapshot, replay it into a fresh store
        // exactly as hydration does, and confirm the data comes back.
        use core_engine::cmd::Command;
        use core_engine::store::KeyValueStore;

        let mut m = HashMap::new();
        m.insert("f".to_string(), "v".into());
        let cmds = snapshot_to_resp_cmds(&[
            entry("str", SnapshotValue::Str("hello".into()), None),
            entry("h", SnapshotValue::Hash(m), None),
            entry("l", SnapshotValue::List(vec!["a".into(), "b".into()]), None),
            entry("st", SnapshotValue::Set(vec!["m".into()]), None),
            entry("z", SnapshotValue::ZSet(vec![("alice".into(), 1.5)]), None),
        ]);

        let store = KeyValueStore::new();
        for frame in &cmds {
            let (value, _) = core_engine::resp::Value::parse(frame).unwrap();
            store.execute(Command::from_value(value).unwrap());
        }

        assert_eq!(
            store.execute(Command::Get("str".into())),
            core_engine::resp::Value::BulkString(Some(b"hello".to_vec()))
        );
        assert_eq!(
            store.execute(Command::HGet("h".into(), "f".into())),
            core_engine::resp::Value::BulkString(Some(b"v".to_vec()))
        );
        assert_eq!(
            store.execute(Command::LLen("l".into())),
            core_engine::resp::Value::Integer(2)
        );
        assert_eq!(
            store.execute(Command::SIsMember("st".into(), "m".into())),
            core_engine::resp::Value::Integer(1)
        );
        assert_eq!(
            store.execute(Command::ZScore("z".into(), "alice".into())),
            core_engine::resp::Value::BulkString(Some(b"1.5".to_vec()))
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Browser tests
//
// These exercise the IndexedDB persistence layer, which cannot run on a native
// target — `wasm_bindgen_test_configure!(run_in_browser)` puts them in a real
// browser with a real IndexedDB.
//
//     wasm-pack test --headless --chrome wasm-edge
//
// Scope: durable storage only (WAL, outbox, meta). The WebSocket and reconnect
// paths need a live server alongside the browser and are not covered here.
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(all(test, target_arch = "wasm32"))]
mod browser_tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    async fn open_db() -> JsValue {
        JsFuture::from(open_recached_db())
            .await
            .expect("IndexedDB should open")
    }

    /// Start every test from an empty database — IndexedDB persists across
    /// tests in the same browser session, so leftover rows would make results
    /// depend on execution order.
    async fn fresh_db() -> JsValue {
        let db = open_db().await;
        JsFuture::from(idb_clear_js(&db)).await.expect("clear");
        db
    }

    fn num(n: f64) -> JsValue {
        JsValue::from_f64(n)
    }

    fn arr(items: &[JsValue]) -> JsValue {
        items.iter().collect::<js_sys::Array>().into()
    }

    fn bytes(b: &[u8]) -> JsValue {
        js_sys::Uint8Array::from(b).into()
    }

    async fn wal_put(db: &JsValue, key: JsValue, cmd: &[u8]) {
        JsFuture::from(idb_write_local_js(
            db,
            &key,
            &bytes(cmd),
            &JsValue::NULL,
            &JsValue::NULL,
        ))
        .await
        .expect("wal put");
    }

    async fn outbox_put(db: &JsValue, key: JsValue, frame: &[u8]) {
        JsFuture::from(idb_write_local_js(
            db,
            &JsValue::NULL,
            &JsValue::NULL,
            &key,
            &bytes(frame),
        ))
        .await
        .expect("outbox put");
    }

    fn text_of(v: &JsValue) -> String {
        match v.as_string() {
            Some(t) => t,
            None => String::from_utf8_lossy(&js_sys::Uint8Array::new(v).to_vec()).into_owned(),
        }
    }

    /// `(keys, values)` from a `readAll`-shaped JS result.
    fn split_pairs(res: &JsValue) -> (Vec<JsValue>, Vec<String>) {
        let pair = js_sys::Array::from(res);
        let keys = js_sys::Array::from(&pair.get(0));
        let vals = js_sys::Array::from(&pair.get(1));
        (
            keys.iter().collect(),
            vals.iter().map(|v| text_of(&v)).collect(),
        )
    }

    async fn wal_rows(db: &JsValue) -> (Vec<JsValue>, Vec<String>) {
        let res = JsFuture::from(idb_read_all(db)).await.expect("wal read");
        split_pairs(&res)
    }

    async fn outbox_rows(db: &JsValue) -> (Vec<JsValue>, Vec<String>) {
        let res = JsFuture::from(idb_outbox_read_all(db))
            .await
            .expect("outbox read");
        split_pairs(&res)
    }

    // ── Engine smoke test ─────────────────────────────────────────────────────

    #[wasm_bindgen_test]
    fn core_engine_operates_in_the_browser() {
        // Regression guard. `core-engine` reads a clock on essentially every
        // operation (TTL checks, LRU recency), and `SystemTime::now()` panics
        // on wasm32-unknown-unknown — which made every store write abort in the
        // browser. Native tests cannot catch this; only a real wasm target can.
        let store = KeyValueStore::new();
        assert_eq!(
            store.execute(Command::Set("k".into(), "v".into(), SetOptions::default())),
            Value::SimpleString("OK".into())
        );
        store.execute(Command::Set(
            "ttl".into(),
            "v".into(),
            SetOptions {
                expiry: Some(core_engine::cmd::SetExpiry::Ex(60)),
                ..Default::default()
            },
        ));
        match store.execute(Command::Ttl("ttl".into())) {
            Value::Integer(n) => assert!(n > 0, "expected a live TTL, got {n}"),
            other => panic!("unexpected TTL reply: {other:?}"),
        }
    }

    // ── Schema ────────────────────────────────────────────────────────────────

    #[wasm_bindgen_test]
    async fn database_opens_with_all_three_stores() {
        let db = open_db().await;
        let names = js_sys::Reflect::get(&db, &JsValue::from_str("objectStoreNames"))
            .expect("objectStoreNames");
        let contains =
            js_sys::Reflect::get(&names, &JsValue::from_str("contains")).expect("contains fn");
        let f = js_sys::Function::from(contains);
        for store in ["wal", "outbox", "meta"] {
            let has = f
                .call1(&names, &JsValue::from_str(store))
                .expect("call contains");
            assert!(has.as_bool().unwrap_or(false), "missing store: {store}");
        }
    }

    // ── Keys ──────────────────────────────────────────────────────────────────

    #[wasm_bindgen_test]
    async fn rows_written_before_0_5_replay_before_session_keyed_rows() {
        // IndexedDB orders numbers before arrays: legacy rows are older.
        let db = fresh_db().await;
        wal_put(
            &db,
            arr(&[num(1.0), JsValue::from_str("tab"), num(0.0)]),
            b"NEW",
        )
        .await;
        wal_put(&db, num(7.0), b"OLD").await;
        let (_, vals) = wal_rows(&db).await;
        assert_eq!(vals, vec!["OLD".to_string(), "NEW".to_string()]);
    }

    #[wasm_bindgen_test]
    async fn two_tabs_never_overwrite_each_others_rows() {
        // Before 0.5 both tabs counted outbox ids from the same number, and
        // the second tab's put replaced the first tab's pending write.
        let db = fresh_db().await;
        outbox_put(&db, arr(&[JsValue::from_str("tab-a"), num(0.0)]), b"A0").await;
        outbox_put(&db, arr(&[JsValue::from_str("tab-b"), num(0.0)]), b"B0").await;
        let (_, vals) = outbox_rows(&db).await;
        assert_eq!(vals, vec!["A0".to_string(), "B0".to_string()]);
    }

    // ── Atomicity ─────────────────────────────────────────────────────────────

    #[wasm_bindgen_test]
    async fn a_local_write_commits_its_wal_record_and_outbox_row_together() {
        let db = fresh_db().await;
        JsFuture::from(idb_write_local_js(
            &db,
            &arr(&[num(1.0), JsValue::from_str("t"), num(0.0)]),
            &bytes(b"SET a 1"),
            &arr(&[JsValue::from_str("t"), num(0.0)]),
            &bytes(b"FRAME"),
        ))
        .await
        .expect("committed");
        assert_eq!(wal_rows(&db).await.1, vec!["SET a 1".to_string()]);
        assert_eq!(outbox_rows(&db).await.1, vec!["FRAME".to_string()]);
    }

    #[wasm_bindgen_test]
    async fn a_failed_local_write_leaves_neither_row_and_rejects() {
        // An object is not a valid IndexedDB key, so the outbox put throws
        // after the WAL put was issued: the transaction aborts, both go, and
        // the failure is a rejection rather than an exception into wasm.
        let db = fresh_db().await;
        let result = JsFuture::from(idb_write_local_js(
            &db,
            &num(1.0),
            &bytes(b"SET a 1"),
            &js_sys::Object::new().into(),
            &bytes(b"FRAME"),
        ))
        .await;
        assert!(result.is_err(), "the write must be reported failed");
        assert!(
            wal_rows(&db).await.1.is_empty(),
            "no WAL record without its row"
        );
        assert!(outbox_rows(&db).await.1.is_empty());
    }

    #[wasm_bindgen_test]
    async fn a_write_to_a_closed_database_rejects_instead_of_throwing() {
        // What clearing site data does to an open connection.
        let db = open_db().await;
        let close = js_sys::Reflect::get(&db, &JsValue::from_str("close")).unwrap();
        js_sys::Function::from(close).call0(&db).unwrap();
        let result = JsFuture::from(idb_outbox_delete_js(&db, &num(1.0))).await;
        assert!(result.is_err());
    }

    // ── Outbox ────────────────────────────────────────────────────────────────

    #[wasm_bindgen_test]
    async fn outbox_rows_survive_a_reopen() {
        let db = fresh_db().await;
        outbox_put(
            &db,
            arr(&[JsValue::from_str("t"), num(7.0)]),
            b"SET offline 1",
        )
        .await;
        drop(db);
        let db2 = open_db().await;
        assert_eq!(outbox_rows(&db2).await.1, vec!["SET offline 1".to_string()]);
    }

    #[wasm_bindgen_test]
    async fn deleting_an_acknowledged_row_leaves_the_others() {
        let db = fresh_db().await;
        for id in [1.0, 2.0, 3.0] {
            outbox_put(&db, arr(&[JsValue::from_str("t"), num(id)]), b"W").await;
        }
        JsFuture::from(idb_outbox_delete_js(
            &db,
            &arr(&[JsValue::from_str("t"), num(2.0)]),
        ))
        .await
        .expect("delete");
        let (keys, _) = outbox_rows(&db).await;
        let ids: Vec<f64> = keys
            .iter()
            .map(|k| js_sys::Array::from(k).get(1).as_f64().unwrap())
            .collect();
        assert_eq!(ids, vec![1.0, 3.0]);
    }

    #[wasm_bindgen_test]
    async fn the_restore_replace_keeps_rows_another_tab_wrote_meanwhile() {
        let db = fresh_db().await;
        outbox_put(&db, num(7.0), b"OLD A").await;
        // Another tab writes after this one read the outbox.
        outbox_put(&db, arr(&[JsValue::from_str("other"), num(0.0)]), b"OTHER").await;
        JsFuture::from(idb_outbox_replace_js(
            &db,
            &arr(&[num(7.0)]),
            &arr(&[arr(&[JsValue::from_str("me"), num(0.0)])]),
            &arr(&[bytes(b"NEW A")]),
        ))
        .await
        .expect("replace");
        assert_eq!(
            outbox_rows(&db).await.1,
            vec!["NEW A".to_string(), "OTHER".to_string()]
        );
    }

    #[wasm_bindgen_test]
    async fn deleting_a_missing_row_is_not_an_error() {
        let db = fresh_db().await;
        JsFuture::from(idb_outbox_delete_js(&db, &num(999.0)))
            .await
            .expect("deleting a missing row should resolve");
    }

    #[wasm_bindgen_test]
    async fn binary_writes_survive_the_outbox() {
        let db = fresh_db().await;
        let binary = vec![0xffu8, 0xfe, 0x00, 0x41, 0x80];
        let frame = to_resp_bytes(&[b"SET", b"k", &binary]);
        outbox_put(&db, num(0.0), &frame).await;
        let res = JsFuture::from(idb_outbox_read_all(&db)).await.unwrap();
        let raw = js_sys::Array::from(&js_sys::Array::from(&res).get(1)).get(0);
        assert_eq!(js_sys::Uint8Array::new(&raw).to_vec(), frame);
    }

    // ── WAL compaction ────────────────────────────────────────────────────────

    #[wasm_bindgen_test]
    async fn wal_replace_swaps_the_records_read_in_one_transaction() {
        let db = fresh_db().await;
        for (seq, cmd) in [(0.0, "SET a 1"), (1.0, "SET b 2")] {
            wal_put(&db, num(seq), cmd.as_bytes()).await;
        }
        // Another tab appends after the read.
        wal_put(
            &db,
            arr(&[num(9e12), JsValue::from_str("other"), num(0.0)]),
            b"SET c 3",
        )
        .await;
        JsFuture::from(idb_wal_replace_js(
            &db,
            &arr(&[num(0.0), num(1.0)]),
            &arr(&[arr(&[num(1.0), JsValue::from_str("me"), num(0.0)])]),
            &arr(&[bytes(b"SET compacted 1")]),
        ))
        .await
        .expect("replace");
        assert_eq!(
            wal_rows(&db).await.1,
            vec!["SET compacted 1".to_string(), "SET c 3".to_string()]
        );
    }

    #[wasm_bindgen_test]
    async fn wal_replace_leaves_the_outbox_untouched() {
        let db = fresh_db().await;
        wal_put(&db, num(0.0), b"SET a 1").await;
        outbox_put(&db, num(5.0), b"PENDING").await;
        JsFuture::from(idb_wal_replace_js(
            &db,
            &arr(&[num(0.0)]),
            &arr(&[]),
            &arr(&[]),
        ))
        .await
        .unwrap();
        assert_eq!(outbox_rows(&db).await.1, vec!["PENDING".to_string()]);
        assert!(wal_rows(&db).await.1.is_empty());
    }

    #[wasm_bindgen_test]
    async fn a_compacted_wal_replays_to_the_same_state_with_absolute_ttls() {
        let db = fresh_db().await;
        let source = KeyValueStore::new();
        source.execute(Command::Set("k".into(), "v".into(), SetOptions::default()));
        source.execute(Command::RPush("l".into(), vec!["a".into(), "b".into()]));
        source.execute(Command::Set(
            "t".into(),
            "v".into(),
            SetOptions {
                expiry: Some(core_engine::cmd::SetExpiry::Ex(60)),
                ..Default::default()
            },
        ));
        let cmds = snapshot_to_resp_cmds(&source.snapshot());
        for (i, c) in cmds.iter().enumerate() {
            wal_put(&db, num(i as f64), c).await;
        }
        let (_, vals) = wal_rows(&db).await;
        let restored = KeyValueStore::new();
        for frame in &vals {
            let (value, _) = Value::parse(frame.as_bytes()).unwrap();
            restored.execute(Command::from_value(value).unwrap());
        }
        assert_eq!(
            restored.execute(Command::Get("k".into())),
            Value::BulkString(Some(b"v".to_vec()))
        );
        assert_eq!(
            restored.execute(Command::LLen("l".into())),
            Value::Integer(2)
        );
        assert!(matches!(
            restored.execute(Command::Ttl("t".into())),
            Value::Integer(n) if (1..=60).contains(&n)
        ));
    }
}
