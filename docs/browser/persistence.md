# Persist a browser cache

Enable `persistence: true` to restore, after a reload: the writes this browser made, the writes still waiting for the server, and — by default — the last state the server synchronized. [Native Kotlin and Swift clients](/guide/client-support) use a different SQLite persistence contract.

::: warning Older releases
Versions 0.1.1 through 0.2.0 had a clock panic that could destroy the WAL during compaction. Versions 0.1.3 through 0.3.0 also had broken npm packaging. Before 0.5, two tabs sharing the database could silently lose writes, and server-applied changes were not persisted. See [browser installation](/browser/getting-started) before using an older package.
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

Omit `connect` for a local-only cache. If IndexedDB is unavailable or initialization fails — some private-browsing modes, storage access denied — `createCache` rejects rather than carrying on without the persistence you asked for. Handle that error, or retry without `persistence` for an in-memory cache.

## Know when a write is on disk

A write call returns once the change is in memory and queued for the server. Its IndexedDB record commits shortly after. `flush()` tells you when:

```typescript
cache.set('draft', text)
await cache.flush() // every write so far is committed to IndexedDB
```

`flush()` resolves when every write made before the call has committed, and rejects with the first IndexedDB failure since the last `flush()`. Failures also reach `onPersistenceError(operation, message)` as they happen, and `status().persistenceFailures` counts them. A write whose record failed is in memory and syncs while the page stays open; it would not survive a reload.

A committed IndexedDB transaction survives a reload or a closed tab. Browsers do not promise it survives a power cut or an OS crash, so `flush()` is not a guarantee against those.

## What IndexedDB stores

The browser opens the database `recached`, schema version 3:

| Object store | Contents | Key |
|---|---|---|
| `wal` | RESP commands that rebuild the local copy: local writes, and the current state of keys the server changed | `[t, session, seq]` — the writing tab's clock, its session, a sequence |
| `outbox` | Pending write frames, including their original `DEDUP` identities | `[session, id]` |
| `meta` | Unused since 0.5 | |

A local write's WAL record and its outbox row commit in **one transaction**, so a reload never shows a write that will not sync, nor syncs one it does not show. Rows written before 0.5 have numeric keys; IndexedDB orders them first, so they replay first. Older WAL rows stored text; hydration accepts both.

Server-applied changes — live-query snapshots, keychanges, mutation pushes — are written as the key's whole current state (`DEL`, then the commands that rebuild it), batched a fraction of a second after they arrive. That is what lets an offline reload show the last synchronized state, including keys the server deleted and writes it refused. Turn it off with `persistSyncedState: false` to persist only this browser's own writes. BroadcastChannel messages are persisted by the tab that made the write.

## Tabs

Every page load is its own sync identity, and every tab keys its rows apart, so two tabs sharing the database neither overwrite each other's pending writes nor make the server skip each other's writes as duplicates. A tab that loads restores *all* pending rows, its own and those of tabs that closed before sending them; if another tab is still open, both may replay a row, and the server applies it once.

Before 0.5 every tab adopted one stored identity and bumped its epoch, and the newer tab's writes made the server skip the older tab's as already applied.

## Restore and reconcile

Startup proceeds in this order:

1. Open IndexedDB.
2. Restore pending outbox rows in the order written, keeping their wire identities, and rewrite them under this session's keys — deleting only the rows read, so another tab's newer rows stay. If the socket is already open, they are sent at once, ahead of anything this session writes.
3. Replay WAL commands into the local engine.
4. Compact the WAL if it exceeds 1,000 rows.
5. Persist any writes made before persistence was enabled, then persist later writes as they happen.

A live query requests current state for its pattern. A fresh `qstate` replaces matching values and removes matching keys absent from the server — except keys a still-queued write covers, which keep their local value until that write is acknowledged. Register live queries again when creating a new cache instance.

## Compact the WAL

Compaction runs during hydration when the WAL contains more than 1,000 entries. It serializes the restored store into snapshot commands and replaces the records it read in one IndexedDB transaction, keeping records another tab appended meanwhile. It leaves the outbox untouched.

Compaction does not run during a session, so a long-lived tab's WAL grows with its writes and with server changes until the next load.

## Expiry

The WAL stores absolute deadlines (`SET … PXAT`, `PEXPIREAT`), so a reload does not restart a key's time to live: a key with a minute left, reloaded an hour later, is gone. Before 0.5 relative `EX`/`PX` commands restarted the clock on every reload. A queued `setEx` still reaches the server in relative form, so a write queued offline starts its time to live when the server applies it.

## Clear persisted state

`clearPersistence()` clears WAL and outbox rows and the in-memory pending-write queue. It leaves current in-memory cache values and identity metadata intact; it does not delete the database or close the socket.

```typescript
cache.disconnect()
await cache.clearPersistence()
window.location.href = '/login'
```

For sign-out, stop sync before clearing and reload into a new cache instance to discard the old in-memory values. Calling `clearPersistence()` alone does not remove another user's data from memory. IndexedDB is shared by origin, and the database name is fixed; separate cache instances on that origin are not isolated storage namespaces.

## Check delivery limits

Persisted writes replay on reconnect, with [duplicate suppression and its documented limits](/server/protocol#crash-consistency). With a server configured, the outbox holds 10,000 writes by default and a write past it throws `OutboxFullError` before changing anything. A persistent cache without `connect` still queues each write for a later page load that connects, but never refuses a write over it: past the cap the oldest queued copy is dropped, `onOutboxFull` reports it, and the write itself stays in memory and in the WAL. See [offline and reconnection](/browser/offline).
