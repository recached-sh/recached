export interface ConnectOptions {
    /** WebSocket URL, e.g. `"ws://localhost:6380"` or `"wss://cache.example.com"`. */
    url: string;
    /**
     * Server password. Required when the server has `RECACHED_PASSWORD` set.
     * Sent immediately after the socket opens via `AUTH`.
     */
    password?: string;
    /**
     * Signed sync-scope token, minted by your application backend. Required when
     * the server has `RECACHED_SYNC_SECRET` set (strict mode): without it the
     * connection receives no pushes and may run no key commands.
     * Sent after `AUTH` via `SYNC TOKEN`.
     */
    syncToken?: string;
    /**
     * Glob patterns to scope this connection's sync to, e.g. `['cart:*']`.
     * Only honoured by servers *without* a sync secret — a bandwidth filter,
     * not an authorization boundary. Use `syncToken` for security.
     *
     * An entry may state its access — `'r=catalog:*'` for read-only,
     * `'rw=cart:42:*'` or a bare pattern for read-write.
     */
    syncScopes?: string[];
    /**
     * Automatically reconnect with exponential backoff (500 ms doubling to a
     * 30 s cap) when the connection drops. On reconnect the session is
     * re-established (AUTH, sync token, live queries) and writes queued while
     * offline are replayed in order.
     *
     * @default true
     */
    reconnect?: boolean;
}
export interface CacheOptions {
    /**
     * Load the IndexedDB WAL and enable write-through persistence so the cache
     * survives page refreshes.
     *
     * Gracefully ignored if IndexedDB is unavailable (e.g. some private-browsing
     * modes or non-browser environments).
     *
     * @default false
     */
    persistence?: boolean;
    /**
     * BroadcastChannel name for cross-tab mutation sharing.
     * All tabs that open with the same name see each other's writes automatically,
     * with no server connection required.
     */
    broadcastChannel?: string;
    /**
     * Connect to a Recached server immediately after the cache is created.
     * Once connected, writes are pushed to the server and server-side mutations
     * are pushed down to the local WASM store automatically.
     */
    connect?: ConnectOptions;
}
export interface RawCache {
    enable_persistence(): Promise<void>;
    clear_persistence(): Promise<void>;
    broadcast(channel_name: string): void;
    connect(url: string): void;
    auth(password: string): string;
    set(key: string, value: string): string;
    setBytes(key: string, value: Uint8Array): string;
    set_ex(key: string, value: string, seconds: number): string;
    get(key: string): string | undefined;
    getBytes(key: string): Uint8Array | undefined;
    del(key: string): number;
    /** `i64` on the Rust side, so wasm-bindgen marshals it as `bigint`. */
    incr_by(key: string, delta: bigint): bigint;
    disconnect(): void;
    set_auto_reconnect(enabled: boolean): void;
    ttl(key: string): number;
    exists(key: string): boolean;
    publish(channel: string, message: string): void;
    publishBytes(channel: string, message: Uint8Array): void;
    subscribe(channel: string): void;
    unsubscribe(channel: string): void;
    jset(key: string, path: string, value: string): string;
    jget(key: string, path?: string): string | undefined;
    jmerge(key: string, patch: string): string;
    sync_token(token: string): void;
    sync_scopes(patterns_csv: string): void;
    live_query(pattern: string): void;
    live_unquery(pattern?: string): void;
    get_matching(pattern: string): Array<[string, string | Uint8Array | null]>;
    set_mutation_callback(cb: () => void): void;
    set_outbox_full_callback(cb: (droppedId: number, pending: number) => void): void;
    pending_writes(): number;
    set_message_callback(cb: (channel: string, message: string) => void): void;
    free(): void;
}
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
export declare class Cache {
    /** @internal */
    readonly raw: RawCache;
    private readonly _mutationListeners;
    private readonly _messageListeners;
    private readonly _outboxFullListeners;
    /** @internal Arrow function so `this` is always bound when passed as a callback. */
    private readonly _notifyMutation;
    /** @internal */
    private readonly _notifyOutboxFull;
    /** @internal */
    private readonly _notifyMessage;
    /** @internal */
    constructor(raw: RawCache);
    /**
     * Subscribe to store mutations from any source — local writes, server
     * WebSocket push, and BroadcastChannel cross-tab sync.
     *
     * Returns an unsubscribe function. Pass directly to React's
     * `useSyncExternalStore` `subscribe` parameter.
     *
     * ```ts
     * useSyncExternalStore(
     *   cache.onMutation.bind(cache),
     *   () => cache.get('key'),
     *   () => null,
     * );
     * ```
     */
    onMutation(cb: () => void): () => void;
    /**
     * Called when the offline write queue overflows and the **oldest** queued
     * write is discarded.
     *
     * The queue holds 10,000 writes. Past that, each new write evicts the oldest
     * one — without this callback that is silent data loss: no error is thrown
     * and the write is simply gone. If a client can plausibly be offline long
     * enough to hit the cap, do not treat the outbox as the system of record;
     * persist the mutation yourself and reconcile on reconnect.
     *
     * Returns an unsubscribe function.
     *
     * ```ts
     * cache.onOutboxFull((droppedId, pending) => {
     *   console.error(`dropped queued write ${droppedId}; ${pending} still pending`);
     * });
     * ```
     */
    onOutboxFull(cb: (droppedId: number, pending: number) => void): () => void;
    /**
     * Writes queued locally and not yet acknowledged by the server.
     *
     * Useful for a "syncing…" indicator, or to apply back-pressure before the
     * queue reaches its 10,000-write cap.
     */
    pendingWrites(): number;
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
    onMessage(channel: string, cb: (msg: string | Uint8Array) => void): () => void;
    /**
     * Return the value for `key`, or `null` if the key does not exist or has expired.
     *
     * Always served from local WASM memory — zero network latency.
     *
     * @throws if the stored value is not valid UTF-8. Values are byte-transparent,
     * so a backend can write binary that syncs into this cache; returning it as a
     * mangled string would be worse than failing. Use {@link getBytes} for those.
     */
    get(key: string): string | null;
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
     * Note: the browser SDK can *read* binary values but cannot yet *write* them
     * — `set()` takes a string. Binary values originate from a backend writing
     * over the RESP port.
     */
    getBytes(key: string): Uint8Array | null;
    /**
     * Return a JSON-parsed value stored under `key`, or `null` if the key is
     * missing, expired, or not valid JSON.
     *
     * ```ts
     * interface User { id: number; name: string }
     * const user = cache.getJSON<User>('user:42'); // User | null
     * ```
     */
    getJSON<T>(key: string): T | null;
    /** Return `true` if `key` exists and has not expired. */
    exists(key: string): boolean;
    /**
     * Return the remaining TTL in seconds.
     * - `-1` — key exists with no expiry
     * - `-2` — key does not exist
     */
    ttl(key: string): number;
    /**
     * Store a string value. Syncs to the server and other tabs when connected.
     */
    set(key: string, value: string): void;
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
    setBytes(key: string, value: Uint8Array): void;
    /**
     * Store a string value with a TTL (seconds). The key is deleted automatically
     * once the TTL elapses.
     */
    setEx(key: string, value: string, seconds: number): void;
    /**
     * Serialize `value` as JSON and store it under `key`.
     * Pass `ttl` (seconds) to have the key expire automatically.
     *
     * ```ts
     * cache.setJSON('user:42', { id: 42, name: 'Alice' }, 300); // expires in 5 min
     * ```
     */
    setJSON<T>(key: string, value: T, ttl?: number): void;
    /**
     * Delete `key`.
     *
     * Returns `true` if the key existed, `false` if it did not.
     * Syncs to the server and other tabs when connected.
     */
    del(key: string): boolean;
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
    incr(key: string, by?: number): number;
    /** Decrement an integer counter. See {@link incr}. */
    decr(key: string, by?: number): number;
    /**
     * Close the server connection and stop reconnecting. Local reads and writes
     * keep working; writes queue and replay on the next connect.
     */
    disconnect(): void;
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
    jset<T>(key: string, path: string, value: T): void;
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
    jget<T>(key: string, path?: string): T | null;
    /**
     * Apply an RFC 7386 JSON Merge Patch to a document: objects merge
     * recursively, `null` fields are removed, arrays and scalars are replaced.
     * Only the patch travels over the wire — not the whole document.
     *
     * ```ts
     * cache.jmerge('doc:42', { title: 'New title', draft: null })
     * ```
     */
    jmerge<T>(key: string, patch: T): void;
    /**
     * Subscribe to a server pub/sub channel. Push messages arrive via the
     * WebSocket `onmessage` callback.
     */
    subscribe(channel: string): void;
    /** Unsubscribe from a server pub/sub channel. */
    unsubscribe(channel: string): void;
    /**
     * Publish a message to a server pub/sub channel. All subscribers — browser
     * and server-side — receive the message.
     */
    publish(channel: string, message: string): void;
    /**
     * Publish raw bytes to a server pub/sub channel.
     *
     * Subscribers receive a `Uint8Array` rather than a string when the payload is
     * not valid UTF-8 — see {@link onMessage}.
     */
    publishBytes(channel: string, message: Uint8Array): void;
    /**
     * Present a signed sync-scope token (strict servers — `RECACHED_SYNC_SECRET`).
     * Mint it on your backend and hand it to the page; see the Sync Scopes docs.
     */
    syncToken(token: string): void;
    /**
     * Scope this connection's sync to glob patterns (servers without a sync
     * secret only). A bandwidth filter, not an authorization boundary.
     *
     * An entry may state its access — `'r=catalog:*'` for read-only,
     * `'rw=cart:42:*'` or a bare pattern for read-write — matching the grant
     * notation a signed token uses.
     */
    syncScopes(patterns: string[]): void;
    /** @internal Ref-counts per live-query pattern so components sharing a
     * pattern don't cancel each other's server subscription. */
    private readonly _liveQueryRefs;
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
     * cache.onMutation(() => {
     *   render(cache.getMatching('cart:42:*'));
     * });
     * // later:
     * stop();
     * ```
     */
    liveQuery(pattern: string): () => void;
    /**
     * Snapshot of local keys matching a glob pattern, as `[key, value]` pairs.
     * Keys holding collection types come back as `null` (read those with typed
     * accessors). A value that is not valid UTF-8 comes back as a `Uint8Array`
     * rather than a mangled string, so narrow the type before treating it as text.
     *
     * Served entirely from local WASM memory — zero network latency.
     */
    getMatching(pattern: string): Array<[string, string | Uint8Array | null]>;
    /**
     * Erase the IndexedDB WAL. The in-memory store is not affected.
     *
     * Use on sign-out so the next session starts with an empty cache.
     */
    clearPersistence(): Promise<void>;
}
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
export declare function init(): Promise<void>;
/**
 * Create a {@link Cache} instance.
 *
 * Loads and initialises the WASM module on first call (subsequent calls reuse
 * the existing module). Options are applied in order:
 * `persistence` → `broadcastChannel` → `connect` + `auth`.
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
export declare function createCache(options?: CacheOptions): Promise<Cache>;
//# sourceMappingURL=sdk.d.ts.map