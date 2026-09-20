# Troubleshooting

Symptoms, causes, and fixes — ordered roughly by how often each one bites.

## Server

### The server will not start, complaining about `RECACHED_ALLOW_IPS`

```
ERROR RECACHED_ALLOW_IPS: '10.0.0.0/8' is not a valid IP address. Exact addresses only —
      CIDR ranges and hostnames are not supported.
```

The allowlist accepts **exact IP addresses only**. Expand the range into individual addresses, or
unset the variable and enforce the boundary at your firewall or security group — which is the better
choice in cloud environments where addresses rotate.

Since v0.2.1 an unparseable entry is fatal at startup. Earlier versions logged a warning and dropped
the entry, which silently produced a narrower allowlist than configured — and if *every* entry was
invalid, an empty allowlist that rejected every connection while the process appeared healthy. If you
are on an older version and the server is unreachable, check the startup log for
`ignoring invalid entry`.

Confirm which mode you are in from the startup log:

```
INFO  IP allowlist ENABLED: [10.0.1.5]     ← only these IPs may connect
WARN  IP allowlist DISABLED. Accepting all connections.
```

See [Security → IP allowlisting](/server/security#ip-allowlisting).

### `NOAUTH Authentication required.`

`RECACHED_PASSWORD` is set and the client did not authenticate.

```bash
redis-cli -p 6379 -a "$RECACHED_PASSWORD" ping
# ioredis
new Redis({ port: 6379, password: process.env.RECACHED_PASSWORD })
```

### Connection closes after a few failed attempts

Five consecutive `AUTH` failures close the connection. Reconnecting resets the counter — this is
brute-force friction, not a ban. Confirm the client is sending the password you think it is.

### `READONLY You can't write against a read only replica.`

You are writing to a replica. Writes go to the primary; replicas serve reads.

If this is a failover scenario and the replica *should* now be primary, promote it:

```bash
redis-cli -p 6379 REPLICAOF NO ONE
```

Note that `REPLICAOF host port` is **not** supported at runtime — re-pointing a server at a different
primary requires a restart with a new `RECACHED_REPLICAOF`.

### New connections are refused under load

`RECACHED_MAX_CONNECTIONS` (default **1024**) is exhausted. Connections past the limit are rejected
outright rather than queued. Watch `recached_connections_active` and raise the limit, or find the
client that is leaking connections.

### The server will not start, complaining about TLS

```
ERROR RECACHED_TLS_CERT is set but RECACHED_TLS_KEY is not — refusing to start
      rather than silently serving plaintext. Set both, or neither.
```

TLS is all-or-nothing: set both variables or neither. Since v0.2.1 setting exactly one is fatal.

Earlier versions fell back to plaintext silently in this case, which is worse than a failed start —
traffic you believed was encrypted was not. If you are on an older version, verify rather than
assume: a `rediss://` client should connect and a plaintext client should be refused. See
[Security → Transport encryption](/server/security#transport-encryption).

### A replica is connected but falling behind

`recached_replication_lag_frames` counts frames sent but not acknowledged by the furthest-behind
replica. Unlike `recached_replication_queue_depth`, it stays high when the primary has written
everything to the socket and the replica is not keeping up — see
[Operations → Reading the two replication gauges](/server/operations#reading-the-two-replication-gauges).

The `RCP1` handshake rejects incompatible replication peers instead of silently using different
framing. Upgrade primary and replicas together.

### Memory keeps growing

There is no built-in memory metric — monitor process RSS. Set `RECACHED_MAX_MEMORY` and
`RECACHED_EVICTION` so the cache bounds itself, and `RECACHED_MAX_KEYS` if key count rather than
value size is the driver. `DBSIZE` reports the current key count on demand.

### A subscriber receives nothing, or cannot parse what it receives

Two separate faults, both fixed in **0.2.2**:

- **Nothing arrives until the subscriber sends another command.** Deliveries were written into a
  buffered writer that only flushed when handling a client command, so a connection that purely
  listened saw nothing. A subscriber that also polls appeared to work, which is why this went
  unnoticed.
- **Frames arrive but the client errors on them.** Pub/sub was delivered as RESP3 Push (`>`) frames
  on every connection. RESP2 has no push type, so a standard Redis client that subscribed without
  sending `HELLO 3` could not parse the frame. Deliveries now follow the negotiated version.

On an older server, neither has a client-side workaround — upgrade.

### `ERR argument N is not valid UTF-8`

A key, hash field, set or sorted-set member, or glob pattern contained bytes that are not valid
UTF-8. **Values are binary-safe** — this error only ever refers to an identifier position. Nothing
was written and the connection is still usable.

Identifiers are looked up, glob-matched and checked against sync scopes as text, so a binary one
would be unreachable through those paths. Hex- or base64-encode the identifier:

```js
await cache.set(`blob:${id.toString('hex')}`, binaryPayload); // value stays raw
```

Before 0.2.2 *values* were also required to be text and binary was silently corrupted. If you are
reading back mangled binary written by an older server, that data cannot be recovered — the bytes
were destroyed on the way in. Re-populate the affected keys.

### `ERR key too large`### `ERR key too large`

A key exceeded the maximum key length. Keys are identifiers, not payloads — put the data in the
value.

## Browser sync

### Reads work but nothing ever updates

Reads come from local WASM memory, so they succeed even with no server connection at all. A cache
that reads fine but never changes is usually **not connected**.

1. Confirm you passed `connect: { url }` to `createCache()` — without it you get a purely local
   cache, which is a supported mode and looks identical until data goes stale.
2. Check the browser console for WebSocket errors.
3. Confirm the URL scheme matches the server: `ws://` for plaintext, `wss://` when TLS is enabled.
   A page served over HTTPS cannot open a `ws://` socket — browsers block mixed content.

### `NOSCOPE key '<key>' is outside this connection's sync scopes`

The connection is scoped and the key falls outside the granted patterns. Scoping is prefix-based, so
a grant of `cart:*` admits `cart:42:item:1` but a grant of `cart:42:*` does not admit `cart:99:*`.

Check what the token actually granted before widening it — the scopes are a security boundary, and
the instinct to broaden them until the error stops is how that boundary gets lost. See
[Sync Scopes](/server/sync-scopes).

### `NOSCOPE key '<key>' is read-only on this connection`

The key is inside a grant, but a read-only one (`r=catalog:*`), and the command would write it.
Reads of the same key succeed, which is what distinguishes this from the error above.

Either the command belongs on your backend over the trusted TCP port, or the grant is wrong. Widen
it to `rw=` only if that page is genuinely allowed to change the data — a read-only grant on shared
state is usually the deliberate choice, not the oversight. Note that read-modify-write commands
(`INCR`, `APPEND`, `GETSET`, `RLCHECK`, `JMERGE`) count as writes. See
[Sync Scopes](/server/sync-scopes).

### Keyspace-wide commands fail on a browser connection

`KEYS`, `SCAN`, `DBSIZE`, `FLUSHDB`, `SAVE`, `BGSAVE`, and `REPLICAOF` are refused entirely on scoped
connections — by design, since they would leak or destroy data outside the connection's scope. Use
`liveQuery(pattern)` / `getMatching(pattern)` to work with a set of keys instead.

### Offline writes went missing

The client outbox holds **10,000 pending writes**. Past that, each new write **evicts the oldest one**
— silently, with no error surfaced to your code.

Register `cache.onOutboxFull((droppedId, pending) => …)` to be told when this happens — without it
the loss is invisible. `cache.pendingWrites()` reports the current depth, so an application can apply
back-pressure before the cap is reached.

If a client can be offline long enough to exceed 10,000 writes, do not rely on the outbox as the
system of record for those mutations. Batch them, or persist them yourself and reconcile on
reconnect.

### A write applied twice

It should not — every store write carries a `DEDUP` envelope and the server skips ids at or below the
client's high-water mark, replying `+DUP`.

Dedup marks live in memory and are checkpointed beside the data snapshot; idle entries are swept
after 24 hours. An AOF can replay a newer mutation than the snapshot without its dedup identity, so a
retry after a server crash can apply a non-idempotent command twice. If your workload cannot tolerate
that, enforce idempotency at the application level.

### Concurrent writes clobber each other

`set` is last-writer-wins **by server arrival order**, which is not the same as wall-clock order.
For values that must merge rather than overwrite, use the operation that matches the shape:

- Counters → `incr` / `decr` (deltas merge additively)
- Documents → `jmerge` (RFC 7386 deep merge)
- Collections → the collection commands, which replay as operations

See [Offline & Reconnection](/browser/offline).

### Live query returned fewer keys than expected

A live query's complete initial state is capped at **10,000 keys** by default. A broader query returns `ERR live query initial state exceeds 10000 keys` instead of a partial snapshot. Narrow the pattern or raise `RECACHED_MAX_QSUB_INITIAL_KEYS` deliberately.

`FLUSHDB` arrives as one sentinel per subscribed pattern rather than one frame per key — if your
client is hand-written, expand it locally.

If collection values arrive as a bare type name (`"hash"`) rather than their contents, the server and
SDK are on different versions — collection values ship complete from 0.2.2 onward, and the two are
released in lockstep.

### Too many live queries

64 per connection. Consolidate with broader patterns rather than opening one subscription per key.

### Cross-tab updates are not appearing

BroadcastChannel is same-origin. Tabs must share protocol, host, and port. It also does not cross
browser profiles, containers, or incognito boundaries.

## Data & persistence

### Data disappeared after restart

Confirm persistence is actually on. `RECACHED_SAVE_INTERVAL=0` disables autosave — which is what the
benchmark configuration uses, so it is easy to inherit from a copied command line.

```bash
redis-cli -p 6379 LASTSAVE   # timestamp of the last successful snapshot
```

If it returns the server start time, no snapshot has ever completed.

### Can I load a Redis RDB file?

No. Snapshots are MessagePack and unrelated to RDB. Migrating from Redis means starting with a cold
cache and letting it fill — see [Migrating from Redis](/guide/use-cases#migrating-from-redis).

## Getting help

Include the server version, the startup log lines (they state which subsystems are enabled), the
exact error string, and whether the client is RESP or the browser SDK:
[open an issue](https://github.com/recached-sh/recached/issues).
