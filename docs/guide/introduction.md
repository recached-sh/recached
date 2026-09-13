# Introduction

## What Recached is

Recached is an in-memory cache server written in Rust. It speaks RESP (the Redis Serialization Protocol) on port 6379, so common clients such as `ioredis`, `node-redis`, `redis-py`, and `Jedis` can use its documented command subset.

Values are binary-safe, as they are in Redis: a value is stored and returned as the exact bytes you
sent. *Keys* and other identifiers must be text — see [Binary values](#binary-values).

That is where the similarity with Redis ends — in two directions.

**Recached is multi-threaded by default, command path included.** Rust's ownership model makes
cross-thread state sharing a build-time property, so Recached threads execution itself over a
sharded keyspace with no configuration. You can
[verify the scaling directly](/guide/benchmarks#thread-scaling) by varying the worker count and
nothing else.

Three qualifications matter. This is an architectural property, **not** a throughput claim, and
certainly not a claim about any other cache — the project publishes no cross-project comparison, so
measure the exact releases and configuration you plan to run. Scaling also depends on your writes
being spread across keys: a single hot key lives on a single shard and does not get faster with more
workers. And it does not provide fully isolated cross-key reads. See
[Concurrency model](/server/commands#concurrency-model).

**The distinguishing feature is elsewhere: the same engine runs where there is no server at all.**
That is the `core-engine` crate: a pure Rust state machine with no network dependencies, no file I/O, and no OS-specific code. It compiles to native x86-64/ARM64 for the server **and** to `wasm32-unknown-unknown` for the browser. Both targets run the same cache logic from the same source. The WebSocket sync layer (port 6380) keeps the two sides consistent in real time.

The result: your backend caches data over RESP as it always has, and every connected browser instance holds a local copy of the cache in WASM memory. Frontend reads never leave the process — no network hop, no serialization, sub-microsecond in practice. Frontend writes propagate to the server and fan out to all other connected clients.

## The core insight

Every caching solution today forces a choice:

- **Server-side cache (Redis, Memcached):** Every frontend read is a network round-trip. The browser is just a display layer; all state lives on the server.
- **Client-side state (Zustand, Redux, SWR):** State lives in the browser, but you write manual staleness checks, manual invalidation, manual sync code. Two caches emerge: one on the server and one in every client.

Recached removes the choice. The `core-engine` is the cache. It runs in both places. The network layer is not a read path — it is only a sync path. Reads always come from local memory.

## Architecture

<figure>
  <img class="light-only" src="/architecture-light.svg" alt="Your backend writes to the Recached server over RESP on port 6379. The server syncs over a WebSocket on port 6380 to the browser or edge runtime, where reads are served from local WebAssembly memory. Writes flow back the same way.">
  <img class="dark-only" src="/architecture-dark.svg" alt="Your backend writes to the Recached server over RESP on port 6379. The server syncs over a WebSocket on port 6380 to the browser or edge runtime, where reads are served from local WebAssembly memory. Writes flow back the same way.">
</figure>

Three crates with hard dependency boundaries:

| Crate | Role |
|---|---|
| `core-engine` | Pure state machine — no networking, no I/O. RESP parser, typed command dispatch, sharded lock-free store (`DashMap`), TTL engine, optional key cap. Compiles to both native and `wasm32`. |
| `server-native` | Tokio TCP server (port 6379) + WebSocket server (port 6380). Persistent read buffers handle fragmented RESP. Per-connection pub/sub via `mpsc` channels. Connection semaphore, auth rate-limiting, sender-ID broadcast filter. |
| `wasm-edge` | `wasm-bindgen` JS bindings. Local reads without a network hop, RESP-over-WebSocket sync. Closure lifecycle managed to avoid memory leaks on reconnect. |

## When to use Recached

Recached is a good fit when:

- **Your frontend reads the same data your backend writes.** User sessions, feature flags, live counters, cart state, active user lists — anything your backend mutates that the UI needs to display instantly.
- **You want live UI without polling.** The WebSocket sync replaces a polling loop without requiring you to build a separate SSE or WebSocket server.
- **You want a frontend-only cache with TTL.** The WASM module works entirely without a server. Call `createCache()` without `connect` and you get a local cache with TTLs, counters, JSON documents, glob queries, optional IndexedDB persistence and cross-tab sync — no Recached server, no Redis, no backend changes required. Pub/sub, live queries and cross-device sync are what you give up; see [no server at all](/guide/use-cases#no-server-at-all) for the full boundary.
- **You need cross-tab sync.** BroadcastChannel support means all open tabs in the same browser share mutations automatically.
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
- **The sync layer (browser sync, live queries, offline outbox, scoped auth) is beta.** The invariants are [specified](/server/protocol), tested, and verified end-to-end — but the code is young and hasn't accumulated real-world miles or third-party security review yet. Concretely: don't expose the WebSocket port to the public internet for multi-tenant data until you've read [Sync Scopes](/server/sync-scopes) and understood the model, and expect occasional sharp edges.

The road to 1.0 is hardening, not features: fuzzing the parser surfaces, automated browser testing, a security pass on the token path, and a protocol freeze once real-world usage has confirmed the design. Bug reports from production-like use are the most valuable contribution the project can receive right now.

## Recached vs Redis

| | Recached | Redis |
|---|---|---|
| Protocol | RESP (compatible) | RESP |
| Browser-side cache | Yes — WASM | No |
| WebSocket sync | Built-in | Not built-in |
| Persistence | Snapshot + AOF | RDB + AOF |
| Replication | Primary/replica + manually fenced promotion | Yes (+ Sentinel/Cluster) |
| Lua scripting | No (WASM scripting on roadmap) | Yes |
| Cluster mode | No | Yes |
| Command coverage | 123 commands | 250+ |
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
| Server | The server is the product (Redis-compatible) | None — runs entirely on-device |
| Superpower | Multi-client sync: scoped auth, live fan-out, offline outbox, deduplicated replay | On-device vector + hybrid search, rich queries |
| Truth model | **Shared truth** across users and devices | **Device-local truth** |

The one-line rule: **TalaDB is where one device's data lives; Recached is how many devices agree.** A notes app with on-device semantic search wants TalaDB. A shared cart, live dashboard, presence, or agent-output streaming wants Recached. An app that needs both — locally queryable data that also syncs across users — is exactly where the two are designed to meet: TalaDB's planned `SyncAdapter` interface can use Recached as its sync backbone.
