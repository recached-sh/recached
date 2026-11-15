# Phase 6J benchmark data and tooling

Indicative only: Docker Desktop Linux VM (8 vCPU) on a 4-core i5-8259U laptop in use during
the runs. Every run directory has `conditions.txt` (host load, VM, binary checksums), a results
CSV, per-cell JSON (`ws/`), `INFO` snapshots before and after each cell (`info/`) and the
server/client/memtier logs as `logs.tar.gz`.

| Path | What |
|---|---|
| `ws-route.sh`, `ws-route.mjs`, `ws_route_row.py` | Sync fan-out harness, scenarios A–J (see the header of `ws-route.mjs`) |
| `run.sh` | Runs one harness script in `recached-6i-bench:local` with `target/6j/linux-bins` mounted |
| `build.sh` | Builds the shipped release profile for Linux in `recached-6g-rust:local` |
| `noauth-push.sh`, `noauth-check/` | 6J-E F1: does an unauthenticated WebSocket receive pushes? (v0.5.1, 6I, 6J-A leak; 6J-B does not) |
| `baseline-6i-D/` | 6I baseline, scenario D, 0–5,000 clients × 1/2/4/8 workers |
| `routing-D/` | 6I vs 6J-A (routing, before review), scenario D, same matrix |
| `routing-AJ/` | 6I vs 6J-A, scenarios A–J, 100/500 clients × 1/4 workers (E invalid: memtier refused key-minimum 0) |
| `headline-reps/` | 6I vs 6J-B (reviewed), scenario D, 0/100/500/1,000 clients × 1/4 workers, 3 repetitions |
| `routing-EJ-b/` | 6I vs 6J-B, scenarios E, J, H |
| `h-keyspace-1k/` | Scenario H with a 1k-key background keyspace (isolates QSUB snapshot cost) |
| `scan-index-prototype/` | 6J-B: ordered-index alternatives, standalone crate |

Binaries: `6i` = merged 6I (`738a6a6`); `6j-a` = routing; `6j-b` = routing + review fixes.
