---
layout: home
title: "Recached: local-first cache and real-time sync engine"
description: "A local-first cache and real-time sync engine, powered by Rust. Read shared cache data from local memory in browsers, Android and Apple apps, and Rust services, and sync updates over WebSocket."

hero:
  name: "Recached"
  text: "Cache locally. Sync everywhere."
  tagline: "A local-first cache and real-time sync engine, powered by Rust. Keep shared cache data in your application, read it from local memory, and sync updates across browser, mobile, and Rust clients."
  image:
    src: /brand/hero.svg
    alt: Connected R and synchronized server, browser, mobile, and Rust clients
  actions:
    - theme: brand
      text: Get Started
      link: /guide/quick-start
    - theme: alt
      text: Explore Architecture
      link: /guide/how-it-works
    - theme: alt
      text: GitHub
      link: https://github.com/recached-sh/recached

features:
  - icon:
      src: /brand/icons/local-reads.svg
      width: 24
      height: 24
      wrap: true
    title: Local reads
    details: Browser, Android, Apple, and embedded Rust clients read cached values from their own memory instead of making a server request for every read.
    link: /guide/how-it-works#read-and-write-locally
    linkText: Read and write locally
  - icon:
      src: /brand/icons/real-time-sync.svg
      width: 24
      height: 24
      wrap: true
    title: Real-time sync
    details: Subscribe to key patterns to receive a snapshot and later changes over WebSocket. Reconnecting clients replay queued writes, then refresh what they watch.
    link: /guide/how-it-works#connect-and-hydrate
    linkText: Connect and hydrate
  - icon:
      src: /brand/icons/rust-engine.svg
      width: 24
      height: 24
      wrap: true
    title: One Rust engine
    details: The server and every client run the same core-engine. Each platform adds its own transport, persistence, and reactive API, so behavior differs by client.
    link: /guide/client-support
    linkText: Compare clients
  - icon:
      src: /brand/icons/offline.svg
      width: 24
      height: 24
      wrap: true
    title: Offline-capable caching
    details: Persist browser data to IndexedDB or native data to SQLite. Writes made while disconnected queue and replay on reconnect, within outbox limits.
    link: /guide/client-support#write-durability-and-replay
    linkText: Durability and replay
  - icon:
      src: /brand/icons/server.svg
      width: 24
      height: 24
      wrap: true
    title: Redis-compatible server interface
    details: Connect ioredis, node-redis, or redis-py over RESP using the supported command subset. The server adds TLS, authentication, metrics, persistence, and replication.
    link: /server/commands
    linkText: Supported commands
  - icon:
      src: /brand/icons/scoped-access.svg
      width: 24
      height: 24
      wrap: true
    title: Scoped sync access
    details: Backend-minted sync tokens limit which keys each connection may read and write. Watching a pattern never grants access to it.
    link: /server/sync-scopes
    linkText: Sync scopes
---

## Keep shared cache data close to your application

Recached runs one Rust cache engine on the server, in browsers as WebAssembly, inside native Android and Apple apps, and in embedded Rust services. Your backend writes through the RESP command subset with the Redis client it already uses. Clients hold the keys they watch in local memory and receive changes over WebSocket, so a UI can render cached data without a request per read and update when the backend changes it.

Recached is a cache and synchronization engine, not a system of record. Keep MySQL or PostgreSQL as the source of truth and publish committed state into Recached. [What Recached is — and isn't](/guide/introduction#what-recached-is-and-isnt) draws the line.

## Architecture

<figure>
  <img class="light-only" src="/architecture-light.svg" alt="Backend RESP clients connect to the Recached server on port 6379. Browser, native Android and Apple apps, and embedded Rust services sync over WebSocket on port 6380 and read local memory.">
  <img class="dark-only" src="/architecture-dark.svg" alt="Backend RESP clients connect to the Recached server on port 6379. Browser, native Android and Apple apps, and embedded Rust services sync over WebSocket on port 6380 and read local memory.">
</figure>

- **Browser:** `recached-edge` runs the engine as WebAssembly, with optional IndexedDB persistence and React and Vue adapters.
- **Android and Apple:** Kotlin and Swift SDKs run the engine natively through UniFFI and persist data and queued writes to SQLite.
- **Rust services:** `recached-embed` holds a watched slice of the cache in your process's memory.
- **Recached server:** executes RESP commands from your backend, persists snapshots and an optional AOF, and fans changes out to permitted clients.
- **Your database (optional):** stays the source of truth. Your backend or an outbox worker publishes committed state to Recached. There is no built-in SQL connector; see [database integration](/guide/database-integration).

[How it works](/guide/how-it-works) covers hydration, reconnect replay, and each client's storage contract.

## Hydrate, then read locally

Register a live query to receive initial server state and later changes:

```typescript
import { createCache } from 'recached-edge'

const cache = await createCache({
  connect: { url: 'ws://localhost:6380' },
})
cache.liveQuery('inventory:*')
cache.onKeyChange('inventory:item:99', () => {
  console.log(cache.get('inventory:item:99'))
})
```

Reads can return a miss before the initial snapshot arrives. On mobile, call `watch(pattern)` to subscribe, then use Kotlin `Flow` or Swift `AsyncStream` to observe local values. [How it works](/guide/how-it-works) covers hydration and reconnect recovery.

## Use cases

**Real-time interfaces.** A backend `INCR` on `metrics:orders:today` reaches every subscribed dashboard without a polling loop or a separate push endpoint. The same path carries shared carts, presence, typing indicators, and reactive UI state.

**Offline-capable apps.** An Android or iOS field app reopens without a network and still reads the reference data and configuration it last synced from SQLite. Writes queue locally and replay when the connection returns. Browser persistence saves local writes but not every server update; see [browser persistence](/browser/persistence).

**Backend services.** A Rust service reads feature flags or tenant configuration from local memory on every request. A change made through the server reaches each instance, and a reconnect re-hydrates whatever the instance missed.

[Use cases](/guide/use-cases) explains each pattern, and where Redis, Memcached, or a database is the better tool.

## Choose your runtime

| Runtime | Package | Status |
|---|---|---|
| [Browser](/browser/getting-started), [React](/react/getting-started), [Vue](/vue/getting-started) | `recached-edge`, `@recached/react`, `@recached/vue` on npm | Sync layer beta |
| [Android](/android/getting-started) | `dev.recached:recached-android` on Maven Central | 0.1.0 preview |
| [iOS and macOS](/ios/getting-started) | `Recached` via SwiftPM | 0.1.0 preview |
| [Rust service](/rust/getting-started) | `recached-embed` as a Git dependency | Unpublished |
| [Server](/server/installation) | Docker image, Homebrew, or `cargo install --git` | Release candidate for cache workloads |

[Client support and limits](/guide/client-support) explains API, persistence, and delivery differences. The [quick start](/guide/quick-start) runs a server and connects a backend and a browser client.

## Use a local cache without a server

Omit `connect` in the browser, or omit the URL in a mobile `RecachedConfig`, to keep data on the client. Browser persistence uses IndexedDB; native SDKs save to SQLite. Cross-device sync needs a server. See [local-only use cases](/guide/use-cases#no-server-at-all) and the [outbox limits](/guide/client-support#write-durability-and-replay).
