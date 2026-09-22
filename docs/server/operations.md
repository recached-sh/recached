# Operations

Running Recached in production: what it exports, what to alert on, and which limits fail closed.

## Metrics endpoint

Recached serves Prometheus metrics on a separate port from the cache itself, so you can expose it to
your monitoring network without exposing the data plane.

```bash
RECACHED_METRICS_PORT=9090 recached-server
curl http://127.0.0.1:9090/metrics
```

The port is set by [`RECACHED_METRICS_PORT`](/server/configuration#environment-variable-reference)
and binds to the same host as `RECACHED_BIND`.

::: warning The metrics port has no authentication
It inherits `RECACHED_BIND` but not `RECACHED_PASSWORD`. Anything that can reach the port can read
your metrics — including key hit/miss volume and per-command traffic. Bind it to a private interface
or firewall it.
:::

## What is exported

Traffic metrics are event-driven:

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `recached_commands_total` | counter | `command` | Commands executed, by command name. |
| `recached_command_errors_total` | counter | `command` | Commands that returned an error, by command name. |
| `recached_connections_total` | counter | `type` = `tcp` \| `ws` | Connections accepted since start, split by transport. |
| `recached_connections_active` | gauge | — | Connections currently open (TCP and WebSocket combined). |
| `recached_keyspace_hits_total` | counter | — | Reads that found a live key. |
| `recached_keyspace_misses_total` | counter | — | Reads that found nothing or an expired key. |
| `recached_command_duration_seconds` | histogram | `command` | End-to-end store execution time by command. |

### Persistence and queue health

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `recached_persistence_healthy` | gauge | — | `1` while persistence is healthy; `0` after an AOF or checkpoint failure. Writes then return `MISCONF` until `SAVE` succeeds. |
| `recached_persistence_errors_total` | counter | `operation` | Persistence failures by operation; `operation="all"` is the aggregate. |
| `recached_snapshot_saves_total` | counter | `status` | Completed and failed checkpoints. |
| `recached_snapshot_duration_seconds` | histogram | — | Time a checkpoint holds the save and all-write barriers. |
| `recached_last_successful_save_timestamp_seconds` | gauge | — | Unix timestamp of the last successful checkpoint. |
| `recached_last_save_age_seconds` | gauge | — | Seconds since the last successful checkpoint. |
| `recached_aof_bytes` | gauge | — | Current AOF file size when AOF is enabled. |
| `recached_notification_overflows_total` | counter | — | WATCH/QSUB clients disconnected because their bounded notification queue filled. |
| `recached_sync_lag_disconnects_total` | counter | — | WebSocket sync clients closed (code `4001`) because they fell behind the mutation fan-out far enough to miss frames. They reconnect and resynchronise via `qstate`, so this is a backpressure signal, not data loss — but a sustained rate means clients cannot drain as fast as you are writing. |
| `recached_pubsub_overflows_total` | counter | — | Pub/sub clients disconnected because their bounded delivery queue filled. |
| `recached_scope_denials_total` | counter | `reason` | Commands refused on scope-limited WebSocket connections. A misconfigured scope fails silently from the server's side — the page just stops working — so this is the signal that it happened. `reason="admin"` is refused by design and is normally non-zero; the other three mean something is wrong. `read_only` is a grant that is too narrow (the key is granted `r=`, the command writes), `out_of_scope` a grant that is missing entirely, and `no_token` a client issuing commands before `SYNC TOKEN`. A rising `read_only` after a deploy usually means a page was given a write it was never granted. |

### Capacity and sync

Sampled every 5 seconds, because these are levels rather than events.

| Metric | Type | Meaning |
|---|---|---|
| `recached_memory_bytes` | gauge | Incremental logical bytes for stored keys and values. This is not process RSS. Compare it with `RECACHED_MAX_MEMORY`. |
| `recached_keys` | gauge | Maintained stored-key count. Expired entries remain until bounded active expiry removes them. Compare it with `RECACHED_MAX_KEYS`. |
| `recached_evictions_total` | counter | Keys evicted since start. A rising rate means the cache is working at its cap. |
| `recached_replicas_connected` | gauge | Replicas currently attached to this primary. |
| `recached_live_queries` | gauge | Registered `QSUB` patterns across all connections. |
| `recached_watched_keys` | gauge | Keys under `WATCH`. |
| `recached_dedup_clients_tracked` | gauge | Clients with duplicate-suppression high-water marks in memory. |
| `recached_replication_queue_depth` | gauge | Deepest replica send queue, in frames — work the primary has not yet put on the wire. |
| `recached_replication_queue_bytes` | gauge | Encoded bytes in the deepest replica send queue. |
| `recached_replication_lag_frames` | gauge | Frames the furthest-behind replica has been sent but has not acknowledged applying. Zero means every replica is caught up. |
| `recached_replication_backlog_bytes` | gauge | Bytes retained for partial replica resynchronization. |
| `recached_replication_syncs_total` | counter | Full and partial synchronizations, labeled by `type`. |
| `recached_replication_sync_duration_seconds` | histogram | Initial full or partial synchronization time, labeled by `type`. |
| `recached_replication_disconnects_total` | counter | Replica disconnects caused by the frame or byte queue limit, labeled by `reason`. |

Recached does not implement `SLOWLOG`. The command histogram identifies which command class is slow; use client-side tracing when you need individual request attribution. Process RSS remains the capacity metric for allocator overhead, network buffers, and fragmentation.

### Reading the two replication gauges

They fail differently, which is why both exist:

- **Queue depth high, lag high** — the primary cannot hand frames off fast enough. The replica's
  channel is backing up, usually a slow or saturated network link. A replica whose queue fills is
  disconnected outright. It resumes from the retained backlog when possible and otherwise receives a fresh snapshot.
- **Queue depth zero, lag high** — everything was written to the socket and the replica is not
  acknowledging it. The frames are in flight, or the replica is applying them slowly, or it is
  wedged. This is the case queue depth alone cannot see, and it is the one worth alerting on.

Lag is measured in frames, not bytes or seconds: one frame is one replicated write command. The
`RCP1` replication handshake rejects incompatible peers; upgrade primary and replicas together.

### What is still not exported

- **Client outbox depth.** That state lives in the browser — read it there with
  `cache.pendingWrites()`.

## Useful queries

```promql
# Command throughput by command
rate(recached_commands_total[1m])

# Error ratio — the single most useful health signal
sum(rate(recached_command_errors_total[5m]))
  / sum(rate(recached_commands_total[5m]))

# Cache hit ratio
sum(rate(recached_keyspace_hits_total[5m]))
  / (sum(rate(recached_keyspace_hits_total[5m])) + sum(rate(recached_keyspace_misses_total[5m])))

# Connection headroom against RECACHED_MAX_CONNECTIONS (default 1024)
recached_connections_active

# WebSocket sync clients specifically
rate(recached_connections_total{type="ws"}[5m])
```

## Suggested alerts

Thresholds are starting points — tune to your traffic.

| Alert | Condition | Why it matters |
|---|---|---|
| Error-rate spike | error ratio > 1% for 5m | Usually a client sending unsupported commands or malformed args after a deploy. |
| Connection saturation | `recached_connections_active` > 80% of `RECACHED_MAX_CONNECTIONS` | New connections are rejected once the semaphore is exhausted — this fails hard, not gracefully. |
| Hit ratio collapse | hit ratio drops sharply vs baseline | Keys expiring faster than expected, an eviction storm, or a cold restart. |
| Traffic flatline | `rate(recached_commands_total[5m]) == 0` while clients are up | The process is alive enough to scrape but not serving. |
| Memory pressure | `recached_memory_bytes` > 80% of `RECACHED_MAX_MEMORY` | Eviction is about to start, or already has. |
| Eviction churn | `rate(recached_evictions_total[5m])` climbing | The working set no longer fits; results will start missing. |
| Replica lost | `recached_replicas_connected` drops | Failover risk — the standby is no longer following. |
| Replica falling behind | `recached_replication_lag_frames` > 1000 for 5m | The standby is not keeping up; a failover now would lose those writes. |
| Persistence unhealthy | `recached_persistence_healthy == 0` | Writes are being refused after an AOF or checkpoint failure. Fix storage, then run `SAVE`. |
| Save stalled | high `recached_snapshot_duration_seconds` or rising `recached_last_save_age_seconds` | Checkpoints pause writers and may be blocked on storage. |
| Slow consumers | increase in either overflow counter | A pub/sub, WATCH, or QSUB client cannot drain its bounded queue. |

## Health checking

There is no dedicated HTTP health endpoint. Use the protocol itself:

```bash
# Liveness — is the cache answering?
redis-cli -p 6379 ping        # → PONG

# With auth enabled
redis-cli -p 6379 -a "$RECACHED_PASSWORD" ping
```

For container orchestration:

```yaml
livenessProbe:
  exec:
    command: ["redis-cli", "-p", "6379", "ping"]
  initialDelaySeconds: 5
  periodSeconds: 10
```

The `/metrics` endpoint returning 200 proves the metrics listener is up, **not** that the cache is
healthy — they are separate listeners. Probe the cache port for liveness and require
`recached_persistence_healthy == 1` for write readiness when persistence is enabled.

For a human-readable snapshot at a terminal — uptime, connected clients, keyspace size, replication
role — use [`INFO`](/server/commands#info):

```bash
redis-cli -p 6379 INFO              # all default sections
redis-cli -p 6379 INFO replication  # just the topology
```

`INFO` and Prometheus serve different jobs and neither replaces the other: `INFO` is a point-in-time
snapshot for an operator or a client's ready-check, while `/metrics` carries the per-command
counters, error counts, and history that dashboards and alerts need. Alert on the metrics, not on
scraped `INFO` output.

## Capacity limits

Hard limits compiled into the server. Exceeding them produces errors rather than degradation, so it
is worth knowing where the walls are:

| Limit | Default | Configurable |
|---|---|---|
| Max connections | 1024 | `RECACHED_MAX_CONNECTIONS` |
| Consecutive auth failures before disconnect | 5 | No |
| Read buffer per TCP connection | 64 MB | No |
| Queued commands per `MULTI` | 10,000 | `RECACHED_MAX_MULTI_QUEUE` |
| `WATCH`ed keys per connection | 1,024 | `RECACHED_MAX_WATCHES_PER_CONN` |
| Live queries (`QSUB`) per connection | 64 | `RECACHED_MAX_LIVE_QUERIES` |
| Pub/sub channel and pattern subscriptions per connection | 1,024 | `RECACHED_MAX_PUBSUB_SUBSCRIPTIONS` |
| Keys allowed in a complete live-query initial state | 10,000 | `RECACHED_MAX_QSUB_INITIAL_KEYS` |
| Keys sampled per eviction pass | 10 | `RECACHED_EVICTION_SAMPLE` |
| Replication frame | 512 MB | No |
| WATCH/QSUB delivery queue | 256 messages and 8 MiB per connection | No |
| Pub/sub delivery queue | 256 messages and 8 MiB per connection | No |
| Replica delivery queue | 4,096 frames and 8 MiB per replica | `RECACHED_REPL_BUFFER` / `RECACHED_REPL_BUFFER_BYTES` |
| Partial-resync backlog | 16 MiB per primary | `RECACHED_REPL_BACKLOG_BYTES` |
| Glob pattern length (`KEYS`, `SCAN MATCH`, `PSUBSCRIBE`, sync scopes) | 1,024 bytes | No |
| Elements reserved up front for an aggregate | 1,024 | No |
| Client outbox (browser, offline writes) | 10,000 writes | via `sync-client` |

The keyspace cap (`RECACHED_MAX_KEYS`) and memory cap (`RECACHED_MAX_MEMORY`) are configured rather
than compiled — see [Configuration](/server/configuration#environment-variable-reference).

## Backup and restore

Snapshots are MessagePack files at `RECACHED_SAVE_PATH`, written atomically (temp file, fsync,
rename, and directory fsync). A single snapshot read therefore sees a complete old or new file, but
a durable deployment is a checkpoint set: snapshot, `.dedup` sidecar, and—when enabled—AOF. Do not
copy those files one by one while writes or another save can run; that can combine files from
different checkpoints.

```bash
# Take a snapshot on demand, then confirm it completed
redis-cli -p 6379 BGSAVE
redis-cli -p 6379 LASTSAVE     # timestamp advances when the save lands

# Stop the server after SAVE, then copy the snapshot, its .dedup sidecar,
# and the AOF (when configured), or capture them with one atomic filesystem
# snapshot while the server is paused.
```

A sidecar file sits next to the snapshot with a `.dedup` extension, holding duplicate-suppression
high-water marks. Back it up as part of the same checkpoint set: without it a restarted server can
re-apply a write a client replays. A missing sidecar is a clean first boot; a present but corrupt
sidecar is a startup error.

To restore, stop the server, put the snapshot (and its `.dedup` sidecar) at `RECACHED_SAVE_PATH`, and
start it — both load at boot. There is **no import path from a Redis RDB file**; the formats are unrelated.

If AOF is enabled, the AOF replays on top of the snapshot. Losing the AOF while keeping the snapshot
costs you every write since the last save.

## Upgrades

Recached is pre-1.0 and the wire protocol is not frozen — see the
[protocol spec](/server/protocol). Read the [changelog](https://github.com/recached-sh/recached/blob/main/CHANGELOG.md)
before upgrading a minor version, and upgrade server and browser SDK together. The replication
protocol identifies itself as `RCP1`; mixed replication protocol versions fail explicitly and are
not supported.

The current versioned snapshot envelope accepts legacy bare-entry snapshots. The reverse is not
guaranteed, so keep a complete copy of the pre-upgrade checkpoint set if you may need to roll back.
