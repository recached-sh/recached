# 6M: cross-connection AOF group commit

| File | What |
|---|---|
| `aof-profile.sh` | One redis-benchmark load per cell of servers × policies × workers × tests × pipeline × clients, against a fresh server. Records throughput, p50/p99, server CPU-seconds and (Recached) the `INFO persistence` AOF counters' delta. Linux and macOS. `STRACE=1` adds `strace -c` runs (Linux) |
| `summarize.py` | `results.csv` → Markdown: rps, µs CPU/request and p99 per server; frames per `write(2)`, writes per million requests and tasks per write for Recached |
| `linux/run.sh` | The dedicated-Linux kit. Builds 6L (`dev` 08e5a7d), 6M, Redis 7.2.5 and Valkey 9.1.0, then runs phases A–E (below) and packs a tarball |
| `mac-indicative/` | Counter runs from the dev Mac. The counters are exact; the timing is not evidence (see each `conditions.txt`) |

## The AOF counters

`INFO persistence` on a server with an AOF now includes:

| Field | Meaning |
|---|---|
| `recached_aof_batches` / `_frames` / `_bytes` | Batches written, logical appends in them, bytes |
| `recached_aof_write_calls` / `_write_usec` | `write(2)` calls (more than batches only on short writes) and time inside them |
| `recached_aof_fsyncs` / `_fsync_usec` | `fdatasync` calls (per batch under `always`, per tick under `everysec`) and time |
| `recached_aof_queue_wait_usec` | Summed over batches: first append → start of write |
| `recached_aof_batch_waiters` | Summed over batches: tasks holding the batch's completion when it was written |
| `recached_aof_inline_batches` / `_blocking_batches` | Written on an async worker vs the blocking pool |
| `recached_aof_lead_busy` | Attempts to lead that found a write in progress |
| `recached_aof_lock_contended` | Queue-lock acquisitions that had to wait |
| `recached_aof_gate_parked` | Connection gates that found their batch not yet written |
| `recached_aof_coalesce_gained` / `_missed` | Coalescing yields after which the batch had / had not gained an append |
| `recached_aof_batch_frames_log2_hist` | Frames per batch: buckets 1, 2–3, 4–7, … , 2048+ |

All but the last five are updated once per batch by its writer, never per command.

## Linux kit phases

| Phase | What | Rough time |
|---|---|---|
| A | 1 worker: 6l, 6m, Redis, Valkey × everysec/no/always × P 1/16 × 1–200 clients; 6m with `RECACHED_AOF_COALESCE=off`; the replication-listener path | 45–75 min |
| B | 4 workers, the same for 6l/6m/Valkey | 20–30 min |
| C | 50 clients: SET (existing/new), INCR, HSET, MSET, GET × everysec/none × 1/4 workers | 25–40 min |
| D | `strace -c` at 1 and 50 clients; `perf record` call graphs if permitted | 10 min |
| E | The 6L soak, run 4×: 6l/6m × reservations on/off (`PHASES=E`) | 4 × `SOAK_SECS` |
| F | `perf stat` instructions and cycles per request, and flat/inclusive profiles: LPUSH, SADD, MSET, SET × P 1/16, 6m vs Redis, no persistence | 10 min |

```sh
curl -fsSLO https://raw.githubusercontent.com/recached-sh/recached/perf/v0.6m-aof-group-commit/benchmarks/v0.6.0/6m/linux/run.sh
bash run.sh                          # A B C D F
PHASES="E" SOAK_SECS=2400 bash run.sh   # the soak, 4 x 40 min
```

Run it natively: not in a Flatpak or toolbox (the 6L runs were inside a Flatpak). The kit records whether it was sandboxed.
