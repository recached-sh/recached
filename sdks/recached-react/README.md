# @recached/react

Official React hooks for [Recached](https://github.com/recached-sh/recached) — a local reactive cache with automatic server sync and cross-tab sharing.

This package targets React in the browser. For native mobile apps, use the [Kotlin](https://recached.dev/android/getting-started) or [Swift](https://recached.dev/ios/getting-started) preview. React Native is planned.

## Features

- **Local reads** — reads are served from local WASM memory without a network round trip
- **Automatic re-renders** — components update when a key changes from any source: local writes, server WebSocket push, or BroadcastChannel cross-tab sync
- **React 18 concurrent-safe** — built on `useSyncExternalStore`, no tearing
- **TypeScript-first** — full type inference including `useKeyJSON<T>`

Browser persistence does not save every server update; see [persistence limits](https://recached.dev/browser/persistence) and [client support](https://recached.dev/guide/client-support).

## Requirements

- React 18 or later
- `recached-edge` 0.3.1 or later (peer dependency)

## Installation

```bash
npm install @recached/react recached-edge
```

## Quick start

Mount `<RecachedProvider>` once near the root of your app, then use `useKey` anywhere inside it.

```tsx
import { RecachedProvider, useKey, useRecached } from '@recached/react';

function App() {
  return (
    <RecachedProvider
      options={{
        persistence: true,
        connect: { url: 'ws://localhost:6380' },
      }}
    >
      <Counter />
    </RecachedProvider>
  );
}

function Counter() {
  const cache = useRecached();
  const count = useKey('count');

  return (
    <button onClick={() => cache.set('count', String(Number(count ?? 0) + 1))}>
      Count: {count ?? 0}
    </button>
  );
}
```

Clicking the button updates `count` in the WASM store, notifies all `useKey('count')` subscribers in the same tab, syncs to the server, and fans out to all other connected tabs and clients — all without a page reload.

## API

### `<RecachedProvider>`

```tsx
<RecachedProvider options={CacheOptions}>
  {children}
</RecachedProvider>
```

Creates and provides a `Cache` instance to all descendants. Renders `null` until the cache is ready (WASM init + optional persistence hydration).

| Prop | Type | Description |
|------|------|-------------|
| `options` | `CacheOptions` | Passed to `createCache`. Controls persistence, BroadcastChannel, and server connection. |
| `cache` | `Cache` | Pass a pre-built `Cache` instance instead of creating one. When set, `options` is ignored. |

```tsx
// With server connection
<RecachedProvider options={{ connect: { url: 'ws://localhost:6380', password: 'secret' } }}>

// With persistence (survives page refresh)
<RecachedProvider options={{ persistence: true }}>

// Cross-tab sync only (no server)
<RecachedProvider options={{ broadcastChannel: 'my-app' }}>

// Pre-built instance (advanced)
const cache = await createCache({ ... });
<RecachedProvider cache={cache}>
```

### `useRecached()`

```ts
function useRecached(): Cache
```

Returns the `Cache` instance from the nearest `<RecachedProvider>`. Use this to call `set`, `setEx`, `setJSON`, `del`, `publish`, and other write or imperative methods.

Throws if called outside a `<RecachedProvider>`.

```tsx
function SaveButton() {
  const cache = useRecached();
  return (
    <button onClick={() => cache.setJSON('user', { id: 1, name: 'Alice' }, 300)}>
      Save
    </button>
  );
}
```

### `useKey(key)`

```ts
function useKey(key: string): string | null
```

Reactively reads a string value. Returns `null` when the key does not exist or has expired. Re-renders the component automatically whenever the key changes — from any mutation source.

```tsx
const theme = useKey('theme'); // "dark" | "light" | null
```

### `useKeyJSON<T>(key)`

```ts
function useKeyJSON<T>(key: string): T | null
```

Same as `useKey` but JSON-parses the value. Returns `null` on a missing key, expired key, or invalid JSON.

```tsx
interface User { id: number; name: string }

function UserCard() {
  const user = useKeyJSON<User>('user:42');
  if (!user) return <Spinner />;
  return <p>{user.name}</p>;
}
```

## Reactivity model

Every write — whether it comes from the same component, another component in the same tab, another tab via BroadcastChannel, or another client via the server — fires the mutation bus, which causes all `useKey` / `useKeyJSON` subscribers to re-read their key and re-render if the value changed.

```
Local write (cache.set)
  └─▶ WASM store update
  └─▶ notify_mutation → re-render all useKey subscribers
  └─▶ WebSocket send → server fan-out → other clients
  └─▶ BroadcastChannel post → other tabs
        └─▶ WASM store update → notify_mutation → re-render
```

## Examples

### Theme toggle

```tsx
function ThemeToggle() {
  const cache = useRecached();
  const theme = useKey('theme') ?? 'light';
  return (
    <button onClick={() => cache.set('theme', theme === 'light' ? 'dark' : 'light')}>
      {theme === 'light' ? '🌙 Dark mode' : '☀️ Light mode'}
    </button>
  );
}
```

### Shared shopping cart

```tsx
interface CartItem { id: string; qty: number }

function Cart() {
  const cache = useRecached();
  const items = useKeyJSON<CartItem[]>('cart') ?? [];

  function addItem(id: string) {
    const updated = [...items, { id, qty: 1 }];
    cache.setJSON('cart', updated, 3600);
  }

  return (
    <ul>
      {items.map((item) => <li key={item.id}>{item.id} × {item.qty}</li>)}
    </ul>
  );
}
```

### With expiry

```tsx
function SessionBanner() {
  const cache = useRecached();
  const session = useKey('session');

  if (!session) return <LoginPrompt />;
  return <p>Logged in — session expires soon</p>;
}

// Elsewhere: set with 30-minute TTL
cache.setEx('session', userId, 1800);
```

## License

Apache-2.0
