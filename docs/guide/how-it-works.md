# How It Works

<figure>
  <img class="light-only" src="/architecture-light.svg" alt="Your backend writes to the Recached server over RESP on port 6379. The server syncs over a WebSocket on port 6380 to the browser or edge runtime, where reads are served from local WebAssembly memory. Writes flow back the same way.">
  <img class="dark-only" src="/architecture-dark.svg" alt="Your backend writes to the Recached server over RESP on port 6379. The server syncs over a WebSocket on port 6380 to the browser or edge runtime, where reads are served from local WebAssembly memory. Writes flow back the same way.">
  <figcaption>
    A write on either side reaches the other automatically — server mutations fan out to every
    connected browser, and browser writes sync back and fan out to other clients. Reads never leave
    the process.
  </figcaption>
</figure>

## The three-crate architecture

Recached is split into three crates with hard dependency boundaries. The key constraint is that `core-engine` has zero dependencies on anything that does not compile to `wasm32`.

### `core-engine`

The state machine. No networking, no file I/O, no OS-specific code.

- **RESP parser:** Recursive descent, depth-limited to 128 levels to prevent stack-overflow DoS. Handles partial reads — the server buffers bytes until a complete command arrives.
- **Command dispatch:** Typed enum over the full command set. Each variant carries its parsed arguments. Dispatch is a match on the enum — no string parsing at execution time.
- **Store:** `Arc<DashMap<String, Entry>>` where each `Entry` holds an `EntryValue` enum (`Str`, `Hash`, `List`, `Set`, `ZSet`) plus `expires_at_ms: Option<u64>` and `written_at_ms: u64` for eviction bookkeeping.
- **TTL engine:** Expiry is stored inline on each entry as an absolute millisecond timestamp. Reads mask expired entries immediately. A background task checks at most 256 TTL-indexed keys per one-second tick and removes expired entries actively.
- **Key cap:** If `RECACHED_MAX_KEYS` is set, every write command checks the store length before inserting. When the cap is reached, the configured eviction policy runs or the command errors.

Because `core-engine` has no network code, it can be embedded anywhere: the Tokio server, a unit test, or a WASM binary.

### `server-native`

The Tokio-based network layer. Depends on `core-engine`.

- Binds two ports: 6379 (TCP, RESP) and 6380 (WebSocket, RESP-over-WS).
- Each TCP connection gets a dedicated task. A persistent read buffer accumulates bytes until the RESP parser signals a complete command; this handles fragmentation from TCP stream splitting.
- Pub/sub delivery: each subscribed connection holds a `tokio::sync::mpsc::Sender`. `PUBLISH` iterates channel subscribers and sends to their channels. The channel is bounded (1024 messages); a slow subscriber is disconnected rather than blocking the publisher.
- Broadcast filter: each WebSocket connection is assigned a `sender_id` (UUID). When the server fans out a mutation to WebSocket clients, it skips the connection whose `sender_id` matches the mutation's origin. This prevents browser clients from double-applying their own writes.

### `wasm-edge`

The browser-side bindings. Depends on `core-engine`. Compiled with `wasm-bindgen`.

- Exposes a `RecachedCache` class to JavaScript/TypeScript.
- All cache reads go directly to the in-memory `core-engine` instance — no WebSocket, no async, no awaiting.
- Writes to the local store immediately, then serializes the command to RESP and sends it over the WebSocket to the server.
- Incoming WebSocket frames are parsed as RESP push messages. Mutations are applied to the local store. If the mutation originated from this client (matched by sender ID), it is skipped to avoid double-apply.

---

## RESP over TCP and WebSocket

RESP (Redis Serialization Protocol) is a simple line-oriented protocol. Every value is prefixed with a type byte:

| Prefix | Type | Example |
|---|---|---|
| `+` | Simple string | `+OK\r\n` |
| `-` | Error | `-ERR wrong type\r\n` |
| `:` | Integer | `:42\r\n` |
| `$` | Bulk string | `$5\r\nhello\r\n` |
| `*` | Array | `*2\r\n$3\r\nGET\r\n$3\r\nfoo\r\n` |

A command is an array of bulk strings. `SET foo bar` is:

```
*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n
```

The server parses this identically whether it arrives over TCP or inside a WebSocket frame. The WebSocket transport is just a framing layer; the payload is RESP.

This is intentional: the WASM client sends RESP frames over WebSocket, so the server does not need a separate browser protocol. And the `core-engine` parser is shared across both targets.

---

## How mutations fan out

### Server to browser

1. A backend service writes `SET cart:user:42 3` over TCP.
2. `server-native` applies the command to the `core-engine` store.
3. After a successful write, `server-native` serializes the mutation as a RESP push message and sends it to every connected WebSocket client (except the sender, if the write came from a WebSocket connection).
4. Each browser's `wasm-edge` receives the frame, parses it as RESP, and applies the same command to its local `core-engine` instance.
5. The listeners of the changed keys (`onKeyChange`, `onPatternChange`, and every `onMutation` listener) are notified synchronously, triggering UI re-renders. Listeners of other keys are not.

### Browser to server

1. JavaScript calls `cache.set('theme', 'dark')`.
2. `wasm-edge` applies the write to the local store immediately (synchronous, 0 ms).
3. `wasm-edge` serializes the command as RESP and sends it over the WebSocket to the server.
4. The server applies the command to its store and fans it out to all other connected WebSocket clients (filtering out the originating connection by sender ID).

### Cross-server mutation to browser

Any RESP command from any TCP client (backend service, CLI tool, another server) that mutates the store triggers the broadcast. The browser does not need a special protocol or API — it just needs to be connected to port 6380.

---

## BroadcastChannel cross-tab sync

When a `broadcastChannel` name is provided to `createCache()`, the `wasm-edge` module opens a `BroadcastChannel` with that name. Every time the local store is mutated (by either a local write or a server push), the mutation is posted to the BroadcastChannel as a serialized RESP frame.

Other tabs in the same browser origin listen on the same channel and apply the mutation to their own local `core-engine` instance. This means:

- All tabs share mutations without each needing an independent WebSocket connection.
- Only one tab needs to hold the WebSocket connection to the server. (In practice, each tab holds its own connection, but the BroadcastChannel sync ensures consistency even if one tab has not received the WebSocket message yet.)

---

## IndexedDB WAL persistence

The write-ahead log (WAL) is an IndexedDB object store that records every mutation in sequence order.

### How it works

On every write (local or received from the server), `wasm-edge` appends an entry to the WAL:

```
{ seq: number, command: string /* RESP-encoded */ }
```

The sequence number is a monotonically increasing integer tracked in the `wasm-edge` module state.

### Hydration on page load

When `createCache({ persistence: true })` is called and the page loads:

1. `wasm-edge` opens the IndexedDB database.
2. It replays all WAL entries in sequence order against the local `core-engine` instance.
3. The cache is now populated from the WAL — without any network request.
4. Only after hydration completes does `wasm-edge` connect to the WebSocket server to receive live mutations.

The effect: the UI can render with cached data immediately on page load, before the WebSocket handshake completes.

### WAL compaction

The WAL is compacted periodically. After a successful WebSocket sync, entries older than the acknowledged sequence are removed from IndexedDB to prevent unbounded growth.

---

## Sender-ID dedup filter

Every WebSocket connection receives a unique sender ID when it connects. This ID is included in the RESP push message metadata that the server sends when broadcasting mutations.

When a browser's `wasm-edge` receives a push message, it checks the sender ID. If the sender ID matches its own, it discards the message — the write was already applied to the local store when the JavaScript called `cache.set()`.

This prevents the double-apply problem: without the filter, a browser write would be applied locally, sent to the server, and then reflected back to the originating browser, applying it a second time.

The filter is a UUID comparison on every incoming WebSocket frame. It is O(1) and adds no meaningful latency.
