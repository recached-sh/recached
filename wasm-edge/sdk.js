// ── Types ─────────────────────────────────────────────────────────────────────
/**
 * Thrown by a write when the queue of writes waiting for the server is full.
 * Nothing was applied, stored or sent, and every queued write is intact.
 * Retry once {@link Cache.pendingWrites} drops — the queue drains as the
 * server acknowledges writes.
 */
export class OutboxFullError extends Error {
    constructor(pending, capacity, message) {
        super(message ?? `outbox full: ${pending} writes pending, capacity ${capacity}`);
        this.name = 'OutboxFullError';
        this.pending = pending;
        this.capacity = capacity;
    }
}
/** Rethrow the wasm side's `OutboxFullError`-named error as the class above. */
function rethrow(e) {
    if (e instanceof Error && e.name === 'OutboxFullError') {
        const fields = e;
        throw new OutboxFullError(fields.pending ?? 0, fields.capacity ?? 0, e.message);
    }
    throw e;
}
/** Run a write, turning a full-outbox refusal into {@link OutboxFullError}. */
function guarded(write) {
    try {
        return write();
    }
    catch (e) {
        rethrow(e);
    }
}
let _module = null;
let _initPromise = null;
async function ensureModule() {
    if (_module)
        return _module;
    if (!_initPromise) {
        _initPromise = (async () => {
            const mod = await import('./pkg/recached_edge.js');
            await mod.default(); // initialise WASM — idempotent on repeated calls
            _module = mod;
            return mod;
        })();
    }
    return _initPromise;
}
/** Add `cb` under `name`, returning a remover that drops the entry once empty. */
function addListener(registry, name, cb) {
    let listeners = registry.get(name);
    if (!listeners)
        registry.set(name, (listeners = new Set()));
    listeners.add(cb);
    return () => {
        const current = registry.get(name);
        if (!current)
            return;
        current.delete(cb);
        if (current.size === 0)
            registry.delete(name);
    };
}
// ── Cache ─────────────────────────────────────────────────────────────────────
/**
 * A typed wrapper around the Recached WASM cache.
 *
 * Obtain an instance via {@link createCache} rather than `new Cache()`.
 *
 * ```ts
 * const cache = await createCache({ persistence: true });
 * cache.set('theme', 'dark');
 * cache.get('theme'); // "dark"
 * ```
 */
export class Cache {
    /** @internal */
    constructor(raw) {
        this._mutationListeners = new Set();
        this._keyListeners = new Map();
        this._patternListeners = new Map();
        this._messageListeners = new Map();
        this._outboxFullListeners = new Set();
        this._statusListeners = new Set();
        this._subscriptionListeners = new Set();
        this._refusedListeners = new Set();
        this._persistenceErrorListeners = new Set();
        /**
         * @internal Arrow function so `this` is always bound when passed as a
         * callback. `keys` names every key the mutation may have changed.
         */
        this._notifyMutation = (keys) => {
            for (const cb of this._mutationListeners)
                cb(keys);
            if (keys.length === 0)
                return;
            for (const key of keys) {
                const listeners = this._keyListeners.get(key);
                if (listeners)
                    for (const cb of listeners)
                        cb();
            }
            // One wasm call per pattern, not per key: a reconnect snapshot can name
            // thousands of keys.
            for (const [pattern, listeners] of this._patternListeners) {
                if (this.raw.anyKeyMatches(pattern, keys))
                    for (const cb of listeners)
                        cb();
            }
        };
        /** @internal */
        this._notifyOutboxFull = (droppedId, pending) => {
            for (const cb of this._outboxFullListeners)
                cb(droppedId, pending);
        };
        /** @internal */
        this._notifyMessage = (channel, message) => {
            const listeners = this._messageListeners.get(channel);
            if (listeners) {
                for (const cb of listeners)
                    cb(message);
            }
        };
        /** @internal Ref-counts per live-query pattern so components sharing a
         * pattern don't cancel each other's server subscription. */
        this._liveQueryRefs = new Map();
        this.raw = raw;
        raw.set_mutation_callback(this._notifyMutation);
        raw.set_message_callback(this._notifyMessage);
        raw.set_outbox_full_callback(this._notifyOutboxFull);
        raw.set_status_callback((status) => {
            for (const cb of this._statusListeners)
                cb(status);
        });
        raw.set_subscription_callback((pattern, state) => {
            for (const cb of this._subscriptionListeners)
                cb(pattern, state);
        });
        raw.set_refused_callback((keys, unreconciled, reason) => {
            for (const cb of this._refusedListeners)
                cb(keys, unreconciled, reason);
        });
        raw.set_persistence_error_callback((operation, message) => {
            for (const cb of this._persistenceErrorListeners)
                cb(operation, message);
        });
    }
    // ── Synchronization status ────────────────────────────────────────────────
    /**
     * Where synchronization stands. `phase` is `'ready'` only once the session
     * is authenticated, every queued write has been replayed and answered, and
     * every live query has its snapshot — an open socket alone is not enough.
     */
    status() {
        return this.raw.sync_status();
    }
    /**
     * Called on every status change, as it happens — never on a timer.
     * Returns an unsubscribe function.
     *
     * ```ts
     * cache.onStatusChange(({ phase, error }) => {
     *   badge.textContent = phase === 'ready' ? 'Synced' : phase;
     *   if (error?.kind === 'auth-failed') signIn();
     * });
     * ```
     */
    onStatusChange(cb) {
        this._statusListeners.add(cb);
        return () => this._statusListeners.delete(cb);
    }
    /** One live query's progress, or `null` when the pattern is not registered. */
    subscriptionState(pattern) {
        return this.raw.subscription_state(pattern);
    }
    /**
     * Called when a live query's progress changes: `state` is `null` once it is
     * stopped. Returns an unsubscribe function.
     */
    onSubscriptionChange(cb) {
        this._subscriptionListeners.add(cb);
        return () => this._subscriptionListeners.delete(cb);
    }
    /**
     * Called when the server refuses a queued write for good — out of the sync
     * token's scope, read-only, the wrong type. The write never reached the
     * server. Keys a live query covers get the server's values back shortly
     * after; `unreconciled` lists the rest, whose local value is the refused
     * one and stays so until the application rewrites or re-reads them.
     *
     * Returns an unsubscribe function.
     */
    onWriteRefused(cb) {
        this._refusedListeners.add(cb);
        return () => this._refusedListeners.delete(cb);
    }
    /**
     * Called when an IndexedDB write fails — quota exceeded, storage cleared,
     * access denied. The change it carried is in memory and syncs while the page
     * stays open, but would not survive a reload. Returns an unsubscribe
     * function.
     */
    onPersistenceError(cb) {
        this._persistenceErrorListeners.add(cb);
        return () => this._persistenceErrorListeners.delete(cb);
    }
    /**
     * Resolves once every write made so far is committed to IndexedDB, and
     * rejects with the first IndexedDB failure since the last `flush()`.
     *
     * Writes return as soon as the change is in memory; this is how to know it
     * is on disk. A committed IndexedDB transaction survives a reload or a
     * closed tab, but browsers do not promise it survives a power cut.
     *
     * ```ts
     * cache.set('draft', text);
     * await cache.flush(); // on disk now
     * ```
     */
    flush() {
        return this.raw.flush();
    }
    /**
     * Subscribe to store mutations from any source — local writes, server
     * WebSocket push, and BroadcastChannel cross-tab sync. The callback receives
     * the keys the mutation may have changed.
     *
     * Returns an unsubscribe function. Pass directly to React's
     * `useSyncExternalStore` `subscribe` parameter. To watch one key or one
     * pattern, prefer {@link onKeyChange} or {@link onPatternChange}, which skip
     * mutations to every other key.
     *
     * ```ts
     * useSyncExternalStore(
     *   cache.onMutation.bind(cache),
     *   () => cache.get('key'),
     *   () => null,
     * );
     * ```
     */
    onMutation(cb) {
        this._mutationListeners.add(cb);
        return () => this._mutationListeners.delete(cb);
    }
    /**
     * Subscribe to changes to one key, from any source. The callback runs only
     * when that key is written, deleted, or replaced by a server snapshot — not
     * on writes to other keys.
     *
     * Returns an unsubscribe function.
     *
     * ```ts
     * const stop = cache.onKeyChange('theme', () => render(cache.get('theme')));
     * ```
     */
    onKeyChange(key, cb) {
        return addListener(this._keyListeners, key, cb);
    }
    /**
     * Subscribe to changes to any key matching a glob pattern, using the same
     * matching rules as {@link liveQuery}. The callback runs once per mutation
     * that touches at least one matching key.
     *
     * This only listens; call {@link liveQuery} as well to have the server keep
     * the matching keys in sync.
     *
     * Returns an unsubscribe function.
     *
     * ```ts
     * const stop = cache.onPatternChange('cart:42:*', () => {
     *   render(cache.getMatching('cart:42:*'));
     * });
     * ```
     */
    onPatternChange(pattern, cb) {
        return addListener(this._patternListeners, pattern, cb);
    }
    /**
     * Only with `outbox: { whenFull: 'evict-oldest' }`: called when the queue
     * of writes waiting for the server overflows and the **oldest** queued write
     * is discarded. That write never reaches the server.
     *
     * By default a full queue instead throws {@link OutboxFullError} from the
     * write that does not fit, before it changes anything, and this is never
     * called.
     *
     * Returns an unsubscribe function.
     *
     * ```ts
     * cache.onOutboxFull((droppedId, pending) => {
     *   console.error(`dropped queued write ${droppedId}; ${pending} still pending`);
     * });
     * ```
     */
    onOutboxFull(cb) {
        this._outboxFullListeners.add(cb);
        return () => this._outboxFullListeners.delete(cb);
    }
    /**
     * Writes queued locally and not yet acknowledged by the server.
     *
     * Useful for a "syncing…" indicator, or to apply back-pressure before the
     * queue reaches its cap (10,000 by default), past which writes throw
     * {@link OutboxFullError}.
     */
    pendingWrites() {
        return this.raw.pending_writes();
    }
    /**
     * Subscribe to pub/sub messages on `channel`.
     *
     * Returns an unsubscribe function. Call it to stop receiving messages.
     * Does not send `UNSUBSCRIBE` to the server — use {@link unsubscribe} for that.
     *
     * ```ts
     * cache.subscribe('notifications');
     * const stop = cache.onMessage('notifications', (msg) => console.log(msg));
     * // later:
     * stop();
     * cache.unsubscribe('notifications');
     * ```
     */
    onMessage(channel, cb) {
        let listeners = this._messageListeners.get(channel);
        if (!listeners) {
            listeners = new Set();
            this._messageListeners.set(channel, listeners);
        }
        listeners.add(cb);
        return () => {
            listeners.delete(cb);
            if (listeners.size === 0)
                this._messageListeners.delete(channel);
        };
    }
    // ── Reads ─────────────────────────────────────────────────────────────────
    /**
     * Return the value for `key`, or `null` if the key does not exist or has expired.
     *
     * Always served from local WASM memory — zero network latency.
     *
     * @throws if the stored value is not valid UTF-8. Values are byte-transparent,
     * so a backend can write binary that syncs into this cache; returning it as a
     * mangled string would be worse than failing. Use {@link getBytes} for those.
     */
    get(key) {
        return this.raw.get(key) ?? null;
    }
    /**
     * Return the value for `key` as raw bytes, or `null` if it does not exist.
     *
     * Use this for values a backend wrote as binary — compressed payloads,
     * protobuf, images — which cannot be represented as a JS string. Text values
     * work here too, as their UTF-8 bytes.
     *
     * ```ts
     * const bytes = cache.getBytes('thumb:42');
     * if (bytes) img.src = URL.createObjectURL(new Blob([bytes]));
     * ```
     *
     * Write them with {@link setBytes}.
     */
    getBytes(key) {
        return this.raw.getBytes(key) ?? null;
    }
    /**
     * Return a JSON-parsed value stored under `key`, or `null` if the key is
     * missing, expired, or not valid JSON.
     *
     * ```ts
     * interface User { id: number; name: string }
     * const user = cache.getJSON<User>('user:42'); // User | null
     * ```
     */
    getJSON(key) {
        let raw;
        try {
            raw = this.get(key);
        }
        catch {
            // Binary value: not JSON by definition, so this is a miss rather than an
            // error — getJSON is documented to return null for anything unparseable.
            return null;
        }
        if (raw === null)
            return null;
        try {
            return JSON.parse(raw);
        }
        catch {
            return null;
        }
    }
    /** Return `true` if `key` exists and has not expired. */
    exists(key) {
        return this.raw.exists(key);
    }
    /**
     * Return the remaining TTL in seconds.
     * - `-1` — key exists with no expiry
     * - `-2` — key does not exist
     */
    ttl(key) {
        return this.raw.ttl(key);
    }
    // ── Writes ────────────────────────────────────────────────────────────────
    /**
     * Store a string value. Syncs to the server and other tabs when connected.
     *
     * Returns once the value is in memory and queued for the server; see
     * {@link flush} for when it is on disk, and {@link onWriteRefused} for a
     * write the server refuses.
     *
     * @throws {OutboxFullError} when the queue of writes waiting for the server
     * is full. Nothing is changed.
     */
    set(key, value) {
        // raw.set fires the mutation callback registered in the constructor, so
        // listeners are already notified — no second _notifyMutation() here.
        guarded(() => this.raw.set(key, value));
    }
    /**
     * Store raw bytes. Syncs to the server and other tabs when connected.
     *
     * Values are byte-transparent: the exact bytes given are stored, replicated,
     * and persisted. Use this for compressed payloads, protobuf, or images —
     * anything a JS string cannot hold. Read them back with {@link getBytes}.
     *
     * ```ts
     * cache.setBytes('thumb:42', new Uint8Array(await blob.arrayBuffer()));
     * ```
     */
    setBytes(key, value) {
        guarded(() => this.raw.setBytes(key, value));
    }
    /**
     * Store a string value with a TTL (seconds). The key is deleted automatically
     * once the TTL elapses.
     */
    setEx(key, value, seconds) {
        guarded(() => this.raw.set_ex(key, value, seconds));
    }
    /**
     * Serialize `value` as JSON and store it under `key`.
     * Pass `ttl` (seconds) to have the key expire automatically.
     *
     * ```ts
     * cache.setJSON('user:42', { id: 42, name: 'Alice' }, 300); // expires in 5 min
     * ```
     */
    setJSON(key, value, ttl) {
        const serialized = JSON.stringify(value);
        if (ttl !== undefined) {
            guarded(() => this.raw.set_ex(key, serialized, ttl));
        }
        else {
            guarded(() => this.raw.set(key, serialized));
        }
    }
    /**
     * Delete `key`.
     *
     * Returns `true` if the key existed, `false` if it did not.
     * Syncs to the server and other tabs when connected.
     */
    del(key) {
        const existed = guarded(() => this.raw.del(key)) === 1;
        return existed;
    }
    /**
     * Increment an integer counter. Returns the new local value.
     *
     * Offline increments queue as *deltas* and merge additively with concurrent
     * increments from other clients when the connection returns — nobody's
     * counts are lost (PN-counter semantics). Throws if the key holds a
     * non-integer value.
     *
     * Counters are 64-bit on the wire; the returned `number` is exact up to
     * `Number.MAX_SAFE_INTEGER` (2^53 - 1) and loses precision beyond it.
     */
    incr(key, by = 1) {
        return Number(guarded(() => this.raw.incr_by(key, BigInt(by))));
    }
    /** Decrement an integer counter. See {@link incr}. */
    decr(key, by = 1) {
        return Number(guarded(() => this.raw.incr_by(key, BigInt(-by))));
    }
    /**
     * Close the server connection and stop reconnecting. Local reads and writes
     * keep working; writes queue and replay on the next connect.
     */
    disconnect() {
        this.raw.disconnect();
    }
    // ── JSON documents ────────────────────────────────────────────────────────
    /**
     * Set part of a JSON document. `path` addresses one location: `"$"` is the
     * whole document, `"$.user.name"` a nested field, `"$.items[2].qty"` an
     * array element. Intermediate objects are created automatically.
     *
     * The value is serialized with `JSON.stringify`. Syncs to the server and
     * other tabs when connected.
     *
     * ```ts
     * cache.jset('doc:42', '$', { title: 'Hello', tags: ['a'] })
     * cache.jset('doc:42', '$.title', 'Hello world')
     * ```
     */
    jset(key, path, value) {
        const err = guarded(() => this.raw.jset(key, path, JSON.stringify(value)));
        if (err !== 'OK')
            throw new Error(err);
    }
    /**
     * Read part of a JSON document from local memory. Returns the parsed value
     * at `path` (default: the whole document), or `null` when the key or path
     * does not exist.
     *
     * ```ts
     * const title = cache.jget<string>('doc:42', '$.title')
     * const doc = cache.jget<Doc>('doc:42')
     * ```
     */
    jget(key, path) {
        const raw = this.raw.jget(key, path);
        if (raw === undefined)
            return null;
        try {
            return JSON.parse(raw);
        }
        catch {
            return null;
        }
    }
    /**
     * Apply an RFC 7386 JSON Merge Patch to a document: objects merge
     * recursively, `null` fields are removed, arrays and scalars are replaced.
     * Only the patch travels over the wire — not the whole document.
     *
     * ```ts
     * cache.jmerge('doc:42', { title: 'New title', draft: null })
     * ```
     */
    jmerge(key, patch) {
        const err = guarded(() => this.raw.jmerge(key, JSON.stringify(patch)));
        if (err !== 'OK')
            throw new Error(err);
    }
    // ── Pub/sub ───────────────────────────────────────────────────────────────
    /**
     * Subscribe to a server pub/sub channel. Push messages arrive via the
     * WebSocket `onmessage` callback.
     */
    subscribe(channel) {
        guarded(() => this.raw.subscribe(channel));
    }
    /** Unsubscribe from a server pub/sub channel. */
    unsubscribe(channel) {
        guarded(() => this.raw.unsubscribe(channel));
    }
    /**
     * Publish a message to a server pub/sub channel. All subscribers — browser
     * and server-side — receive the message.
     */
    publish(channel, message) {
        guarded(() => this.raw.publish(channel, message));
    }
    /**
     * Publish raw bytes to a server pub/sub channel.
     *
     * Subscribers receive a `Uint8Array` rather than a string when the payload is
     * not valid UTF-8 — see {@link onMessage}.
     */
    publishBytes(channel, message) {
        guarded(() => this.raw.publishBytes(channel, message));
    }
    // ── Sync scoping & live queries ───────────────────────────────────────────
    /**
     * Present a signed sync-scope token (strict servers — `RECACHED_SYNC_SECRET`).
     * Mint it on your backend and hand it to the page; see the Sync Scopes docs.
     */
    syncToken(token) {
        this.raw.sync_token(token);
    }
    /**
     * Scope this connection's sync to glob patterns (servers without a sync
     * secret only). A bandwidth filter, not an authorization boundary.
     *
     * An entry may state its access — `'r=catalog:*'` for read-only,
     * `'rw=cart:42:*'` or a bare pattern for read-write — matching the grant
     * notation a signed token uses.
     */
    syncScopes(patterns) {
        this.raw.sync_scopes(patterns.join(','));
    }
    /**
     * Start a live query: the server sends the current state of every key
     * matching `pattern` (merged into the local store), then streams every
     * change to matching keys — including keys created later.
     *
     * Returns a stop function. Calls are ref-counted per pattern: the server
     * subscription ends when the last caller stops.
     *
     * ```ts
     * const stop = cache.liveQuery('cart:42:*');
     * cache.onPatternChange('cart:42:*', () => {
     *   render(cache.getMatching('cart:42:*'));
     * });
     * // later:
     * stop();
     * ```
     */
    liveQuery(pattern) {
        const refs = this._liveQueryRefs.get(pattern) ?? 0;
        this._liveQueryRefs.set(pattern, refs + 1);
        if (refs === 0) {
            this.raw.live_query(pattern);
        }
        let stopped = false;
        return () => {
            if (stopped)
                return;
            stopped = true;
            const now = (this._liveQueryRefs.get(pattern) ?? 1) - 1;
            if (now <= 0) {
                this._liveQueryRefs.delete(pattern);
                this.raw.live_unquery(pattern);
            }
            else {
                this._liveQueryRefs.set(pattern, now);
            }
        };
    }
    /**
     * Snapshot of local keys matching a glob pattern, as `[key, value]` pairs.
     * Keys holding collection types come back as `null` (read those with typed
     * accessors). A value that is not valid UTF-8 comes back as a `Uint8Array`
     * rather than a mangled string, so narrow the type before treating it as text.
     *
     * Served entirely from local WASM memory — zero network latency.
     */
    getMatching(pattern) {
        return this.raw.get_matching(pattern);
    }
    // ── Persistence ───────────────────────────────────────────────────────────
    /**
     * Erase the IndexedDB WAL. The in-memory store is not affected.
     *
     * Use on sign-out so the next session starts with an empty cache.
     */
    clearPersistence() {
        return this.raw.clear_persistence();
    }
}
// ── Public API ────────────────────────────────────────────────────────────────
/**
 * Initialise the WASM module eagerly.
 *
 * `createCache` calls this automatically on first use, so calling `init()`
 * directly is only necessary when you want to front-load the WASM download
 * (e.g. during a loading screen).
 *
 * ```ts
 * import { init, createCache } from 'recached-edge';
 *
 * await init(); // start downloading WASM now
 * // ... other app setup ...
 * const cache = await createCache(); // resolves immediately — already loaded
 * ```
 */
export async function init() {
    await ensureModule();
}
/**
 * Create a {@link Cache} instance.
 *
 * Loads and initialises the WASM module on first call (subsequent calls reuse
 * the existing module). Options are applied in order:
 * `outbox` and `persistSyncedState` → `persistence` → `broadcastChannel` →
 * `connect` + `auth`.
 *
 * ```ts
 * import { createCache } from 'recached-edge';
 *
 * // Local-only, survives refresh
 * const cache = await createCache({ persistence: true });
 *
 * // With server sync
 * const cache = await createCache({
 *   persistence: true,
 *   connect: { url: 'ws://localhost:6380', password: 'secret' },
 * });
 *
 * cache.set('theme', 'dark');
 * cache.getJSON<User>('user:42'); // User | null
 *
 * // --- page refresh ---
 * const cache = await createCache({ persistence: true });
 * cache.get('theme'); // "dark" — restored from IndexedDB, zero network
 * ```
 */
export async function createCache(options = {}) {
    const mod = await ensureModule();
    const raw = new mod.RecachedCache();
    if (options.outbox?.maxPending !== undefined) {
        raw.set_max_pending(options.outbox.maxPending);
    }
    if (options.outbox?.whenFull !== undefined) {
        raw.set_overflow_policy(options.outbox.whenFull);
    }
    if (options.persistSyncedState === false) {
        raw.set_persist_synced_state(false);
    }
    if (options.persistence) {
        await raw.enable_persistence();
    }
    if (options.broadcastChannel) {
        raw.broadcast(options.broadcastChannel);
    }
    if (options.connect) {
        raw.connect(options.connect.url);
        if (options.connect.password) {
            raw.auth(options.connect.password);
        }
        if (options.connect.syncToken) {
            raw.sync_token(options.connect.syncToken);
        }
        else if (options.connect.syncScopes?.length) {
            raw.sync_scopes(options.connect.syncScopes.join(','));
        }
        if (options.connect.reconnect === false) {
            raw.set_auto_reconnect(false);
        }
    }
    return new Cache(raw);
}
