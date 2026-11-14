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
