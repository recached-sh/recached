# Composables Reference

## `RecachedPlugin`

```ts
app.use(RecachedPlugin, options?: CacheOptions)
```

Vue plugin that creates the cache and provides it to the entire app via `inject`. Install it before `app.mount`.

### Options

| Option | Type | Description |
|--------|------|-------------|
| `connect` | `{ url: string; password?: string }` | WebSocket URL of the Recached server and optional auth password. |
| `persistence` | `boolean` | Restore the local-write WAL and outbox; save later local writes asynchronously. Server pushes are not appended to the WAL. |
| `broadcastChannel` | `string` | BroadcastChannel name for cross-tab mutation sharing without a server. |

### Examples

```ts
// With server connection
app.use(RecachedPlugin, { connect: { url: 'ws://localhost:6380' } })

// With persistence only (no server)
app.use(RecachedPlugin, { persistence: true })

// With cross-tab sync only
app.use(RecachedPlugin, { broadcastChannel: 'my-app' })

// Full setup
app.use(RecachedPlugin, {
  persistence: true,
  broadcastChannel: 'my-app',
  connect: { url: 'wss://cache.example.com', password: 'secret' },
})
```

---

## `useRecached()`

```ts
function useRecached(): Cache
```

Returns the `Cache` instance provided by `RecachedPlugin`. Use it to call write and imperative methods: `set`, `setEx`, `setJSON`, `del`, `publish`, `subscribe`, `clearPersistence`.

Throws if `RecachedPlugin` has not been installed.

### Example

```vue
<script setup lang="ts">
import { useRecached } from '@recached/vue'

const cache = useRecached()

function save() {
  cache.setJSON('user:42', { id: 42, name: 'Alice' }, 300)
}
</script>

<template>
  <button @click="save">Save</button>
</template>
```

### Writing from event handlers

Reads (`useKey`, `useKeyJSON`) are reactive. Writes are always done imperatively via `useRecached()`:

```vue
<script setup lang="ts">
import { useKey, useRecached } from '@recached/vue'

const cache = useRecached()
const count = useKey('count')
const n = computed(() => Number(count.value ?? 0))
</script>

<template>
  <div>
    <p>{{ n }}</p>
    <button @click="cache.set('count', String(n + 1))">+</button>
    <button @click="cache.set('count', String(n - 1))">−</button>
  </div>
</template>
```

---

## `useKey(key)`

```ts
function useKey(key: string): Ref<string | null>
```

Reactively reads a string value from the cache. Returns a `Ref<string | null>` — `null` when the key does not exist or has expired.

The ref updates automatically whenever `key` is mutated — from any source:

- A write in the same component or another component in the same tab
- A write received from the server (WebSocket fan-out)
- A write from another tab (BroadcastChannel sync)

Writes to other keys do not touch the ref. The change listener is cleaned up automatically via `onUnmounted`.

### Example

```vue
<script setup lang="ts">
import { useKey } from '@recached/vue'

const status = useKey('system:status')
</script>

<template>
  <span :class="`badge badge--${status ?? 'unknown'}`">
    {{ status ?? 'unknown' }}
  </span>
</template>
```

### Dynamic keys

Pass any reactive expression's resolved value; the composable reads the current key at call time. To track a dynamic key reactively, use `watchEffect` with `useRecached`:

```vue
<script setup lang="ts">
import { watchEffect, ref } from 'vue'
import { useRecached } from '@recached/vue'

const props = defineProps<{ userId: string }>()
const cache = useRecached()
const status = ref<string | null>(null)

watchEffect((onCleanup) => {
  const key = `user:${props.userId}:status`
  status.value = cache.get(key)
  onCleanup(cache.onKeyChange(key, () => {
    status.value = cache.get(key)
  }))
})
</script>
```

For a fixed key, `useKey` is simpler and preferred.

---

## `useKeyJSON<T>(key)`

```ts
function useKeyJSON<T>(key: string): Ref<T | null>
```

Same as `useKey` but JSON-parses the stored string. Returns `null` when the key is missing, expired, or the stored value is not valid JSON.

### Example

```vue
<script setup lang="ts">
import { useKeyJSON, useRecached } from '@recached/vue'

interface CartItem {
  id: string
  name: string
  qty: number
  price: number
}

const cache = useRecached()
const items = useKeyJSON<CartItem[]>('cart')
const total = computed(() =>
  (items.value ?? []).reduce((sum, item) => sum + item.price * item.qty, 0)
)

function removeItem(id: string) {
  cache.setJSON('cart', (items.value ?? []).filter((i) => i.id !== id))
}
</script>

<template>
  <div>
    <ul>
      <li v-for="item in items ?? []" :key="item.id">
        {{ item.name }} × {{ item.qty }}
        <button @click="removeItem(item.id)">Remove</button>
      </li>
    </ul>
    <p>Total: ${{ total.toFixed(2) }}</p>
  </div>
</template>
```

### Writing JSON

Use `cache.setJSON<T>()` from `useRecached()` to write structured data:

```ts
cache.setJSON<CartItem[]>('cart', updatedItems)          // no expiry
cache.setJSON<CartItem[]>('cart', updatedItems, 3600)    // expires in 1 hour
```

---

## `useKeys(pattern)`

```ts
function useKeys(pattern: string): Ref<Array<[key: string, value: string | null]>>
```

Live query: reactively read **every key matching a glob pattern**. On setup, the server sends the current state of all matching keys (merged into the local WASM store), then streams every change to matching keys — including keys created after subscribing. The `Ref` updates on each change; reads never leave the browser.

Pairs are sorted by key. Values are strings; keys holding collection types come back as `null`. The server subscription is ref-counted per pattern and ends when the last component using it unmounts.

Under [strict sync scoping](/server/sync-scopes), the pattern must sit inside the connection's granted scopes.

### Example

```vue
<script setup lang="ts">
import { useKeys } from '@recached/vue'

const items = useKeys('cart:42:item:*')
</script>

<template>
  <ul>
    <li v-for="[key, qty] in items" :key="key">
      {{ key.split(':').pop() }}: {{ qty }}
    </li>
  </ul>
</template>
```

---

## `usePubSub(channel, handler)`

```ts
function usePubSub(channel: string, handler: (msg: string) => void): void
```

Subscribe to a server pub/sub channel for the lifetime of the component. Sends `SUBSCRIBE` on setup and `UNSUBSCRIBE` on `onUnmounted`. The `handler` is called with each incoming message string.

### Example

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

### Publishing

Use `cache.publish()` from `useRecached()` to send messages:

```ts
const cache = useRecached()
cache.publish('alerts', 'Deployment complete')
```

All subscribers — Vue components, React components, server-side RESP clients — receive the message.

---

## Reactivity model

Every mutation — regardless of source — fires the internal notification bus, which updates all `useKey` and `useKeyJSON` refs:

```
cache.set('key', value)           ← local write
  │
  ├─ WASM store updated
  ├─ notify all useKey('key') subscribers → Ref.value updated → Vue re-renders
  ├─ WebSocket send → server → fan-out to other clients
  │     └─ WASM store updated → notify → Ref.value updated → Vue re-renders
  └─ BroadcastChannel post → other tabs
        └─ WASM store updated → notify → Ref.value updated → Vue re-renders
```

The change listener registered inside each composable is removed by `onUnmounted`, so components that unmount leave no dangling listeners.

---

## TypeScript

All composables are fully typed. `useKeyJSON<T>` infers the generic from the type argument:

```ts
const user = useKeyJSON<User>('user:42')   // Ref<User | null>
const tags = useKeyJSON<string[]>('tags')  // Ref<string[] | null>
```

`useRecached()` returns the full `Cache` type with all methods typed.
