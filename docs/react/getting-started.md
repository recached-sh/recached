# Getting Started

`@recached/react` is the official React hooks package for Recached. It wraps the `recached-edge` WASM SDK and bridges it to React's rendering model — components re-render automatically whenever a key changes, whether the mutation came from the same component, another tab, or the server.

## Requirements

- React 18 or later
- `recached-edge` 0.1.4 or later

## Install

```bash
npm install @recached/react recached-edge
# or
pnpm add @recached/react recached-edge
# or
yarn add @recached/react recached-edge
```

---

## Setup

Mount `<RecachedProvider>` once near the root of your app. Every hook must be a descendant of it.

```tsx
// main.tsx / app.tsx
import { RecachedProvider } from '@recached/react'

export default function App() {
  return (
    <RecachedProvider
      options={{
        persistence: true,
        connect: { url: 'ws://localhost:6380' },
      }}
    >
      <YourApp />
    </RecachedProvider>
  )
}
```

`<RecachedProvider>` renders `null` until the WASM module has initialized and (optionally) replayed the IndexedDB WAL. Your app appears only when the cache is ready — no blank-state flicker on refresh.

### Options

| Option | Description |
|--------|-------------|
| `persistence` | Restore the local-write WAL and outbox, then save later local writes asynchronously. See [persistence limits](/browser/persistence); server pushes are not appended to the WAL. |
| `broadcastChannel` | Name for cross-tab sync. Tabs with the same name share mutations automatically via the BroadcastChannel API, no server needed. |
| `connect.url` | WebSocket URL of the Recached server (`ws://` or `wss://`). Mutations are pushed to the server and server-side mutations are pushed down. |
| `connect.password` | Server password when `RECACHED_PASSWORD` is configured. |

All options are optional. Omit `connect` for a local-only in-memory cache.

---

## First component

```tsx
import { useKey, useRecached } from '@recached/react'

function ThemeToggle() {
  const cache = useRecached()
  const theme = useKey('theme') ?? 'light'

  return (
    <button onClick={() => cache.set('theme', theme === 'light' ? 'dark' : 'light')}>
      {theme === 'light' ? '🌙 Dark mode' : '☀️ Light mode'}
    </button>
  )
}
```

`useKey('theme')` re-renders `ThemeToggle` every time the `theme` key changes — from any source. Click in one tab, every other tab updates instantly.

---

## With Next.js App Router

The WASM module and WebSocket must run in a client component. Wrap your providers in a `'use client'` boundary:

```tsx
// app/providers.tsx
'use client'

import { RecachedProvider } from '@recached/react'
import type { ReactNode } from 'react'

export function Providers({ children }: { children: ReactNode }) {
  return (
    <RecachedProvider
      options={{
        persistence: true,
        connect: { url: process.env.NEXT_PUBLIC_CACHE_URL! },
      }}
    >
      {children}
    </RecachedProvider>
  )
}
```

```tsx
// app/layout.tsx
import { Providers } from './providers'

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html>
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  )
}
```

Individual components that use `useKey` or `useRecached` must also be client components (`'use client'`).

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

## With a pre-built cache instance

If you need to create the cache outside React (e.g. to read from it before the tree renders), pass it via the `cache` prop instead of `options`:

```ts
// lib/cache.ts
import { createCache } from 'recached-edge'

export const cache = await createCache({
  persistence: true,
  connect: { url: 'ws://localhost:6380' },
})
```

```tsx
// main.tsx
import { RecachedProvider } from '@recached/react'
import { cache } from './lib/cache'

<RecachedProvider cache={cache}>
  <App />
</RecachedProvider>
```

When the `cache` prop is set, `<RecachedProvider>` skips initialization and uses the instance directly.
