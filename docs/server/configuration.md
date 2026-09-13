# Configuration

Recached is configured entirely through environment variables. There is no config file.

## Environment variable reference

| Variable | Default | Description |
|---|---|---|
| `RECACHED_BIND` | `0.0.0.0` | Network interface all listeners (TCP, WebSocket, replication, metrics) bind to. Defaults to `0.0.0.0` (all interfaces). Set to `127.0.0.1` to restrict the server to localhost — strongly recommended unless the server is deliberately public. |
| `RECACHED_PASSWORD` | _(none)_ | Require clients to authenticate with `AUTH <password>`. If unset, the server accepts connections without authentication. After 5 consecutive failed `AUTH` attempts, the connection is closed. The password is compared in constant time. |
| `RECACHED_ALLOW_IPS` | _(allow all)_ | Comma-separated list of exact IP addresses allowed to connect, applied to the RESP, WebSocket and replication listeners. Any connection from an IP not in the list is immediately closed. An entry that is not a valid IP address — including CIDR ranges and hostnames — makes the server **refuse to start**, rather than silently applying a narrower allowlist than configured. |
| `RECACHED_ALLOWED_ORIGINS` | _(allow all)_ | Comma-separated list of exact origins (`https://app.example.com`, `http://localhost:3000`) permitted to open the WebSocket sync port. A handshake from any other origin is refused with `403`. `null` admits sandboxed iframes and `file://` documents. Clients that send no `Origin` header at all — every native client — are admitted. Unset accepts all origins with a startup warning. See [Security](/server/security#the-sync-port-is-a-different-threat-model). |
| `RECACHED_HANDSHAKE_TIMEOUT` | `10` | Seconds a connection may take to complete its TLS and/or WebSocket handshake before being dropped. The connection permit is taken before the handshake runs, so without this a peer that connects and then says nothing would hold one of `RECACHED_MAX_CONNECTIONS` slots indefinitely. |
| `RECACHED_WS_MAX_MESSAGE_BYTES` | `8388608` (8 MiB) | Largest complete WebSocket message the sync port will reassemble. A WebSocket message is buffered in full **before** the RESP parser sees it, so this cap — not `MAX_BULK_STRING_BYTES` — is what bounds memory, and it applies before `AUTH`. A client that opens a fragmented message and never finishes it holds this much memory per connection, so the worst case is this value times `RECACHED_MAX_CONNECTIONS`. Prior to 0.3.4 this was the library default of 64 MiB, measured at ~68 MiB held per unauthenticated connection. Raise it only if you genuinely push frames larger than 8 MiB through the browser transport. |
| `RECACHED_WS_MAX_FRAME_BYTES` | `8388608` (8 MiB) | Largest single WebSocket frame. Automatically clamped to `RECACHED_WS_MAX_MESSAGE_BYTES`, since one frame can never usefully exceed a whole message. |
| `RECACHED_SYNC_SECRET` | _(none)_ | Enables **strict sync scoping** on the WebSocket port: clients receive no mutation pushes and may run no key commands until they present a signed scope token (`SYNC TOKEN <token>`), and are then restricted to the keys their token grants. Without it, every WebSocket client receives every mutation. See [Sync Scopes](/server/sync-scopes). |
| `RECACHED_MAX_KEYS` | _(unlimited)_ | Maximum number of keys in the store. When this limit is reached, behavior depends on `RECACHED_EVICTION`. If set to `noeviction` (the default), write commands that would exceed the cap return an error. |
| `RECACHED_EVICTION` | `noeviction` | Eviction policy when `RECACHED_MAX_KEYS` is reached. See eviction policies below. |
| `RECACHED_PORT` | `6379` | TCP port the RESP listener binds. Set it to run a second instance on one host — alongside a primary, for example — or to move off 6379, the first port a commodity scanner probes. The port is not a security control (`RECACHED_BIND`, `RECACHED_PASSWORD`, TLS and the allowlists are), so changing it hides nothing on its own. An invalid value, `0`, or a value equal to `RECACHED_WS_PORT` makes the server **refuse to start**, rather than falling back to 6379 and serving the keyspace on a port the operator believes is closed. Ports below 1024 require root on Unix. |
| `RECACHED_WS_PORT` | `6380` | TCP port the WebSocket sync listener binds. Same rules as `RECACHED_PORT`, and the two must differ. Running more than one instance per host means giving each its own `RECACHED_PORT`, `RECACHED_WS_PORT` and `RECACHED_METRICS_PORT`. |
| `RECACHED_METRICS_PORT` | `9091` | Port for the Prometheus metrics HTTP server. Metrics are available at `/metrics`. Set to `0` to disable the exporter entirely. An invalid value makes the server **refuse to start**. Before 0.3.0, `0` bound an OS-assigned ephemeral port instead of disabling anything, so metrics stayed exposed on an unpredictable port; and a collision on this port aborted startup with a panic rather than an explanation. |
| `RECACHED_SAVE_PATH` | `recached.rdb` | Path to the snapshot file. The server loads this file on startup and writes to it on `SAVE`, `BGSAVE`, autosave, and clean shutdown. |
| `RECACHED_SAVE` | _(none)_ | Multi-condition autosave policy as comma-separated `seconds:changes` pairs. A snapshot is triggered when **any** condition is satisfied: `elapsed_since_last_save >= seconds` **and** `dirty_writes >= changes`. Example: `"900:1,300:10,60:10000"` — save after 1 write in 15 min, 10 writes in 5 min, or 10 000 writes in 1 min. When set, `RECACHED_SAVE_INTERVAL` is ignored. Skips saves when no writes have occurred since the last snapshot. Use `0` or an empty value to disable autosave; malformed conditions make startup fail. |
| `RECACHED_SAVE_INTERVAL` | `900` | Autosave interval in seconds (single-condition fallback when `RECACHED_SAVE` is not set). The server saves automatically at this interval if at least one write has occurred since the last save. Set to `0` to disable autosave entirely (manual `SAVE`/`BGSAVE` still work). |
| `RECACHED_AOF_PATH` | _(disabled)_ | Path to the append-only file. When set, every write command is appended to this file in addition to snapshot saves. On startup the snapshot is loaded first, then AOF commands are replayed for the delta. The AOF is truncated after each successful snapshot save. |
| `RECACHED_AOF_SYNC` | `everysec` | AOF fsync policy. `always`: fsync after every write — **see the throughput warning below before choosing this**. `everysec`: fsync once per second (default) — at most one second of acknowledged writes lost to a power cut. `no`: no explicit fsync, the OS decides. Prior to 0.2.4 all three only flushed to the operating system, so nothing was durable against power loss or a kernel panic regardless of the setting. |
| `RECACHED_WORKER_THREADS` | _(one per core)_ | Threads that execute commands. Unset means one per available core — inside a container that is the cgroup's CPU allowance, not the host's core count. Set it to share a machine with other processes, or to `1` to make command execution single-threaded, which is the baseline the [scaling benchmark](/guide/benchmarks#thread-scaling) measures its ratios against. A value that is not an integer in `1`–`1024` makes the server **refuse to start** rather than silently fall back. The count the server actually built is logged at startup. |
| `RECACHED_MAX_CONNECTIONS` | `1024` | Maximum number of concurrent connections (TCP + WebSocket + attached replicas, sharing one budget). New connections are dropped when the limit is reached. |
| `RECACHED_EVICTION_SAMPLE` | `10` | Keys sampled per eviction pass. A larger sample approximates true LRU/TTL ordering more closely at the cost of more work per eviction — the knob Redis exposes as `maxmemory-samples`. |
| `RECACHED_MAX_MULTI_QUEUE` | `10000` | Commands that may be queued inside one `MULTI`. |
| `RECACHED_MAX_WATCHES_PER_CONN` | `1024` | Keys a single connection may `WATCH`. |
| `RECACHED_MAX_LIVE_QUERIES` | `64` | Live queries (`QSUB`) a single connection may hold. |
| `RECACHED_MAX_PUBSUB_SUBSCRIPTIONS` | `1024` | Combined exact-channel and pattern subscriptions a single connection may hold. Repeating an existing subscription is idempotent and does not consume another slot. |
| `RECACHED_MAX_QSUB_INITIAL_KEYS` | `10000` | Maximum keys in a live query's complete initial `qstate` reply. A pattern matching more keys is refused instead of returning an unsafe partial snapshot. Narrow the pattern or raise this limit deliberately. |
| `RECACHED_REPL_ENABLE` | _(disabled)_ | Set to `1`/`true`/`yes`/`on` to bind the replication listener. **Required on every node that serves replicas**, including a replica serving sub-replicas. Without it the port is not bound at all. Enabling it on any interface other than loopback without `RECACHED_REPL_PASSWORD` makes the server **refuse to start**. An unrecognised value is also a startup error. |
| `RECACHED_REPL_PORT` | `6381` | TCP port the replication listener binds, when `RECACHED_REPL_ENABLE` is set. Honours `RECACHED_ALLOW_IPS` and counts against `RECACHED_MAX_CONNECTIONS`. |
| `RECACHED_REPLICAOF` | _(none)_ | Set to `host:port` to run this server as a read-only replica. The first connection receives a full snapshot and then streams writes. A reconnect resumes from its last applied offset when the primary still has that offset in its backlog; otherwise it receives a fresh snapshot. Independent of `RECACHED_REPL_ENABLE`, which governs only whether *this* node accepts replicas. |
| `RECACHED_REPL_PASSWORD` | _(none)_ | Shared secret for the replication channel. When set, replicas must send this password during the handshake before receiving any data. Must match on both primary and replica. **Mandatory** when the replication listener is enabled on a non-loopback interface. Failed attempts are throttled per source address. |
| `RECACHED_REPL_TLS_CA` | _(none)_ | Path to a PEM file holding the **CA certificate** that issued the primary's certificate. Setting it enables **TLS on the outbound replication connection** and makes the primary's identity verified rather than assumed. Note this needs a real two-certificate chain — a single self-signed certificate is rejected as `CaUsedAsEndEntity`; see [Security → Encrypting replication](/server/security#encrypting-replication) for the `openssl` recipe. A public bundle such as `/etc/ssl/certs/ca-certificates.crt` works if the primary's certificate is publicly issued. Set on **replicas**. |
| `RECACHED_REPL_TLS_SERVERNAME` | _(host of `RECACHED_REPLICAOF`)_ | Name to verify the primary's certificate against. Override when `RECACHED_REPLICAOF` points at an IP but the certificate names a host — a cert for `primary.internal` does not validate against `10.0.1.5` unless it carries that IP as a SAN. |
| `RECACHED_FAILOVER_TIMEOUT` | _(deprecated; ignored)_ | Retained for configuration compatibility. Recached logs a warning and never promotes automatically because timeout-only promotion has no quorum or fencing. Fence the old primary, then send `REPLICAOF NO ONE` to the chosen replica. |
| `RECACHED_REPL_BUFFER` | `4096` | Maximum pending frames per replica. Reaching either this limit or `RECACHED_REPL_BUFFER_BYTES` disconnects the replica. It then attempts a partial resync and falls back to a snapshot when its offset is no longer retained. |
| `RECACHED_REPL_BUFFER_BYTES` | `8mb` | Maximum encoded bytes queued for one replica. This is the primary memory bound; the frame count separately protects workloads made of many tiny writes. |
| `RECACHED_REPL_BACKLOG_BYTES` | `16mb` | Primary-wide backlog retained for partial replica resynchronization, including while no replica is connected. Once an offset falls out of this byte window, reconnecting from it requires a full snapshot. |
| `RECACHED_MAX_MEMORY` | _(unlimited)_ | Maximum logical bytes for stored keys and values. Accepts a byte count or suffix such as `512mb`, `2gb`, or `1073741824`. This counter is maintained incrementally and is not process RSS. Writes enforce the limit immediately with the selected eviction policy; the background loop also checks it. |
| `RECACHED_TLS_CERT` | _(none)_ | Path to a PEM-encoded TLS certificate file. TLS is enabled on RESP, WebSocket, and the replication listener when this and `RECACHED_TLS_KEY` are set. It does not cover the metrics port. |
| `RECACHED_TLS_KEY` | _(none)_ | Path to a PEM-encoded TLS private key file. Set both this and `RECACHED_TLS_CERT` or neither — if exactly one is present the server **refuses to start** rather than silently serving plaintext. |
| `RUST_LOG` | `info` | Log level. Accepts `error`, `warn`, `info`, `debug`, `trace`. Module-specific: `RUST_LOG=recached=debug,tokio=warn`. |

---

## Eviction policies

Eviction runs inline when a write would exceed `RECACHED_MAX_KEYS` or the logical `RECACHED_MAX_MEMORY` counter. Victim selection examines at most `RECACHED_EVICTION_SAMPLE` indexed candidates per eviction, independent of total key count. An implicit eviction is propagated as an ordered `DEL` to AOF, replicas, browser peers, and key watchers.

| Policy | Behavior |
|---|---|
| `noeviction` | Write commands that would exceed the key or memory cap return an error, and the previous values remain intact. Existing keys are never evicted. Default behavior. |
| `lru` | Evicts the least-recently-used key. Applies to all keys regardless of TTL. |
| `allkeys-random` | Evicts a randomly selected key. Lower overhead than LRU. |
| `volatile-lru` | Evicts the least-recently-used key that has a TTL set. If no keys have a TTL, falls back to `noeviction`. |
| `volatile-ttl` | Evicts the key with the shortest remaining TTL. Prioritizes keys that are closest to expiring. Falls back to `noeviction` if no keys have a TTL. |

For most applications, `lru` is the right default when a key cap is configured.

---

## Durability: what survives a server crash? {#durability}

Recached persists data on the server with the same two mechanisms Redis uses — periodic snapshots (≈ RDB) and an optional append-only file (≈ AOF). What you lose when the process dies depends entirely on which are enabled:

| Configuration | Writes lost on crash |
|---|---|
| Defaults (snapshot every 15 min) | Everything since the last snapshot — up to 15 minutes |
| `RECACHED_SAVE="900:1,300:10,60:10000"` | Bounded by the tightest matching condition — busy servers snapshot every minute |
| Snapshot + AOF `everysec` | At most ~1 second |
| Snapshot + AOF `always` | Writes completed by the configured fsync boundary |
| `RECACHED_SAVE_INTERVAL=0`, no AOF | Everything — pure in-memory cache |

**Recovery order on restart:** the snapshot is loaded first, then AOF commands not covered by that snapshot are replayed on top. Each checkpoint records an AOF marker in the snapshot, so a crash after installing the snapshot but before truncating the AOF does not apply covered non-idempotent writes twice. Snapshot writes are atomic (write, fsync, rename, and directory fsync). Snapshot, AOF, or dedup corruption is a startup error rather than an empty-cache fallback. A clean shutdown attempts a final checkpoint and reports any failure.

A checkpoint serializes all save triggers and pauses writes while it installs the durable snapshot and truncates the covered AOF. Reads continue. If a snapshot, AOF append, or fsync fails at runtime, Recached reports `MISCONF` for later writes until a `SAVE` succeeds; the failed checkpoint does not clear dirty state or truncate the AOF.

Two recached-specific durability properties worth knowing:

- **Browser clients survive server loss independently.** Clients created with `persistence: true` keep their own IndexedDB-backed copy. If the server dies, browsers continue serving local reads from the last-synced state and re-sync automatically when it returns — a server crash doesn't blank your users' UIs.
- **Rate-limiter attempt state is deliberately transient.** `RLSET` config survives restarts (snapshot, AOF, and replication); in-window attempt counts restart clean by design.

---

## Common configurations

### Development (no auth, verbose logging)

```bash
RUST_LOG=debug recached-server
```

### Production with auth and key cap

```bash
RECACHED_PASSWORD="a-strong-random-secret" \
RECACHED_MAX_KEYS="1000000" \
RECACHED_EVICTION="lru" \
RECACHED_METRICS_PORT="9091" \
RUST_LOG="info" \
recached-server
```

### With IP allowlist (restrict to local network)

```bash
RECACHED_PASSWORD="secret" \
RECACHED_ALLOW_IPS="127.0.0.1,10.0.1.5,192.168.1.100" \
recached-server
```

`RECACHED_ALLOW_IPS` accepts **exact IP addresses only**. CIDR ranges and hostnames are not supported, and an entry that is not a valid address makes the server refuse to start rather than quietly applying a narrower allowlist than you wrote. In cloud environments where addresses rotate, prefer TLS plus authentication and enforce network boundaries with security groups.

### With TLS (secure RESP and WSS)

Generate a self-signed certificate for local testing:

```bash
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem \
  -days 365 -nodes -subj "/CN=localhost"
```

Start with TLS:

```bash
RECACHED_TLS_CERT="./cert.pem" \
RECACHED_TLS_KEY="./key.pem" \
RECACHED_PASSWORD="secret" \
recached-server
```

Clients connecting over RESP must now use `rediss://` (RESP over TLS). Browser clients use `wss://` instead of `ws://`.

```typescript
// Backend
const cache = new Redis('rediss://127.0.0.1:6379')
```

```typescript
// Browser
import { createCache } from 'recached-edge'
const cache = await createCache({
  connect: { url: 'wss://your.domain:6380' },
})
```

For a production certificate, use Let's Encrypt via `certbot` or provide the cert/key from your certificate authority.

### Prometheus metrics scrape

```bash
RECACHED_METRICS_PORT="9091" recached-server
```

Configure your Prometheus instance to scrape:

```yaml
scrape_configs:
  - job_name: recached
    static_configs:
      - targets: ['localhost:9091']
    metrics_path: /metrics
    scrape_interval: 15s
```

Available metrics include key count, command counts and latency, active connections, persistence health and save latency, replication lag and sync latency, memory estimates, evictions, and WebSocket client count.

### With AOF + snapshot (strong durability)

Combining snapshots with an append-only file means you lose at most a few writes on crash, rather than up to 15 minutes of writes with snapshots alone.

```bash
RECACHED_SAVE_PATH="/data/recached.rdb" \
RECACHED_AOF_PATH="/data/recached.aof" \
RECACHED_AOF_SYNC="everysec" \
recached-server
```

On startup: snapshot is loaded first, then any AOF commands written after the snapshot are replayed. The AOF is automatically truncated after each successful snapshot save.

#### What each sync mode costs

From 0.2.4 these modes genuinely `fsync`. Before that they only flushed to the operating system, so
they were all roughly the speed of `no` and none of them survived a power cut — if you benchmarked
`always` on an earlier version, that number was measuring the wrong thing.

| Mode | Worst-case loss on power cut | Measured append cost |
|---|---|---|
| `no` | Everything the OS has not written back | ~40–50 µs |
| `everysec` | Up to one second of writes | ~40–50 µs |
| `always` | Nothing | **~20 ms** |

::: danger `always` costs roughly 400× more per write
The fsync happens while the AOF lock is held, so *every* writer in the process queues behind one disk
barrier. Measured on macOS/APFS that is around 20 ms per append — **tens of writes per second, not
tens of thousands.** The server logs a warning at startup when you select it.

Choose `always` only when losing one second of acknowledged writes is genuinely unacceptable and you
have measured the throughput on your own hardware. `everysec` is the default because it costs nothing
measurable and bounds the loss to a second. Linux with ext4 or xfs is typically far faster than APFS
here, so measure rather than assuming these numbers transfer.

Note also that on macOS `fsync()` asks the OS to write back but does not force the drive to flush its
own cache — `F_FULLFSYNC` is required for that, and Recached does not currently issue it. So on macOS
`always` pays most of the cost of durability without the last step of the guarantee. Treat macOS as a
development platform for this setting.
:::

### With leader-follower replication

Run a primary and one or more read-only replicas. Replicas receive a full snapshot on first connect, then stream every subsequent write. Reconnects use the `RCP1` run id and applied offset to request only missing backlog frames when possible.

::: warning The replication listener is opt-in
`RECACHED_REPL_ENABLE=1` is required on any node that accepts replicas. Without it port 6381 is
not bound and replicas cannot attach. This changed in 0.2.4 — the port previously opened on every
node by default, unauthenticated, which meant anyone who could reach it received the entire
keyspace regardless of `RECACHED_PASSWORD`.
:::

```bash
# Primary (serves replicas, so the listener is enabled and authenticated)
RECACHED_SAVE_PATH="/data/recached.rdb" \
RECACHED_REPL_ENABLE=1 \
RECACHED_REPL_PORT="6381" \
RECACHED_REPL_PASSWORD="repl-secret" \
recached-server

# Replica (connects to primary, rejects writes)
RECACHED_REPLICAOF="primary-host:6381" \
RECACHED_REPL_PASSWORD="repl-secret" \
recached-server
```

The replica above does **not** set `RECACHED_REPL_ENABLE`: it consumes replication but does not
serve it. Add the variable only if that replica should itself accept sub-replicas (multi-tier
replication).

Replicas reconnect automatically with exponential backoff (2s → 4s → … → 30s cap) if the primary is temporarily unavailable. Write commands sent to a replica return `-READONLY`.

Each replica queue is bounded by both frames (`RECACHED_REPL_BUFFER`, default 4096) and encoded bytes (`RECACHED_REPL_BUFFER_BYTES`, default 8 MiB). A full queue disconnects that replica without blocking primary writes. The primary retains `RECACHED_REPL_BACKLOG_BYTES` (default 16 MiB) for partial resync; a reconnect falls back to a full snapshot only after its offset leaves that window or the primary restarts.

To promote a replica to primary at runtime (manual failover), send `REPLICAOF NO ONE` over any RESP connection to the replica. It immediately starts accepting writes.

### With manually fenced failover {#manual-failover}

Recached does not promote a replica from an unreachable-primary timeout. A timeout cannot distinguish a failed primary from a network partition, so automatic promotion could create two writable primaries.

Promote a replica only after an external system or operator has fenced the old primary:

1. Stop the old primary or revoke its client network access.
2. Check `recached_replication_lag_frames` on the chosen replica's upstream before the failure, if that metric is available.
3. Send `REPLICAOF NO ONE` to the chosen replica.
4. Point clients and remaining replicas at the new primary.
5. Rebuild the old primary as a replica before restoring its client access.

```bash
redis-cli -h replica-host -p 6379 REPLICAOF NO ONE
```

`RECACHED_FAILOVER_TIMEOUT` is deprecated and ignored. Use an orchestrator that supplies leader election and fencing if you need unattended failover.

### With multi-condition autosave (RECACHED_SAVE)

Fine-grained save policy that triggers on the first matching condition:

```bash
RECACHED_SAVE_PATH="/data/recached.rdb" \
RECACHED_SAVE="900:1,300:10,60:10000" \
recached-server
```

This matches Redis's default save policy: save after 1 write in 15 min, 10 writes in 5 min, or 10 000 writes in 1 min. Saves are skipped automatically when no writes have occurred since the last snapshot, so idle servers incur zero I/O.

### With snapshot persistence

By default the server saves a snapshot every 15 minutes to `recached.rdb` in the working directory. On startup it restores from that file automatically.

```bash
# Custom path and 5-minute autosave
RECACHED_SAVE_PATH="/var/lib/recached/dump.rdb" \
RECACHED_SAVE_INTERVAL="300" \
recached-server
```

```bash
# Disable autosave — trigger saves manually with BGSAVE
RECACHED_SAVE_PATH="/data/recached.rdb" \
RECACHED_SAVE_INTERVAL="0" \
recached-server
```

The snapshot file is written atomically and durably through a unique temporary file, fsync, rename, and parent-directory fsync. Expired keys are skipped on restore. Saves are serialized and pause writes until the snapshot/AOF checkpoint is complete. On SIGTERM or Ctrl-C, the server attempts a final snapshot before exiting and logs a failure if it cannot complete.

### High-connection workloads

The server accepts up to 1024 concurrent connections by default (enforced with a connection semaphore), configurable via `RECACHED_MAX_CONNECTIONS`:

```bash
RECACHED_MAX_CONNECTIONS="4096" recached-server
```

For most web applications, 1024 concurrent connections to the cache server is more than enough. Browser clients each hold one WebSocket connection; backend services typically hold a small connection pool (2–10 connections).

---

## Notes on sensitive configuration

- By default every listener binds `0.0.0.0` (all interfaces). On a shared or internet-facing host, set `RECACHED_BIND=127.0.0.1` (or a specific private interface) **and** `RECACHED_PASSWORD`, or place the server behind a firewall. The Prometheus metrics port is unauthenticated, so it should never be exposed publicly.
- Never commit `RECACHED_PASSWORD` to source control. Use an environment file (see [Installation — systemd service](/server/installation#systemd-service)) or a secrets manager (Vault, AWS Secrets Manager, Doppler).
- The password is compared in constant time to prevent timing attacks, but the brute-force lockout (5 failed attempts → disconnect) is the primary protection. Use a long random password.
- TLS is strongly recommended for any deployment where the cache server is reachable over a network that you do not fully control. Without TLS, `RECACHED_PASSWORD` is sent in plaintext on initial `AUTH`.
