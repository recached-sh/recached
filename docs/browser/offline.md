# Offline & Reconnection

Browsers go offline. Recached is built so that when they do, the app keeps working — and when the connection returns, state converges without glue code.

## What happens automatically

**While connected**, every local write applies instantly to WASM memory and streams to the server.

**When the connection drops:**

- Reads keep working — they never left local memory to begin with.
- Writes keep working: they apply locally and queue as *operations* in an outbox. It holds 10 000 writes by default; a write past that throws `OutboxFullError` and changes nothing, so nothing already queued is lost. With persistence enabled, each write and its queued row commit to IndexedDB together, shortly after the call returns; `await cache.flush()` confirms it. See [persistence](/browser/persistence).
- `cache.status()` and `onStatusChange` report where synchronization stands. `'ready'` means authenticated, every queued write replayed and answered, and every live query hydrated — not merely an open socket. `lastSyncedAt` is when that last completed.
- The client reconnects with exponential backoff: a 500 ms base, doubling to a 30 s cap, with jitter between half and all of each delay.

**When the connection returns**, the client re-establishes the session in order:

1. `AUTH` (the password is remembered)
2. `CLIENT DELTA ON`, `CLIENT EXPIRY ON`, `CLIENT PUSHFRAMES ON` and `CLIENT DEDUPBARRIER ON` (connection state, so re-sent every time)
3. `SYNC TOKEN` / sync scopes (remembered)
4. The outbox replays FIFO
5. Every active live query is re-subscribed — the fresh `qstate` re-hydrates local keys with whatever happened server-side while you were away, and already contains the replayed writes. A key a still-queued write covers keeps its local value until that write is acknowledged, so a snapshot never rolls back a write you already made

A queued write is retired from the outbox only when the server accepts it or refuses it for good. A write refused for a reason that can clear — no sync token accepted yet, say — stays queued, and the client reconnects to replay it; one refused for good (a read-only key, a wrong type) is dropped and reported to `onWriteRefused(keys, unreconciled, reason)`, since replaying it would be refused again. `unreconciled` lists the keys no live query covers: their local value is the refused one, and nothing will correct it. See [the protocol reference](/server/protocol#the-ordering-invariant-acknowledgment-correlation). A write that was sent but unacknowledged when the connection died is re-sent on reconnect. Every store write carries a `DEDUP` envelope (a per-client id plus a monotonic write id), and a running server skips ids it has already applied, replying `+DUP` so the outbox can retire the entry.

Each queued row keeps its identity inside its frame across reloads. The server records each `DEDUP` write's mark with the write itself in its AOF, so with an AOF a retried `INCR` whose reply was lost is not applied twice after a server crash either. The [protocol page](/server/protocol#crash-consistency) lists the cases where a retry can still run again; use application-level idempotency when duplicates are unacceptable.

Nothing to call, nothing to configure. Disable with `createCache({ connect: { reconnect: false } })` or stop a connection deliberately with `cache.disconnect()`.

## Merge semantics — what happens to conflicting writes

Recached queues *operations*, not final values. That choice decides how offline changes merge with concurrent changes from other clients:

| Write type | Offline behavior | Merge result |
|---|---|---|
| `incr` / `decr` | queues the **delta** (`INCRBY`) | **Additive** — your +2 and their +3 make +5, nobody's counts are lost (operation replay; not a general CRDT) |
| `sadd` / `srem`-style collection ops | queues the operation | Operations replay — an offline `SADD` survives a concurrent server-side change to the same set |
| `jmerge` | queues the **patch** | Deep-merges into the current document — fields others changed while you were offline are preserved unless your patch touches them |
| `set` / `del` / `jset` | queues the command | **Last-writer-wins by arrival at the server** — your offline write overwrites the value when it replays |

Use the operation forms when concurrent edits matter: `cache.incr('cart:count')` instead of read-modify-`set`, `cache.jmerge(key, patch)` instead of `jset(key, '$', wholeDoc)`. The wire format is the same either way — the semantics are not.

```ts
// ❌ read-modify-write: offline, this clobbers everyone else's increments
cache.set('cart:count', String(Number(cache.get('cart:count')) + 1))

// ✅ delta: merges additively no matter who else incremented meanwhile
cache.incr('cart:count')
```

## Limits to know about

- **Persistence does not await every write.** A write's IndexedDB transaction commits shortly after the call returns; closing the tab in between can lose it. `await cache.flush()` when it matters.
- **Durability requires persistence.** Without `persistence: true`, the outbox is in-memory: offline writes replay within the tab session but are lost on reload. With it, unacknowledged writes are restored from IndexedDB on startup and re-sent on the next connect.
- **Duplicate suppression is not exactly-once.** See [its limits](/server/protocol#crash-consistency): marks swept after 24 idle hours on a busy server, a promoted replica, writes the server's persistence policy lost.
- **LWW means arrival order, not wall-clock order.** A `set` replayed from a client that was offline for an hour overwrites the server's newer value for that key. Prefer operation forms for anything multiple parties write.
- `clearPersistence()` (sign-out) discards unsent offline writes along with the local state.
- Reconnection uses `window.setTimeout` — in non-browser environments without a `window`, auto-reconnect is inactive.
- **The outbox also fills in local-only mode.** A cache created with `persistence: true` but no `connect` still records a row per write, for a later page load that connects. It never refuses a write over it: past 10,000 the oldest queued copy is dropped with an `offline write queue full` warning. The store and its WAL are unaffected. See [no server at all](/guide/use-cases#no-server-at-all).
- **Writes after `subscribe()` and binary writes need a 0.5 server.** Older servers refuse both for good.
