# v0.6.0 Phase 6O-G: transaction atomicity to outside readers (two-phase EXEC)

Read `V0.6O-G-RELEASE-GATE.md` first.

| Report | What it holds |
|---|---|
| `V0.6O-G-BASELINE.md` | Starting SHA, CI, environment, binaries, the F1 reproduction on the baseline |
| `V0.6O-G-ATOMICITY-IMPLEMENTATION.md` | PR 1: `KeyValueStore::apply_transaction`, the scope, the stripes and marks, the read bypass, store-side containment, deviations |
| `V0.6O-G-TWO-PHASE-EXEC.md` | PR 2: the four stages, the sync/async boundary, stream ordering, the AOF batch and its limits, memory |
| `V0.6O-G-NOTIFICATION-CORRECTNESS.md` | Per-key coalescing: effective writes, the OR rule, full values, delta/QSUB convergence |
| `V0.6O-G-PANIC-AND-RECOVERY.md` | PR 3: fail-closed, run-id rotation, sub-replicas, WebSocket sync-gap, known limits |
| `V0.6O-G-CORRECTNESS-TESTS.md` | Suites, the brief's checklist, mutation results |
| `V0.6O-G-INDEPENDENT-REVIEW.md` | Reviews A, B, C and D, every finding and its resolution |
| `V0.6O-G-PERFORMANCE.md` | Native measurements: commands, transactions, stalls, store-stripe contention, memory, the layout control; review D |
| `V0.6O-G-RELEASE-GATE.md` | Gates G1–G5, the twenty-two questions, decisions, follow-up coordination, scorecard |

## Tools

| Path | Use |
|---|---|
| `tools/txbench` | MULTI/EXEC load generator (6O-E/F) with 6O-G modes (`set-set`, `mset10`, `wide20`, `setn` with `TX_N`, `appendn`, `flushdb`) and roles (`READERS`, `READ_MODE`; writer and reader latency) |
| `tools/g-driver.sh`, `tools/g-driver2.sh`, `tools/g-driver3.sh`, `tools/g-tx.sh` | The native validation drivers (campaign 1; campaign 2 after review D part 1; campaign 3 after part 2) and the transaction-cell runner (generator CPU, foreign CPU, evictions, replica state, compiler/load wait per cell) |
| `tools/summarize_g.py`, `tools/pair_ratio.py` | Per-cell medians and per-repetition paired ratios for every candidate label against the first; paired ratio between any two labels (the layout control) |
| `tools/build-prof.sh`, `tools/build-jcc.sh`, `tools/g-prof.sh`, `tools/g-uprof.sh`, `tools/uprof_ratio.py` | Profiling builds (symbols, frame pointers), layout-control builds (32-byte branch alignment), native sampled profiles and counters of small `EXEC`s, counters per transaction |
| `../6n-c5/tools/symbolize.py`, `tools/prof_lines.py` | Symbolized flat/inclusive profiles (the 6N-C5 tool), and leaf samples of one function by source line (addr2line) |
| `tools/g-mem.py` | 10,000 `APPEND`s to an 8 MiB (`MEM_MIB`) value with a TCP WATCHer and a WebSocket live query; peak RSS |

## Raw data

| Path | Content |
|---|---|
| `raw/g0/` | The F1 probe on `25948a1` (10/10 `[new, old]`) |
| `raw/tests/` | Full suites at `9c8bf04` and `47e6442` (both fan-out modes), the `watch_expiry` flake check |
| `raw/mutation/` | Mutation scripts and logs (store and server) |
| `raw/review/` | The reviewers' probe files, as written; review D's reports |
| `raw/perf/` | Every benchmark cell, binary hashes, host conditions, summaries: campaign 1 (`tx-47e6442`, `cmds`, `cmds3w`), campaign 2 (`c2-tx`, `c2-capped`, `c2-stall`, `c2-cmds`, `c2-mem`, `c2-prof`, `g-driver2.log`), campaign 3 (`c3-uprof`, `c3-layout`, `c3-stall`, `c3-cmds`, `g-driver3.log`), in-process (`inproc`) |
