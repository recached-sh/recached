# 6L dedicated-Linux run

`run.sh` builds Recached at the 6K baseline and at the 6L branch, plus Redis 7.2.5 and Valkey 9.1.0, all from source. It then runs, pinned (server and load generator on disjoint CPUs):

| Phase | What |
|---|---|
| A | in-process capped writes, gated vs reserved, 1/2/4/8 threads |
| B | server capped `SET`/`INCR` at 1/2/4/8 workers: 6k, 6l (reservations off), 6l (on), interleaved |
| C | standard battery, one worker: 6k, 6l, Redis, Valkey (throughput, p50/p95/p99, server CPU-seconds) |
| D | as C with AOF everysec |
| E | RSS per 32-byte key at 1M keys |

```sh
curl -fsSLO https://raw.githubusercontent.com/recached-sh/recached/perf/v0.6l-capped-reservations/benchmarks/v0.6.0/6l/linux/run.sh
bash run.sh                 # or: PHASES="A B" REPS=3 bash run.sh
```

It writes `~/recached-6l-bench/recached-6l-linux-<host>-<time>.tar.gz`, which holds:
- the raw CSVs
- server logs
- `conditions.txt`: CPU, governor, boost, load, virtualization
- `binaries.txt`: commits, versions and SHA-256

The prerequisites are in the script's header. Nothing is installed system-wide.

**Not covered here:**
- replicated `SET`, QSUB snapshots and WebSocket sync propagation
- AOF `always`
- the 30–60 minute soak

Those need the soak harness from 6J/6K.
