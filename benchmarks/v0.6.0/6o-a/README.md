# v0.6.0 Phase 6O-A: standalone AOF P=1 CPU

Verdict and scorecard: [V0.6O-A-RELEASE-GATE.md](V0.6O-A-RELEASE-GATE.md) (**PARTIAL**: SET P=1 0.81× → 0.86× standalone Redis; target 0.90×).

| Report | Content |
|---|---|
| [V0.6O-A-BASELINE.md](V0.6O-A-BASELINE.md) | The corrected comparison: standalone Recached vs standalone Redis/Valkey, and the AOF policy ablation |
| [V0.6O-A-CPU-PROFILE.md](V0.6O-A-CPU-PROFILE.md) | The SET-with-AOF path from the source; counters, LBR samples and allocations; ranked bottlenecks; the gate investigation |
| [V0.6O-A-OPTIMIZATION-DESIGN.md](V0.6O-A-OPTIMIZATION-DESIGN.md) | Candidates, trade-offs, the chosen encoder |
| [V0.6O-A-IMPLEMENTATION.md](V0.6O-A-IMPLEMENTATION.md) | PR 2's changes and allocations per command |
| [V0.6O-A-CORRECTNESS-TESTS.md](V0.6O-A-CORRECTNESS-TESTS.md) | Byte parity, the baseline fixture, the suites, mutation testing |
| [V0.6O-A-INDEPENDENT-REVIEW.md](V0.6O-A-INDEPENDENT-REVIEW.md) | Findings and resolutions |
| [V0.6O-A-PERFORMANCE.md](V0.6O-A-PERFORMANCE.md) | Before and after on the native host, against Redis and Valkey |

| Kit and data | |
|---|---|
| [`run-6oa.sh`](run-6oa.sh), [`summarize-6oa.py`](summarize-6oa.py) | The kit. Derived from `../6n-c5/run-c5.sh`, with no replication anywhere: every Redis/Valkey cell is a single server. It adds the SET/INCR/HSET workloads, the AOF policy kinds, per-cell host pressure, an IO-settle wait and alternating order |
| [`build.sh`](build.sh), [`a0.sh`](a0.sh), [`a1.sh`](a1.sh), [`v.sh`](v.sh) | Builds (release + profiling) and the drivers that wait for a quiet host |
| `fedora-i5-9400f-20261121-a0/`, `-a1-ablation/`, `-a1-samples/` | Baseline, ablation, profile samples (`a697f54`) |
| `fedora-i5-9400f-20261121-v1/` … `-v4-samples/` | Validation: base vs PR 2 vs Redis/Valkey |
| [`binaries.sha256`](binaries.sha256), [`request-path-allocs.txt`](request-path-allocs.txt), [`suites/`](suites/), [`mutation/`](mutation/) | Provenance, allocations per command, test summaries, mutants |

Reproduce, on a quiet native Linux host:

```sh
RC_BINS="base=<rc-base> opt=<rc-opt>" RC_SHAS="base=<sha> opt=<sha>" ALTERNATE=1 REPS=5 \
CELLS="base/rc-everysec-set-P1 opt/rc-everysec-set-P1 redis-set-P1 valkey-set-P1" \
  bash benchmarks/v0.6.0/6o-a/run-6oa.sh
```

Profiling uses `../6n-c5/tools/uprof.c` (`UPROF=<binary>`, `RECORD_CELLS=…`) and `../6n-c5/tools/symbolize.py` with [`focus-6oa.args`](fedora-i5-9400f-20261121-a1-samples/focus-6oa.args).
