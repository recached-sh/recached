# Offline & Reconnection

Browsers go offline. Recached is built so that when they do, the app keeps working — and when the connection returns, state converges without glue code.

## What happens automatically

**While connected**, every local write applies instantly to WASM memory and streams to the server.

**When the connection drops:**

- Reads keep working — they never left local memory to begin with.
- Writes keep working: they apply locally and queue as *operations* in a durable outbox (up to 10 000; beyond that the oldest queued write is dropped with a console warning). With persistence enabled, the outbox lives in IndexedDB — offline writes survive a full page reload and still reach the server.
- The client reconnects with exponential backoff: 500 ms, doubling to a 30 s cap.

**When the connection returns**, the client re-establishes the session in order:

1. `AUTH` (the password is remembered)
2. `CLIENT DELTA ON` (connection state, so re-sent every time)
3. `SYNC TOKEN` / sync scopes (remembered)
3. Every active live query is re-subscribed — the fresh `qstate` re-hydrates local keys with whatever happened server-side while you were away
4. The outbox replays FIFO

A queued write is retired from the outbox only when the server replies. A write that was sent but unacknowledged when the connection died is re-sent on reconnect. Every store write carries a `DEDUP` envelope (a per-client id plus a monotonic write id), and a running server skips ids it has already applied, replying `+DUP` so the outbox can retire the entry.

The client identity and counters are persisted with the outbox in IndexedDB. Server high-water marks are checkpointed beside the data snapshot, so a completed snapshot restores both together. An AOF can replay newer data than that snapshot without the corresponding dedup mark; after such a server crash, retrying a non-idempotent write such as `INCR` can apply it twice. Use application-level idempotency when duplicates are unacceptable.

Nothing to call, nothing to configure. Disable with `createCache({ connect: { reconnect: false } })` or stop a connection deliberately with `cache.disconnect()`.

## Merge semantics — what happens to conflicting writes

Recached queues *operations*, not final values. That choice decides how offline changes merge with concurrent changes from other clients:

| Write type | Offline behavior | Merge result |
|---|---|---|
| `incr` / `decr` | queues the **delta** (`INCRBY`) | **Additive** — your +2 and their +3 make +5, nobody's counts are lost (PN-counter semantics) |
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

- **Durability requires persistence.** Without `persistence: true`, the outbox is in-memory: offline writes replay within the tab session but are lost on reload. With it, unacknowledged writes are restored from IndexedDB on startup and re-sent on the next connect.
- **Duplicate suppression is not a transaction log.** It covers reconnects to the running server and marks included in a completed snapshot. AOF replay after the latest snapshot can reopen a duplicate window.
- **LWW means arrival order, not wall-clock order.** A `set` replayed from a client that was offline for an hour overwrites the server's newer value for that key. Prefer operation forms for anything multiple parties write.
- `clearPersistence()` (sign-out) discards unsent offline writes along with the local state.
- Reconnection uses `window.setTimeout` — in non-browser environments without a `window`, auto-reconnect is inactive.
- **The outbox also fills in local-only mode.** A cache created with `persistence: true` but no `connect` still records a row per write for a replay that can never happen, and warns `offline write queue full` past 10,000 of them. Nothing is lost — the store and its WAL are unaffected — but the queue is pure overhead there. See [no server at all](/guide/use-cases#no-server-at-all).
