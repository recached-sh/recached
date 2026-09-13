<div align="center">
  <img src="recached.jpg" alt="Recached" width="800" />
  <h1>Recached</h1>
  <p><b>A multi-threaded Rust cache that runs on your backend <em>and</em> inside the browser.</b></p>
  <p>Every core on the server, every tab in the browser — one engine.</p>

  <a href="https://recached.dev"><img src="https://img.shields.io/badge/Docs-recached.dev-blue.svg" alt="Docs"></a>
  <a href="https://www.npmjs.com/package/recached-edge"><img src="https://img.shields.io/npm/v/recached-edge?label=npm" alt="npm"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Language-Rust-orange.svg?logo=rust" alt="Rust"></a>
  <a href="https://webassembly.org"><img src="https://img.shields.io/badge/Ecosystem-WebAssembly-yellow.svg" alt="Wasm"></a>
  <a href="LICENSE.md"><img src="https://img.shields.io/badge/License-Apache_2.0-green.svg" alt="Apache 2.0"></a>
</div>

---

Every caching solution forces a choice: server-side caches like Redis mean every frontend read is a network round-trip; client-side state like Zustand or SWR means two caches — one on the server and one in every client, with manual staleness code gluing them together. **Recached removes the choice.**

The same Rust cache engine runs natively on your server (RESP on port 6379) and as WebAssembly inside the browser. Common Redis clients work with Recached's documented command subset. Browser reads come from local WASM memory; the WebSocket is a sync path, not a read path.

**Multi-threaded is the default, not a flag.** Recached executes commands on every core, over a sharded keyspace, with no configuration. Redis and Valkey keep the command path on a single thread and offer *I/O* threading as an opt-in (`io-threads`, off by default) — a reasonable choice in C, where sharing mutable state across threads is checked by review rather than by the compiler. Rust's ownership model makes that checkable at build time, so threading the command path is a design decision rather than a risk to be opted into. You can [verify the scaling directly](#benchmarks) by varying the worker count and nothing else.

**And the round-trip is the part no server-side cache can answer.** Redis and Valkey can be tuned, sharded and scaled, and every frontend read still costs a network hop, because the hop *is* the architecture. That is the half of Recached with no equivalent.

> [!NOTE]
> Recached is not a full Redis replacement. It covers the subset most applications actually need: strings, expiry, counters, all collection types, transactions, pub/sub, and observable keys. Best fit: reactive UIs, session caches, browser-side API response caching, and rate limiting.
>
> Notably absent: **Lua scripting (`EVAL`)**, **blocking operations** (`BLPOP`, `BRPOP`, `LMOVE`) and **streams** (`XADD`). Your Redis *client* will connect unchanged, but libraries built on those primitives — BullMQ, node-redlock, rate-limiter-flexible — ship Lua and will not run. `RLCHECK`/`RLSET` cover rate limiting natively instead. Run `COMMAND COUNT` against a live server for the exact surface (123 commands today).

**→ Full documentation, use cases, API reference, and guides at [recached.dev](https://recached.dev)**

---

## Install

```bash
# Docker
docker run -p 6379:6379 -p 6380:6380 ghcr.io/recached-sh/recached:latest

# Homebrew (macOS) — the tap is this repo, and Homebrew 6+ wants third-party taps trusted
brew tap recached-sh/recached https://github.com/recached-sh/recached.git
brew trust recached-sh/recached   # Homebrew 6.0+ only; older versions do not have it
brew install recached && recached-server

# Cargo (from source — the crate is not on crates.io yet)
cargo install --git https://github.com/recached-sh/recached.git recached && recached-server
```

```bash
# Browser / Edge (npm)
npm install recached-edge
```

```bash
# Rust service (embedded client — reads come from local process memory)
cargo add --git https://github.com/recached-sh/recached.git recached-embed
```

> [!IMPORTANT]
> **Install `recached-edge@^0.3.4`.** Every published version from 0.1.3 to 0.3.0 shipped without
> wasm-pack's `snippets/` directory and failed to import at all; 0.3.1 is the first release that
> installs from npm. See the [changelog](CHANGELOG.md) for details.

---

## How it works

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/architecture-dark.svg">
    <img src="assets/architecture-light.svg" width="880" alt="Your backend writes to the Recached server over RESP on port 6379. The server syncs over a WebSocket on port 6380 to the browser or edge runtime, where reads are served from local WebAssembly memory. Writes flow back the same way.">
  </picture>
</p>

Any mutation on the server is pushed to all connected browser instances automatically. Any write from the browser is pushed to the server and fanned out to other clients. Reads always come from local WASM memory — no network hop.

A **Rust service** can be one of those connected clients too, via [`recached-embed`](https://recached.dev/rust/getting-started) — same engine, same sync protocol, holding its slice of the cache in its own heap instead of a tab's. Useful for config-shaped data read on every request: fare tables, feature flags, tenant settings, entitlement checks.

---

## Quick look

**Backend** — a RESP client using the supported command subset, port 6379:

```javascript
import Redis from 'ioredis';
const cache = new Redis('redis://127.0.0.1:6379');
await cache.set('inventory:item:99', '42');
```

**Browser** — WebAssembly, port 6380:

```typescript
import { createCache } from 'recached-edge';

const cache = await createCache({
  persistence: true,                        // survives page refresh via IndexedDB
  connect: { url: 'ws://127.0.0.1:6380' }, // syncs with the server
});

cache.get('inventory:item:99'); // "42" — from local WASM memory, 0 ms
```

Both examples are plaintext, which is the default. Set `RECACHED_TLS_CERT` and `RECACHED_TLS_KEY`
and the same ports serve TLS — connect with `rediss://` and `wss://` instead. Before exposing either
port beyond localhost, work through
[recached.dev/server/security](https://recached.dev/server/security): a default server has no
password, no TLS, and no restriction on which web pages may open the sync socket.

`6379` and `6380` are defaults, not fixtures — set `RECACHED_PORT` and `RECACHED_WS_PORT` (plus
`RECACHED_METRICS_PORT`) to move them, which is also what running two instances on one host takes.

**The browser half also runs alone.** Drop `connect` and `recached-edge` never opens a socket — the
same engine runs in WASM as a standalone client cache with TTLs, counters, JSON documents, glob
queries, IndexedDB persistence and cross-tab sync, with no Recached server and no backend changes:

```typescript
const cache = await createCache({ persistence: true, broadcastChannel: 'my-app' });
cache.setJSON('user:42', user, 60); // expires on its own, survives a refresh
```

What you give up is what needs a peer: pub/sub, live queries and cross-device sync. See
[use cases: no server at all](https://recached.dev/guide/use-cases#no-server-at-all).

---

## Benchmarks

Recached's measured performance claim is narrow: command execution scales across worker threads. The project does not publish a current Redis or Valkey comparison. The previous three-way table used Recached v0.1.8 and Redis 7.2.5, so it was removed instead of presenting stale results as current evidence.

The scaling run below changed only `RECACHED_WORKER_THREADS`. One binary, one workload, one fixed four-core CPU set — Intel i5-9400F, `powersave` governor, `PIN=1 SERVER_CPUS=0-3 BENCH_CPUS=4-5`, no persistence. Measured 2026-09-13 with `redis-benchmark` 8.10.1 (`-n 1000000 -c 50 -d 64 -r 100000 -P 16`):

| Worker threads | 1 | 2 | 4 |
|---|---:|---:|---:|
| `GET` | 819,672 | 1,689,189 | 1,658,375 |
| `SET` | 316,857 | 580,720 | 769,823 |
| `INCR` | 330,688 | 602,047 | 761,615 |
| **Total, keys spread over 100k** | **1,467,217** | **2,871,956** | **3,189,812** |
| Change from one thread | baseline | +96% | **+117%** |

**Scaling requires your writes to be spread across keys.** `redis-benchmark`'s collection tests (`LPUSH`, `SADD`, `HSET`, `ZADD`) push every operation into a single key, and that workload does not scale — it *regresses* about 27%, from 1,550,566 req/s on one thread to 1,134,772 on four, because one key lives on one shard and extra workers only add contention. Which half describes your deployment depends on whether you have hot keys. See [the benchmark guide](https://recached.dev/guide/benchmarks) for both tables.

One thread is the baseline because that is how Redis and Valkey execute commands — it is not a recommended deployment. This is evidence for parallel command execution on this build and host, not a cross-project claim. Run [`scripts/bench-scaling.sh`](scripts/bench-scaling.sh) against the commit you plan to deploy.

On the same host, a server-side write reaches a subscribed browser over WebSocket in **151 µs at p50** (p99 698 µs) — measured with the project's own harness, since no RESP benchmark can see that path. Browser *reads* are a local WebAssembly memory lookup and never leave the tab.

For a current cross-project run, use [`scripts/bench-docker.sh`](scripts/bench-docker.sh). It pins server and load-generator CPU sets, records image versions, measures pipelined and unpipelined workloads, and writes RSS delta per live key for strings, small hashes, and small sets. Publish the generated `conditions.txt` with any numbers.

Recached's product distinction remains the browser engine: browser reads use local WebAssembly memory and avoid a server round trip.

## Maturity

Being honest about where things stand:

- **The cache server is a release candidate for cache workloads.** It includes persistence, ordered primary/replica replication, TLS, hardened parsers, metrics, and load/chaos tests. It has not completed broad production validation or an independent security audit. Treat it as a cache, not a system of record.
- **The sync layer (browser sync, live queries, offline outbox, scoped auth) is beta** — the invariants are [specified](https://recached.dev/server/protocol) and tested end-to-end, but the code is young and hasn't had real-world miles or third-party security review yet. Don't put the WebSocket port on the public internet for multi-tenant data without reading [Sync Scopes](https://recached.dev/server/sync-scopes) first.

- **The embedded Rust client (`recached-embed`) is brand new and unpublished** — it works end-to-end against a live server and is covered by a live test suite, but it has no production miles and is not on crates.io.

The road to 1.0 is hardening, not features. Bug reports from production-like use are the most valuable contribution right now.

---

## Contributing

Bug reports, PRs, and feedback are all welcome.

1. Fork the repo and create a branch: `git checkout -b feat/my-feature`
2. Make your changes — server logic lives in `server-native/`, WASM bindings in `wasm-edge/`
3. Run `cargo test --workspace` before opening a PR
4. Open a pull request with a clear description

Open an issue before large features or architectural changes. Areas where contributions are especially welcome:

- **Benchmarks** — run [`scripts/benchmark.sh`](scripts/benchmark.sh) on multi-core server hardware and share the results
- **Client examples** — React, Vue, or SvelteKit demos using `recached-edge`
- **Bug reports** — edge cases in the RESP parser, TTL eviction, pub/sub delivery, or WebSocket sync

See [recached.dev/roadmap](https://recached.dev/roadmap) for what's planned.

Reach out: [dennis@thinkgrid.dev](mailto:dennis@thinkgrid.dev)

## License

Apache License 2.0 — © 2026 ThinkGrid Labs
