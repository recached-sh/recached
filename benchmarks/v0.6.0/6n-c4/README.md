# v0.6.0 Phase 6N-C4: adaptive AOF coalescing correction

This phase corrects the coalesced fan-out heuristic (PR #136, `dev` `625f88a`), profiles the replicated primary's CPU without `perf`, and revalidates Gate M1 on native Fedora.

**Result:**
- **M1 PARTIAL:** coalesced ÷ standalone is 0.805 at P=1 and 0.460 at P=16, against a threshold of 0.85.
- The P=16 regression is fixed: `coalesced` is now 1.14× `inline`, where 6N-C measured 0.69×.
- The P=1 gap is replication fan-out, not the AOF.
- Option A is **not** recommended as the next step and was not implemented.

Start with the [release gate](V0.6N-C4-RELEASE-GATE.md).

| File | |
|---|---|
| [V0.6N-C4-ROOT-CAUSE.md](V0.6N-C4-ROOT-CAUSE.md) | Why P=16 regressed: two defects, and the ablation that separates them |
| [V0.6N-C4-IMPLEMENTATION.md](V0.6N-C4-IMPLEMENTATION.md) | The change |
| [V0.6N-C4-CORRECTNESS-TESTS.md](V0.6N-C4-CORRECTNESS-TESTS.md) | Tests and mutations |
| [V0.6N-C4-INDEPENDENT-REVIEW.md](V0.6N-C4-INDEPENDENT-REVIEW.md) | Review findings |
| [V0.6N-C4-CPU-PROFILING.md](V0.6N-C4-CPU-PROFILING.md) | CPU attribution and ranked bottlenecks |
| [V0.6N-C4-M1-PERFORMANCE.md](V0.6N-C4-M1-PERFORMANCE.md) | The M1 measurement |
| [V0.6N-C4-RELEASE-GATE.md](V0.6N-C4-RELEASE-GATE.md) | Status, the Option A assessment, and the next milestone |
| `run-m1.sh`, `summarize-c4.py` | The kit, derived from `../6n-c/run-m1.sh`. It adds separate P=1/P=16 request counts, wall-clock and generator CPU, `/proc` counters, and ablation cells. |
| `fedora-i5-9400f-20261119-*` | Raw data: `m1`, `profile`, `ablation` (root cause), `ceiling` (generator probe) |
| `mutation/` | The mutation script and its output |

**Reproduce:** run `RC_SHA=<sha> RC_BIN=<binary> RC_SHA256=<sha256> bash run-m1.sh` natively on an idle host. Profiling cells: `CELLS="rc-noaof-P1 rc-repl-noaof-P1 rc-coalesced-norep-P1 …"`.
