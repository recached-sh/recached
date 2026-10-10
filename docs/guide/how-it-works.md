# How it works

Recached clients keep local cache data in memory and synchronize with the server over WebSocket. The same Rust engine executes cache operations in the server, browser, native mobile SDKs, and embedded Rust client; each adapter owns its transport and persistence.

<figure>
  <img class="light-only" src="/architecture-light.svg" alt="Backend RESP clients connect to the Recached server on port 6379. Browser, native Android and Apple apps, and embedded Rust services sync over WebSocket on port 6380 and read local memory.">
  <img class="dark-only" src="/architecture-dark.svg" alt="Backend RESP clients connect to the Recached server on port 6379. Browser, native Android and Apple apps, and embedded Rust services sync over WebSocket on port 6380 and read local memory.">
  <figcaption>Backend commands execute on the server. Client reads use local memory; writes and updates travel over WebSocket.</figcaption>
</figure>

## Shared engine and platform adapters

The workspace contains six crates:

| Crate / directory | Role |
|---|---|
| `core-engine` | RESP parsing, typed commands, a sharded `DashMap` store, TTLs, collections, JSON, and eviction. No networking or file persistence. Uses the native clock or JavaScript `Date.now()` for expiry. |
| `server-native` (package `recached`) | Tokio RESP TCP and WebSocket servers, authentication, scope checks, change delivery, snapshots/AOF, and replication. |
| `sync-client` | Platform-neutral session replay, ordered reply matching, outbox, `DEDUP` envelopes, backoff, and applying server state to local memory. |
| `wasm-edge` | Browser WebSocket transport, WebAssembly bindings, optional IndexedDB persistence, and opt-in BroadcastChannel sharing. |
| `recached-embed` | Async Rust service adapter with local reads, awaited watch hydration, and an in-memory outbox. |
| `recached-mobile` | UniFFI surface for Kotlin and Swift, local reads, transactional SQLite persistence, and changed-key reporting. No sockets or async runtime. |

The platform wrappers live in [recached-kotlin](https://github.com/recached-sh/recached-kotlin) and [recached-swift](https://github.com/recached-sh/recached-swift). Kotlin owns an OkHttp WebSocket and routes changes to `Flow` observers. Swift owns a `URLSessionWebSocketTask` and routes changes to `AsyncStream` and SwiftUI models.

`DashMap` uses shard locks; it is not lock-free. Server workers execute commands concurrently, but a single hot key still contends on its shard. Single-key commands are atomic; multi-key commands and transactions do not isolate concurrent readers from intermediate results. See [the concurrency model](/server/commands#concurrency-model).

## Connect and hydrate

Clients connect to the WebSocket port, 6380 by default. Backend RESP clients use port 6379. With server TLS configured, use `wss://` for sync and `rediss://` for RESP.

A socket opening does not fetch the keyspace. Register a live query with browser `liveQuery(pattern)`, mobile `watch(pattern)`, or Rust `watch(pattern).await?`. The server replies with a complete `qstate` snapshot for the pattern and delivers later changes. A pattern exceeding the initial snapshot limit returns an error instead of partial state.

On each reconnect, `sync-client` sends the session in this order:

1. `AUTH`, when a password is configured
2. `CLIENT DELTA ON` and `CLIENT EXPIRY ON`
3. `SYNC TOKEN` or open-mode sync patterns, when configured
4. Queued writes, oldest first
5. `QSUB` for each active live query

Replaying writes before snapshots prevents a reconnect snapshot from temporarily erasing accepted offline edits. Applying the new snapshot also removes matching local keys absent from the server, including deletions missed while offline. Re-register mobile watches after reopening the cache; they are not stored in SQLite.

Authorization is separate from watches. With no sync secret or scopes, the default server fans mutations out to every WebSocket client. Scoped connections receive only permitted keys. Use backend-minted [sync tokens](/server/sync-scopes) for per-user data.

## Read and write locally

Client reads consult local `core-engine` memory without waiting for the network. Until hydration completes, a browser or mobile read may return a saved value or a miss. Embedded Rust reads outside hydrated watches return `NotHydrated`.

Browser and mobile writes apply locally, then enter an outbox for sending or reconnect replay. A successful local write does not mean the server accepted it. Embedded Rust writes await the server reply; their queue is held in memory. The server can refuse a write because of authorization, type, or resource limits; see [write durability and replay](/guide/client-support#write-durability-and-replay).

The server orders command replies on each connection. Pushes can interleave with replies, so `sync-client` distinguishes mutations, pub/sub messages, `keychange`, `keydelta`, and `qstate`. It uses a FIFO to retire the write acknowledged by each reply.

Server changes reach clients in two forms:

- Mutation pushes carry replayable commands for keys not already covered by that connection's watches or live queries. The server excludes the originating connection from this fan-out.
- Watched keys receive authoritative `keychange` values or negotiated `keydelta` operations. These can reach the writing client too, to reconcile its local value with the server.

Changed-key notifications let observers re-read their key or pattern. Mobile observers suppress equal values and coalesce updates for slow consumers; they do not deliver a durable history of every mutation.

## Persist browser and mobile data

Each adapter has a different storage contract:

| Adapter | What is saved | When it is saved |
|---|---|---|
| Browser | Local API write commands in an IndexedDB WAL, pending writes, client identity and epoch | Asynchronously; writes do not await a storage commit |
| Native mobile | Materialized per-key snapshots, pending writes, client identity and epoch in SQLite | A successful local write commits data and its outbox row before returning; received frames commit changed keys and retired rows together |
| Embedded Rust | No on-disk client persistence | Store and outbox live only in the process |

Browser startup replays the WAL and restores queued writes before connecting. Incoming server pushes and cross-tab messages do not append to the browser WAL. Compaction runs during hydration when the WAL exceeds 1,000 rows, replacing it atomically with snapshot commands while leaving the outbox intact. See [browser persistence](/browser/persistence) for TTL and cold-start limits.

Mobile startup restores the saved state before opening its socket, so saved server-pushed values remain readable offline after an app restart. SQLite runs in WAL mode with `synchronous = NORMAL`; an app kill preserves committed writes, but OS crashes or power loss can lose recent commits. See [native client limits](/guide/client-support).

## Share browser writes across tabs

Setting `broadcastChannel` shares local write commands with tabs on the same origin and channel. Receiving tabs apply them to memory and notify listeners, but do not forward them to a server or persist them to their own WAL. Server pushes are not rebroadcast.

BroadcastChannel does not elect a connection leader or proxy server traffic. Each tab that needs server updates must connect independently. It has no role in native mobile or cross-device sync.

## Read the protocol contract

TCP and WebSocket carry RESP (Redis Serialization Protocol). WebSocket commands and replies use text frames for valid UTF-8 and binary frames for other bytes. Values are binary-safe; keys, fields, members, patterns, and channels must be UTF-8 text.

The [wire protocol reference](/server/protocol) defines frame shapes, reply ordering, negotiated expiry/deltas, and duplicate suppression. `DEDUP` covers replay while the server retains the corresponding client mark; it does not guarantee exactly-once delivery across every server crash or retention window.
