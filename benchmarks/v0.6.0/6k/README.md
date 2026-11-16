# Phase 6K benchmark data

**Indicative only.** Shared 4-core i5-8259U laptop (8 logical CPUs). Docker
Desktop Linux VM (8 vCPU, 4 GiB) for the server runs. The host was under
heavy unrelated load and ~98% disk usage at times; every directory's
`conditions.txt` records the load at start. Within each run, the builds or
code paths being compared were measured in the same interleaved sequence.

| Path | What |
|---|---|
| `qsub-snapshot-micro/` | 6K-A: QSUB snapshot cost, pre-6K full walk (verbatim copy) vs `MatchPlan`, in-process, release. `run1` = prefix plan only (shows the 0.7× regression at 100% match), `run2` = with the quarter-keyspace scan fallback |
| `live-query-routing-micro/` | 6K-B: per-write live-query matching, pre-6K-B loop (verbatim copy) vs `LiveQueries`, 10–10,000 patterns. `run1` = `df703db` and `run2` (names borrowed) were 0.8× at 10 all-matching patterns; `run3` = `7f4b8d4` (subscribers in the index buckets, the shipped design) is ≥1.1× in every cell |
| `h-qsub-churn/` | 6K-A/B end to end: the 6J harness (`../6j/ws-route.sh`), scenario H. Every client QSUBs/QUNSUBs `t<i>:q*` while memtier writes `bg:1..100000` at 12,000/s; strict token mode; 1 worker; 500 and 1,000 clients; 3 interleaved reps |
| `capped-admission-micro/` | 6K-C: capped-store SET throughput, admission by `DashMap::len()` (pre-6K) vs the published key count, 1/2/4/8 threads. +17% at 1 thread, +11% at 2, parity at 4–8 (the capacity gate itself is the limit there) |

Binaries for `h-qsub-churn` (Linux release profile, `../6j/build.sh`,
checksums in its `conditions.txt`):

| Label | Source |
|---|---|
| `6j-dev` | dev `2a4097e` (6J merged) |
| `6k-a` | `c2d80c1` (6K-A) |
| `6k-b` | `df703db` (6K-A + the first 6K-B index; the final bucket layout, `7f4b8d4`, changes how matches are delivered, not which patterns are checked — scenario H's writes match none) |

Reproduce scenario H:

```sh
BINS_DIR=<dir with recached-server-{6j-dev,6k-a,6k-b}> \
  benchmarks/v0.6.0/6j/run.sh <out> SERVERS="6j-dev 6k-a 6k-b" SCENARIOS=H \
  CLIENT_SET="500 1000" WORKER_SET=1 REPS=3 TEST_TIME=10
```
