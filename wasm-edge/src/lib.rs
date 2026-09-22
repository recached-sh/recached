use core_engine::cmd::{Command, SetOptions};
use core_engine::resp::Value;
use core_engine::store::{KeyValueStore, SnapshotEntry, SnapshotValue, format_score};
use js_sys::Promise;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use sync_client::{Incoming, SyncClient, is_replayable_mutation, to_resp, to_resp_bytes};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{BroadcastChannel, Event, MessageEvent, WebSocket};

// ── IndexedDB JS glue ─────────────────────────────────────────────────────────
//
// WAL schema: object store "wal", out-of-line keys (seq number as f64),
// values are RESP-encoded command strings. IDB returns entries in ascending
// key order, so keys[i] and vals[i] always correspond after idbReadAll.

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
    });
}
export function idbReadAll(db) { return readAll(db, 'wal'); }
export function idbOutboxReadAll(db) { return readAll(db, 'outbox'); }
export function idbAppend(db, seq, cmd) {
    return new Promise((resolve, reject) => {
        const tx  = db.transaction('wal', 'readwrite');
        const req = tx.objectStore('wal').put(cmd, seq);
        req.onsuccess = () => resolve(undefined);
        req.onerror   = (e) => reject(e.target.error);
    });
}
export function idbOutboxPut(db, id, cmd) {
    return new Promise((resolve, reject) => {
        const tx  = db.transaction('outbox', 'readwrite');
        const req = tx.objectStore('outbox').put(cmd, id);
        req.onsuccess = () => resolve(undefined);
        req.onerror   = (e) => reject(e.target.error);
    });
}
export function idbOutboxDelete(db, id) {
    return new Promise((resolve, reject) => {
        const tx  = db.transaction('outbox', 'readwrite');
        const req = tx.objectStore('outbox').delete(id);
        req.onsuccess = () => resolve(undefined);
        req.onerror   = (e) => reject(e.target.error);
    });
}
export function idbOutboxReplace(db, ids, cmds) {
    return new Promise((resolve, reject) => {
        // Renumbering must be one transaction. Deleting and inserting rows one
        // at a time can overwrite an old id that has not been visited yet,
        // losing a pending write if the tab closes during hydration.
        const tx    = db.transaction('outbox', 'readwrite');
        const store = tx.objectStore('outbox');
        store.clear();
        for (let i = 0; i < ids.length; i++) store.put(cmds[i], ids[i]);
        tx.oncomplete = () => resolve(undefined);
        tx.onerror    = (e) => reject(e.target.error);
        tx.onabort    = (e) => reject(e.target.error);
    });
}
export function idbWalReplace(db, cmds) {
    return new Promise((resolve, reject) => {
        // One transaction: the clear and the rewrite commit together or not at
        // all. Clearing in a separate transaction (as this used to) leaves the
        // WAL empty if the tab closes before the snapshot is written, losing the
        // entire persisted cache.
        const tx    = db.transaction('wal', 'readwrite');
        const store = tx.objectStore('wal');
        store.clear();
        for (let i = 0; i < cmds.length; i++) store.put(cmds[i], i);
        tx.oncomplete = () => resolve(undefined);
        tx.onerror    = (e) => reject(e.target.error);
        tx.onabort    = (e) => reject(e.target.error);
    });
}
export function idbWalClear(db) {
    return new Promise((resolve, reject) => {
        const tx  = db.transaction('wal', 'readwrite');
        const req = tx.objectStore('wal').clear();
        req.onsuccess = () => resolve(undefined);
        req.onerror   = (e) => reject(e.target.error);
    });
}
export function idbMetaGet(db, key) {
    return new Promise((resolve, reject) => {
        const tx  = db.transaction('meta', 'readonly');
        const req = tx.objectStore('meta').get(key);
        req.onsuccess = (e) => resolve(e.target.result);
        req.onerror   = (e) => reject(e.target.error);
    });
}
export function idbMetaPut(db, key, val) {
    return new Promise((resolve, reject) => {
        const tx  = db.transaction('meta', 'readwrite');
        const req = tx.objectStore('meta').put(val, key);
        req.onsuccess = () => resolve(undefined);
        req.onerror   = (e) => reject(e.target.error);
    });
}
export function idbClear(db) {
    return new Promise((resolve, reject) => {
        const tx  = db.transaction(['wal', 'outbox'], 'readwrite');
        tx.objectStore('wal').clear();
        tx.objectStore('outbox').clear();
        tx.oncomplete = () => resolve(undefined);
        tx.onerror    = (e) => reject(e.target.error);
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
    #[wasm_bindgen(js_name = "idbAppend")]
    fn idb_append_js(db: &JsValue, seq: f64, cmd: &[u8]) -> Promise;
    #[wasm_bindgen(js_name = "idbOutboxPut")]
    fn idb_outbox_put_js(db: &JsValue, id: f64, cmd: &[u8]) -> Promise;
    #[wasm_bindgen(js_name = "idbOutboxDelete")]
    fn idb_outbox_delete_js(db: &JsValue, id: f64) -> Promise;
    #[wasm_bindgen(js_name = "idbOutboxReplace")]
    fn idb_outbox_replace_js(db: &JsValue, ids: &JsValue, cmds: &JsValue) -> Promise;
    #[wasm_bindgen(js_name = "idbWalClear")]
    fn idb_wal_clear_js(db: &JsValue) -> Promise;
    #[wasm_bindgen(js_name = "idbWalReplace")]
    fn idb_wal_replace_js(db: &JsValue, cmds: &JsValue) -> Promise;
    #[wasm_bindgen(js_name = "idbMetaGet")]
    fn idb_meta_get(db: &JsValue, key: &str) -> Promise;
    #[wasm_bindgen(js_name = "idbMetaPut")]
    fn idb_meta_put(db: &JsValue, key: &str, val: &JsValue) -> Promise;
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
                    let rem_ms = exp.saturating_sub(now_ms);
                    vec![
                        t("SET"),
                        t(&e.key),
                        s.as_slice().to_vec(),
                        t("PX"),
                        t(&rem_ms.to_string()),
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
// `sync-client` crate (shared with the planned mobile bindings). This layer
// only adapts it to the browser: WebSocket I/O, IndexedDB persistence,
// setTimeout reconnect timers, and JS callbacks.

/// Event closures for the *current* socket. Replaced wholesale on reconnect;
/// dropping the old ones detaches them.
#[derive(Default)]
struct WsHandlers {
    onmessage: Option<Closure<dyn FnMut(MessageEvent)>>,
    onopen: Option<Closure<dyn FnMut(Event)>>,
    onclose: Option<Closure<dyn FnMut(Event)>>,
}

/// Everything the reconnect machinery needs, clonable into JS closures.
/// The socket itself lives behind `Rc<RefCell<…>>` so a reconnect fired from
/// an event handler can replace it.
#[derive(Clone)]
struct WsShared {
    /// The platform-neutral sync state machine.
    core: Rc<RefCell<SyncClient>>,
    on_mutation: Rc<RefCell<Option<js_sys::Function>>>,
    on_message: Rc<RefCell<Option<js_sys::Function>>>,
    /// Invoked when the offline outbox overflows and a queued write is dropped.
    on_outbox_full: Rc<RefCell<Option<js_sys::Function>>>,
    ws: Rc<RefCell<Option<WebSocket>>>,
    handlers: Rc<RefCell<WsHandlers>>,
    /// Shared with `RecachedCache.idb` — the open IndexedDB handle, if any.
    idb: Rc<RefCell<Option<JsValue>>>,
    url: Rc<RefCell<Option<String>>>,
    auto_reconnect: Rc<Cell<bool>>,
    /// Socket generation: bumped by `open_socket`. Stale sockets' close events
    /// (e.g. one replaced by an explicit `connect`) must not schedule
    /// reconnects over the live socket.
    generation: Rc<Cell<u64>>,
    /// Keeps the pending reconnect timer's callback alive until it fires.
    reconnect_cb: ReconnectCallback,
}

/// Handle to the pending reconnect timer's callback, kept alive until it fires.
type ReconnectCallback = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

/// Delete an acknowledged (or evicted) outbox row from durable storage.
fn outbox_delete(sh: &WsShared, id: u64) {
    if let Some(db) = sh.idb.borrow().clone() {
        spawn_local(async move {
            let _ = JsFuture::from(idb_outbox_delete_js(&db, id as f64)).await;
        });
    }
}

/// Persist an outbox row to durable storage.
fn outbox_put(sh: &WsShared, id: u64, frame: &[u8]) {
    if let Some(db) = sh.idb.borrow().clone() {
        let cmd = frame.to_vec();
        spawn_local(async move {
            let _ = JsFuture::from(idb_outbox_put_js(&db, id as f64, &cmd)).await;
        });
    }
}

/// Execute the effects the core requested for an incoming frame.
fn dispatch_incoming(sh: &WsShared, incoming: Incoming) {
    match incoming {
        Incoming::Applied => notify_mutation(&sh.on_mutation),
        Incoming::PubSub { channel, message } => {
            if let Some(f) = sh.on_message.borrow().as_ref() {
                let payload = match std::str::from_utf8(&message) {
                    Ok(text) => JsValue::from_str(text),
                    // A binary publish cannot be a JS string; the listener gets
                    // the bytes rather than a lossy rendering of them.
                    Err(_) => js_sys::Uint8Array::from(&message[..]).into(),
                };
                let _ = f.call2(&JsValue::NULL, &JsValue::from_str(&channel), &payload);
            }
        }
        Incoming::Reply { retired } => {
            if let Some(id) = retired {
                outbox_delete(sh, id);
            }
        }
        Incoming::AppliedReply { retired } => {
            if let Some(id) = retired {
                outbox_delete(sh, id);
            }
            notify_mutation(&sh.on_mutation);
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
}

/// Queue a write through the core and mirror its effects: durable outbox row,
/// possible overflow eviction, and an immediate send when connected.
fn queue_write(sh: &WsShared, encoded: &[u8], dedup: bool) {
    // Local-only mode (no connect() and no persistence): nothing to sync to.
    if sh.url.borrow().is_none() && sh.idb.borrow().is_none() {
        return;
    }
    let connected = matches!(
        sh.ws.borrow().as_ref(),
        Some(ws) if ws.ready_state() == WebSocket::OPEN
    );
    let e = sh
        .core
        .borrow_mut()
        .enqueue_write(encoded, dedup, connected);
    if let Some(old) = e.dropped {
        outbox_delete(sh, old);
        web_sys::console::warn_1(&JsValue::from_str(
            "recached: offline write queue full — dropped the oldest queued write",
        ));
        // A silent drop is data loss the application cannot detect. Give it the
        // dropped frame so it can persist or reconcile the write itself.
        if let Some(cb) = sh.on_outbox_full.borrow().as_ref() {
            let _ = cb.call2(
                &JsValue::NULL,
                &JsValue::from_f64(old as f64),
                &JsValue::from_f64(sh.core.borrow().outbox_len() as f64),
            );
        }
    }
    outbox_put(sh, e.id, &e.frame);
    if e.send_now
        && let Some(ws) = sh.ws.borrow().as_ref()
    {
        ws_send_frame(ws, &e.frame);
    }
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

    let ws = match WebSocket::new(&url) {
        Ok(w) => w,
        Err(_) => {
            schedule_reconnect(sh);
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
    }) as Box<dyn FnMut(MessageEvent)>);
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

    // ── onopen: the core decides what to send (session, then outbox) ─────
    let sh_open = sh.clone();
    let ws_for_open = ws.clone();
    let onopen = Closure::wrap(Box::new(move |_e: Event| {
        for frame in sh_open.core.borrow_mut().on_open() {
            ws_send_frame(&ws_for_open, &frame);
        }
    }) as Box<dyn FnMut(Event)>);
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));

    // ── onclose: reconnect with backoff (unless this socket is stale) ────
    let sh_close = sh.clone();
    let onclose = Closure::wrap(Box::new(move |_e: Event| {
        if sh_close.generation.get() == generation {
            schedule_reconnect(&sh_close);
        }
    }) as Box<dyn FnMut(Event)>);
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));

    let mut h = sh.handlers.borrow_mut();
    h.onmessage = Some(onmessage);
    h.onopen = Some(onopen);
    h.onclose = Some(onclose);
    *sh.ws.borrow_mut() = Some(ws);
}

/// Schedule an `open_socket` retry at the core's backoff delay. No-op when
/// auto-reconnect is off or there is no window (non-browser contexts).
fn schedule_reconnect(sh: &WsShared) {
    if !sh.auto_reconnect.get() {
        return;
    }
    let delay = sh.core.borrow_mut().on_close();
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
    /// Monotonically-increasing WAL sequence counter.
    seq: Rc<Cell<u64>>,
    /// JS callback invoked after every mutation (local or server/BC-pushed).
    on_mutation: Rc<RefCell<Option<js_sys::Function>>>,
    /// JS callback invoked when a pub/sub message arrives: `cb(channel, message)`.
    on_message: Rc<RefCell<Option<js_sys::Function>>>,
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

// ── persistence helper ────────────────────────────────────────────────────────

fn persist_cmd(idb: &Rc<RefCell<Option<JsValue>>>, seq: &Rc<Cell<u64>>, encoded: &[u8]) {
    let maybe_db = idb.borrow().as_ref().cloned();
    if let Some(db) = maybe_db {
        let s = seq.get();
        seq.set(s + 1);
        let cmd = encoded.to_vec();
        spawn_local(async move {
            let _ = JsFuture::from(idb_append_js(&db, s as f64, &cmd)).await;
        });
    }
}

fn notify_mutation(on_mut: &Rc<RefCell<Option<js_sys::Function>>>) {
    if let Some(f) = on_mut.borrow().as_ref() {
        let _ = f.call0(&JsValue::NULL);
    }
}

// ── public API ────────────────────────────────────────────────────────────────

#[wasm_bindgen]
impl RecachedCache {
    #[wasm_bindgen(constructor)]
    pub fn new() -> RecachedCache {
        let store = Arc::new(KeyValueStore::new());
        let on_mutation = Rc::new(RefCell::new(None));
        let on_outbox_full: Rc<RefCell<Option<js_sys::Function>>> = Rc::new(RefCell::new(None));
        let on_message = Rc::new(RefCell::new(None));
        let idb = Rc::new(RefCell::new(None));
        let core = Rc::new(RefCell::new(SyncClient::new(
            Arc::clone(&store),
            random_client_id(),
        )));
        RecachedCache {
            shared: WsShared {
                core,
                on_mutation: Rc::clone(&on_mutation),
                on_message: Rc::clone(&on_message),
                on_outbox_full: Rc::clone(&on_outbox_full),
                ws: Rc::new(RefCell::new(None)),
                handlers: Rc::new(RefCell::new(WsHandlers::default())),
                idb: Rc::clone(&idb),
                url: Rc::new(RefCell::new(None)),
                auto_reconnect: Rc::new(Cell::new(false)),
                generation: Rc::new(Cell::new(0)),
                reconnect_cb: Rc::new(RefCell::new(None)),
            },
            store,
            bc: None,
            idb,
            seq: Rc::new(Cell::new(0)),
            on_mutation,
            on_message,
            _onbc: None,
            ws_is_plaintext: false,
        }
    }

    /// Queue a store write (DEDUP-wrapped to suppress reconnect replays).
    fn ws_enqueue(&self, encoded: &[u8]) {
        queue_write(&self.shared, encoded, true);
    }

    /// Queue a connection-scoped command (pub/sub) — replayed on reconnect
    /// but never dedup-wrapped: skipping a re-sent SUBSCRIBE would silently
    /// drop the subscription.
    fn ws_enqueue_nodedup(&self, encoded: &[u8]) {
        queue_write(&self.shared, encoded, false);
    }

    /// Register a JS callback invoked after every mutation from any source
    /// (local write, server WebSocket push, or BroadcastChannel sync).
    /// The SDK's `onMutation()` wires this up automatically.
    pub fn set_mutation_callback(&mut self, cb: js_sys::Function) {
        *self.on_mutation.borrow_mut() = Some(cb);
    }

    /// Register a JS callback invoked when the offline outbox overflows and the
    /// oldest queued write is dropped. Signature:
    /// `cb(droppedId: number, pendingWrites: number)`.
    ///
    /// Without this an overflow is invisible to the application: the write is
    /// discarded, no error is raised, and the data is simply gone. The SDK's
    /// `onOutboxFull()` wires this up automatically.
    pub fn set_outbox_full_callback(&mut self, cb: js_sys::Function) {
        *self.shared.on_outbox_full.borrow_mut() = Some(cb);
    }

    /// Writes queued locally and not yet acknowledged by the server. Poll this
    /// to show sync state, or to back off before the queue overflows.
    pub fn pending_writes(&self) -> usize {
        self.shared.core.borrow().outbox_len()
    }

    /// Register a JS callback invoked when a pub/sub message arrives.
    /// Signature: `cb(channel: string, message: string)`.
    /// The SDK's `onMessage()` wires this up automatically.
    pub fn set_message_callback(&mut self, cb: js_sys::Function) {
        *self.on_message.borrow_mut() = Some(cb);
    }

    /// Open the IndexedDB WAL, replay all stored commands into the in-memory
    /// store, and enable persistence for future writes.
    ///
    /// Call once at startup before reading or writing if you want the cache to
    /// survive page refreshes. Safe to call with no server connection.
    ///
    /// ```js
    /// await cache.enable_persistence();
    /// ```
    pub fn enable_persistence(&self) -> Promise {
        let store = Arc::clone(&self.store);
        let idb_cell = Rc::clone(&self.idb);
        let seq_cell = Rc::clone(&self.seq);
        let core = Rc::clone(&self.shared.core);

        wasm_bindgen_futures::future_to_promise(async move {
            let db = JsFuture::from(open_recached_db()).await?;

            // ── duplicate-suppression identity ───────────────────────────
            // Adopt the stored client id (so dedup high-water marks span
            // sessions) unless this session already sent writes under the
            // random one — switching ids mid-stream would fragment the mark.
            {
                let stored = JsFuture::from(idb_meta_get(&db, "client_id")).await?;
                let adopted = match stored.as_string() {
                    Some(cid) if !cid.is_empty() => core.borrow_mut().adopt_client_id(cid),
                    _ => false,
                };
                if !adopted {
                    let current = core.borrow().client_id().to_string();
                    let _ = JsFuture::from(idb_meta_put(
                        &db,
                        "client_id",
                        &JsValue::from_str(&current),
                    ))
                    .await;
                }
                // Bump the session epoch so this session's dedup ids are
                // strictly above every id the previous session ever sent.
                let prev = JsFuture::from(idb_meta_get(&db, "epoch"))
                    .await?
                    .as_f64()
                    .unwrap_or(0.0) as u32;
                let next = prev.saturating_add(1);
                core.borrow_mut().set_epoch(next);
                let _ = JsFuture::from(idb_meta_put(&db, "epoch", &JsValue::from_f64(next as f64)))
                    .await;
            }

            // Restore the durable outbox: writes from a previous session that
            // never got a server acknowledgment. They re-send on the next
            // (re)connect, *before* writes queued in this session. Restored
            // entries receive fresh ids after anything queued this session.
            // Their durable rows are swapped atomically below, so renumbering
            // cannot overwrite an old row that has not been visited yet.
            {
                let result = JsFuture::from(idb_outbox_read_all(&db)).await?;
                let pair = js_sys::Array::from(&result);
                let keys = js_sys::Array::from(&pair.get(0));
                let vals = js_sys::Array::from(&pair.get(1));
                let mut restored: Vec<(u64, Vec<u8>)> = Vec::with_capacity(keys.length() as usize);
                for i in 0..keys.length() {
                    let id = keys.get(i).as_f64().unwrap_or(0.0) as u64;
                    let raw = vals.get(i);
                    let cmd = match raw.as_string() {
                        // Rows written by 0.2.1 and earlier, when frames were
                        // strings rather than Uint8Array.
                        Some(t) => t.into_bytes(),
                        None => js_sys::Uint8Array::new(&raw).to_vec(),
                    };
                    if !cmd.is_empty() {
                        restored.push((id, cmd));
                    }
                }
                // Collect before awaiting: holding the `RefCell` borrow across an
                // await lets an incoming WebSocket frame re-enter and panic on a
                // double borrow, killing the client mid-hydration.
                let renumbered = core.borrow_mut().restore_outbox(restored);
                let ids = js_sys::Array::new();
                let frames = js_sys::Array::new();
                for (_, new_id, frame) in renumbered {
                    ids.push(&JsValue::from_f64(new_id as f64));
                    frames.push(&js_sys::Uint8Array::from(frame.as_slice()));
                }
                JsFuture::from(idb_outbox_replace_js(&db, &ids, &frames)).await?;
            }

            let result = JsFuture::from(idb_read_all(&db)).await?;
            let pair = js_sys::Array::from(&result);
            let keys = js_sys::Array::from(&pair.get(0));
            let vals = js_sys::Array::from(&pair.get(1));

            let entry_count = keys.length();
            let mut max_seq: u64 = 0;
            for i in 0..entry_count {
                let s = keys.get(i).as_f64().unwrap_or(0.0) as u64;
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
                    if s > max_seq {
                        max_seq = s;
                    }
                }
            }

            // If the WAL grew large, compact: rewrite it as minimal snapshot
            // commands. This keeps startup replay fast regardless of how many
            // writes accumulated between refreshes.
            // Note: there is a brief data-loss window between idb_clear_js and
            // writing the new snapshot. If the tab is closed during compaction,
            // the WAL will be empty on next load and in-memory state is lost.
            let next_seq = if entry_count > WAL_COMPACT_THRESHOLD {
                // WAL only — the outbox holds writes still awaiting server
                // acknowledgment and must survive compaction.
                // Atomic: the old log is only dropped if the replacement
                // commits with it. Previously this cleared first and wrote
                // after, so an interruption in between left an empty WAL and
                // the persisted cache was gone.
                let cmds = snapshot_to_resp_cmds(&store.snapshot());
                let arr = js_sys::Array::new();
                for cmd in &cmds {
                    arr.push(&js_sys::Uint8Array::from(&cmd[..]));
                }
                JsFuture::from(idb_wal_replace_js(&db, &arr)).await?;
                cmds.len() as u64
            } else if entry_count == 0 {
                0
            } else {
                max_seq + 1
            };

            seq_cell.set(next_seq);
            *idb_cell.borrow_mut() = Some(db);

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
        let on_mut = Rc::clone(&self.on_mutation);

        let onbc = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Some(raw) = event_frame_bytes(&e)
                && let Ok((value, _)) = Value::parse(&raw)
                && let Ok(cmd) = Command::from_value(value)
                && is_replayable_mutation(&cmd)
            {
                store_clone.execute(cmd);
                notify_mutation(&on_mut);
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

    /// Send a session command now if the socket is open; otherwise it will be
    /// sent by `onopen` from the stored session state — session commands must
    /// not sit in the offline write queue or they would be sent twice.
    /// True when the socket exists and is OPEN.
    fn socket_open(&self) -> bool {
        matches!(
            self.shared.ws.borrow().as_ref(),
            Some(ws) if ws.ready_state() == WebSocket::OPEN
        )
    }

    /// Send a session frame the core produced (it has already recorded the
    /// inflight reply slot).
    fn send_session_frame(&self, frame: Option<Vec<u8>>) {
        if let Some(frame) = frame
            && let Some(ws) = self.shared.ws.borrow().as_ref()
        {
            ws_send_frame(ws, &frame);
        }
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
    pub fn incr_by(&self, key: &str, delta: i64) -> Result<i64, JsValue> {
        let resp = self.store.execute(Command::IncrBy(key.to_string(), delta));
        let n = match resp {
            Value::Integer(n) => n,
            Value::Error(e) => return Err(JsValue::from_str(&e)),
            _ => 0,
        };
        let delta_s = delta.to_string();
        let encoded = to_resp(&["INCRBY", key, &delta_s]);
        self.ws_enqueue(&encoded);
        if let Some(bc) = &self.bc {
            bc_post(bc, &encoded);
        }
        persist_cmd(&self.idb, &self.seq, &encoded);
        notify_mutation(&self.on_mutation);
        Ok(n)
    }

    /// Set JSON at a path (`"$"` = whole document) — locally, on the server,
    /// and across tabs. `value` must be valid JSON text. Returns "OK" or an
    /// error string.
    pub fn jset(&self, key: &str, path: &str, value: &str) -> String {
        let resp = self.store.execute(Command::JSet(
            key.to_string(),
            path.to_string(),
            value.to_string(),
        ));
        if let Value::Error(e) = resp {
            return e;
        }
        let encoded = to_resp(&["JSET", key, path, value]);
        self.ws_enqueue(&encoded);
        if let Some(bc) = &self.bc {
            bc_post(bc, &encoded);
        }
        persist_cmd(&self.idb, &self.seq, &encoded);
        notify_mutation(&self.on_mutation);
        "OK".to_string()
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
    /// deletes the document. Returns "OK" or an error string.
    pub fn jmerge(&self, key: &str, patch: &str) -> String {
        let resp = self
            .store
            .execute(Command::JMerge(key.to_string(), patch.to_string()));
        if let Value::Error(e) = resp {
            return e;
        }
        let encoded = to_resp(&["JMERGE", key, patch]);
        self.ws_enqueue(&encoded);
        if let Some(bc) = &self.bc {
            bc_post(bc, &encoded);
        }
        persist_cmd(&self.idb, &self.seq, &encoded);
        notify_mutation(&self.on_mutation);
        "OK".to_string()
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

    /// Set a key-value pair locally, sync to the server, and fan out to other tabs if broadcast() was called.
    pub fn set(&self, key: &str, value: &str) -> String {
        let resp = self.store.execute(Command::Set(
            key.to_string(),
            value.as_bytes().to_vec(),
            SetOptions::default(),
        ));

        let encoded = to_resp(&["SET", key, value]);
        self.ws_enqueue(&encoded);
        if let Some(bc) = &self.bc {
            bc_post(bc, &encoded);
        }
        persist_cmd(&self.idb, &self.seq, &encoded);
        notify_mutation(&self.on_mutation);

        match resp {
            Value::SimpleString(s) => s,
            Value::Error(e) => e,
            _ => "ERR".to_string(),
        }
    }

    /// Set a key to raw bytes, synced to the server and other tabs.
    ///
    /// Values are byte-transparent, so this stores and replicates exactly the
    /// bytes given — compressed payloads, protobuf, images. `set()` is the
    /// right call for text; this exists for everything a JS string cannot hold.
    #[wasm_bindgen(js_name = "setBytes")]
    pub fn set_bytes(&self, key: &str, value: &[u8]) -> String {
        let resp = self.store.execute(Command::Set(
            key.to_string(),
            value.to_vec(),
            SetOptions::default(),
        ));

        let encoded = to_resp_bytes(&[b"SET", key.as_bytes(), value]);
        self.ws_enqueue(&encoded);
        if let Some(bc) = &self.bc {
            bc_post(bc, &encoded);
        }
        persist_cmd(&self.idb, &self.seq, &encoded);
        notify_mutation(&self.on_mutation);

        match resp {
            Value::SimpleString(s) => s,
            Value::Error(e) => e,
            _ => "ERR".to_string(),
        }
    }

    /// Set a key with a TTL in seconds, synced to the server and other tabs if broadcast() was called.
    pub fn set_ex(&self, key: &str, value: &str, seconds: u32) -> String {
        let opts = SetOptions {
            expiry: Some(core_engine::cmd::SetExpiry::Ex(seconds as u64)),
            ..Default::default()
        };
        let resp = self.store.execute(Command::Set(
            key.to_string(),
            value.as_bytes().to_vec(),
            opts,
        ));

        let encoded = to_resp(&["SET", key, value, "EX", &seconds.to_string()]);
        self.ws_enqueue(&encoded);
        if let Some(bc) = &self.bc {
            bc_post(bc, &encoded);
        }
        persist_cmd(&self.idb, &self.seq, &encoded);
        notify_mutation(&self.on_mutation);

        match resp {
            Value::SimpleString(s) => s,
            Value::Error(e) => e,
            _ => "ERR".to_string(),
        }
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

    /// Delete a key locally, sync to the server, and fan out to other tabs if broadcast() was called.
    pub fn del(&self, key: &str) -> i32 {
        let resp = self.store.execute(Command::Del(vec![key.to_string()]));

        let encoded = to_resp(&["DEL", key]);
        self.ws_enqueue(&encoded);
        if let Some(bc) = &self.bc {
            bc_post(bc, &encoded);
        }
        persist_cmd(&self.idb, &self.seq, &encoded);
        notify_mutation(&self.on_mutation);

        match resp {
            Value::Integer(i) => i as i32,
            _ => 0,
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
    pub fn publish(&self, channel: &str, message: &str) {
        self.ws_enqueue_nodedup(&to_resp(&["PUBLISH", channel, message]));
    }

    /// Publish raw bytes to a channel on the server.
    ///
    /// Subscribers receive a `Uint8Array` rather than a string when the payload
    /// is not valid UTF-8.
    #[wasm_bindgen(js_name = "publishBytes")]
    pub fn publish_bytes(&self, channel: &str, message: &[u8]) {
        self.ws_enqueue_nodedup(&to_resp_bytes(&[b"PUBLISH", channel.as_bytes(), message]));
    }

    /// Subscribe to a channel on the server. Push messages arrive via the `onmessage` callback.
    pub fn subscribe(&self, channel: &str) {
        self.ws_enqueue_nodedup(&to_resp(&["SUBSCRIBE", channel]));
    }

    /// Unsubscribe from a channel on the server.
    pub fn unsubscribe(&self, channel: &str) {
        self.ws_enqueue_nodedup(&to_resp(&["UNSUBSCRIBE", channel]));
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
    fn strings_with_a_ttl_carry_remaining_time_not_absolute() {
        // PX is relative: persisting an absolute timestamp here would set a TTL
        // decades in the future.
        let exp = now_ms() + 60_000;
        let s = joined(&[entry("k", SnapshotValue::Str("v".into()), Some(exp))]);
        assert!(s.contains("PX"), "{s}");
        assert!(
            !s.contains(&exp.to_string()),
            "absolute expiry leaked into a relative PX: {s}"
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

    /// `(keys, values)` from a `readAll`-shaped JS result.
    fn split_pairs(res: &JsValue) -> (Vec<f64>, Vec<String>) {
        let arr = js_sys::Array::from(res);
        let keys = js_sys::Array::from(&arr.get(0));
        let vals = js_sys::Array::from(&arr.get(1));
        (
            keys.iter().map(|k| k.as_f64().unwrap_or(-1.0)).collect(),
            vals.iter()
                .map(|v| match v.as_string() {
                    Some(t) => t,
                    // WAL frames are stored as Uint8Array so binary values
                    // persist; these tests all use text payloads.
                    None => {
                        String::from_utf8_lossy(&js_sys::Uint8Array::new(&v).to_vec()).into_owned()
                    }
                })
                .collect(),
        )
    }

    async fn wal_rows(db: &JsValue) -> (Vec<f64>, Vec<String>) {
        let res = JsFuture::from(idb_read_all(db)).await.expect("wal read");
        split_pairs(&res)
    }

    async fn outbox_rows(db: &JsValue) -> (Vec<f64>, Vec<String>) {
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
        use core_engine::cmd::{Command, SetOptions};
        use core_engine::store::KeyValueStore;

        let store = KeyValueStore::new();
        assert_eq!(
            store.execute(Command::Set("k".into(), "v".into(), SetOptions::default())),
            core_engine::resp::Value::SimpleString("OK".into())
        );
        assert_eq!(
            store.execute(Command::Get("k".into())),
            core_engine::resp::Value::BulkString(Some(b"v".to_vec()))
        );
        // TTL paths read the clock twice — set and check.
        store.execute(Command::Set(
            "ttl".into(),
            "v".into(),
            SetOptions {
                expiry: Some(core_engine::cmd::SetExpiry::Ex(60)),
                ..Default::default()
            },
        ));
        match store.execute(Command::Ttl("ttl".into())) {
            core_engine::resp::Value::Integer(n) => assert!(n > 0, "expected a live TTL, got {n}"),
            other => panic!("unexpected TTL reply: {other:?}"),
        }
    }

    // ── Schema ────────────────────────────────────────────────────────────────

    #[wasm_bindgen_test]
    async fn database_opens_with_all_three_stores() {
        // A missing store means an upgrade path was skipped and every write to
        // it throws at runtime.
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
            assert!(
                has.as_bool().unwrap_or(false),
                "missing object store: {store}"
            );
        }
    }

    // ── WAL ───────────────────────────────────────────────────────────────────

    #[wasm_bindgen_test]
    async fn wal_appends_are_read_back_in_sequence_order() {
        // Replay order is the whole point of the WAL: applying writes out of
        // order reconstructs the wrong state on reload.
        let db = fresh_db().await;
        for (seq, cmd) in [(1.0, "SET a 1"), (2.0, "SET b 2"), (3.0, "SET a 3")] {
            JsFuture::from(idb_append_js(&db, seq, cmd.as_bytes()))
                .await
                .expect("append");
        }
        let (keys, vals) = wal_rows(&db).await;
        assert_eq!(keys, vec![1.0, 2.0, 3.0], "IndexedDB returns keys sorted");
        assert_eq!(vals[2], "SET a 3", "last write for a key must come last");
    }

    #[wasm_bindgen_test]
    async fn wal_clear_leaves_the_outbox_intact() {
        // Compaction clears the WAL only. Wiping the outbox here would discard
        // writes the server has not acknowledged yet — silent data loss.
        let db = fresh_db().await;
        JsFuture::from(idb_append_js(&db, 1.0, b"SET a 1"))
            .await
            .expect("append");
        JsFuture::from(idb_outbox_put_js(&db, 10.0, b"PENDING WRITE"))
            .await
            .expect("outbox put");

        JsFuture::from(idb_wal_clear_js(&db))
            .await
            .expect("wal clear");

        assert!(wal_rows(&db).await.0.is_empty(), "WAL should be empty");
        let (ids, _) = outbox_rows(&db).await;
        assert_eq!(ids, vec![10.0], "pending write must survive WAL compaction");
    }

    #[wasm_bindgen_test]
    async fn full_clear_empties_every_store() {
        let db = open_db().await;
        JsFuture::from(idb_append_js(&db, 1.0, b"SET a 1"))
            .await
            .unwrap();
        JsFuture::from(idb_outbox_put_js(&db, 1.0, b"SET b 2"))
            .await
            .unwrap();

        JsFuture::from(idb_clear_js(&db)).await.expect("clear");

        assert!(wal_rows(&db).await.0.is_empty());
        assert!(outbox_rows(&db).await.0.is_empty());
    }

    // ── Outbox ────────────────────────────────────────────────────────────────

    #[wasm_bindgen_test]
    async fn outbox_rows_survive_a_reopen() {
        // The durability guarantee: a write made offline must still be there
        // after the page is reloaded and the database reopened.
        let db = fresh_db().await;
        JsFuture::from(idb_outbox_put_js(&db, 7.0, b"SET offline 1"))
            .await
            .expect("put");
        drop(db);

        let db2 = open_db().await;
        let (ids, vals) = outbox_rows(&db2).await;
        assert_eq!(ids, vec![7.0]);
        assert_eq!(vals[0], "SET offline 1");
    }

    #[wasm_bindgen_test]
    async fn deleting_an_acknowledged_row_leaves_the_others() {
        let db = fresh_db().await;
        for (id, cmd) in [(1.0, "SET a 1"), (2.0, "SET b 2"), (3.0, "SET c 3")] {
            JsFuture::from(idb_outbox_put_js(&db, id, cmd.as_bytes()))
                .await
                .unwrap();
        }
        JsFuture::from(idb_outbox_delete_js(&db, 2.0))
            .await
            .expect("delete");

        let (ids, _) = outbox_rows(&db).await;
        assert_eq!(ids, vec![1.0, 3.0], "only the acknowledged row is retired");
    }

    #[wasm_bindgen_test]
    async fn putting_the_same_id_replaces_rather_than_duplicates() {
        // IndexedDB outbox keys are unique; putting the same id must replace
        // rather than leave two copies that would replay twice.
        let db = fresh_db().await;
        JsFuture::from(idb_outbox_put_js(&db, 5.0, b"FIRST"))
            .await
            .unwrap();
        JsFuture::from(idb_outbox_put_js(&db, 5.0, b"SECOND"))
            .await
            .unwrap();

        let (ids, vals) = outbox_rows(&db).await;
        assert_eq!(ids.len(), 1, "same key must overwrite");
        assert_eq!(vals[0], "SECOND");
    }

    #[wasm_bindgen_test]
    async fn outbox_replace_atomically_installs_renumbered_rows() {
        let db = fresh_db().await;
        for (id, cmd) in [(7.0, b"OLD A".as_slice()), (8.0, b"OLD B".as_slice())] {
            JsFuture::from(idb_outbox_put_js(&db, id, cmd))
                .await
                .unwrap();
        }

        let ids = js_sys::Array::new();
        ids.push(&JsValue::from_f64(1.0));
        ids.push(&JsValue::from_f64(2.0));
        let frames = js_sys::Array::new();
        frames.push(&js_sys::Uint8Array::from(b"NEW A".as_slice()));
        frames.push(&js_sys::Uint8Array::from(b"NEW B".as_slice()));
        JsFuture::from(idb_outbox_replace_js(&db, &ids, &frames))
            .await
            .expect("atomic outbox replacement");

        let (stored_ids, values) = outbox_rows(&db).await;
        assert_eq!(stored_ids, vec![1.0, 2.0]);
        assert_eq!(values, vec!["NEW A".to_string(), "NEW B".to_string()]);
    }

    #[wasm_bindgen_test]
    async fn deleting_a_missing_row_is_not_an_error() {
        // Acknowledgements can race a restore; a delete for an already-retired
        // id must not reject and abort the hydration loop.
        let db = fresh_db().await;
        JsFuture::from(idb_outbox_delete_js(&db, 999.0))
            .await
            .expect("deleting a missing row should resolve");
    }

    // ── Atomic WAL compaction ─────────────────────────────────────────────────

    #[wasm_bindgen_test]
    async fn wal_replace_swaps_contents_in_one_transaction() {
        let db = fresh_db().await;
        for (seq, cmd) in [(0.0, "SET a 1"), (1.0, "SET b 2"), (2.0, "SET c 3")] {
            JsFuture::from(idb_append_js(&db, seq, cmd.as_bytes()))
                .await
                .unwrap();
        }

        let arr = js_sys::Array::new();
        arr.push(&JsValue::from_str("SET compacted 1"));
        JsFuture::from(idb_wal_replace_js(&db, &arr))
            .await
            .expect("replace");

        let (keys, vals) = wal_rows(&db).await;
        assert_eq!(keys, vec![0.0], "old entries must be gone");
        assert_eq!(vals, vec!["SET compacted 1".to_string()]);
    }

    #[wasm_bindgen_test]
    async fn wal_replace_with_an_empty_snapshot_empties_the_log() {
        // An empty store compacts to zero commands; the WAL should end empty
        // rather than retaining stale entries.
        let db = fresh_db().await;
        JsFuture::from(idb_append_js(&db, 0.0, b"SET a 1"))
            .await
            .unwrap();

        let empty = js_sys::Array::new();
        JsFuture::from(idb_wal_replace_js(&db, &empty))
            .await
            .expect("replace");

        assert!(wal_rows(&db).await.0.is_empty());
    }

    #[wasm_bindgen_test]
    async fn wal_replace_leaves_the_outbox_untouched() {
        // Compaction must never drop writes still awaiting acknowledgement.
        let db = fresh_db().await;
        JsFuture::from(idb_append_js(&db, 0.0, b"SET a 1"))
            .await
            .unwrap();
        JsFuture::from(idb_outbox_put_js(&db, 5.0, b"PENDING"))
            .await
            .unwrap();

        let arr = js_sys::Array::new();
        arr.push(&JsValue::from_str("SET compacted 1"));
        JsFuture::from(idb_wal_replace_js(&db, &arr)).await.unwrap();

        let (ids, vals) = outbox_rows(&db).await;
        assert_eq!(ids, vec![5.0]);
        assert_eq!(vals[0], "PENDING");
    }

    #[wasm_bindgen_test]
    async fn binary_writes_survive_the_outbox() {
        // An offline write is stored in IndexedDB and replayed on reconnect. A
        // binary value has to come back out of that queue byte-identical, or
        // the replayed write differs from the one the application made.
        let db = fresh_db().await;
        let binary = vec![0xffu8, 0xfe, 0x00, 0x41, 0x80];
        let frame = to_resp_bytes(&[b"SET", b"k", &binary]);

        JsFuture::from(idb_outbox_put_js(&db, 0.0, &frame))
            .await
            .expect("outbox put");

        let res = JsFuture::from(idb_outbox_read_all(&db))
            .await
            .expect("outbox read");
        let pair = js_sys::Array::from(&res);
        let vals = js_sys::Array::from(&pair.get(1));
        let raw = vals.get(0);
        let stored = match raw.as_string() {
            Some(t) => t.into_bytes(),
            None => js_sys::Uint8Array::new(&raw).to_vec(),
        };

        assert_eq!(stored, frame, "outbox row must round-trip unchanged");

        // And it still parses into the write the application made.
        let (value, _) = core_engine::resp::Value::parse(&stored).unwrap();
        let store = KeyValueStore::new();
        store.execute(Command::from_value(value).unwrap());
        assert_eq!(
            store.execute(Command::Get("k".into())),
            Value::BulkString(Some(binary))
        );
    }

    #[wasm_bindgen_test]
    async fn binary_values_survive_the_wal() {
        // A backend can write a binary value that reaches this browser through
        // a live query. The WAL persists frames in IndexedDB, so it has to
        // carry those bytes intact — a UTF-8 round trip here would corrupt on
        // reload exactly the values the store holds correctly in memory.
        let db = fresh_db().await;
        let binary = vec![0xffu8, 0xfe, 0x00, 0x41, 0x80];

        let source = KeyValueStore::new();
        source.execute(Command::Set(
            "b".into(),
            binary.clone(),
            core_engine::cmd::SetOptions::default(),
        ));

        let cmds = snapshot_to_resp_cmds(&source.snapshot());
        let arr = js_sys::Array::new();
        for c in &cmds {
            arr.push(&js_sys::Uint8Array::from(&c[..]));
        }
        JsFuture::from(idb_wal_replace_js(&db, &arr)).await.unwrap();

        // Read the raw rows back and replay them, as a page load would.
        let res = JsFuture::from(idb_read_all(&db)).await.expect("wal read");
        let pair = js_sys::Array::from(&res);
        let vals = js_sys::Array::from(&pair.get(1));
        let restored = KeyValueStore::new();
        for i in 0..vals.length() {
            let raw = vals.get(i);
            let bytes = match raw.as_string() {
                Some(t) => t.into_bytes(),
                None => js_sys::Uint8Array::new(&raw).to_vec(),
            };
            let (value, _) = core_engine::resp::Value::parse(&bytes).unwrap();
            restored.execute(Command::from_value(value).unwrap());
        }

        assert_eq!(
            restored.execute(Command::Get("b".into())),
            Value::BulkString(Some(binary)),
            "binary value must survive the WAL round trip"
        );
    }

    #[wasm_bindgen_test]
    async fn compacted_wal_replays_to_the_same_state() {
        // End to end: snapshot a store, compact through the atomic path, read
        // back, and rebuild — the persisted cache must be intact.
        use core_engine::cmd::Command;
        use core_engine::store::KeyValueStore;

        let db = fresh_db().await;
        let source = KeyValueStore::new();
        source.execute(Command::Set(
            "k".into(),
            "v".into(),
            core_engine::cmd::SetOptions::default(),
        ));
        source.execute(Command::RPush("l".into(), vec!["a".into(), "b".into()]));

        let cmds = snapshot_to_resp_cmds(&source.snapshot());
        let arr = js_sys::Array::new();
        for c in &cmds {
            arr.push(&js_sys::Uint8Array::from(&c[..]));
        }
        JsFuture::from(idb_wal_replace_js(&db, &arr)).await.unwrap();

        let (_, vals) = wal_rows(&db).await;
        let restored = KeyValueStore::new();
        for frame in &vals {
            let (value, _) = core_engine::resp::Value::parse(frame.as_bytes()).unwrap();
            restored.execute(Command::from_value(value).unwrap());
        }
        assert_eq!(
            restored.execute(Command::Get("k".into())),
            core_engine::resp::Value::BulkString(Some(b"v".to_vec()))
        );
        assert_eq!(
            restored.execute(Command::LLen("l".into())),
            core_engine::resp::Value::Integer(2)
        );
    }

    // ── Meta (client identity + epoch) ────────────────────────────────────────

    #[wasm_bindgen_test]
    async fn meta_round_trips_the_client_identity() {
        // The client id and epoch suppress duplicate delivery across
        // reloads; losing them re-issues ids the server has already seen.
        let db = fresh_db().await;
        JsFuture::from(idb_meta_put(
            &db,
            "client_id",
            &JsValue::from_str("abc-123"),
        ))
        .await
        .expect("meta put");
        let got = JsFuture::from(idb_meta_get(&db, "client_id"))
            .await
            .expect("meta get");
        assert_eq!(got.as_string().as_deref(), Some("abc-123"));
    }

    #[wasm_bindgen_test]
    async fn meta_stores_a_numeric_epoch() {
        let db = fresh_db().await;
        JsFuture::from(idb_meta_put(&db, "epoch", &JsValue::from_f64(4.0)))
            .await
            .expect("meta put");
        let got = JsFuture::from(idb_meta_get(&db, "epoch"))
            .await
            .expect("get");
        assert_eq!(got.as_f64(), Some(4.0));
    }

    #[wasm_bindgen_test]
    async fn missing_meta_reads_as_undefined_not_an_error() {
        // First run on a new browser: absence must be a normal value the caller
        // can branch on, not a rejected promise.
        let db = fresh_db().await;
        let got = JsFuture::from(idb_meta_get(&db, "never_written"))
            .await
            .expect("missing key should resolve");
        assert!(got.is_undefined() || got.is_null(), "got {got:?}");
    }

    // ── WAL + outbox interaction ──────────────────────────────────────────────

    #[wasm_bindgen_test]
    async fn a_snapshot_written_to_the_wal_replays_into_a_store() {
        // End-to-end for the hydration path: encode a snapshot the way
        // compaction does, persist it, read it back, and rebuild the cache.
        use core_engine::cmd::Command;
        use core_engine::store::KeyValueStore;

        let db = fresh_db().await;
        let cmds = snapshot_to_resp_cmds(&[SnapshotEntry {
            key: "hydrated".into(),
            value: SnapshotValue::Str("value".into()),
            expires_at_ms: None,
        }]);
        for (i, cmd) in cmds.iter().enumerate() {
            JsFuture::from(idb_append_js(&db, i as f64, cmd))
                .await
                .expect("append");
        }

        let (_, vals) = wal_rows(&db).await;
        let store = KeyValueStore::new();
        for frame in &vals {
            let (value, _) = core_engine::resp::Value::parse(frame.as_bytes()).unwrap();
            store.execute(Command::from_value(value).unwrap());
        }
        assert_eq!(
            store.execute(Command::Get("hydrated".into())),
            core_engine::resp::Value::BulkString(Some(b"value".to_vec()))
        );
    }
}
