# v0.6.0 benchmark data

Raw output of `scripts/bench-redis-valkey.sh`. Every run directory has `conditions.txt` (CPU, OS,
versions, binary checksums, load average at start) and `results.csv` and/or `memory.csv`.

**Host for all of it:** MacBook Pro, Intel i5-8259U (4C/8T), 8 GB, macOS 15.8.1, used
interactively during the runs. No CPU pinning, no governor control. Load averages are recorded
per run and were often far above the core count.

Binaries (checksums in each `conditions.txt`):

| Label | Build |
|---|---|
| `baseline` / `v051` | `7cdbd0d` — v0.5.1 as released on `dev` |
| `recached` / `v06pre` | `da7fc95` — v0.6 phases 6A–6C, before memory work |
| `opt1` | E1–E3 (shared index keys, conditional all-keys index, overwrite skip) |
| `opt2` / `v06opt` | E1–E7 (adds 56-byte entries and FLUSHDB shrink) — the branch as committed |
| `redis` | redis-server 7.2.5 (Homebrew, libc malloc) |
| `valkey` | valkey-server 9.1.0 (Homebrew, libc malloc) |

## Datasets and how far to trust them

| Directory | What | Trust |
|---|---|---|
| `mem-final/` | 3 reps × 5 servers × {1M × 32 B, 100k × 1 KiB} × {string, hash, ttl}, fresh process per scenario | **Use.** String and TTL ranges are tight; summarise with `scripts/bench-memory-aggregate.py`. Hash rows are noisy (3× spread) — inconclusive |
| `mem-1m-20261112`, `mem-100k-1k-*` | Single runs, used to choose the optimisation | Indicative; superseded by `mem-final/` |
| `tput-single-20261112/` | 5 interleaved rounds × 5 servers, 100k requests per test | **Do not use for throughput or latency.** 100k requests is too few for `redis-benchmark`'s ~250 ms timing resolution: rates are quantised (199,601 = 100k / 0.501 s). The server-CPU-per-million column is not quantised but was measured at load 6–18 |
| `ab-pre-vs-opt-20261112/` | `v06pre` vs `v06opt`, 3 rounds, 300k (P1) / 2M (P16) requests | **Inconclusive.** Load average 31–55 with an unrelated process at 140% CPU and 3.4 GB swapped; `v06pre` always ran before `v06opt` within a round while load rose, biasing against `v06opt`. IQRs ±40% |

| `hotpath-20261113/` | `core-engine/examples/hotpath.rs`: in-process `execute` cost, `da7fc95` vs `opt2`, ABBA ×2, best of 9 × 1M ops per run | **Use for engine CPU.** Per-run minimums do not overlap for any write; GET unchanged. Engine only — excludes networking and the server's propagation path |

No throughput or latency comparison between servers from this directory should be published.
Re-run on a quiet, pinned Linux host (`PIN=1`) before drawing CPU or latency conclusions.

## Phase 6G (2026-11-14) — memory and performance parity

Reports: `docs/reports/v0.6.0/V0.6G-*.md`. Linux data come from a Docker Desktop VM (kernel
7.0.12-linuxkit, 8 vCPU, 3.9 GB, swap never used — `6g-linux/vm-swap-monitor.log`) on the same
laptop; server and load generator pinned to disjoint vCPUs.

| Directory | What | Trust |
|---|---|---|
| `6g-linux/mem-ref/`, `6g-linux/mem-opt/` | Linux RSS matrix: 10 shapes × 3 reps, fresh process each; v0.5.1, v0.6 current, optimized (+ bg thread, + glibc malloc), Redis 7.2.5, Valkey 9.1.0. Summary: `6g-linux/memory-summary.md` | **Use.** Page accounting, no swap, tight ranges |
| `6g-linux/tput/` | memtier matrix: 27 cells × 5 servers × 3 reps (single and 4-worker). Summary: `6g-linux/throughput-summary.md`; raw JSON in `tput-json.tar.gz` | **Indicative only.** Non-dedicated host; ±15% run-to-run |
| `6g-linux/tput-bg/` | final build with vs without jemalloc's background thread | Indicative; no consistent difference |
| `6g-linux/*.sh`, `summarize.py` | image builds (Redis/Valkey/memtier from source), Recached Linux builds, drivers, summariser | — |
| `6g-layout-prototype/` | standalone crate: candidate keyspace layouts, counting allocator + jemalloc stats | **Use** for live/allocated bytes; ns/op single-run |
| `6g-hashpath/abab.txt` | `core-engine/examples/hashpath.rs`, table vs packed hash by field count | Use the low-load runs (first `table` run overlapped a build; discarded) |
| `6g-cpu-c123/` | native ABAB of C1–C3; `c2-c3-unproven.patch` = the two reverted changes | C1 effect clears noise; C2/C3 do not |
| `6g-cpu-c4/` | native A/B of the ephemeral write-path fix; `presence*.csv` = with 20k claims held | `presence*`: **use** (440× effect). `abab.csv`: noise |
| `6g-perf/` | Linux `perf` flat profiles of the final build (SET P16 1 worker; hot INCR 1 and 4 workers) | Use for shares, not absolute rates |
| `6g-hotpath-x1/` | partial ABAB, stopped early: its X1 build predates `write_slot` | Do not use |

## Phase 6H (2026-11-14/15) — request path and hot keys

Reports: `docs/reports/v0.6.0/V0.6H-*.md`. Same laptop and Docker Desktop VM as 6G. `base`/`6g` =
dev `42e5ca8` (6G merged; the 6G Linux binary reused), `6h` = `eafd080`, `nospin` = 6H with the
store-stripe spin disabled.

| Directory | What | Trust |
|---|---|---|
| `6h-request-path/` | in-process request path (`server-native/src/tests/request_path.rs`): ns and allocations per pipelined command through the real `handle_tcp`, and hot-key scaling at 1/2/4 workers; macOS, step builds `base`, `a1`…`a6`; `abba-n.sh` (forward + backward interleaving), `summarize.py` | **Use for relative change** between steps. Allocation counts exact. Host load noted per run (2–27) |
| `6h-linux/inproc/` | the same two benchmarks on Linux (VM): `base`, `nospin`, `6h` | **Use** — the primary evidence for the decoder, by-reference writes and the stripe spin |
| `6h-linux/tput/rep-1..5/` | memtier matrix, 6G vs 6H vs Redis 7.2.5 vs Valkey 9.1.0, single and 4-worker; each repetition started only at host load < 5 (`gated.sh`); summary `throughput-summary.md`; raw JSON `tput-json.tar.gz` | **Indicative only** — non-dedicated host, per-cell spread ±15–25% |
| `6h-linux/mem/` | 6G memory script, 6G vs 6H, 3 shapes × 3 reps | **Use** |
| `6h-linux/*.sh` | `build-test-bin.sh` (Linux server + test binaries), `run.sh`, `gated.sh`, `inproc.sh` | — |

## Phase 6I (2026-11-15) — production paths and storage execution

Reports: `docs/reports/v0.6.0/V0.6I-*.md`. Same laptop and Docker Desktop VM as 6G/6H (kernel
7.0.12-linuxkit, 8 vCPU, 3.9 GB). New bench image `recached-6i-bench:local` =
`recached-6g-bench:local` + Node 22.23.4 (copied from `node:22.23.4-bookworm-slim` with
`docker cp`/`docker commit`; no registry pull). Binaries are frozen in `target/6i/linux-bins`
with `SHA256SUMS`: `6h` (dev `cbae25e`, the 6H build reused), `6i` (rc2 = this branch's
commits `dd32ccc..3bd72c1`, minus a one-line DEDUP-commit reordering), `6i-rc3` (+ adaptive
expiry). Exact sources: `6i-prod/source/`.

**What is new in the method.** 6G/6H measured a memory-only server with metrics off and no
consumer. 6I measures the configurations production runs, before and after the changes:

| Driver | Configurations |
|---|---|
| `6i-prod/prod.sh` (via `run.sh`, `gated.sh`) | `standalone` (metrics off — the 6G/6H method), `standalone-metrics` (default exporter on, scraped every 5 s), `aof-everysec`, `aof-always`, `repl` (primary + replica; replica lag by probe keys timed write-to-readable on the replica, `repl_check.py`; convergence verified by DBSIZE + 900 sampled values), `capped` (1 GB cap never reached) |
| `6i-prod/ws.sh` + `ws-sync.mjs` | strict token-scoped WebSocket clients (one tenant each, every 4th WATCHing), periodic reconnects, a background writer on keys no client may see, a probe writer (in-scope SETs timed to the client, DEDUP retries); leaks, lag disconnects (close code), reconnect time, per-client RSS |
| `6i-prod/ttl-churn.sh` | expired data in memory: SET … EX 1 load, then idle, sampling DBSIZE and RSS |
| `6i-prod/perf.sh`, `build-sym.sh` | `perf` profiles in `recached-6g-perf:local` of a symbolized build |

Durability is **not** identical across engines: Redis/Valkey `appendfsync always` fsync once per
event-loop iteration; Recached `always` once per batch. Redis/Valkey AOF rewrite is disabled in
these runs; the VM's virtual disk makes every fsync figure indicative only.

| Directory | What | Trust |
|---|---|---|
| `6i-baseline/rep-1/` | the production matrix on 6H, Redis, Valkey **before** any change | Indicative; overlapped host compiles (`host-activity.txt`). The AOF and capped defects are 20–300× effects, far outside its noise |
| `6i-perf/` | perf profiles, AOF and capped, at each step of the AOF fix | Use for shares; rates are with the profiler attached |
| `6i-inproc/`, `6i-final/inproc/` | in-process request path, 6H vs 6I (metrics off and on, INCR split, hot-key scaling) | **Use** for relative cost; load ~1.0 |
| `6i-final/ws/` | WebSocket sync matrix, 6H vs 6I, 1/10/100/500 clients, 1 and 4 workers, 2 reps | Indicative; Recached-only functionality, not a Redis comparison |
| `6i-final/ttl/`, `6i-final/ttl-rc3/` | expiry backlog, 6H/6I/Redis and rc3 | Use: a memory trend, not a rate |
| `6i-final2/standalone/` | final build (`a326427`): v0.5.1, 6G, 6H, 6I, Redis, Valkey, standalone, 1 and 4 workers, INCR variants, 3 reps (`battery2.sh` A; `summary.md` from `summarize.py prod`, `best-rep.md` from `best_rep.py`) | Indicative; **heavily disturbed** (host load up to 160 between reps): 1-worker medians unreliable, `best-rep.md` understates Redis too. 4-worker ranges narrow |
| `6i-final2/prod/`, `6i-final2/aof-always/` | production configurations, 6H / 6I / Redis / Valkey (`battery2.sh` B, C) | Indicative; disturbed like the above — gaps to Redis are lower bounds |
| `6i-final2/standalone-1w/`, `6i-final2/capped2/` | the gate cells re-measured on a quiet host, 6H / 6I / Redis (`battery3.sh` G, H) | **Numbers of record** for the gate; still the VM, still indicative |
| `6i-final2/mem/` | RSS per key, 1M keys, 6H / 6I / Redis / Valkey (`../6g-linux/memory.sh`) | Use; same method as 6G/6H |
| `6i-final2/inproc/hot-key.csv` | in-process hot-key scaling, 6H vs 6I, 1/2/4 workers | Use for relative cost |
| `6i-final2/perf-*` | perf of INCR new key / existing key and capped SET at 4 workers (`top.txt`; `perf.data` not kept) | Use for shares; VM `cpu-clock` sampling |
| `6i-final2/soak/` | 10-minute sustained topology with failure injection, ledger crash-recovery (`soak.sh`) | Use: correctness; the lag probe covers only the first ~3 min |
