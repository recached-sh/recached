# 6L-D replication CPU, in process

`bench_replicated_set_cpu` in `server-native/src/tests/repl_cpu_bench.rs` drives the real write path, `execute_ordered_write`:
- 1/4/8 writer tasks on a 4-worker runtime
- 400k `SET`s of 32-byte values over 10k keys per writer
- a primary with two in-process replicas, each drained
- optionally 8 WebSocket subscribers routed everything, each building the push's message as a connection does (`push_message`)
- the Prometheus recorder installed, as in production

| Build | Commit |
|---|---|
| `base` | harness only (`27b6704`) |
| `r1` | + one synchronous lock for backlog and replica queues (`39ed80f`) |
| `r2` | + one shared frame per write and one WebSocket message per push (`214b916`), **reverted** |
| `r1p` | R1 + the backlog gauge set outside the lock, R2 reverted (`4f95796`), **the kept code** |

```sh
# build each commit's release test binary into a directory as bench-<build>, then:
benchmarks/v0.6.0/6l/replication-cpu/run.sh <bin-dir> <out-dir> 5
```

`run.sh` interleaves the builds within each repetition. `summarize.py` reports the median and spread.

CPU is the whole process (writers, runtime, drains), sampled with `ps` on macOS (10 ms resolution). It is not the server's CPU over a socket.

Results from the 4-core laptop are **indicative only**. Load average was 6–14 throughout because another build shared the host; interleaving spreads that noise across builds.

| Directory | Builds compared | Outcome |
|---|---|---|
| `macos-i5-8259u/` | base, r1, r2 | R2 no better than R1, so R2 dropped |
| `macos-i5-8259u-r1p/` | base, r1, r1p | r1p kept: e.g. 4 writers +62% ops/s and −24% CPU/op with no browser; +87% / −38% with 8 browsers |
