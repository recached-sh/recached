# API Reference

Full TypeScript API for the `recached-edge` package.

> **Scope:** The browser SDK exposes a focused set of string-oriented cache operations. Full key-type commands (hashes, lists, sorted sets) are available on the server over RESP (port 6379) from any Redis-compatible client — they are not part of the browser SDK surface.

---

## `init()`

Initializes the WASM module. `createCache()` calls this automatically, so you only need `init()` if you want to pre-load the WASM binary before creating a cache instance.

```typescript
import { init } from 'recached-edge'

await init() // start downloading WASM now
// ... other app setup ...
const cache = await createCache() // resolves immediately — already loaded
```

---

## `createCache(options?)`

Factory function that initializes the WASM module (idempotent), creates a `Cache` instance, and applies the provided options. Returns a fully-ready cache.

```typescript
import { createCache } from 'recached-edge'

async function createCache(options?: CacheOptions): Promise<Cache>
```

### `CacheOptions`

```typescript
interface CacheOptions {
  /** Load the IndexedDB WAL and write-through every mutation. Default: false. */
  persistence?: boolean

  /**
   * BroadcastChannel name. All tabs opened with the same name share mutations
   * automatically, without a server connection.
   */
  broadcastChannel?: string

  /**
   * Connect to a Recached server immediately. Writes are forwarded to the server;
   * server-side mutations are pushed down to the local WASM store.
   */
  connect?: ConnectOptions
}

interface ConnectOptions {
  /** WebSocket URL, e.g. `"ws://localhost:6380"` or `"wss://cache.example.com"`. */
  url: string
  /** Server password. Required when the server has `RECACHED_PASSWORD` set. */
  password?: string
}
```

### Examples

```typescript
// Local-only
const cache = await createCache()

// With persistence (survives page refresh)
const cache = await createCache({ persistence: true })

// With server sync
const cache = await createCache({
  connect: { url: 'ws://localhost:6380' },
})

// With auth
const cache = await createCache({
  connect: { url: 'ws://localhost:6380', password: 'my-secret' },
})

// Full options
const cache = await createCache({
  persistence: true,
  broadcastChannel: 'my-app',
  connect: { url: 'wss://cache.example.com', password: 'secret' },
})
```

---

## `Cache` class

The main cache interface. Obtain instances via `createCache()`.

---

### Reads

#### `get(key)`

Returns the value for a key, or `null` if the key does not exist or has expired.

```typescript
get(key: string): string | null
```

```typescript
cache.set('name', 'Alice')
cache.get('name')     // 'Alice'
cache.get('missing')  // null
```

::: warning Throws on binary values
Values are byte-transparent, so a backend can write bytes that are not valid UTF-8 and they will sync
into this cache. `get()` throws rather than returning a mangled string — use
[`getBytes()`](#getbytes-key) when a value may not be text.
:::

#### `getBytes(key)`

Returns the value for a key as raw bytes, or `null` if it does not exist or has expired. Works for
any value; text values come back as their UTF-8 bytes.

```typescript
getBytes(key: string): Uint8Array | null
```

```typescript
const bytes = cache.getBytes('thumb:42')
if (bytes) img.src = URL.createObjectURL(new Blob([bytes]))
```

#### `getJSON<T>(key)`

Returns a JSON-parsed value, or `null` if the key is missing, expired, or not valid JSON.

```typescript
getJSON<T>(key: string): T | null
```

```typescript
interface User { id: number; name: string }
const user = cache.getJSON<User>('user:42') // User | null
```

#### `exists(key)`

Returns `true` if the key exists and has not expired.

```typescript
exists(key: string): boolean
```

#### `ttl(key)`

Returns the remaining TTL in seconds. Returns `-1` if the key has no expiry, `-2` if the key does not exist.

```typescript
ttl(key: string): number
```

---

### Writes

All write methods also notify `onMutation` listeners and, when connected, forward the change to the server and other tabs via BroadcastChannel.

#### `set(key, value)`

Sets a key to a string value. Overwrites any existing value and removes any existing TTL.

```typescript
set(key: string, value: string): void
```

#### `setBytes(key, value)`

Sets a key to raw bytes. Values are byte-transparent: the exact bytes are stored, synced to the
server, replicated, and persisted — through the offline outbox and IndexedDB unchanged.

```typescript
setBytes(key: string, value: Uint8Array): void
```

```typescript
cache.setBytes('thumb:42', new Uint8Array(await blob.arrayBuffer()))
```

#### `setEx(key, value, seconds)`

Sets a key with a TTL in seconds. The key is deleted automatically when the TTL elapses.

```typescript
setEx(key: string, value: string, seconds: number): void
```

```typescript
cache.setEx('session', JSON.stringify({ userId: 1 }), 3600)
```

#### `setJSON<T>(key, value, ttl?)`

Serializes `value` as JSON and stores it. Pass `ttl` (seconds) to set an expiry.

```typescript
setJSON<T>(key: string, value: T, ttl?: number): void
```

```typescript
cache.setJSON('user:42', { id: 42, name: 'Alice' })
cache.setJSON('session', { userId: 1 }, 3600) // expires in 1h
```

#### `del(key)`

Deletes a key. Returns `true` if the key existed, `false` if it did not.

```typescript
del(key: string): boolean
```

---

### Reactivity

#### `onMutation(callback)`

Registers a callback that fires whenever the local store changes from any source — local writes, server pushes, or BroadcastChannel cross-tab messages.

Returns an unsubscribe function. Pass it directly to React's `useSyncExternalStore` or call it in a cleanup function.

```typescript
onMutation(cb: () => void): () => void
```

```typescript
// Manual wiring
const unsubscribe = cache.onMutation(() => {
  const count = cache.get('cart:count')
  document.getElementById('badge')!.textContent = count ?? '0'
})

// React (useSyncExternalStore)
const count = useSyncExternalStore(
  (cb) => cache.onMutation(cb),
  () => cache.get('cart:count'),
  () => null,
)

// Cleanup
unsubscribe()
```

The callback receives no arguments — it signals that _something_ changed. Read the keys you care about inside the callback.

---

### Counters & connection

#### `incr(key, by?)` / `decr(key, by?)`

Increment or decrement an integer counter. Returns the new local value; throws if the key holds a non-integer. Offline increments queue as **deltas** and merge additively with everyone else's on reconnect — see [Offline & Reconnection](/browser/offline).

```typescript
incr(key: string, by?: number): number
decr(key: string, by?: number): number
```

#### `disconnect()`

Close the server connection and stop auto-reconnecting. Local reads and writes keep working; writes queue and replay on the next `connect`. Reconnection behavior itself is automatic and configured via `createCache({ connect: { reconnect } })`.

```typescript
disconnect(): void
```

---

### JSON documents {#json-documents}

Native JSON documents shared between server and browser. A `jset`/`jmerge` from any client — or `JSET`/`JMERGE` from the backend over TCP — updates every connected browser's local copy automatically.

#### `jset(key, path, value)`

Set part of a document. `"$"` is the whole document, `"$.user.name"` a nested field, `"$.items[2]"` an array element. Intermediate objects are auto-created. The value is `JSON.stringify`-ed for you. Throws on invalid paths.

```typescript
jset<T>(key: string, path: string, value: T): void
```

```typescript
cache.jset('doc:42', '$', { title: 'Hello', meta: { views: 0 } })
cache.jset('doc:42', '$.meta.views', 17)
```

#### `jget(key, path?)`

Read part of a document from local WASM memory, parsed. Returns `null` when the key or path does not exist.

```typescript
jget<T>(key: string, path?: string): T | null
```

```typescript
const views = cache.jget<number>('doc:42', '$.meta.views') // 17
const doc = cache.jget<Doc>('doc:42')
```

#### `jmerge(key, patch)`

RFC 7386 JSON Merge Patch: objects merge recursively, `null` fields are removed, arrays and scalars are replaced. Only the patch travels over the wire.

```typescript
jmerge<T>(key: string, patch: T): void
```

```typescript
cache.jmerge('doc:42', { title: 'Final', draft: null })
```

---

### Live queries & sync scoping

#### `liveQuery(pattern)`

Start a live query: the server sends the current state of every key matching the glob pattern (merged into the local store), then streams every change to matching keys — including keys created after subscribing. The mutation callback fires on each change.

Returns a stop function. Calls are ref-counted per pattern: the server subscription ends when the last caller stops.

```typescript
liveQuery(pattern: string): () => void
```

```typescript
const stop = cache.liveQuery('cart:42:*')
const unsub = cache.onMutation(() => {
  render(cache.getMatching('cart:42:*'))
})
// later:
unsub(); stop()
```

Using React or Vue? [`useKeys(pattern)`](/react/hooks-reference#usekeys-pattern) wraps this in one line.

#### `getMatching(pattern)`

Snapshot of local keys matching a glob pattern, as `[key, value]` pairs sorted by key. Values are strings; keys holding collection types come back as `null`. Served entirely from local WASM memory.

```typescript
getMatching(pattern: string): Array<[string, string | null]>
```

#### `syncToken(token)` / `syncScopes(patterns)`

Scope this connection's sync. `syncToken` presents a signed scope token — required on servers running with `RECACHED_SYNC_SECRET`; `syncScopes` sets plain glob patterns as a bandwidth filter on servers without one. Either way, an entry may state its access: `r=catalog:*` is read-only, `rw=cart:42:*` and bare patterns are read-write. Usually you pass these via `createCache({ connect: { syncToken } })` instead. See [Sync Scopes](/server/sync-scopes).

```typescript
syncToken(token: string): void
syncScopes(patterns: string[]): void
```

---

### Pub/Sub

Pub/Sub requires a server connection. Messages are delivered via the WebSocket and routed to subscribers on the receiving end.

#### `subscribe(channel)`

Subscribe to a pub/sub channel, so the server starts delivering that channel's messages to this
client. Subscribing alone does not surface anything — register a handler with
[`onMessage`](#onmessage-channel-callback) to actually receive them.

```typescript
subscribe(channel: string): void
```

#### `onMessage(channel, callback)` {#onmessage-channel-callback}

Register a handler for messages on a channel. Returns an unsubscribe function that removes **this
handler only** — it does not leave the channel; call `unsubscribe(channel)` for that. Multiple
handlers can be registered on the same channel.

```typescript
onMessage(channel: string, cb: (msg: string | Uint8Array) => void): () => void
```

```typescript
cache.subscribe('notifications');

const stop = cache.onMessage('notifications', (msg) => {
  console.log('got', msg);
});

// Later — remove the handler, then leave the channel.
stop();
cache.unsubscribe('notifications');
```

#### `unsubscribe(channel)`

Unsubscribe from a pub/sub channel.

```typescript
unsubscribe(channel: string): void
```

#### `publish(channel, message)`

Publish a message to a pub/sub channel. All server-side and browser-side subscribers receive it.

```typescript
publish(channel: string, message: string): void
```

#### `publishBytes(channel, message)`

Publish raw bytes to a pub/sub channel. Subscribers receive a `Uint8Array` rather than a string when
the payload is not valid UTF-8.

```typescript
publishBytes(channel: string, message: Uint8Array): void
```

---

### Persistence

#### `clearPersistence()`

Deletes the IndexedDB WAL database. Use on sign-out so the next session starts clean.

```typescript
clearPersistence(): Promise<void>
```

```typescript
async function signOut() {
  await cache.clearPersistence()
  window.location.href = '/login'
}
```

Persistence is enabled via `createCache({ persistence: true })`, not on the `Cache` instance itself.

---

### Escape hatch

#### `cache.raw`

Direct access to the underlying WASM instance (`RecachedCache` from wasm-bindgen). Use this when you need an operation that is not yet exposed in the TypeScript wrapper.

```typescript
get raw(): RawCache
```

Available methods on `raw`: `set()`, `setBytes()`, `set_ex()`, `get()`, `getBytes()`, `del()`, `ttl()`, `exists()`, `subscribe()`, `unsubscribe()`, `publish()`, `publishBytes()`, `connect()`, `auth()`, `broadcast()`, `enable_persistence()`, `clear_persistence()`, `set_mutation_callback()`, `free()`.

> Writes through `cache.raw` bypass the `onMutation` notification bus. Use the typed `Cache` methods when possible.
