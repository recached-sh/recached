# Sync Scopes

By default, every mutation on the server is pushed to **every** connected WebSocket client. That is the right behavior for a single-user dashboard or an internal tool — and a data leak for a multi-user application, where user A's session keys would be pushed into user B's browser.

Sync scopes fix this. Each WebSocket connection declares which keys it may see, the server authorizes the declaration, and the mutation fan-out delivers only matching keys.

## The SYNC command

WebSocket-only (the TCP port is for trusted backends and is unaffected).

| Form | Behavior |
|---|---|
| `SYNC` | Returns this connection's current grants. |
| `SYNC TOKEN <token>` | Sets scopes from a signed token — requires `RECACHED_SYNC_SECRET` on the server. |
| `SYNC <pattern> [pattern ...]` | Sets scopes directly. Only allowed when no secret is configured — this is a bandwidth filter, **not** an authorization boundary. |

Patterns are the same globs `KEYS` uses: `*` matches any sequence of bytes and `?` matches exactly
one byte — e.g. `cart:42:*`, `catalog:*`.

## Read-only and read-write grants

Each scope entry carries the access it confers:

| Entry | Grants |
|---|---|
| `r=catalog:*` | Read only. The connection is pushed these keys and may read them; every write is refused. |
| `rw=cart:42:*` | Read and write. |
| `cart:42:*` | Read and write — a bare pattern is read-write, so tokens minted before write scoping keep working unchanged. |

This is the difference between a browser that *follows* a shared read model and one that can rewrite
it. A connection granted `catalog:*` unqualified can `SET catalog:price:99 0`; one granted
`r=catalog:*` cannot:

```
> GET catalog:price:99
"1499"
> SET catalog:price:99 0
-NOSCOPE key 'catalog:price:99' is read-only on this connection
```

A key outside every grant reports differently, so a misconfigured grant is distinguishable from a
missing one:

```
> GET session:8f21
-NOSCOPE key 'session:8f21' is outside this connection's sync scopes
```

**Read-only grants still receive the fan-out.** That is the point: the browser is pushed every
catalog change and can hold live queries (`QSUB`) and `WATCH` on those keys. It simply cannot cause
a change.

Access is checked **per key**, not per command, which matters for commands that read some keys and
write another. `SINTERSTORE mine:out theirs:a theirs:b` requires write on `mine:out` and only read
on the sources, so `r=theirs:*,rw=mine:*` is enough.

Read-modify-write commands — `INCR`, `APPEND`, `GETSET`, `RLCHECK`, `JMERGE` — require write.
There is no write-without-read grant, so nothing is lost by treating them as plain writes.

::: warning `r=` and `rw=` are reserved entry prefixes
A pattern whose own text starts with `r=` or `rw=` cannot be written as a scope entry — it is read
as an access prefix. The failure is closed (such an entry grants less than its author intended, on a
different prefix, rather than more), and real key patterns do not look like this.
:::

::: warning Character classes are not supported
`[abc]` is **not** a character class here; the brackets match literally. Earlier versions of this
page said otherwise. A scope written as `user:[12]:*` therefore grants access to keys beginning with
the literal text `user:[12]:`, and matches nothing a normal application writes — it fails closed, but
it does not grant what its author intended. Enumerate the prefixes instead:
`user:1:*,user:2:*`.

Patterns are capped at 1,024 bytes. Matching is byte-wise, so `?` matches one *byte* and a
multi-byte UTF-8 character spans several positions.
:::

## Two modes

**Open mode** (no `RECACHED_SYNC_SECRET` set — the default) is backward compatible: connections that never call `SYNC` receive every mutation, exactly as before. A connection that calls `SYNC cart:*` receives only matching keys — useful for cutting bandwidth, but any client can choose any patterns, so it protects nothing.

**Strict mode** (`RECACHED_SYNC_SECRET` set) makes scopes an authorization boundary:

- A connection receives **no pushes** and may run **no key commands** until it presents a valid `SYNC TOKEN`.
- Every command is then checked against the granted scopes — reads and writes alike. `GET secret-key` outside your scopes returns `-NOSCOPE`, the same as a write.
- Keyspace-wide and administrative commands (`KEYS`, `SCAN`, `DBSIZE`, `FLUSHDB`, `SAVE`, `BGSAVE`, `REPLICAOF`) are refused entirely on scoped connections.
- Literal `SYNC <pattern>` is rejected — patterns must come from a signed token.

`PING`, `AUTH`, `MULTI`/`EXEC`/`DISCARD`, and pub/sub commands are always available. Note that pub/sub **channels** are not keys and are not scoped — don't put per-user secrets on broadcast channels.

## Scope tokens

A token is minted by your application backend — the only party that knows which user is asking — and handed to the browser client:

```
base64url(payload) + "." + base64url(hmac_sha256(secret, base64url(payload)))
```

The payload is comma-separated patterns, with an optional `|<unix-expiry-seconds>` suffix. Minting in Node.js:

```js
import crypto from 'node:crypto';

function mintSyncToken(secret, entries, expiresInSecs = 3600) {
  const expiry = Math.floor(Date.now() / 1000) + expiresInSecs;
  const payload = Buffer.from(`${entries.join(',')}|${expiry}`)
    .toString('base64url');
  const sig = crypto.createHmac('sha256', secret)
    .update(payload)
    .digest('base64url');
  return `${payload}.${sig}`;
}

// In your session/login handler:
const token = mintSyncToken(process.env.RECACHED_SYNC_SECRET, [
  `rw=cart:${userId}:*`,   // the user's own cart — theirs to change
  `rw=profile:${userId}`,
  'r=catalog:*',           // shared read model — followed, never written
]);
// → include the token in the page payload / session API response
```

The HMAC is computed over the base64url payload *text*, so there are no byte-canonicalization pitfalls — one `createHmac` call as shown is the whole minting story in any language.

The browser client then sends, over its WebSocket connection:

```
SYNC TOKEN <token>
```

and receives the granted entries as confirmation, each in the `r=`/`rw=` notation above, so the
reply can be fed straight back into `SYNC`. From that moment it receives pushes for — and can
operate on — exactly those keys, at exactly that access.

::: warning Expiry is checked at presentation time
The token's expiry is validated when `SYNC TOKEN` is presented, not continuously. An established connection keeps its scopes for its lifetime. Short-lived tokens bound the window in which a leaked token is usable — they do not cut off live connections.
:::

## Server setup

```bash
RECACHED_SYNC_SECRET="a-long-random-secret" \
RECACHED_PASSWORD="another-secret" \
RECACHED_BIND="0.0.0.0" \
recached-server
```

Use both secrets: `RECACHED_PASSWORD` gates who may connect at all; `RECACHED_SYNC_SECRET` gates what each connection may see. For browser deployments the WS port is public by definition — strict mode is the difference between "any visitor sees the whole keyspace" and "each visitor sees their own keys".

## Interaction with transactions and WATCH

Scope checks run before a command is queued inside `MULTI`, so an out-of-scope command errors at queue time rather than sneaking into `EXEC`. `WATCH` is scope-checked like any other key command — a connection cannot observe keys outside its grant. It observes rather than mutates, so a read-only grant is enough to `WATCH`; the writes inside the transaction are each checked on their own.

## Replication and multi-tier setups

Scope filtering happens per-connection at the fan-out edge. Replicas receive the full write stream (they serve their own WebSocket clients, which are scoped independently), and the AOF/snapshot are unaffected.
