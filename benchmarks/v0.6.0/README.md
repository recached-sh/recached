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
