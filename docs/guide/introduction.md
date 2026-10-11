# Introduction

Recached is a Rust cache and sync engine for backends, browsers, native Android and Apple apps, and embedded Rust services. Client adapters keep local cache data in memory, exchange writes and updates over WebSocket, and expose APIs for reacting to changes.

## Where the engine runs

Your backend uses the server's RESP (Redis Serialization Protocol) command subset on port 6379. Browser clients run `core-engine` as WebAssembly. Kotlin and Swift clients run it natively through UniFFI, while `recached-embed` holds data inside a Rust service process.

Client reads use local memory without a server round trip. A server connection alone does not hydrate the cache: register browser `liveQuery(pattern)` or native `watch(pattern)` for an initial snapshot and updates. Saved or locally written values remain readable offline, but they can be stale until synchronization finishes.

| Client | Transport | Local storage | Reactive API |
|---|---|---|---|
| Browser `recached-edge` | Browser WebSocket | Memory; optional IndexedDB WAL and outbox | Key and pattern listeners; React/Vue adapters |
| Android `recached-android` | OkHttp WebSocket | Memory and SQLite data/outbox | Kotlin `Flow` |
| Swift `Recached` | `URLSessionWebSocketTask` | Memory and SQLite data/outbox | `AsyncStream` and SwiftUI models |
| Rust `recached-embed` | Tokio WebSocket | Memory and in-memory outbox | Pub/sub receiver |

The Kotlin and Swift SDKs are implemented, unreleased previews. React Native, Flutter, and WASI deployment remain planned. See [client support and limits](/guide/client-support) for installation status and adapter differences.

## Shared state across clients

If MySQL or PostgreSQL owns your business records, use Recached to synchronize a derived cache of committed state. Send durable edits through your application API and publish database changes through an outbox worker. Direct client cache writes do not update SQL automatically. See [using Recached with a main database](/guide/database-integration).

A backend write can update browser tabs and native apps through the same sync protocol. Clients register the patterns they need to hydrate and reconcile after reconnecting. Sync scopes control which keys each connection may read, write, and receive; a watch pattern is not an access boundary.

The server executes commands across worker threads over a sharded `DashMap` store. `DashMap` uses locks per shard. Scaling depends on the workload and key distribution, and multi-key writes do not isolate concurrent readers. See [benchmarks](/guide/benchmarks#thread-scaling) and [the concurrency model](/server/commands#concurrency-model).

## Architecture

<figure>
  <img class="light-only" src="/architecture-light.svg" alt="Backend RESP clients connect to the Recached server on port 6379. Browser, native Android and Apple apps, and embedded Rust services sync over WebSocket on port 6380 and read local memory.">
  <img class="dark-only" src="/architecture-dark.svg" alt="Backend RESP clients connect to the Recached server on port 6379. Browser, native Android and Apple apps, and embedded Rust services sync over WebSocket on port 6380 and read local memory.">
</figure>

The workspace separates `core-engine`, `sync-client`, the server, and three client adapters: `wasm-edge`, `recached-embed`, and `recached-mobile`. Platform wrappers live in the Kotlin and Swift repositories. [How it works](/guide/how-it-works) explains their transport, storage, and reconnect contracts.

## When to use Recached

Recached is a good fit when:

- **Your browser or native app reads shared backend state.** Feature flags, live counters, cart state, and presence can update through the sync connection.
- **You want live UI without polling.** The WebSocket sync replaces a polling loop without requiring you to build a separate SSE or WebSocket server.
- **You want a frontend-only cache with TTL.** The WASM module works entirely without a server. Call `createCache()` without `connect` and you get a local cache with TTLs, counters, JSON documents, glob queries, optional IndexedDB persistence and cross-tab sync — no Recached server, no Redis, no backend changes required. Pub/sub, live queries and cross-device sync are what you give up; see [no server at all](/guide/use-cases#no-server-at-all) for the full boundary.
- **You need native offline reads and queued writes.** Kotlin and Swift restore SQLite data before connecting and persist successful local writes with their outbox rows. See [Android](/android/getting-started) and [Apple](/ios/getting-started).
- **You need cross-tab sync.** BroadcastChannel shares local writes between tabs on the same origin and channel.
- **You want to reuse a Redis client** for Recached's documented command subset (strings, expiry, counters, collections, transactions, and pub/sub).

## When Recached is not the right fit

- **You need very high-durability persistence.** Recached supports snapshots (RDB-style) and AOF, but it is still primarily an in-memory cache. If you cannot tolerate any data loss between fsync intervals, a purpose-built database is the right tool.
- **You need unattended failover.** Recached supports primary/replica replication but has no quorum, leader election, or fencing service. Promotion requires an operator or orchestrator to fence the old primary and send `REPLICAOF NO ONE`. `RECACHED_FAILOVER_TIMEOUT` is deprecated and ignored.
- **You depend on uncommon Redis commands.** Recached implements the commands most applications use, not all 250+. Latency introspection (`SLOWLOG`, `INFO latencystats`), Lua scripting, and cluster mode are out of scope. `INFO` and `COMMAND` expose the supported compatibility subset. RESP3 is supported for protocol negotiation and pub/sub delivery (`HELLO 3`), not for the full RESP3 type surface.
- **You need very large datasets.** Recached is an in-memory cache — it is not a database. If your working set does not fit in RAM, Redis with RDB persistence or a proper database is the right tool.

## Binary values

**Values are binary-safe.** A value is stored and returned as the exact bytes you sent — compressed
payloads, protobuf, images, serialized objects — with no encoding step and no size penalty.

**Identifiers must be text.** Keys, hash fields, set and sorted-set members, glob patterns and
pub/sub channel names must be valid UTF-8, and a command carrying a binary one is rejected:

```
ERR argument 1 is not valid UTF-8. Keys, fields, members and patterns must be text;
    only values may be binary
```

Nothing is stored when this happens and the connection stays usable. This is narrower than Redis,
where keys are binary-safe too — but keys are looked up, glob-matched and checked against sync scopes
as text, and a binary key would be unreachable through those paths. Keys are identifiers in practice,
so this is rarely felt.

Commands that interpret a value still require the right shape: `INCR` on a binary value returns
`ERR value is not an integer`, and JSON documents must be UTF-8 because JSON is defined that way.
Those are type errors, not encoding losses — the stored bytes are unchanged either way.

**The browser SDK handles binary too.** `cache.setBytes(key, uint8array)` writes it,
`cache.getBytes(key)` reads it back, and `cache.publishBytes(channel, uint8array)` publishes it.
Binary values survive the offline outbox, cross-tab sync and IndexedDB persistence unchanged.

`cache.get()` **throws** on a binary value rather than returning mangled text, and `getJSON()`
treats one as a miss — reach for `getBytes()` when a value may not be text. A binary pub/sub payload
arrives at an `onMessage` listener as a `Uint8Array` instead of a string.

Before 0.2.2 values were stored as UTF-8 strings and binary was silently replaced with U+FFFD: `SET`
returned `OK` and `GET` returned different bytes than were written, on every transport. If you are
upgrading from an earlier version, data already corrupted that way cannot be recovered — the bytes
were destroyed on the way in.

## Maturity

Honest status, per layer:

- **The cache server is a release candidate for cache workloads.** It includes atomic snapshots, an append-only file, ordered replication, TLS, constant-time authentication, hardened parsers, Prometheus metrics, and load/chaos tests. It still needs broad production validation and an independent security audit. Treat it as a cache, not a system of record.
- **The shared sync layer (browser, mobile, and Rust clients) is beta.** The invariants are [specified](/server/protocol), tested, and verified end-to-end — but the code is young and hasn't accumulated real-world miles or third-party security review yet. Concretely: don't expose the WebSocket port to the public internet for multi-tenant data until you've read [Sync Scopes](/server/sync-scopes) and understood the model, and expect occasional sharp edges.

- **The Kotlin and Swift SDKs are unreleased 0.1.0 previews.** Their APIs, transports, SQLite persistence, and observers are implemented; package configs do not yet pin a released mobile core. Build from source and review [client limits](/guide/client-support).

The road to 1.0 is hardening, not features: fuzzing the parser surfaces, automated browser testing, a security pass on the token path, and a protocol freeze once real-world usage has confirmed the design. Bug reports from production-like use are the most valuable contribution the project can receive right now.

## Recached vs Redis

| | Recached | Redis |
|---|---|---|
| Protocol | RESP (compatible) | RESP |
| Local client cache | Browser WASM, native Kotlin/Swift previews, embedded Rust | No built-in client replica |
| WebSocket sync | Built-in | Not built-in |
| Persistence | Snapshot + AOF | RDB + AOF |
| Replication | Primary/replica + manually fenced promotion | Yes (+ Sentinel/Cluster) |
| Lua scripting | No (WASM scripting on roadmap) | Yes |
| Cluster mode | No | Yes |
| Command coverage | 124 catalog entries | 250+ |
| License | Apache 2.0 | AGPLv3 / RSALv2 + SSPLv1 (BSD-3 up to 7.2; Valkey stayed BSD-3) |

## Recached vs SWR / React Query

SWR and React Query are data-fetching libraries. They manage HTTP request lifecycles, deduplication, background revalidation, and cache invalidation in the context of a single page app. They are framework-level dependencies with their own mental models.

Recached is a cache primitive. It has no concept of HTTP, components, or rendering. It is closer to Redis in the browser than to SWR. Use Recached when you need a shared, server-synchronized cache that multiple components (or multiple tabs) can read from. Use SWR or React Query when you need request deduplication and automatic revalidation of HTTP endpoints.

They can coexist: Recached for your server-synced live state, SWR for your HTTP data fetching.

## Recached vs Zustand / Redux

Zustand and Redux are UI state managers. They are excellent for component state, UI interactions, form state, and modal visibility. They have no concept of expiry or server sync.

Recached replaces the manual caching layer developers build on top of Zustand or Redux: the `fetchedAt` timestamp tracking, the staleness checks, the manual invalidation on mutation. It does not replace UI state management — it replaces the cache you bolted onto it.

## Recached vs TalaDB

[TalaDB](https://taladb.dev) is our sibling project at ThinkGrid Labs, and the two are deliberately complementary, not competing:

| | Recached | TalaDB |
|---|---|---|
| What it is | Cache + **sync fabric** between backend and clients | Embedded **database** inside the app |
| Data model | Keys — strings, collections, JSON | Documents with MongoDB-like queries, indexes, ACID transactions |
| Server | Optional for local caches; required for cross-device sync | None — runs entirely on-device |
| Superpower | Multi-client sync: scoped auth, live fan-out, offline outbox, deduplicated replay | On-device vector + hybrid search, rich queries |
| Truth model | **Shared truth** across users and devices | **Device-local truth** |

The one-line rule: **TalaDB is where one device's data lives; Recached is how many devices agree.** A notes app with on-device semantic search wants TalaDB. A shared cart, live dashboard, presence, or agent-output streaming wants Recached. An app that needs both — locally queryable data that also syncs across users — is exactly where the two are designed to meet: TalaDB's planned `SyncAdapter` interface can use Recached as its sync backbone.
