# v0.6.0 Phase 6O-H: logged expirations, timeless AOF replay, replication correctness

Implements `6o-f/V0.6O-F-TTL-DESIGN.md`, Expiry Option E. **Verdict: PASS WITH CONDITIONS** (`V0.6O-H-RELEASE-GATE.md`).

| Report | Contents |
|---|---|
| `V0.6O-H-BASELINE.md` | `f9a364d`, its binaries, the defects reproduced, the Redis differential on the base |
| `V0.6O-H-PROTOCOL-DESIGN.md` | The AOF marker and authorship rules, replay, file boundaries, replication capability (PR 1) |
| `V0.6O-H-EXPIRATION-CAPTURE.md` | Store-level capture of observed expirations (#183) |
| `V0.6O-H-EXPIRATION-PROPAGATION.md` | `[DEL][frame]`, the sweeper's batches, DEDUP, sync clients (#184, #185) |
| `V0.6O-H-TRANSACTION-INTEGRATION.md` | Two-phase EXEC, notifications, WATCH and delta clients |
| `V0.6O-H-AOF-REPLAY.md` | Timeless replay, legacy and mixed files, checkpoints (#186, #188) |
| `V0.6O-H-REPLICATION-COMPATIBILITY.md` | `caps=expirylog1`, mixed versions, the full-sync checkpoint (#187, #189, #190) |
| `V0.6O-H-CORRECTNESS-TESTS.md` | Tests, the Redis differentials, mutants, fault injection, suites |
| `V0.6O-H-INDEPENDENT-REVIEW.md` | Reviews A–E and their resolutions |
| `V0.6O-H-PERFORMANCE.md` | Native measurements: request path, AOF, `always`, Redis, replication, replay, sweeper, stalled replicas, review E's fixes |
| `V0.6O-H-RELEASE-GATE.md` | Acceptance gates H1–H6, conditions, the 25-question assessment, the scorecard, the next milestone |

| Raw | Contents |
|---|---|
| `raw/h0-marker-compat/` | `tools/marker_compat.py` run against v0.4.0, v0.5.1, v0.5.2, `410235b`, `f9a364d` |
| `raw/h0-baseline/` | The 6O-F reproductions (`RECACHED_REPRODUCE_6OF_COUNTER=1`) on `f9a364d`: 8 fail, 3 runs of 3 |
| `raw/h0-review-c/` | Review C's own reproductions (replica AOF after a full sync, shutdown race, ephemeral seed and discard, array records, fake primary handshakes) |
| `raw/diff/` | `tools/expiry_restart_diff.py` and `tools/replica_diff.py` against Redis 7.2.5 |
| `raw/mutation/` | Mutant definitions and results per PR |
| `raw/suites/` | Suite logs of the work in progress |
| `raw/perf/` | Every native cell: `results-{cmds,aof,always,redis}` (request path), `results-repl` (replay, sweep, replication), `results-lag-*` (stalled replicas: `-logs` and `-fixe` on tmpfs or early builds; `-hdd`, `-hdd2` on the HDD), `results-always2` (review E's F1), `results-native3` (the legacy-replay A/B), and `kit/` |

Process-level logs are stored as `.txt` (`*.log` is ignored by the repository).
