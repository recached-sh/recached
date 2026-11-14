# Upgrading to 0.6

This page is the procedure for moving a deployment from **0.5.1 to 0.6.0**, and back. Every
mixed-version behaviour described here is checked by
[`scripts/compat-matrix.sh`](https://github.com/recached-sh/recached/blob/main/scripts/compat-matrix.sh),
which runs real processes built from the `v0.5.1` tag against the 0.6 tree, in CI.

Read the [changelog](https://github.com/recached-sh/recached/blob/main/CHANGELOG.md) first. Most
of 0.6 changes nothing for a deployment that uses no sync tokens. These are the changes that need
action:

| If you… | You must… |
|---|---|
| run replicas | upgrade **replicas before the primary**, or run the primary with `RECACHED_REPL_DEDUP=off` until they are upgraded |
| use pub/sub from browsers in strict mode | add `chr=`/`chrw=` channel grants to tokens, or set `RECACHED_SYNC_PUBSUB=open` for the transition |
| mint sync tokens | keep minting the two-segment form (no `sub=`) until **every** server is on 0.6 |
| keep sockets open longer than their token lives | renew with `SYNC TOKEN` (`syncToken(newToken)` in the SDKs) before expiry: 0.6 closes the socket at expiry |
| pipeline over RESP and send malformed frames | expect the connection to close on a protocol error, as Redis does |

## What does not change

- **Snapshots and the AOF.** 0.6 reads a 0.5.1 snapshot and replays a 0.5.1 AOF unchanged (tested
  with every value type and a TTL). Files written by 0.6 use the same formats. One record is new
  to an old server — see [rolling back](#rolling-back).
- **Two-segment tokens** (`entries|expiry`) verify on 0.6 exactly as on 0.5.1.
- **Clients.** The 0.5.1 Rust embed and mobile clients pass their full live suites against a 0.6
  server, and the 0.6 clients against a 0.5.1 server.
- **Memory layout changes are in memory only.** Nothing on disk records them.

## Upgrade order

### 1. Replicas

Upgrade each replica and restart it. A 0.6 replica follows a 0.5.1 primary in the old stream
format and logs a warning that a promotion may re-run retried writes (the 0.5 behaviour). Check
`INFO replication` on the primary until every replica is connected again.

### 2. Primary

Upgrade the primary last. A 0.6 primary negotiates DEDUP-aware replication with 0.6 replicas, so a
promoted replica no longer re-runs a client's retried write ([details](/server/protocol#dedup-aware-replication-0-6)).

If you must upgrade the primary first, start it with `RECACHED_REPL_DEDUP=off`. Without that, it
**refuses** 0.5.1 replicas at the handshake and logs why. The replicas keep retrying and hold
their old data, which is stale but not corrupted. With `RECACHED_REPL_DEDUP=off`:

- 0.5.1 replicas receive the 0.5 stream.
- `COPY` is the one 0.6 write that replicates verbatim, and 0.5.1 does not know it. A 0.5.1
  replica that receives one drops the stream, reconnects and **resynchronizes in full**. It
  converges, and it does not skip the write. But every `COPY` costs each old replica a full
  synchronization, so avoid `COPY` until the replicas are upgraded.

Remove `RECACHED_REPL_DEDUP=off` once every replica is on 0.6, and restart the primary.

### 3. Tokens and clients

Once every server — primaries and replicas, since replicas serve WebSocket clients too — is on 0.6:

1. Add `chr=`/`chrw=` grants for the channels browsers use (or set `RECACHED_SYNC_PUBSUB=open`
   while you do).
2. Start minting claims tokens: `entries|expiry|sub=<user-id>`. A 0.5.1 server **refuses** such a
   token rather than accept it without its claims, so minting them early breaks every client of
   an old server.
3. Make sure clients renew before expiry. 0.6 closes a socket whose token expires with close code
   `4002`, and one whose replacement token drops a held scope or changes `sub` with `4003`. A
   client that reconnects with the expired token is refused, so the app must fetch a fresh one.

### 4. Optional: the multitenant profile

When browsers of **different users or tenants** share a server, turn on the fail-closed profile:

```bash
RECACHED_SECURITY=multitenant
RECACHED_SYNC_SECRET=<at least 32 random bytes>
RECACHED_ALLOWED_ORIGINS=https://app.example.com
RECACHED_PASSWORD=<backend password>      # must be non-empty
RECACHED_MAX_MEMORY=2gb                   # or RECACHED_MAX_KEYS
# optional:
RECACHED_SYNC_MAX_TOKEN_TTL=3600          # default 86400
RECACHED_SYNC_AUDIENCE=app.example.com    # tokens must then carry aud=
```

The server refuses to start and lists every missing setting. Under the profile, tokens must carry
an expiry and `sub`, and the WebSocket credential is the token itself (`RECACHED_WS_AUTH=token`):
browsers no longer need `RECACHED_PASSWORD`, which unlocks the unscoped RESP port. See the
[security checklist](/server/security#multitenant-profile).

An empty `RECACHED_PASSWORD` or `RECACHED_REPL_PASSWORD` is treated as **unset** in 0.6. Before,
it was a password that `AUTH ""` satisfied.

## Rolling back

0.6 → 0.5.1 is supported with one step first: **run `SAVE` on 0.6, then stop it and start 0.5.1.**
`SAVE` writes a snapshot and truncates the AOF, and 0.5.1 starts from the snapshot with all the
data (tested).

Without the `SAVE`:

- An AOF holding only 0.5-era records (including `DEDUP` envelopes) replays on 0.5.1.
- An AOF holding a `COPY` makes 0.5.1 **refuse to start** (`ERR unknown command 'COPY'`). Nothing
  is lost: start 0.6 again on the same files, run `SAVE`, and retry.

Every other 0.6 write — `GETDEL`, `GETEX`, `RENAMENX`, `ZPOPMIN`/`ZPOPMAX` — is logged and
replicated as a primitive 0.5.1 already knows (`DEL`, `PEXPIREAT`/`PERSIST`, `RENAME`, `ZREM`).

For replicated deployments, roll back the **primary first**: a 0.6 replica follows a 0.5.1
primary, but a 0.6 primary refuses 0.5.1 replicas unless `RECACHED_REPL_DEDUP=off`.

Tokens: stop minting claims tokens before rolling back, or clients of the old server are refused
until their next token.

## New commands

`GETDEL`, `GETEX`, `EXPIRETIME`, `PEXPIRETIME`, `RENAMENX`, `COPY`, `HSTRLEN`, `ZPOPMIN`,
`ZPOPMAX` and `ECHO`. Each is compared reply for reply, errors included, against a pinned Redis
7.2.5 in CI. Known differences: `COPY … DB n` is refused for `n ≠ 0` (one database), and RESP3
score replies keep the RESP2 shape. See [commands](/server/commands#redis-compatibility-of-the-0-6-additions).

On scoped WebSocket connections, a push of a command that names several keys — `COPY src dst`,
`SUNIONSTORE dst src…` — reaches only sockets that may read **every** key the frame names. A
socket that may read the destination but not a source gets no raw push for that write. Its live
queries and watches on the destination still receive the new value.

## Memory

0.6 stores small keys in about a third less memory. RSS growth at 1M keys with 32-byte values went
from 532 to 373 bytes per key, and with a TTL from 637 to 427 (median of 3 runs on one macOS
host). Overwriting a field of a small hash no longer grows it: through 0.5.1, the first `HSET`
overwrite of a hash whose field table was exactly full — 3, 7, 14, 28 … fields — doubled the
table (228 bytes of live heap per 3-field hash in 0.6's layout). That is why overwriting many
3-field hashes appeared to double RSS.

Throughput and latency of 0.6 against 0.5.1 have not been measured on a controlled host. Plan
capacity from your own load test, not from these figures.

## Known limitations

- The RESP port stays all-or-nothing: one password, no ACLs.
- Tokens cannot be revoked before they expire. Rotating `RECACHED_SYNC_SECRET` and restarting
  invalidates all of them.
- After a mass `DEL`, the keyspace table keeps its capacity until `FLUSHDB`. Memory is not
  returned to the OS, but it is reused by new keys.
