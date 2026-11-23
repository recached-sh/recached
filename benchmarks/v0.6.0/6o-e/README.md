# v0.6.0 Phase 6O-E: transaction read isolation, WATCH correctness

Read `V0.6O-E-RELEASE-GATE.md` first.

| Report | What it holds |
|---|---|
| `V0.6O-E-BASELINE.md` | Starting SHA, CI, toolchain, worktrees, historical numbers |
| `V0.6O-E-TRANSACTION-ROOT-CAUSE.md` | E0: the call graph, every writer class, the exact invalid history, the deterministic reproduction, adjacent findings (F1–F4) |
| `V0.6O-E-TRANSACTION-DESIGN.md` | E1: options A/B/C, the selected boundary (read keys in scope, plus one clock instant), proof obligations O1–O7, performance risk |
| `V0.6O-E-TRANSACTION-FIX.md` | E1: what changed, the tests, the review-driven changes |
| `V0.6O-E-WATCH-EXPIRATION.md` | E2: Redis 7.2's rules, the fix, the differential |
| `V0.6O-E-WATCH-OVERFLOW.md` | E3: TCP WATCH as compare-and-set only, the invalidated flag, the behaviour change |
| `V0.6O-E-CORRECTNESS-TESTS.md` | Tests, reproductions before and after, full suites, mutation results, the differential |
| `V0.6O-E-INDEPENDENT-REVIEW.md` | Both reviewers, every round and finding |
| `V0.6O-E-PERFORMANCE.md` | Native benchmarks: commands, transactions, contention, against Redis |
| `V0.6O-E-RELEASE-GATE.md` | Gates E1–E6, the fifteen questions, remaining issues, scorecard, decisions |

## Tools

| Path | Use |
|---|---|
| `tools/exec_isolation_stress.py PORT MODE SECONDS` | Read-only MULTI/EXEC against MSET or two SETs over TCP; counts illegal states (from 6O-D's tool) |
| `tools/watch_expiry_diff.py NAME=PORT …` | 31 WATCH/expiry cases against several servers, as a table |
| `tools/txbench/` | MULTI/EXEC load generator (std-only Rust; `cargo build --release` inside it). `txbench PORT MODE KEYSET CLIENTS SECS [WRITERS]` |
| `tools/summarize_tx.py tx.log` | Medians and ratios for `tx.sh` results |
| `tx.sh` | Native transaction benchmarks: fresh populated server per cell, interleaved labels, uprof and schedstat. `redis` is a label |
| `e-driver.sh` | The full 6O-E performance run. Stages: `tx`, `primary`, `p16`, `3w`, `aof`. Drives `kit/run-6od.sh` (6O-D's kit) |
| `build.sh` | Release build per label, one target directory each |
| `mutation/mutate-6oe.py WORKTREE SPEC.json LOG FILTER…` | Mutation runner: baseline first, refuses mutants that do not apply or compile. Specs: `e1*.json`, `e23.json`, `e3-review.json`, `e2-final.json`, `e3-m6b.json` |

## Raw data

| Path | Content |
|---|---|
| `raw/e0-reproduction/` | Reproductions on `1f9232f` (10 runs), the stress run, the WATCH differential before the fix, tests run against pre-fix commits |
| `raw/e1-tests-66e6293/` | Full suites at the first E1 commit |
| `raw/final-tests-9d07c23/` | Full suites, both fan-out modes, and the Redis differential at the final code |
| `raw/bench/` | Every benchmark cell, the summaries, `driver.log`, `binaries.sha256` |
| `raw/mutation/` | Every mutation log and spec, including the discarded shared-target run (`e23-INVALID-shared-target.log`) |

The reproduction tests run only with `RECACHED_REPRODUCE_6OE=1` on the investigation commit. The F1 probe runs only with `RECACHED_PROBE_6OE_F1=1`.
