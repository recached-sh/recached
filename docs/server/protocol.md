# Wire Protocol

This page is **normative**: client SDKs (browser `recached-edge`, the planned mobile bindings) and the server implement exactly what is written here. If code and this page disagree, one of them has a bug. The reference client implementation is the platform-neutral [`sync-client`](https://github.com/recached-sh/recached/tree/main/sync-client) crate.

## Transports

| Port | Transport | Framing | Audience |
|---|---|---|---|
| 6379 | TCP | RESP2 by default, RESP3 after `HELLO 3`, pipelined | Trusted backends using the supported Redis command subset |
| 6380 | WebSocket | One RESP value per frame — **text**, or **binary** for bytes that are not valid UTF-8 | Untrusted browsers / apps |
| 6381 | TCP or TLS | `RCP1` length-prefixed snapshot and offset frames | Recached replicas only |

### Replication protocol version

Replication peers begin with the four-byte `RCP1` marker and a primary run id. The replica returns
the run id and last offset it applied. The primary replies with either a full snapshot at the current
offset or every missing frame retained in `RECACHED_REPL_BACKLOG_BYTES`. Live frames carry a
monotonic 64-bit offset, and acknowledgments report the last applied offset.

The per-replica stream is bounded by both `RECACHED_REPL_BUFFER` frames and
`RECACHED_REPL_BUFFER_BYTES`. Overflow disconnects only the lagging replica; reconnect then attempts
partial resynchronization. Protocol versions other than `RCP1` are rejected, so primary and replica
server versions should be upgraded together.

### Protocol version (TCP)

A TCP connection starts in **RESP2**. `HELLO 3` switches it to RESP3; `HELLO 2` switches back; a
bare `HELLO` reports without changing anything. An unsupported version is refused with `-NOPROTO`
and leaves the connection on the protocol it already had, so a client can probe and fall back.

The version changes exactly one thing on the wire today: **pub/sub deliveries are RESP3 Push (`>`)
frames on a RESP3 connection and plain arrays (`*`) on a RESP2 one.** RESP2 has no push type, so
sending `>` to a RESP2 client is unparseable — before 0.2.2 the server did exactly that, which broke
standard Redis clients that subscribed without negotiating.

`HELLO` requires authentication when a password is set; the pre-auth reply is `-NOAUTH` and carries
no server details.

### Binary frames (WebSocket)

The WebSocket spec requires text frames to be well-formed UTF-8. A command or reply carrying bytes
that are not valid UTF-8 therefore travels in a **binary** frame instead; everything else stays in
text frames, so existing clients are unaffected. A client must accept both.

::: tip Values are binary-safe; identifiers are not
A value is stored and returned as the exact bytes sent. Keys, hash fields, set and sorted-set
members, glob patterns and channel names must be valid UTF-8 — they are looked up, matched and routed
as text — and a command carrying a binary one is rejected with
`ERR argument <n> is not valid UTF-8. Keys, fields, members and patterns must be text; only values
may be binary`. Nothing is stored, and the connection stays usable.

Before 0.2.2 values were stored as UTF-8 strings and binary was silently replaced with U+FFFD on
every transport.
:::

## Frame taxonomy (WebSocket)

Every frame a client receives is exactly one of:

| Kind | Shape | Meaning |
|---|---|---|
| **Reply** | any RESP value not matching the rows below | Response to one command this connection sent |
| **Mutation push** | RESP3 Push `>N` whose elements form a replayable command (`SET`, `HSET`, `JSET`, `JMERGE`, …) | Another client/backend mutated a key in scope — apply to the local store |
| **Pub/sub push** | RESP3 Push `>3` = `["message", channel, payload]` | Pub/sub delivery. The WebSocket transport is always RESP3 — `HELLO 2` on it is refused, because the frame taxonomy below depends on the push type existing |
| **Keychange push** | Array `["keychange", key, value]` | A watched or live-queried key changed, or the server removed it on its own initiative. `value` is a full string, nil for deletion, or a type-tagged collection containing its complete current value. |
| **Query state** | Array `["qstate", pattern, k1, v1, …]` | **Both** the reply to a `QSUB` **and** initial state to apply (same value encoding as keychange) |

### Server-initiated removal

Not every removal follows a client command. Reads mask an expired key immediately, while a background task checks at most 256 TTL-indexed keys per one-second tick and actively removes expired entries. The server announces each removal as a keychange with a nil value.

Capacity eviction is also propagated as an ordered `DEL`. AOF replay, replicas, browser peers, and key watchers therefore remove the same victim as the primary.

A successful `qstate` is complete for its pattern, so reconnect reconciliation can safely remove local keys absent from it. If the pattern matches more than `RECACHED_MAX_QSUB_INITIAL_KEYS`, the server returns an error instead of a truncated snapshot; narrow the pattern or raise the limit deliberately. Snapshot capture and registration share an ordering barrier, but response serialization and socket writes occur after that barrier is released.

WATCH, QSUB, and pub/sub delivery queues are limited to 256 messages and 8 MiB per connection. A
client that cannot drain its queue is disconnected and must reconcile after reconnecting.

The same rule governs the mutation fan-out that feeds scoped and legacy sync connections, which is
buffered separately from the queues above. A client that falls far enough behind that the server can
no longer replay the mutations it missed is **closed with WebSocket code `4001`** and the reason
`sync stream gap; reconnect to resynchronise`. It is not left connected: a mutation frame carries no
sequence number, so a client cannot detect a gap on its own, and a silently truncated stream would
leave the local replica permanently wrong while still reporting itself in sync. On reconnect the
client replays `AUTH`, `SYNC TOKEN` and every `QSUB`, and the resulting `qstate` is authoritative for
its pattern — which is what makes reconciliation correct. Server-side, the close is counted by
`recached_sync_lag_disconnects_total`; a non-zero rate means clients are being fed faster than they
can drain, not that anything is corrupt.

Expiry deletion is eventual. A local copy does not expire on its own clock, and the bounded sweep may take multiple ticks to reach a key in a large volatile keyspace. Carry and compare a deadline in the value when exact expiry matters.

## The ordering invariant (acknowledgment correlation)

> The server sends **exactly one reply per command, in the order commands were received**. Pushes may interleave anywhere, but are always distinguishable by the table above.

This is the invariant that makes reply correlation cheap: a client keeps a FIFO of sent commands; each incoming *reply* acknowledges the oldest entry. `qstate` counts as the reply to its `QSUB`; `keychange` and all `>` pushes are never replies. Acknowledgment means the server accepted and applied the write. Crash durability still follows the configured snapshot and AOF fsync policy.

## Session establishment

On every (re)connect, in this order:

1. `AUTH password` — required first when `RECACHED_PASSWORD` is set
2. `SYNC TOKEN token` (strict scoping) or `SYNC pattern…` (open mode) — see [Sync Scopes](/server/sync-scopes)
3. `QSUB pattern` per live query — the `qstate` replies re-hydrate local state
4. Replay of the outbox (unacknowledged writes), oldest first

## Deduplicated write replay (`DEDUP`)

Store writes that must not double-apply on replay are wrapped:

```
DEDUP <client-id> <wire-id> <command> <args…>
```

- `client-id`: 1–64 chars, stable per client, **unguessable** (a guessable id lets another authenticated client poison your high-water mark); browsers use `crypto.randomUUID()`
- `wire-id`: `(session-epoch << 32) | write-counter` — strictly increasing across a client's lifetime, including across page reloads (the epoch is persisted and bumped per session)
- Server behavior: per `client-id`, ids at or below the high-water mark reply `+DUP` and do **not** execute. A higher id is committed only after its wrapped write succeeds. Marks are persisted in the snapshot's `.dedup` sidecar and swept after 24 h idle.
- `+DUP` is a normal reply — it acknowledges and retires the write.
- Scope checks, replica write-rejection, and metrics apply to the *wrapped* command.
- Connection-scoped commands (`SUBSCRIBE`, `UNSUBSCRIBE`, `PUBLISH`) are **never** wrapped — a replayed subscribe must re-execute, not be skipped.

The sidecar and snapshot are one checkpoint, so neither can claim a write absent from that snapshot.
The AOF currently records the inner mutation rather than its dedup identity. If the AOF replays a
write newer than the snapshot, a client retry after that restart can apply a non-idempotent command
twice. `DEDUP` is reconnect duplicate suppression, not a cross-crash transaction log.

## Token format (strict scoping)

```
base64url(payload) "." base64url(hmac_sha256(secret, base64url(payload)))
payload = "pattern1,pattern2[|unix-expiry-seconds]"
```

The HMAC is computed over the base64url payload *text*. Expiry is validated when the token is presented.

## Limits

| Limit | Value |
|---|---|
| `qstate` initial-state entries | 10 000 keys |
| Live queries per connection | 64 |
| Client outbox | 10 000 writes (oldest dropped) |
| `DEDUP` client id | 64 chars |
| Watched keys per connection | 1 024 |
| Bulk string / total message | 64 MB |
| WATCH/QSUB delivery | 256 messages and 8 MiB per connection |
| Pub/sub delivery | 256 messages and 8 MiB per connection |
| Replica delivery | 4 096 messages and 8 MiB per replica by default |
| Partial-resync backlog | 16 MiB per primary by default |

## Compatibility rules

- **Snapshot format**: current files use a versioned envelope containing the AOF checkpoint id and
  entries. The loader also accepts legacy bare-entry snapshots. Stored-value enum variants remain
  append-only because rmp-serde encodes them by index.
- **Tagged frames**: new server-initiated Array frames must carry a new first-element tag (like `keychange` / `qstate`); clients ignore unknown tags. Untagged Arrays are replies by definition.
- **New commands** never change the one-reply-per-command invariant.
- **Replication**: the `RCP1` marker is mandatory; incompatible peers fail instead of interpreting a
  snapshot or command with the wrong framing.
- **Client IndexedDB schema**: bumps create missing object stores idempotently; existing stores are never dropped in an upgrade.
