# v0.6.0 Phase 6N-C5: replication fan-out efficiency

This phase measured what a connected replica costs the primary, removed most of that cost, and investigated the 4-worker tail latency.

| Result | |
|---|---|
| R1 (replication fan-out) | **PASS.** −0.68 µs/op at P=1 and −0.82 at P=16; throughput +13% and +35%. |
| R2 (4-worker p99) | **Not met** with 4 workers on 2 CPUs. The cause is worker oversubscription. With 2 workers on 2 CPUs, `coalesced` p99 is 0.551 ms, the best of any cell. |
| M1 (P=1) | **PASS:** 0.921 |
| M1 (P=16) | **FAIL:** 0.612. The per-command AOF wait remains. |
| Option A | Assessed, **not implemented, needs approval** |
| v0.6.0 | **NOT READY** |

Start with the [release gate](V0.6N-C5-RELEASE-GATE.md).

| Report | |
|---|---|
| [V0.6N-C5-REPLICATION-PROFILE.md](V0.6N-C5-REPLICATION-PROFILE.md) | What a replica costs, measured with user-space counters and LBR call stacks |
| [V0.6N-C5-REPLICATION-OPTIMIZATION.md](V0.6N-C5-REPLICATION-OPTIMIZATION.md) | The sender's held flush (#141): mechanism, guarantees, memory, rollback |
| [V0.6N-C5-MULTIWORKER-LATENCY.md](V0.6N-C5-MULTIWORKER-LATENCY.md) | F1 and F2 ablation, the oversubscription finding, review of #142 |
| [V0.6N-C5-CORRECTNESS-TESTS.md](V0.6N-C5-CORRECTNESS-TESTS.md) | New tests, mutants, the brief's 15 scenarios, suites |
| [V0.6N-C5-INDEPENDENT-REVIEW.md](V0.6N-C5-INDEPENDENT-REVIEW.md) | Three review rounds of #141, and the recorded R1 exposure |
| [V0.6N-C5-PERFORMANCE.md](V0.6N-C5-PERFORMANCE.md) | Final validation of `dev` `4328153` against the 6N-C4 binary, Redis and Valkey |
| [V0.6N-C5-RELEASE-GATE.md](V0.6N-C5-RELEASE-GATE.md) | Verdicts, the ten questions, the Option A assessment, blockers, next milestone |

| Tooling | |
|---|---|
| [`run-c5.sh`](run-c5.sh), [`summarize-c5.py`](summarize-c5.py) | The kit, derived from `../6n-c4/run-m1.sh`. It runs several binaries in one interleaved run (`RC_BINS`, `LABEL_ENV_<label>`), records the user/system CPU split, `uprof` counters and samples, replication socket segments, and replica CPU. |
| [`tools/uprof.c`](tools/uprof.c), [`tools/symbolize.py`](tools/symbolize.py) | User-space `perf_event_open` counters and LBR call-stack sampling. It needs no `perf` and no privileges. |
| [`mutation/`](mutation/) | #141's mutation script and its results |

| Raw data (`fedora-i5-9400f-20261120-…`) | |
|---|---|
| `profile`, `samples` | 6N-C5A profile of `c61b920` |
| `w4-ablation`, `w4-diag` | F1/F2 ablation and the oversubscription and fsync diagnostic |
| `queue-peak` | Replica queue peak, old sender against new |
| `final-main`, `final-w4`, `final-w2` | Final validation of `4328153` |

**Reproduce:** run natively on an idle host:

`RC_BINS="base=<bin> final=<bin>" UPROF=<uprof> CELLS="base/rc-coalesced-P1 final/rc-coalesced-P1 redis-P1 …" bash run-c5.sh`

Cell names are `<label>/<kind>-P<n>` or `<label>/<kind>-w<n>`. Redis and Valkey cells take no label.
