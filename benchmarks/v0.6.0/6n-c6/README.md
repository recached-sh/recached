# v0.6.0 Phase 6N-C6: replica queue and backpressure, ACK safety

| Result | |
|---|---|
| Healthy-replica overflow (6N-C5 R1) | **Fixed.** The replica queue is sized by bytes (approved Option B). On the real server at 100 clients × P=64, `dev` disconnects its healthy replica 3–5 times per run and the fix never does. |
| Slow replicas | Still dropped, now by bytes, and the connection is closed at once (review M1) |
| ACK read | **Fixed:** cancel-safe, no protocol change |
| Defaults | One approved change: the derived frame cap (65,536, was 4096) and a 128 B per-frame charge |
| Performance | No material regression: CPU per operation within ±1.5% |
| Verdict | **PASS WITH CONDITIONS** |

Start with the [release gate](V0.6N-C6-RELEASE-GATE.md).

| Report | |
|---|---|
| [ROOT-CAUSE](V0.6N-C6-ROOT-CAUSE.md) | Reproduction, trace, tokio scheduling mechanism, resource limits *(PR #146)* |
| [BACKPRESSURE-DESIGN](V0.6N-C6-BACKPRESSURE-DESIGN.md) | Options compared, memory bounds, the decision *(PR #146)* |
| [IMPLEMENTATION](V0.6N-C6-IMPLEMENTATION.md) | What #144 changed, accounting, rollback |
| [ACK-SAFETY](V0.6N-C6-ACK-SAFETY.md) | #145: the defect, the fix, tests and mutants |
| [CORRECTNESS-TESTS](V0.6N-C6-CORRECTNESS-TESTS.md) | Scenarios A–I, mutation testing, suites |
| [INDEPENDENT-REVIEW](V0.6N-C6-INDEPENDENT-REVIEW.md) | Two reviews, findings and resolutions |
| [PERFORMANCE](V0.6N-C6-PERFORMANCE.md) | Native Fedora validation, order effect, burst, slow and two-replica cells |
| [RELEASE-GATE](V0.6N-C6-RELEASE-GATE.md) | Verdict, the twelve questions, blockers |

| Tooling | |
|---|---|
| `run-all.sh`, `run-reversed.sh` | How the runs below were driven: `../6n-c5/run-c5.sh`, unchanged, then the extra cells |
| `run-c6-extra.sh` | `MODE=slow` (a replica stopped for 2 s), `MODE=two` (two replicas), `MODE=burst` (100 × P=64 without AOF) |
| `mutation/` | Mutation scripts and results for #144 and #145 |
| `suites/` | Full-suite result lines |
| `../6l/soak/repl_check.py` | Fixed: the lag probe survives a primary stall longer than its socket timeout |

| Raw data (`fedora-i5-9400f-20261120-…`) | |
|---|---|
| `main` | The 18-cell matrix, base first, 3 reps, with `host-pressure.txt` |
| `reversed` | The four AOF cells with c6 first |
| `burst`, `slow`, `two` | The extra cells |
| `aborted-probe` | The first attempt, stopped by the probe crash |
