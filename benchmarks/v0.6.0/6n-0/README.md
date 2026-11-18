# v0.6.0 Phase 6N-0: release-candidate validation kit

6N-0 validates a release candidate before any further AOF optimization. A release candidate is a dev commit plus a binary built from it. The kit extends the 6M kit rather than replacing it:

- [`../6m/linux/run.sh`](../6m/linux/run.sh) builds and runs everything.
- [`../6m/aof-profile.sh`](../6m/aof-profile.sh) runs each matrix cell.
- [`../6l/soak/`](../6l/soak) runs the soak.
- [`observers.mjs`](observers.mjs) is new: WebSocket and WATCH/QSUB observers.

Raw results from a run go in this directory, one folder per run, with the tarball's `conditions.txt` and `binaries.txt` kept.

## Running it

Run it on dedicated native Linux, not a Flatpak or toolbox sandbox and not a shared or loaded host. See the prerequisites at the top of `run.sh`.

```sh
git clone https://github.com/recached-sh/recached && cd recached
NEW_REF=<the RC's 40-hex dev SHA> bash benchmarks/v0.6.0/6m/linux/run.sh          # G H I, ~3-4 h
NEW_REF=<sha> PHASES=E SOAK_SECS=2400 bash benchmarks/v0.6.0/6m/linux/run.sh       # soak, 5 x 40 min
```

To measure the recorded RC binary itself instead of a fresh build of the same commit, add `NEW_BIN=<path> NEW_SHA256=<hex>`.

## Provenance rules (what changed from 6M)

- **`NEW_REF` is required and must be a commit SHA.** The 6M default (`origin/perf/v0.6m-aof-group-commit`) resolved to a branch whose history `dev` never had, so the kit now refuses a branch name or an unset ref.
- **Checked-out trees:** each must be exactly the resolved commit with no local edits, or the kit stops.
- **Binaries:** each label builds in its own `CARGO_TARGET_DIR`. A shared one can hand back a binary built from another worktree.
- **Checksums:** each binary's SHA-256 is recorded at build time and re-verified before every phase and at the end. `NEW_SHA256`, if given, must match.
- **Kit scripts:** these come from the checkout `run.sh` runs from, not from the tree under test. Its commit and any local edits are recorded in `conditions.txt`.

## Phases

| Phase | What | Comparable with Redis? |
|---|---|---|
| G | SET matrix: 6l / 6m / 6m-off / Redis 7.2.5 / Valkey 9.1.0 × workers 1, 4 × P 1, 16 × clients 1, 8, 50, 100 × AOF off / everysec / always | yes |
| H | HGET and new-key INCR (same matrix, AOF off / everysec) | yes |
| H | AOF with replication enabled; with 200 WebSocket `SYNC key:*` subscribers; with 50 TCP clients each WATCHing 200 keys; with 100 WebSocket `QSUB` clients | **no**, Recached-specific sync paths, reported separately |
| I | Open-loop, rate-limited SET at `RATES` req/s for every server, so latency compares at equal throughput | yes. Needs `valkey-benchmark --rps`; if the build lacks it, the kit writes `I-skipped.txt` and the cell is NOT VERIFIED |
| E | The 6L soak: 6l/6m × reservations on/off, plus **run 5**: 6m with reservations on and a **binding memory cap** | — |

Each matrix row records:

- throughput, p50/p95/p99, server CPU-seconds and µs/request, and RSS;
- for Recached, the `INFO persistence` AOF counters: batches, frames, `write(2)` calls, fsyncs, queue wait, waiters, and frames-per-batch histogram.

`summarize.py` turns each `results.csv` into medians across reps.

"6m-off" is the build under test with `RECACHED_AOF_COALESCE=off`. That is the closest available build to "6M before coalescing", since 6M's counters and coalescing landed together.

## Run 5: binding memory cap

`MEMORY_BINDING=1` changes the soak so that memory, not the key cap, binds:

- The cap is `MAX_MEMORY_BINDING` (default 192 MB).
- The key cap is set out of reach.
- A large-value writer runs alongside: `BIG_RATE` (40) values/s of `BIG_SIZE` (128 KiB). 7 in 8 are `big:*` with EX 60; 1 in 8 overwrite `bigfix:*` with no TTL.
- `INFO memory` is sampled at 20 Hz.

The verdict adds:

- **`used_memory` never above `maxmemory`, with no tolerance.** The engine's own accounting must hold the hard limit. A sampling allowance in a monitoring script is not permission for the engine to exceed it.
- **The cap was actually binding:** peak ≥ 90% of the limit and evictions or OOM refusals seen. A run where it never bound fails as "not exercised" rather than passing.
- **Memory after the crash-recovery restart is within the limit.**
- **The only refusals are OOM:** under `volatile-lru`, OOM is the documented answer once nothing evictable is left, and is counted. Every other error still fails.

Capped reservations stay off by default (`RECACHED_CAPPED_RESERVATIONS`). This run measures them; it does not change the default.
