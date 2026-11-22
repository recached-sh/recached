# v0.6.0 Phase 6O-C: performance-gate closure

Verdict and scorecard: [V0.6O-C-RELEASE-GATE.md](V0.6O-C-RELEASE-GATE.md).

**PARTIAL, with design decisions required.**
- SET (0.923×) and LPUSH (0.929×) meet 0.90× Redis in every repetition.
- SADD improved to 0.894×, not met.
- MSET (0.819×) awaits approval of its design.
- The P=16 tail regressions and the one-client CPU overhead are fixed.
- The pre-existing WATCH overflow High is fixed in #157.

Merged into `dev`: #157, #158, #159. Open: #160 (C3), #163 (follow-ups), #162 (the MSET design).

| Report | Content |
|---|---|
| [V0.6O-C-BASELINE.md](V0.6O-C-BASELINE.md) | Starting point, binaries, host, kit |
| [V0.6O-C-REPLY-COALESCING.md](V0.6O-C-REPLY-COALESCING.md) | C1 (#158): lone-command yields, doubling back-off, migration neutral; tail and CPU results, decision counters, the 1-worker P≥4 trade-off |
| [V0.6O-C-BORROWED-COMMANDS.md](V0.6O-C-BORROWED-COMMANDS.md) | C2 (#159): LPUSH, RPUSH, SADD borrowed; results; SADD's 6P remainder |
| [V0.6O-C-TCP-SPLIT-PROTOTYPE.md](V0.6O-C-TCP-SPLIT-PROTOTYPE.md) | C3 (#160): the `into_split` experiment, its measurement, the early-FIN defect, adoption |
| [V0.6O-C-MSET-DESIGN.md](V0.6O-C-MSET-DESIGN.md) | C4: the one-lookup MSET design and its review. **Not implemented: awaiting approval** |
| [V0.6O-C-CORRECTNESS-TESTS.md](V0.6O-C-CORRECTNESS-TESTS.md) | Suites on the combined tree in both fan-out modes, new tests, 30/30 mutants |
| [V0.6O-C-INDEPENDENT-REVIEW.md](V0.6O-C-INDEPENDENT-REVIEW.md) | Every review round and its resolution |
| [V0.6O-C-PERFORMANCE.md](V0.6O-C-PERFORMANCE.md) | The final validation: primary gate (5 repetitions), controls, three workers, AOF carryover, uncertainty |
| [V0.6O-C-RELEASE-GATE.md](V0.6O-C-RELEASE-GATE.md) | Gates C1–C6, the fourteen questions, scorecard, decisions, follow-ups |

| Kit and tools | |
|---|---|
| [`run-6oc.sh`](run-6oc.sh), [`summarize-6ob.py`](summarize-6ob.py) | `../6o-b/run-6ob.sh` plus `rpush` and the diagnostic `reply_coalesce_diag` INFO line; 6O-B's summarizer |
| [`tools/diag-patch.py`](tools/diag-patch.py) | Adds process-wide reply-coalescing counters to a diagnostic build. Never merged; diagnostic cells only |
| [`tools/`](tools/) | 6O-B's syscall shim, TCP cost probe, rusage and decomposition tools, unchanged |
| [`build.sh`](build.sh) | Release build of a bench source, one Cargo target directory per build |
| [`c1.sh`](c1.sh), [`v1.sh`](v1.sh), [`v2.sh`](v2.sh) | Drivers: C1's first commit; validation run 1 (C1, C2, C3 separately); the final validation |
| [`suites.sh`](suites.sh) | CI's commands, locally, in both fan-out modes |
| [`mutation/`](mutation/) | `mutate-6oc.py` (groups c1, c2, watch) and its results |

Reproduce on a quiet native Linux host (server on CPU 0, redis-benchmark on CPUs 3–5):

```sh
RC_BINS="base=<rc-base> final=<rc-final>" RC_SHAS="base=<sha> final=<sha>" ALTERNATE=1 REPS=5 \
REQ_C50_P1=6000000 CELLS="base/rc-noaof-lpush-c50-P1 final/rc-noaof-lpush-c50-P1 redis-noaof-lpush-c50-P1" \
  bash benchmarks/v0.6.0/6o-c/run-6oc.sh
```

| Raw data (`fedora-i5-9400f-20261122-*`) | |
|---|---|
| `c1a/` | C1's first commit (`a63c024`): 1 and 3 workers |
| `v1/` | Validation run 1: `c1v2-*` (C1 final, and its decision counters), `c2`, `c3-1w`, `c3-3w` |
| `final/` | The combined build (`3cc620a`): `final-primary`, `final-secondary`, `final-3w`, `final-aof`; the drivers' log; the suite summaries |

Per-cell `server.log` files are excluded by the repository's `.gitignore`. Every number above is in each run's `results.txt`, `summary.md` and per-cell `bench.csv`.
