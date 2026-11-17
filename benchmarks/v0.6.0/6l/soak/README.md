# 6L-G production soak

The run takes 30–60 minutes on a production-shaped topology, with failures injected. At the end it prints a PASS/FAIL verdict for each check.

It builds on the 6J soak (`../../6j/soak/`) with five changes:
- It runs natively on Linux (no Docker), using the binary the 6L kit built from `dev`.
- Both nodes use the multitenant security profile.
- There is a binding key cap, with capped-write reservations **on**.
- SCAN is checked on both nodes.
- Scope narrowing and full resync of a *live* replica are exercised.

## Run (Fedora, after `../linux/run.sh`)

```bash
sudo dnf install -y nodejs python3            # Node 22+ (global WebSocket), Python 3.9+
bash soak.sh                                   # 40 min (SOAK_SECS=2400), ~950 WebSocket clients
SOAK_SECS=3600 bash soak.sh                    # 60 min
RESERVATIONS=off bash soak.sh                  # the same soak on the default capacity path
```

**Defaults:**

| Setting | Default |
|---|---|
| Binary | `$HOME/recached-6l-bench/bin/recached-server-6l` (set `SERVER_BIN` to change) |
| CPUs | primary 0–2 (3 workers), replica 3, load generators 4–5. Change with `PRIMARY_CPUS`, `REPLICA_CPUS`, `LOAD_CPUS`, `PRIMARY_WORKERS` |
| Results | `$HOME/recached-6l-soak/results-<UTC stamp>.tar.gz` |

The exit status is the verdict's. The run needs no root, and binds only `127.0.0.1` ports 7700–7729.

## Topology and load

- **Both nodes:** `RECACHED_SECURITY=multitenant`, which means:
  - password on RESP, token-only WebSocket, allowed origins;
  - `RECACHED_MAX_KEYS` with `volatile-lru`, plus `RECACHED_MAX_MEMORY`.
- **Key cap:** set just above every key the run can create without a TTL. The `ttl:*` churn (EX 10, far more live keys than the headroom) therefore keeps it binding: TTL keys are evicted all the time, and durable keys never are.
- **Primary:** AOF `everysec`, replication listener, `RECACHED_CAPPED_RESERVATIONS=on`. Its replication queue and backlog are small (1 MiB / 2 MiB), so a stalled replica overflows both.
- **Load:**

| Component | What it does | From |
|---|---|---|
| `soak6l.py load bg` | 12 connections × 300/s. SET:GET 1:4 over 200k preloaded `bg:*` keys | new; replaces memtier (not packaged for Fedora) |
| `soak6l.py load ttl` | 4 × 1,000/s `SET ttl:<random> EX 10` | new |
| `ledger.py write` / `cas` | Acknowledged-write ledger: pipelined INCR + DEDUP. WATCH/MULTI/EXEC compare-and-set | 6J, plus AUTH |
| `ws-sync.mjs` | 600 tenants (`rw=t<i>:*`), 5% reconnecting. Probe writer with DEDUP retries | 6J, plus AUTH |
| `../../6j/ws-route.mjs` G/H/J | Token renewal every second; QSUB churn; 10% slow readers | 6J, unchanged |
| `ws-6l.mjs narrow` | 100 clients alternate `rw=n<i>:*` and `rw=n<i>:a:*` on one socket; odd clients hold a QSUB that narrowing revokes. `CLIENT PUSHFRAMES ON`, and grants switch synchronously at the token reply | new |
| `ws-6l.mjs replica` | 100 clients on the replica's WebSocket port. They reconnect after every close and record close codes | new |
| `soak6l.py scan` | Full SCAN traversals of both nodes, with COUNT 10/100/1000 | new |
| `repl_check.py probe` | Replica lag, 20 probes/s | 6J, plus AUTH and a 3 s timeout |

**Injected:**
- `BGSAVE` every 60 s.
- Replica `SIGKILL` and restart every 240 s.
- **Every 300 s, a live-replica full resync:** the replica is `SIGSTOP`ped for 15 s while 600 × 64 KiB writes overflow its queue and the backlog. On `SIGCONT` it must take a full snapshot *while its WebSocket clients are still connected*.
- **At the end:** quiesce, compare, `SIGKILL` the primary, restart it from snapshot + AOF, compare again.

## Checks (`verdict.txt`, `verdict.json`)

**Correctness and durability:**

| Check | Pass condition |
|---|---|
| Ledger after the primary's crash recovery, and on the replica | No acknowledged INCR lost; DEDUP firsts applied exactly once |
| Compare-and-set | Committed transactions + racing INCRs = counter growth |
| DEDUP retries | Every retry answered `DUP`, none applied twice |
| Replica == primary, quiesced, and again after the primary's crash recovery | The same keys (except `ttl:*`/`stall:*`) with the same values |

**Caps (6L-A, 6L-B):**

| Check | Pass condition |
|---|---|
| Follower mode | Replica `evicted_keys` = 0. A follower applies its primary's evictions and makes none of its own |
| Cap enforcement | Sampled DBSIZE never above the key cap (+ workers + 1: DBSIZE is exact only with no write in flight) |
| Cap was binding | Primary `evicted_keys` > 0; INFO only, reported if the soak never evicted |
| No refused writes | Background and TTL load saw no errors |

**Full resync of a live replica (6L-A):**

| Check | Pass condition |
|---|---|
| Stalls force a full resync | Every stall raised `recached_replica_full_resync_total` |
| WebSocket clients notified | The replica's WebSocket clients saw `4001` closes |

**Scoping:**

| Check | Pass condition |
|---|---|
| No push outside the client's grants | Every client group. A narrowing client is judged by the grants in force when each push arrived |
| Renewals and QSUB churn | No failures |
| Narrowing that revokes a held QSUB | Closed with `4003` |
| Narrowing that revokes nothing | Closes nothing; in-scope pushes keep arriving |

**SCAN (6L-C):**

| Check | Pass condition |
|---|---|
| Ordering | Every traversal in strictly increasing byte order |
| Stable keys | Every `bg:*`/`stable:*` key (present throughout, never evictable) returned exactly once, under churn, eviction and replica restarts |
| Overlap with a full resync | A replica traversal that overlapped a full resync is reported, not failed |

**Health:**
- No `panicked at` lines.
- RSS growth over the second half, restart time and replica lag are INFO.

## Reading the numbers

The two load-generator CPUs are saturated by design: an unthrottled ledger plus ~950 WebSocket clients. Latencies here measure a busy production shape and are not benchmarks; use `../linux/run.sh` for performance. Probe timeouts during stalls and restarts are expected, and counted separately.

## Smoke run (any OS)

```bash
SERVER_BIN=target/debug/recached-server SOAK_SECS=150 WS_CLIENTS=40 ROUTE_CLIENTS=15 NARROW_CLIENTS=10 \
  REPLICA_CLIENTS=10 BG_KEYS=20000 STABLE_KEYS=2000 STALL_EVERY=80 RESTART_EVERY=50 BURST_N=300 \
  bash benchmarks/v0.6.0/6l/soak/soak.sh
```

On macOS, `taskset` is skipped and `ps` replaces `/proc`.
