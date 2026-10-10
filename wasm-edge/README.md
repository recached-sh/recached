# recached-edge

The browser WebAssembly client for [Recached](https://github.com/recached-sh/recached), a Rust cache and sync engine shared with native Kotlin, Swift, and embedded Rust clients. Reads use local WebAssembly memory; writes and server updates travel over WebSocket when connected.

## Install

```bash
npm install recached-edge
```

Use 0.3.1 or later for the corrected npm packaging. Earlier releases have additional runtime and persistence defects; see [browser installation](https://recached.dev/browser/getting-started).

## Hydrate and observe server data

Register a live query to receive initial server state and later changes:

```typescript
import { createCache } from 'recached-edge';

const cache = await createCache({
  persistence: true,
  connect: { url: 'ws://localhost:6380' },
});
const stopWatching = cache.liveQuery('user:*');
const stopListening = cache.onKeyChange('user:theme', () => {
  console.log(cache.get('user:theme'));
});

cache.set('user:theme', 'dark');
cache.get('user:theme'); // local value; server acceptance is asynchronous
```

`liveQuery` requests a snapshot asynchronously. `onKeyChange` listens to local changes and does not create a server subscription. Stop them with their returned functions when no longer needed. A connection alone does not fetch the server's keyspace.

For a deployed server, use `wss://` and a backend-minted `connect.syncToken` when strict [sync scopes](https://recached.dev/server/sync-scopes) are enabled. Watches are not an authorization boundary.

## Configure the browser adapter

`createCache(options?)` initializes WebAssembly, restores IndexedDB when enabled, and starts the configured transport. It returns before WebSocket authentication or live-query hydration finishes.

| Option | Type | Default | Effect |
|---|---|---|---|
| `persistence` | `boolean` | `false` | Restore the local-write WAL and outbox; save later local writes asynchronously |
| `broadcastChannel` | `string` | Unset | Share local write commands with tabs on the same origin and channel |
| `connect.url` | `string` | Unset | WebSocket sync endpoint |
| `connect.password` | `string` | Unset | Send `AUTH` when the socket opens |
| `connect.syncToken` | `string` | Unset | Present backend-minted scoped access |
| `connect.syncScopes` | `string[]` | Unset | Open-mode bandwidth filter; not authorization |
| `connect.reconnect` | `boolean` | `true` | Reconnect with jittered exponential backoff |

Omit `connect` to use a local cache:

```typescript
const cache = await createCache({ broadcastChannel: 'my-app' });
cache.setJSON('prefs', { fontSize: 16 }, 3600);
const prefs = cache.getJSON<{ fontSize: number }>('prefs');
```

BroadcastChannel shares local writes, not server pushes or pub/sub, and does not proxy server traffic between tabs. Each tab that needs server updates must connect independently.

## Read and write values

The TypeScript `Cache` exposes a focused API rather than the full server command catalog:

| Task | Methods |
|---|---|
| Read text / bytes / parsed JSON | `get`, `getBytes`, `getJSON` |
| Read metadata / matching entries | `exists`, `ttl`, `getMatching` |
| Write text / bytes / serialized JSON | `set`, `setBytes`, `setEx`, `setJSON` |
| Delete / increment / decrement | `del`, `incr`, `decr` |
| JSON path operations / merge patch | `jset`, `jget`, `jmerge` |
| Observe local changes | `onKeyChange`, `onPatternChange`, `onMutation` |
| Manage server subscriptions | `liveQuery`, `liveUnquery` |
| Pub/sub | `subscribe`, `unsubscribe`, `publish`, `publishBytes`, `onMessage` |
| Manage sync / storage | `connect`, `disconnect`, `syncToken`, `syncScopes`, `clearPersistence` |
| Track queued writes | `pendingWrites`, `onOutboxFull` |

`get` returns `null` for a miss and throws on non-UTF-8 values; use `getBytes` for binary data. `getJSON` returns `null` for invalid JSON or binary data. Collection commands are available on the server; the browser wrapper does not expose individual hash/list/set/sorted-set operations.

Use [the TypeScript API reference](https://recached.dev/browser/api-reference) for signatures. `cache.raw` exposes the generated `RecachedCache`; its names and return types differ from the wrapper, so use it only when you need the bindings directly.

## Persistence and reconnect limits

Local writes apply in memory and queue for delivery; a return from `set` is not a server acknowledgment. Reconnect replays queued writes before refreshing live-query snapshots. `DEDUP` suppresses repeated operations while the server retains the client's mark, with server-crash and retention limits.

IndexedDB saves local API writes asynchronously. Server pushes and BroadcastChannel messages do not append to the WAL, and relative TTL commands restart their TTL on replay. An offline cold start therefore does not guarantee the last server-synced state. Native mobile clients instead persist changed-key snapshots and write rows transactionally in SQLite.

The outbox holds 10,000 writes and drops the oldest on overflow; `onOutboxFull` reports it. Persistent local-only mode also accumulates pending rows. `clearPersistence()` clears WAL and outbox rows without clearing in-memory values, identity metadata, or the connection. See [browser persistence](https://recached.dev/browser/persistence) and [offline recovery](https://recached.dev/browser/offline).

## Framework and native clients

Use [recached-react](../sdks/recached-react/README.md) or [recached-vue](../sdks/recached-vue/README.md) for browser framework integration. For native apps, use the [Kotlin](https://recached.dev/android/getting-started) or [Swift](https://recached.dev/ios/getting-started) preview; these run Rust through UniFFI rather than this WASM package.

## Runtime support

The browser adapter needs WebAssembly and browser Web APIs. Server sync needs `WebSocket`; persistence needs IndexedDB; tab sharing needs BroadcastChannel. Auto-reconnect uses `window.setTimeout`. A WASI build for edge runtimes is planned, not an implemented Cloudflare Workers or Deno Deploy integration.

## License

Apache-2.0.
