# Use Cases

Where Recached earns its place next to — or instead of — Redis and Memcached, and where it does not.

This page is written for the evaluation, not the pitch. If your problem is on the "reach for
something else" list, that is the honest answer.

## The one-sentence test

> **Does a browser, mobile app, or edge worker need to read the same data your backend writes?**

If **no** — you are caching database rows for your own API, computing fragment caches, storing
sessions read only by the server — Redis or Memcached is the right tool and Recached offers you
nothing they do not already do better, with a decade more hardening.

If **yes**, you are currently paying for that in one of three ways: polling, a bespoke WebSocket
layer, or a second client-side cache you invalidate by hand. Removing that cost is the entire
reason Recached exists.

There is a third answer the question does not cover: **no, and there is no server involved at all.**
The browser client is a complete cache on its own — see
[no server at all](#no-server-at-all) below. That is a narrower pitch
than the one this page is mostly about, and it is judged against `Map`, IndexedDB wrappers and
React Query rather than against Redis.

## Where the difference actually shows up

### Live dashboards and counters

**The usual shape.** Backend writes a metric to Redis. Frontend polls `GET /api/metrics` every few
seconds. You tune the interval as a tradeoff between staleness and load, and it is wrong in both
directions — stale for users, wasteful for the server.

**With Recached.** The backend does the same `INCR` it always did. Every subscribed browser gets the
new value pushed over the sync WebSocket and re-renders. No polling loop, no `/api/metrics` endpoint,
no interval to tune.

```typescript
const online = useKey('metrics:users:online'); // re-renders on every server-side change
```

The comparison is not "Redis is slow." Redis is fast. The comparison is that with Redis you still
have to *build the delivery mechanism*, and with Recached the cache is the delivery mechanism.

### Shared carts, collaborative state, presence

Multiple clients read and write the same keys and each needs to see the others' changes. Classically
this is Redis plus a pub/sub channel plus a WebSocket server plus reconnection and replay logic —
several hundred lines that are easy to get subtly wrong under packet loss.

Recached ships that path: writes replay from a durable outbox after reconnect, `incr`/`decr` merge
additively rather than clobbering, `jmerge` deep-merges documents, and a `DEDUP` envelope suppresses
ordinary reconnect replays. See [Offline & Reconnection](/browser/offline) for the server-crash boundary.

### Feature flags and config that must flip instantly

Flags read on every render are the pathological case for a network cache: either you round-trip
constantly or you cache locally and accept staleness. Recached holds the flag in WASM memory (read
cost: a local map lookup) while a server-side flip propagates in milliseconds. You get local-read
speed *and* immediate invalidation, which is normally a tradeoff.

### Offline-capable and flaky-network UIs

Reads are served from local memory whether or not the network is up. Writes queue in an
IndexedDB-backed outbox and replay on reconnect. Redis and Memcached have nothing to say here — they
are unreachable when the network is down, by definition.

### Cross-tab consistency

Multiple tabs of the same app share mutations through BroadcastChannel without any server round-trip.
With Redis this is either polling per tab or a `storage` event protocol you write yourself.

### No server at all

The client cache on its own. Omit `connect` and `recached-edge` never opens a socket. Nothing else changes: the same
`core-engine` state machine that runs on the server runs in WASM in the tab, so you get the full
local command surface with no backend of any kind.

```typescript
const cache = await createCache({
  persistence: true,            // IndexedDB WAL — survives refresh
  broadcastChannel: 'my-app',   // cross-tab — needs no server
})                              // no `connect` — nothing is networked
```

**What you get without a server:** reads and writes (`get`/`set`/`del`, `getJSON`/`setJSON`),
TTL that expires on its own, `incr`/`decr`, JSON documents with path writes and merge patches
(`jset`/`jget`/`jmerge`), glob snapshots over the keyspace (`getMatching`), change notification via
`onMutation`, refresh-survival through the IndexedDB WAL, and cross-tab fan-out through
BroadcastChannel.

**What is inert without a server**, because it has nothing to talk to — these do not throw, they
simply do nothing:

| API | Why it needs a server |
|---|---|
| `publish` / `subscribe` / `unsubscribe` / `onMessage` | Pub/sub is brokered by the server; there is no local loopback, and messages do **not** travel over BroadcastChannel |
| `liveQuery` | The initial state snapshot and the change stream are both server-sent |
| `syncToken` / `syncScopes` | Scope grants are a server-side authorization decision |
| `pendingWrites` / `onOutboxFull` | They describe a replay queue for a server that will never connect |

Cross-*device* sync is the other obvious absence: two browsers with no server between them share
nothing.

**Where this earns its place.** A TTL cache in front of `fetch` (the pattern in
[Getting Started](/browser/getting-started#without-a-server-local-only-cache)) is the common one —
declaring expiry once at write time instead of hand-rolling `fetchedAt` comparisons in a Zustand or
Redux store. Beyond that: state that must survive a refresh without you writing an IndexedDB schema,
tabs that must agree without a `storage`-event protocol, and read-heavy derived state where a
local map lookup per render is the point.

**Where it does not.** If you only need a request cache with revalidation, React Query and SWR do
that with far less machinery and no WebAssembly to load. `recached-edge` ships a ~550 KB `.wasm`
binary; that cost buys Redis semantics, and if you are not using them it buys nothing. The honest
framing: pick local-only Recached when you want the *data model* — TTLs, counters, JSON paths, glob
queries — not merely somewhere to park fetched JSON.

**One wart to know about.** With `persistence: true` and no `connect`, every write still records an
outbox row in IndexedDB for a replay that cannot happen, and after 10,000 of them you will see
`offline write queue full` warnings in the console. It is wasted I/O and a misleading message, not
data loss — the local store and its WAL are unaffected. Pass `persistence: false`, or ignore the
warning, until this is fixed.

## Where you should reach for something else

Being specific here is more useful than a feature grid.

**Pure server-side caching → Redis or Memcached.** Query result caches, rendered fragments,
rate-limit counters read only by your API. Recached will do this correctly, but you would be adopting
a young project to solve a problem two extremely mature ones already solve.

**Raw single-node throughput above all → measure the candidates yourself on your target hardware.** Recached publishes no cross-project comparison and does not claim to be the fastest cache server. Its worker threads provide parallel command execution, but that architectural property does not predict the fastest server for your command mix, pipeline depth, persistence settings, or hardware — and it does nothing for a workload concentrated on a few hot keys. Measure before switching.

**Pure memory-efficient blob caching at scale → Memcached.** Memcached's slab allocator and
multi-threaded simplicity are excellent for large, uniform, ephemeral values. Recached is not
optimized for that shape, and the sync machinery is dead weight if nothing subscribes.

**System of record → a database.** Recached is an in-memory cache with snapshot and AOF persistence.
It is not durable storage. See the persistence caveats in
[when Recached is not the right fit](/guide/introduction#when-recached-is-not-the-right-fit).

**Working set larger than RAM → a database, or Redis with eviction tuned.** There is no disk-backed
tier.

**You need Lua scripting, cluster mode, or `INFO`/`SLOWLOG` introspection → Redis.** These are
explicitly out of scope; see [Commands](/server/commands). RESP3 exists only for protocol
negotiation and pub/sub framing (`HELLO 3`), not the full type surface.

**You need several keys to change together, atomically → Redis.** Because Recached executes on every core, a command that spans keys (`MSET`, `SMOVE`) is not isolated from concurrent readers. `MULTI`/`EXEC` reserves its write keys against other writers, but a reader can still observe intermediate command results. Single-key commands are atomic, and `WATCH`-based compare-and-swap works. Use Redis when readers must observe a multi-key transaction as one indivisible state change.
See [Concurrency model](/server/commands#concurrency-model).

## Compared directly

| | Recached | Redis | Memcached |
|---|---|---|---|
| Server-side cache | Yes | Yes | Yes |
| Client reads without a network hop | **Yes** — WASM, in-process | No | No |
| Push to browsers on change | **Built in** | Pub/Sub + your own WS server | No |
| Offline writes + replay | **Built in** | No | No |
| Cross-tab sync | **Built in** | No | No |
| Data structures | Strings, hashes, lists, sets, sorted sets, JSON | All of those + streams, bitmaps, HLL, geo | Strings only |
| Persistence | Snapshot + AOF | RDB + AOF | None |
| Replication | Primary/replica + manually fenced promotion | Primary/replica + Sentinel + Cluster | None |
| Scripting | No (WASM scripting on roadmap) | Lua | No |
| Cluster / sharding | No | Yes | Client-side sharding |
| Threading | Multi-threaded — commands execute on every core | Threaded I/O (`io-threads`, off by default); command execution on one thread | Multi-threaded |
| Cross-key atomicity | Single-key only ([why](/server/commands#concurrency-model)) | Everything, including `MULTI`/`EXEC` | Single-key only |
| Command coverage | 123 | 250+ | ~15 |
| Maturity | Server release candidate; sync layer beta | 15+ years | 20+ years |
| License | Apache 2.0 | AGPLv3 / RSALv2 + SSPLv1 (BSD-3 up to 7.2) | BSD-3 |

Read the [maturity statement](/guide/introduction#maturity) before putting the sync layer in front of
users. The server half and the sync half are at different levels of hardening, and the docs do not
pretend otherwise.

## Using Recached alongside Redis

These are not mutually exclusive, and for most teams the honest first step is not a migration.

A common split: keep Redis for everything server-only — session storage, job queues, rate limiting,
query caches — and add Recached for the specific slice of state the UI needs live. That slice is
usually small (a dozen keys: presence, counters, flags, cart) and it is exactly the slice that costs
the most to build by hand.

This also bounds your risk. If the sync layer disappoints, you have replaced a polling loop, not your
cache tier.

## Migrating from Redis

Recached speaks RESP, so existing clients connect unchanged:

```javascript
const cache = new Redis('redis://127.0.0.1:6379'); // now pointing at Recached
```

Before switching a workload over, check:

1. **Command coverage.** Diff your actual command usage against [Commands](/server/commands).
   `MONITOR` on your existing Redis for a representative window is the fastest way to get that list.
   Lua scripts, cluster commands, streams, and `INFO`-based tooling will not carry over.
2. **Key encoding.** Values are binary-safe, but keys, hash fields, set members and glob patterns
   must be valid UTF-8 — Redis allows binary there. Applications that use binary keys are rare; if
   yours does, that is a blocker. See [Binary values](/guide/introduction#binary-values).
3. **Persistence expectations.** Confirm snapshot + AOF semantics match what you assume today.
4. **Replication topology.** Promotion requires external fencing and `REPLICAOF NO ONE`; there is no Sentinel or quorum election.
5. **Eviction.** Review the key cap and TTL behaviour in
   [Configuration](/server/configuration) against your memory budget.

There is no data migration path from an RDB file — treat it as a cold cache and let it fill.
