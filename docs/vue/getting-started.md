# Getting Started

`@recached/vue` is the official Vue 3 composables package for Recached. It wraps the `recached-edge` WASM SDK and bridges it to Vue's reactivity system — `Ref`s update automatically whenever a key changes, whether the mutation came from the same component, another tab, or the server.

## Requirements

- Vue 3 or later
- `recached-edge` 0.1.4 or later

## Install

```bash
npm install @recached/vue recached-edge
# or
pnpm add @recached/vue recached-edge
# or
yarn add @recached/vue recached-edge
```

---

## Setup

Install `RecachedPlugin` once in your app entry before mounting. All composables depend on it.

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

Because `install` is async (it awaits `createCache`), the cache is ready by the time your components mount.

### Options

| Option | Description |
|--------|-------------|
| `persistence` | Restore the local-write WAL and outbox, then save later local writes asynchronously. See [persistence limits](/browser/persistence); server pushes are not appended to the WAL. |
| `broadcastChannel` | Name for cross-tab sync. Tabs with the same name share mutations automatically via the BroadcastChannel API — no server needed. |
| `connect.url` | WebSocket URL of the Recached server (`ws://` or `wss://`). Mutations are pushed to the server and server-side mutations are pushed down. |
| `connect.password` | Server password when `RECACHED_PASSWORD` is configured. |

All options are optional. Omit `connect` for a local-only in-memory cache.

---

## First component

```vue
<script setup lang="ts">
import { useKey, useRecached } from '@recached/vue'

const cache = useRecached()
const theme = useKey('theme')
</script>

<template>
  <button @click="cache.set('theme', theme === 'light' ? 'dark' : 'light')">
    {{ theme === 'light' ? '🌙 Dark mode' : '☀️ Light mode' }}
  </button>
</template>
```

`useKey('theme')` returns a `Ref<string | null>` that updates every time the `theme` key changes — from any source. Click in one tab, every other tab updates instantly.

---

## Reactive JSON values

Use `useKeyJSON<T>` to get a typed, JSON-parsed `Ref`:

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
      <button @click="addItem(item.id)">+</button>
    </li>
  </ul>
</template>
```

---

## Pub/Sub

Use `usePubSub` to subscribe to server messages for the lifetime of a component:

```vue
<script setup lang="ts">
import { ref } from 'vue'
import { usePubSub } from '@recached/vue'

const notifications = ref<string[]>([])

usePubSub('alerts', (msg) => {
  notifications.value.push(msg)
})
</script>

<template>
  <ul>
    <li v-for="(n, i) in notifications" :key="i">{{ n }}</li>
  </ul>
</template>
```

`SUBSCRIBE` is sent on setup; `UNSUBSCRIBE` is sent on `onUnmounted` automatically.

---

## With Vite

No extra config needed — Vite handles WASM imports natively. If you encounter issues with the WASM file not being served correctly, add to `vite.config.ts`:

```ts
import { defineConfig } from 'vite'

export default defineConfig({
  optimizeDeps: {
    exclude: ['recached-edge'],
  },
})
```

---

## With Nuxt

WASM and WebSocket must run on the client. Wrap the plugin registration in a client-only plugin:

```ts
// plugins/recached.client.ts
import { RecachedPlugin } from '@recached/vue'

export default defineNuxtPlugin((nuxtApp) => {
  nuxtApp.vueApp.use(RecachedPlugin, {
    persistence: true,
    connect: { url: useRuntimeConfig().public.cacheUrl },
  })
})
```

```ts
// nuxt.config.ts
export default defineNuxtConfig({
  runtimeConfig: {
    public: {
      cacheUrl: process.env.NUXT_PUBLIC_CACHE_URL ?? 'ws://localhost:6380',
    },
  },
})
```

Components that call `useKey`, `useKeyJSON`, or `usePubSub` must be wrapped in `<ClientOnly>` or marked with `<script setup client>` to avoid SSR execution.

---

## With a pre-built cache instance

If you need to create the cache outside Vue (e.g. to seed it before the app mounts), provide it directly via `app.provide`:

```ts
// main.ts
import { createApp } from 'vue'
import { createCache } from 'recached-edge'
import { CACHE_KEY } from '@recached/vue'
import App from './App.vue'

const cache = await createCache({
  persistence: true,
  connect: { url: 'ws://localhost:6380' },
})

const app = createApp(App)
app.provide(CACHE_KEY, cache)
app.mount('#app')
```

`useRecached()` reads from the same `CACHE_KEY` symbol, so all composables continue to work normally.
