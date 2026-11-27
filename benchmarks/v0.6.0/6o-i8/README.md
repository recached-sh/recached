# v0.6.0 Phase 6O-I8: final performance investigation and release-risk closure

Phase 6O-I (transaction framing) left two measured but unexplained costs in its decision D6: 10,000-write transactions beside outside writers, and a replica with its own AOF under overload. This phase measures and explains both, without changing production code.

| Report | |
|---|---|
| `V0.6O-I8-BASELINE.md` | the three builds (H `6068337`, I `0a1f95e`, P `54ff38a`), reproducible bit for bit; the host; profiling tools |
| `V0.6O-I8-BENCHMARK-METHODOLOGY.md` | the driver, the generator, validity and comparability, repetition, driver versions, limits |
| `V0.6O-I8-LARGE-TRANSACTION-CONTENTION.md` | Investigation 1: CPU, transactions, outside writers, locks |
| `V0.6O-I8-MEMORY-INVESTIGATION.md` | Investigation 1: the RSS |
| `V0.6O-I8-REPLICA-SUSTAINABLE-THROUGHPUT.md` | Investigation 2: the sustainable rate of a replica with its own AOF |
| `V0.6O-I8-REPLICA-RESYNC-ROOT-CAUSE.md` | Investigation 2: why 6O-I resynchronized more under overload |
| `V0.6O-I8-CORRECTNESS-VALIDATION.md` | CI, local suites, the benchmarks' own checks, the CI flake |
| `V0.6O-I8-INDEPENDENT-REVIEW.md` | reviews A–D and what was done about every finding |
| `V0.6O-I8-RELEASE-GATE.md` | gates I8-A to I8-E, decisions, the PR 3 proposal, the engineering assessment |

| Directory | |
|---|---|
| `tools/` | `i8bench` (generator and verifier), `i8run.py` (driver), summarizers, chart and timeline tools, build and test scripts |
| `charts/` | RSS time series |
| `raw/<campaign>/` | per campaign: its config, driver log, every run's record (`runs.jsonl`), `summary.md`/`.json`, and per run the generator's output, the 1 Hz series (gzipped), both servers' logs, perf output; `MANIFEST-not-copied.txt` lists the large files left out, with their SHA-256 |
| `raw/builds/`, `raw/cfg/`, `raw/diag-patch/`, `raw/px-patch/`, `raw/methodology/`, `raw/reviews/`, `raw/tests/` | build logs, campaign configs and queue, the temporary instrumentation (never merged), the experimental px patch (not proposed without approval), the verifier's negative check, the reviews, the correctness runs |

Logs are stored as `.txt` (`*.log` is ignored by the repository).
