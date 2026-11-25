# v0.6.0 Phase 6O-F: expiry through replay, transaction atomicity design, stripe study

Read `V0.6O-F-RELEASE-GATE.md` first.

| Report | What it holds |
|---|---|
| `V0.6O-F-BASELINE.md` | Starting SHA, CI, worktrees, binaries, historical numbers |
| `V0.6O-F-ROOT-CAUSE.md` | F0: expiring-counter resurrection (and the wider class), transaction partial visibility, the restart blockers found on the way |
| `V0.6O-F-TTL-DESIGN.md` | F1: why the counter-frame fix was rejected; the recommended design (logged expirations, timeless application); compatibility; approvals needed |
| `V0.6O-F-TTL-FIX.md` | F1, as shipped in #172: the deadline bound, the legacy clamp, the tests, limitations, rollback |
| `V0.6O-F-TRANSACTION-ATOMICITY-DESIGN.md` | F2: options A–E, Option E (two-phase EXEC) revision 5, proof obligations, AOF/replica implications, tests, milestones (first merged in #173; this copy carries the F3 review's correction) |
| `V0.6O-F-STRIPE-SCALING.md` | F3 (in #174): two layers, transactions, barriers, capped stores, ordinary commands, memory, per-key ordering, decision gate |
| `V0.6O-F-CORRECTNESS-TESTS.md` | Full suites, expiry tests, the F1 probe, mutation results |
| `V0.6O-F-INDEPENDENT-REVIEW.md` | All four reviews, every round and finding |
| `V0.6O-F-PERFORMANCE.md` | #172 against `410235b`; the stripe-study summary; gate F4 |
| `V0.6O-F-RELEASE-GATE.md` | Gates, the nineteen questions, decisions requested, what remains, scorecard |

## Tools

| Path | Use |
|---|---|
| `counter_restart_diff.py NAME=BIN[,REPLAY_BIN] …` | The expiring-counter timeline end to end: AOF `always`, SIGKILL, restart after an 800 ms deadline, against Redis (`NAME=redis`) and Recached builds. A second binary replays the first one's files (upgrade/downgrade) |
| `tools/summarize_stripes.py tx.log [BASE]` | Stripe-study summaries (in #174, with `tools/txbench`, `stripe-experiment/`) |

## Raw data

| Path | Content |
|---|---|
| `raw/f0-reproduction/` | Counter tests on `410235b` (10/15 failing, 5 runs), the Redis comparison, the F1 probe (10/10 `[new, old]`) |
| `raw/f1-tests/` | Full suites at `efa8d35` (both fan-out modes), the Redis differential, the gated reproductions, cross-version and upgrade/downgrade runs |
| `raw/f1-perf/` | #172 against `410235b` (6O-D kit, 5 reps), binary hashes |
| `raw/review/` | The reviewers' probe files |
| `raw/stripes/` | (#174) every stripe-study cell, summaries, hashes, driver logs |
