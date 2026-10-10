# Persist a browser cache

Enable `persistence: true` to restore local API writes and pending server writes from IndexedDB after a page reload. Browser persistence does not save each server push, so it does not guarantee the last server-synced state on an offline cold start. [Native Kotlin and Swift clients](/guide/client-support) use a different SQLite persistence contract.

::: warning Older releases
Versions 0.1.1 through 0.2.0 had a clock panic that could destroy the WAL during compaction. Versions 0.1.3 through 0.3.0 also had broken npm packaging. See [browser installation](/browser/getting-started) before using an older package.
:::

## Enable persistence before connecting

`createCache` restores persisted data before opening the sync connection:

```typescript
import { createCache } from 'recached-edge'

const cache = await createCache({
  persistence: true,
  connect: { url: 'ws://localhost:6380' },
})
cache.liveQuery('todo:*')
```

Omit `connect` for a local-only cache. If IndexedDB is unavailable or initialization fails, `createCache` rejects. Handle that error or omit persistence when you need an in-memory fallback.

## What IndexedDB stores

The browser opens the database `recached`, schema version 3:

| Object store | Contents |
|---|---|
| `wal` | RESP command bytes from local API writes, keyed by a numeric sequence |
| `outbox` | Pending write frames, including their original `DEDUP` identities |
| `meta` | Client identity and session epoch |

Older WAL rows stored text; hydration accepts both those rows and current byte arrays. There is no per-entry timestamp field.

Local `set`, `setEx`, `del`, counter, and JSON writes append to the WAL. Incoming WebSocket state and BroadcastChannel messages change memory and notify listeners without appending to it. Reads use memory only.

Writes start IndexedDB operations asynchronously. A synchronous `cache.set()` returning does not confirm a durable storage commit, and closing a tab immediately can lose an unfinished persistence operation. WAL and outbox writes are separate transactions, unlike mobile's combined SQLite transaction.

## Restore and reconcile

Startup proceeds in this order:

1. Open IndexedDB and load the client identity; bump the session epoch.
2. Restore pending outbox frames, preserving their wire identities.
3. Replay WAL commands into the local engine.
4. Compact the WAL if it exceeds 1,000 rows.
5. Enable persistence for later local writes, then open the socket when configured.

A live query requests current state for its pattern. The server does not replay every missed browser mutation or send its entire keyspace automatically on connect. A fresh `qstate` replaces matching values and removes matching keys absent from the server. Register live queries again when creating a new cache instance.

Offline startup can restore local writes, but saved WAL commands may not represent the most recent server state. Use [mobile previews](/guide/client-support#write-durability-and-replay) when comparing persistence guarantees across platforms.

## Compact the WAL

Compaction runs during hydration when the WAL contains more than 1,000 entries. It serializes the restored store into snapshot commands and replaces the WAL in one IndexedDB transaction. It leaves the outbox untouched.

Compaction does not run after every successful WebSocket sync, and it does not keep the WAL bounded throughout a long-lived tab session. The WAL grows again as local writes append.

## TTL replay limits

Local TTL writes are logged as relative `SET ... EX` commands. Replaying them starts the TTL again. Compacted string snapshots also use relative `PX`; non-string snapshot expiries use `PEXPIREAT`. Browser persistence therefore does not guarantee the original expiry across reloads.

During a connected session, negotiated expiry metadata lets watched values expire locally while offline. That does not change the WAL replay limitation. Native SQLite snapshots instead store absolute expiry timestamps and discard expired entries when restoring.

## Clear persisted state

`clearPersistence()` clears WAL and outbox rows and the in-memory pending-write queue. It leaves current in-memory cache values and identity metadata intact; it does not delete the database or close the socket.

```typescript
cache.disconnect()
await cache.clearPersistence()
window.location.href = '/login'
```

For sign-out, stop sync before clearing and reload into a new cache instance to discard the old in-memory values. Calling `clearPersistence()` alone does not remove another user's data from memory. IndexedDB is shared by origin, and the database name is fixed; separate cache instances on that origin are not isolated storage namespaces.

## Check delivery limits

Persisted writes replay on reconnect, but [duplicate suppression has server-crash and retention limits](/server/protocol#deduplicated-write-replay-dedup). The outbox drops the oldest write after 10,000 pending entries; `onOutboxFull` reports overflow. A local-only persistent browser cache still accumulates outbox entries even without a server URL. See [offline and reconnection](/browser/offline).
