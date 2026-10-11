# Getting Started (Browser)

::: danger Install `recached-edge@^0.3.1` — every earlier version is unusable
Two separate defects, both fixed as of **0.3.1**:

**Packaging (0.1.3 – 0.3.0).** The published tarball omitted wasm-pack's `snippets/` directory,
which the generated glue imports on its first line, and published wasm-pack's `pkg/` output instead
of the SDK — so `npm install recached-edge` failed at module resolution before any application code
ran, and `createCache` was never on npm at all. Fixed by publishing the SDK package (with
`snippets/`) and gating every release on a tarball that is packed and imported in CI.

**Clock panic (0.1.1 – 0.2.0).** `core-engine` read the clock via `std::time::SystemTime::now()`,
which **panics** on `wasm32-unknown-unknown`. The clock is read on nearly every operation, so **no
store write completes** on those versions — and a client whose write-ahead log passed the compaction
threshold erased its own persisted cache before the replacement snapshot was written. Fixed in 0.2.1.

The Recached **server is unaffected** by both: it runs on a native target where the clock works
normally, and it ships as a binary rather than through npm.
:::

The `recached-edge` package is the TypeScript SDK for the browser WASM client. It gives you a `Cache` class backed by the same `core-engine` as the server, with optional WebSocket sync to a Recached server instance.

## Install

```bash
npm install recached-edge
# or
pnpm add recached-edge
# or
yarn add recached-edge
```

---

## Initialize

Use `createCache()` — it initializes the WASM module and returns a ready `Cache` instance.

```typescript
import { createCache } from 'recached-edge'

// Local-only, in-memory cache (no server connection)
const cache = await createCache()
```

For bundlers that support top-level await (Vite, Next.js, modern webpack):

```typescript
// lib/cache.ts — shared singleton
import { createCache } from 'recached-edge'

export const cache = await createCache()
```

---

## Connect to a server

Pass `connect` to `createCache()` to enable WebSocket sync. Omit it for local-only mode.

```typescript
import { createCache } from 'recached-edge'

// Connect to the Recached server WebSocket port
const cache = await createCache({
  connect: { url: 'ws://localhost:6380' },
})

// With TLS (production)
const cache = await createCache({
  connect: { url: 'wss://cache.yourdomain.com:6380' },
})

// With auth (if RECACHED_PASSWORD is set on the server)
const cache = await createCache({
  connect: { url: 'ws://localhost:6380', password: 'your-secret' },
})
```

Once connected, permitted server mutations update the local WASM store. Register `cache.liveQuery(pattern)` for an initial snapshot and reconnect reconciliation; connecting alone does not fetch existing keys. Local writes queue for server delivery, and accepted changes fan out according to sync scopes. See [client support and limits](/guide/client-support).

---

## Basic usage

```typescript
// Strings
cache.set('theme', 'dark')
console.log(cache.get('theme')) // 'dark'

// With expiry (seconds)
cache.setEx('session:token', 'abc123', 3600)

// Check existence and TTL
cache.exists('theme')       // true
cache.ttl('session:token')  // remaining seconds

// Delete
cache.del('theme')
cache.get('theme')          // null

// React to changes to one key (from any source — local, server, or other tabs)
const unsubscribe = cache.onKeyChange('cart:count', () => {
  const count = cache.get('cart:count')
  console.log('Cart count is now:', count)
})

// Stop listening
unsubscribe()
```

---

## `createCache` options

```typescript
import { createCache } from 'recached-edge'

const cache = await createCache({
  // Enable IndexedDB persistence (survives page refresh)
  persistence: true,

  // BroadcastChannel name for cross-tab mutation sharing
  broadcastChannel: 'my-app-cache',

  // Connect to the Recached server WebSocket port
  connect: {
    url: 'ws://localhost:6380',
    // Server password (if RECACHED_PASSWORD is set)
    password: 'your-secret',
  },
})
```

All three options are independent — you can use persistence and cross-tab sync without a server connection.

---

## React

If you are using React, install the official hooks package instead:

```bash
npm install @recached/react
```

```tsx
import { RecachedProvider, useKey } from '@recached/react'

function App() {
  return (
    <RecachedProvider options={{ connect: { url: 'ws://localhost:6380' } }}>
      <CartBadge userId={42} />
    </RecachedProvider>
  )
}

function CartBadge({ userId }: { userId: number }) {
  const count = useKey(`cart:${userId}:count`)
  return <span className="badge">{count ?? '0'}</span>
}
```

See the [React hooks docs](/react/getting-started) for the full guide.

---

## Vue

If you are using Vue 3, install the official composables package instead:

```bash
npm install @recached/vue
```

```ts
// main.ts
import { createApp } from 'vue'
import { RecachedPlugin } from '@recached/vue'
import App from './App.vue'

const app = createApp(App)
app.use(RecachedPlugin, { connect: { url: 'ws://localhost:6380' } })
app.mount('#app')
```

```vue
<!-- CartBadge.vue -->
<script setup lang="ts">
import { useKey } from '@recached/vue'
const props = defineProps<{ userId: number }>()
const count = useKey(`cart:${props.userId}:count`)
</script>

<template>
  <span class="badge">{{ count ?? '0' }}</span>
</template>
```

See the [Vue composables docs](/vue/getting-started) for the full guide.

---

## Without a server (local-only cache)

Do not pass `connect` to `createCache()`. The WASM module runs as a pure in-memory cache with TTL — no server, no WebSocket, no backend changes required.

This is a supported mode, not a degraded one: the same `core-engine` that runs on the server runs in
the tab, so the local command surface is identical either way.

```typescript
const cache = await createCache({
  persistence: true,            // IndexedDB WAL — survives refresh, no server needed
  broadcastChannel: 'my-app',   // cross-tab fan-out — no server needed
})                              // no `connect` — nothing is networked
```

**Works with no server:** `get`/`set`/`del`, `getJSON`/`setJSON`, `getBytes`/`setBytes`, `setEx` and
TTL expiry, `exists`/`ttl`, `incr`/`decr`, `jset`/`jget`/`jmerge`, `getMatching`, `onKeyChange`/`onMutation`,
`persistence`, `broadcastChannel`.

**Silently does nothing with no server** — these do not throw, they have nowhere to send to:
`publish`, `subscribe`/`unsubscribe`/`onMessage` (pub/sub is server-brokered and does *not* fall back
to BroadcastChannel), `liveQuery`, `syncToken`/`syncScopes`, and `pendingWrites`/`onOutboxFull`.

::: warning `persistence: true` with no `connect`
Every write still records an outbox row in IndexedDB for a replay that can never happen, and past
10,000 writes the console shows `offline write queue full`. Wasted I/O and a misleading warning —
your data and the WAL are fine. Use `persistence: false` if the noise matters to you.
:::

```typescript
import { createCache } from 'recached-edge'

const cache = await createCache() // no connect option — local-only

async function getUser(id: number): Promise<User> {
  const key = `user:${id}`
  const cached = cache.getJSON<User>(key)
  if (cached !== null) return cached

  const user: User = await fetch(`/api/users/${id}`).then(r => r.json())
  cache.setJSON(key, user, 60) // cache for 60s
  return user
}

async function getProducts(): Promise<Product[]> {
  const cached = cache.getJSON<Product[]>('products')
  if (cached !== null) return cached

  const products: Product[] = await fetch('/api/products').then(r => r.json())
  cache.setJSON('products', products, 300) // cache for 5 minutes
  return products
}

// Invalidate on mutation
async function updateUserName(id: number, name: string): Promise<void> {
  await fetch(`/api/users/${id}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name }),
  })
  cache.del(`user:${id}`) // next call to getUser() will refetch
}
```

This pattern replaces the manual `fetchedAt` timestamp approach you might use with Zustand or Redux. TTL is declared once at write time; `get()` returns `null` automatically when the entry has expired.

---

## Manual reactivity (non-React frameworks)

`onKeyChange(key, cb)` fires whenever that key changes — from a local write, a server push, or a cross-tab BroadcastChannel message. It is the low-level hook `useKey` and `useKeyJSON` use internally; `onPatternChange(pattern, cb)` does the same for a glob pattern, and `onMutation(cb)` fires on every change to any key.

```typescript
// Svelte
import { onMount, onDestroy } from 'svelte'
import { writable } from 'svelte/store'
import { cache } from '../lib/cache'

export let productId: string

const key = `stock:${productId}`
const stock = writable<string | null>(cache.get(key))

let unsubscribe: () => void

onMount(() => {
  stock.set(cache.get(key))
  unsubscribe = cache.onKeyChange(key, () => stock.set(cache.get(key)))
})

onDestroy(() => unsubscribe?.())
```

The callback receives no arguments — it signals that something changed. Read the specific key you care about inside the callback.

---

## Bundler configuration

### Vite

Vite handles WASM imports natively. No extra config needed for most setups.

If you see issues with the WASM file not being served, add to `vite.config.ts`:

```typescript
import { defineConfig } from 'vite'

export default defineConfig({
  optimizeDeps: {
    exclude: ['recached-edge'],
  },
})
```

### Next.js (App Router)

The cache is browser-only: `createCache()` is async, fetches a `.wasm` file, and touches
`indexedDB` and `BroadcastChannel`. So it must be created in a client component after hydration,
never at module scope in anything the server renders.

`@recached/react` does this for you — its provider builds the cache in an effect:

```tsx
// app/providers.tsx
'use client'

import { RecachedProvider } from '@recached/react'

export function Providers({ children }: { children: React.ReactNode }) {
  return (
    // Local-only: drop `connect` and no socket is ever opened.
    <RecachedProvider options={{ persistence: true, broadcastChannel: 'my-app' }}>
      {children}
    </RecachedProvider>
  )
}
```

```tsx
// app/layout.tsx — a server component; only Providers is client-side
import { Providers } from './providers'

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body><Providers>{children}</Providers></body>
    </html>
  )
}
```

::: warning The provider renders `null` until the cache is ready
`createCache()` resolves in an effect, which does not run during SSR — so anything inside
`<RecachedProvider>` is absent from the server-rendered HTML and appears on hydration. Wrapping your
entire app therefore opts the whole page out of SSR. Mount it around the subtree that actually reads
the cache, and keep content you need server-rendered (or indexed) outside it.
:::

Without the React SDK, do the same thing by hand — build the cache in `useEffect`, or reach for
`next/dynamic` with `ssr: false` on the component that uses it:

```tsx
'use client'

import { useEffect, useState } from 'react'
import { createCache, type Cache } from 'recached-edge'

export function useLocalCache(): Cache | null {
  const [cache, setCache] = useState<Cache | null>(null)
  useEffect(() => {
    let cancelled = false
    createCache({ persistence: true }).then((c) => !cancelled && setCache(c))
    return () => { cancelled = true }
  }, [])
  return cache
}
```

### webpack

Add to your webpack config:

```javascript
module.exports = {
  experiments: {
    asyncWebAssembly: true,
  },
}
```
