---
layout: home
title: "Recached: Rust cache for servers, browsers, and native apps"
description: "A Rust cache and sync engine for servers, browser WebAssembly, native Kotlin and Swift apps, and embedded Rust services."

hero:
  name: "Recached ⚡"
  text: "Shared cache. Local reads."
  tagline: "Rust on the server, WebAssembly in the browser, and native Kotlin and Swift previews. Shared state over WebSocket."
  image:
    src: /recached.jpg
    alt: Recached
  actions:
    - theme: brand
      text: Get Started
      link: /guide/quick-start
    - theme: alt
      text: How It Works
      link: /guide/how-it-works
    - theme: alt
      text: GitHub
      link: https://github.com/recached-sh/recached
    - theme: alt
      text: npm
      link: https://www.npmjs.com/package/recached-edge

features:
  - icon: ⚡
    title: Local reads without a network hop
    details: Browser, native mobile, and embedded Rust clients read their local cache in memory without a server round trip.
  - icon: 🔄
    title: Automatic WebSocket sync
    details: Live queries hydrate client data and receive changes. Scoped tokens control access, and reconnects replay queued writes before refreshing watched state.
  - icon: 🦀
    title: Redis-compatible command subset
    details: Speaks RESP on port 6379 and works with common clients such as ioredis, node-redis, and redis-py. Check COMMAND for the supported subset before migrating.
  - icon: 🌐
    title: Native mobile previews
    details: Kotlin Flow and Swift AsyncStream with SQLite data and outbox persistence. Saved server state is readable offline after an app restart. Build from source while packages await release.
  - icon: 📡
    title: Cross-tab sync
    details: BroadcastChannel support means tabs on the same origin and channel share local writes, with no server connection required.
  - icon: 🔒
    title: Hardened cache server
    details: TLS, Prometheus metrics, password authentication, IP allowlists, connection limits, bounded eviction, and ordered replication. Release-candidate maturity.
---

## Cache shared state in your application

Recached runs one Rust engine on the server, in browser WebAssembly, and inside native apps and Rust services. Your backend writes through the RESP command subset. Clients read local memory and synchronize shared state over WebSocket.

Choose [the browser SDK](/browser/getting-started), [Android with Kotlin](/android/getting-started), [iOS or macOS with Swift](/ios/getting-started), or [embedded Rust](/rust/getting-started). Kotlin and Swift are implemented previews that currently require source builds. [Client support and limits](/guide/client-support) explains their API and persistence differences.

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

## Use a local cache without a server

Omit `connect` in the browser, or omit the URL in a mobile `RecachedConfig`, to keep data on the client. Browser persistence uses IndexedDB; native SDKs save to SQLite. Cross-device sync needs a server. See [local-only use cases](/guide/use-cases#no-server-at-all) and the [outbox limits](/guide/client-support#write-durability-and-replay).
