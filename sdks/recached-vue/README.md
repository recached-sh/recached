# @recached/vue

Official Vue 3 composables for [Recached](https://github.com/recached-sh/recached) — a local reactive cache with automatic server sync and cross-tab sharing.

This package targets Vue in the browser. Native mobile apps use the [Kotlin](https://recached.dev/android/getting-started) or [Swift](https://recached.dev/ios/getting-started) preview.

## Features

- **Local reads** — reads are served from local WASM memory without a network round trip
- **Automatic reactivity** — `Ref`s update when a key changes from any source: local writes, server WebSocket push, or BroadcastChannel cross-tab sync
- **Vue 3 Composition API** — composables that integrate naturally with `<script setup>`
- **TypeScript-first** — full type inference including `useKeyJSON<T>`

Browser persistence does not save every server update; see [persistence limits](https://recached.dev/browser/persistence) and [client support](https://recached.dev/guide/client-support).

## Requirements

- Vue 3 or later
- `recached-edge` 0.3.1 or later (peer dependency)

## Installation

```bash
npm install @recached/vue recached-edge
```

## Quick start

Install `RecachedPlugin` once in your app entry, then use `useKey` anywhere in your components.

```ts
// main.ts
import { createApp } from 'vue'
import { RecachedPlugin } from '@recached/vue'
import App from './App.vue'

const app = createApp(App)
app.use(RecachedPlugin, {
  persistence: true,
  connect: { url: 'ws://localhost:6380' },
})
app.mount('#app')
```

```vue
<!-- Counter.vue -->
<script setup lang="ts">
import { useKey, useRecached } from '@recached/vue'

const cache = useRecached()
const count = useKey('count')
</script>

<template>
  <button @click="cache.set('count', String(Number(count ?? 0) + 1))">
    Count: {{ count ?? 0 }}
  </button>
</template>
```

Clicking the button updates `count` in the WASM store, notifies all `useKey('count')` subscribers in the same tab, syncs to the server, and fans out to all other connected tabs and clients — all without a page reload.

## API

### `RecachedPlugin`

Vue plugin. Install it before mounting your app and pass `CacheOptions` as the second argument to `app.use`.

```ts
app.use(RecachedPlugin, options?: CacheOptions)
```

| Option | Type | Description |
|--------|------|-------------|
| `connect` | `{ url: string; password?: string }` | Connect to a Recached server on the given WebSocket URL. |
| `persistence` | `boolean` | Restore the local-write WAL and outbox; save later local writes asynchronously. Server pushes are not appended to the WAL. |
| `broadcastChannel` | `string` | Share local writes across tabs on the same origin and channel. No server required. |

```ts
// With server connection
app.use(RecachedPlugin, { connect: { url: 'ws://localhost:6380', password: 'secret' } })

// With persistence (survives page refresh)
app.use(RecachedPlugin, { persistence: true })

// Cross-tab sync only (no server)
app.use(RecachedPlugin, { broadcastChannel: 'my-app' })

// All three combined
app.use(RecachedPlugin, {
  persistence: true,
  broadcastChannel: 'my-app',
  connect: { url: 'wss://cache.example.com:6380' },
})
```

### `useRecached()`

```ts
function useRecached(): Cache
```

Returns the `Cache` instance provided by `RecachedPlugin`. Use this to call `set`, `setEx`, `setJSON`, `del`, `publish`, and other write or imperative methods.

Throws if called before `RecachedPlugin` has been installed.

```vue
<script setup lang="ts">
import { useRecached } from '@recached/vue'

const cache = useRecached()
cache.set('theme', 'dark')
</script>
```

### `useKey(key)`

```ts
function useKey(key: string): Ref<string | null>
```

Reactively reads a string value. Returns a `Ref<string | null>` — `null` when the key does not exist or has expired. The ref updates automatically whenever the key changes from any mutation source.

Use `cache.set()` to write; the ref reflects the store, it is not a two-way binding target.

```vue
<script setup lang="ts">
import { useKey, useRecached } from '@recached/vue'

const theme = useKey('theme') // Ref<string | null>
const cache = useRecached()
</script>

<template>
  <button @click="cache.set('theme', theme === 'dark' ? 'light' : 'dark')">
    {{ theme ?? 'light' }}
  </button>
</template>
```

### `useKeyJSON<T>(key)`

```ts
function useKeyJSON<T>(key: string): Ref<T | null>
```

Same as `useKey` but JSON-parses the value. Returns `null` on a missing key, expired key, or invalid JSON.

```vue
<script setup lang="ts">
import { useKeyJSON } from '@recached/vue'

interface User { id: number; name: string }

const user = useKeyJSON<User>('user:42') // Ref<User | null>
</script>

<template>
  <p v-if="user">{{ user.name }}</p>
  <Spinner v-else />
</template>
```

### `usePubSub(channel, handler)`

```ts
function usePubSub(channel: string, handler: (msg: string) => void): void
```

Subscribe to a server pub/sub channel for the lifetime of the component. Sends `SUBSCRIBE` on setup and `UNSUBSCRIBE` on `onUnmounted`.

```vue
<script setup lang="ts">
import { usePubSub } from '@recached/vue'

usePubSub('alerts', (msg) => {
  console.log('New alert:', msg)
})
</script>
```

## Reactivity model

Every write — whether it comes from the same component, another component in the same tab, another tab via BroadcastChannel, or another client via the server — fires the mutation bus, which causes all `useKey` / `useKeyJSON` subscribers to re-read their key and update their `Ref`.

```
Local write (cache.set)
  └─▶ WASM store update
  └─▶ notify_mutation → update all useKey Refs → Vue re-renders
  └─▶ WebSocket send → server fan-out → other clients
  └─▶ BroadcastChannel post → other tabs
        └─▶ WASM store update → notify_mutation → update Refs → Vue re-renders
```

## Examples

### Theme toggle

```vue
<script setup lang="ts">
import { useKey, useRecached } from '@recached/vue'

const theme = useKey('theme')
const cache = useRecached()
</script>

<template>
  <button @click="cache.set('theme', theme === 'light' ? 'dark' : 'light')">
    {{ theme === 'light' ? '🌙 Dark mode' : '☀️ Light mode' }}
  </button>
</template>
```

### Shared shopping cart

```vue
<script setup lang="ts">
import { useKeyJSON, useRecached } from '@recached/vue'

interface CartItem { id: string; qty: number }

const items = useKeyJSON<CartItem[]>('cart')
const cache = useRecached()

function addItem(id: string) {
  const updated = [...(items.value ?? []), { id, qty: 1 }]
  cache.setJSON('cart', updated, 3600)
}
</script>

<template>
  <ul>
    <li v-for="item in items ?? []" :key="item.id">
      {{ item.id }} × {{ item.qty }}
    </li>
  </ul>
</template>
```

### Live notifications via pub/sub

```vue
<script setup lang="ts">
import { ref } from 'vue'
import { usePubSub } from '@recached/vue'

const alerts = ref<string[]>([])
usePubSub('alerts', (msg) => alerts.value.push(msg))
</script>

<template>
  <ul>
    <li v-for="(alert, i) in alerts" :key="i">{{ alert }}</li>
  </ul>
</template>
```

### With expiry

```vue
<script setup lang="ts">
import { useKey, useRecached } from '@recached/vue'

const session = useKey('session')
const cache = useRecached()

// Set with 30-minute TTL
cache.setEx('session', userId, 1800)
</script>

<template>
  <LoginPrompt v-if="!session" />
  <p v-else>Logged in — session expires soon</p>
</template>
```

## License

Apache-2.0
