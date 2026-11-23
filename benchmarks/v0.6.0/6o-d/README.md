# v0.6.0 Phase 6O-D: MSET atomicity

Start with the release gate.

| Report | What |
|---|---|
| [V0.6O-D-RELEASE-GATE.md](V0.6O-D-RELEASE-GATE.md) | Verdict (**APPROVAL REQUIRED**), gates D1–D5, the 17 questions, scorecard, decisions |
| [V0.6O-D-BASELINE.md](V0.6O-D-BASELINE.md) | Starting state, versions, provenance |
| [V0.6O-D-ATOMICITY-ROOT-CAUSE.md](V0.6O-D-ATOMICITY-ROOT-CAUSE.md) | Deterministic reproduction, the exact interleaving, call graph, affected paths |
| [V0.6O-D-ATOMICITY-DESIGN.md](V0.6O-D-ATOMICITY-DESIGN.md) | Alternatives, the selected protocol, its correctness argument and lock order |
| [V0.6O-D-ATOMICITY-FIX.md](V0.6O-D-ATOMICITY-FIX.md) | The implementation, changed locking behaviour, read-side cost |
| [V0.6O-D-MSET-A.md](V0.6O-D-MSET-A.md), [V0.6O-D-MSET-B.md](V0.6O-D-MSET-B.md) | **NOT IMPLEMENTED — APPROVAL PENDING** |
| [V0.6O-D-CORRECTNESS-TESTS.md](V0.6O-D-CORRECTNESS-TESTS.md) | Deterministic interleavings, load, suites, Redis differential, mutation testing |
| [V0.6O-D-INDEPENDENT-REVIEW.md](V0.6O-D-INDEPENDENT-REVIEW.md) | Three review rounds and their resolutions |
| [V0.6O-D-PERFORMANCE.md](V0.6O-D-PERFORMANCE.md) | Native results by stage, GET/MGET controls, mixed contention, AOF |

The other files:
- **Data:** `fedora-i5-9400f-20261123-d1/` holds the raw native results, with SHA256SUMS. `raw/` holds the reproduction output, audit runs, suite logs and mutation summaries.
- **Kit:** `run-6od.sh` (the 6O-C kit plus `mget`), `summarize-6ob.py`, `d1.sh`, `mixed.sh` and `build.sh`.
- **Tools:** `tools/` holds the EXEC-isolation and WATCH-expiry probes. `mutation/mutate-6od.py` is the mutation runner.

Run the kit natively (`flatpak-spawn --host bash d1.sh` from a sandbox). The binaries were built by `build.sh` from `69f0d1c` (`rc-base`) and `1965943` (`rc-fix`).
