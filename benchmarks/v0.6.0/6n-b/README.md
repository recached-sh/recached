# v0.6.0 Phase 6N-B: watermark-gated AOF fan-out (design and review only)

**What this is.** A design and an independent review of how a primary with replication, WebSocket sync, QSUB or WATCH could batch its AOF writes as a standalone primary already does. 6N-1 measured that path at **1.00 `write(2)` per command and 0.45× standalone throughput** ([Test B](../6n-1/V0.6N-1-REPLICATION-AOF-PROFILE.md)).

**What it is not:**
- no production code changed;
- nothing was benchmarked;
- nothing was released.

| | |
|---|---|
| Source studied | `dev` `a90e08d41530b170b8a750768dbcd3423ba2c193` |
| Recommendation | [`V0.6N-B-DESIGN-DECISION.md`](V0.6N-B-DESIGN-DECISION.md) |

## Documents

| # | Document | Contents |
|---|---|---|
| 1 | [`V0.6N-B-CURRENT-ARCHITECTURE.md`](V0.6N-B-CURRENT-ARCHITECTURE.md) | The mutation, persistence and propagation pipeline as built; why replication forces one write per command |
| 2 | [`V0.6N-B-ORDERING-INVARIANTS.md`](V0.6N-B-ORDERING-INVARIANTS.md) | G1–G10 (reconstructed), the brief's A–H, new N1–N12, happens-before diagrams, the model results |
| 3 | [`V0.6N-B-WATERMARK-DESIGN.md`](V0.6N-B-WATERMARK-DESIGN.md) | The preferred design: commit seq, watermarks, staging, release, barriers, bounds |
| 4 | [`V0.6N-B-ALTERNATIVES.md`](V0.6N-B-ALTERNATIVES.md) | Options A–D compared, and why A (with D as a first step) was selected |
| 5 | [`V0.6N-B-FAILURE-MODE-ANALYSIS.md`](V0.6N-B-FAILURE-MODE-ANALYSIS.md) | The ten scenarios, crash points, backpressure, shutdown |
| 6 | [`V0.6N-B-INDEPENDENT-SECURITY-REVIEW.md`](V0.6N-B-INDEPENDENT-SECURITY-REVIEW.md) | Findings, counterexamples, resolutions, reviewer verdict |
| 7 | [`V0.6N-B-IMPLEMENTATION-PLAN.md`](V0.6N-B-IMPLEMENTATION-PLAN.md) | Future PRs 0–7, dependencies, tests, rollback |
| 8 | [`V0.6N-B-VALIDATION-PLAN.md`](V0.6N-B-VALIDATION-PLAN.md) | Deterministic tests, fault matrix, performance cases, decision rules |
| 9 | [`V0.6N-B-DESIGN-DECISION.md`](V0.6N-B-DESIGN-DECISION.md) | The recommendation and its conditions |

## Model

[`model/ordering_model.py`](model/ordering_model.py) is an exhaustive interleaving model of the core ordering claims. It needs only Python 3 and no dependencies.

```sh
python3 -I benchmarks/v0.6.0/6n-b/model/ordering_model.py
```

It exits 0 when every variant behaves as recorded in [`model/ordering_model.out`](model/ordering_model.out):
- today's inline design is clean;
- each partial fix leaves a violation;
- the full design is clean.
