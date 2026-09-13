# Benchmarks

Recached does not publish a current Redis or Valkey performance table. The older comparisons used different Recached releases and outdated server images; keeping their numbers on a current product page would make them look more conclusive than they are.

The supported claim is narrower: Recached schedules command execution across worker threads. Measure throughput, latency, and memory again for the exact commit, images, host, and workload you plan to deploy.

## Thread-scaling evidence

Measured 2026-09-13 with `redis-benchmark` 8.10.1 via [`scripts/bench-scaling.sh`](https://github.com/recached-sh/recached/blob/main/scripts/bench-scaling.sh). Only `RECACHED_WORKER_THREADS` varies: one binary, one workload, one fixed four-core server CPU set.

### Conditions

| | |
|---|---|
| CPU | Intel i5-9400F, 6 cores, no SMT, `powersave` governor |
| Placement | `PIN=1 SERVER_CPUS=0-3 BENCH_CPUS=4-5` |
| Persistence | `RECACHED_SAVE_INTERVAL=0`, no AOF, no replicas |
| Workload | `-n 1000000 -c 50 -d 64 -r 100000 -P 16` |

The governor is `powersave` and could not be changed on the test host, so absolute figures are conservative. The ratios between columns are unaffected, which is the point of holding everything but the worker count fixed.

### Scaling depends on key distribution

This is the result worth internalising, and it is not visible in an aggregate number.

| Command | 1 thread | 2 threads | 4 threads | 4-thread change |
|---|---:|---:|---:|---:|
| `GET` | 819,672 | 1,689,189 | 1,658,375 | +102% |
| `SET` | 316,857 | 580,720 | 769,823 | +143% |
| `INCR` | 330,688 | 602,047 | 761,615 | +130% |
| **Key-distributed total** | **1,467,217** | **2,871,956** | **3,189,812** | **+117%** |
| `SADD` | 474,608 | 331,455 | 333,778 | −30% |
| `LPUSH` | 397,772 | 282,885 | 291,630 | −27% |
| `HSET` | 345,185 | 248,818 | 258,799 | −25% |
| `ZADD` | 333,000 | 241,196 | 250,564 | −25% |
| **Single-key total** | **1,550,566** | **1,104,355** | **1,134,772** | **−27%** |

The first three tests spread writes over 100,000 keys (`-r 100000`) and scale with worker count. The last four are `redis-benchmark`'s collection tests, which push every operation into **one** key — `mylist`, `myset`, `myhash`, `myzset`. A single key lives on a single shard, so extra workers cannot execute those commands in parallel; they only add contention and cache-line traffic, and throughput drops by about a quarter before flattening.

Neither half is the "real" number. Which one describes your deployment depends entirely on whether your writes are spread across keys or concentrated on a few hot ones. If you have one hot key, adding cores will not help it, and this table shows roughly what it costs.

`GET` also stops improving between two and four threads. At 1.7M requests/s the load generator has only two cores to the server's four, so that plateau is the harness, not the server.

### Latency, unpipelined

`-P 1 -c 16`, where each request is a full round trip. Throughput here is bounded by client concurrency rather than by the server, so read the latency column, not the rate.

| Command | 1 thread | 2 threads | 4 threads |
|---|---:|---:|---:|
| `SET` p50 | 183 µs | 95 µs | 135 µs |
| `GET` p50 | 95 µs | 87 µs | 95 µs |
| `INCR` p50 | 183 µs | 95 µs | 127 µs |

### Browser push latency

The tables above measure the RESP port. The sync path was measured separately, over a real WebSocket, on the same host: a `SET` on the RESP port arriving at a subscribed browser client as a `keychange`.

| p50 | p90 | p99 | max |
|---:|---:|---:|---:|
| 151 µs | 261 µs | 698 µs | 2.5 ms |

`redis-benchmark` cannot measure this path, so it comes from the project's own WebSocket harness. It is the *push* path — the server telling a browser something changed — not what a browser read costs, which is a local WebAssembly memory lookup with no server involved.

### A note on collection sizes

Before 0.3.4, every write to a collection key recomputed that key's memory footprint by walking it, which made building an N-element collection O(N²). `HSET` into a 100k-field hash ran at 2,838 ops/s where `SET` managed 349,650, and halved again with every doubling of the field count. Collections now maintain their size incrementally, and the same test runs at 380,228 ops/s and stays flat as the hash grows.

If you are benchmarking a release before 0.3.4, or comparing against published numbers from one, this is the difference.

## Run the current suite

### Reproducible Linux comparison

Build the current Recached image, then run the Docker harness:

```bash
docker build -t recached-bench:local .
scripts/bench-docker.sh
```

The default run:

- pins each server to `SERVER_CPUS` and the load generator to a disjoint `BENCH_CPUS` set;
- compares pinned Recached, Redis, and Valkey images with persistence disabled;
- records pipelined and unpipelined `redis-benchmark` CSV files;
- reruns the same Recached image with several worker counts; and
- measures process-RSS growth per live key for string, small-hash, and small-set workloads.

Results go to `bench-results/` by default. Keep `conditions.txt` beside the CSV files whenever results are shared.

Use quick mode only as a harness smoke test:

```bash
scripts/bench-docker.sh --quick
```

Useful overrides include:

```bash
SERVER_CPUS=0-7 BENCH_CPUS=8-15 N=500000 MEMORY_KEYS=250000 MEMORY_DATA=256 scripts/bench-docker.sh
```

Keep the CPU sets disjoint. Do not compare unpipelined Docker Desktop results with native-loopback results: virtualization network overhead can dominate one-command-per-round-trip measurements.

### Thread scaling without Docker

Use the same release binary while changing only the worker count:

```bash
cargo build --release --package recached
THREADS="1 2 4 8" scripts/bench-scaling.sh
```

On Linux, isolate the server and load generator when the host has enough cores:

```bash
PIN=1 SERVER_CPUS=0-3 BENCH_CPUS=4-7 scripts/bench-scaling.sh
```

The script verifies the worker count reported by the server. Compare columns within one run; do not combine absolute numbers from different machines.

### A server already running

`scripts/benchmark.sh` measures whichever RESP server is listening at the selected host and port:

```bash
RECACHED_BIND=127.0.0.1 RECACHED_SAVE_INTERVAL=0 ./target/release/recached-server
scripts/benchmark.sh
```

Repeat with identical settings for each server. Record binary versions, configuration, CPU placement, persistence mode, request count, concurrency, value size, key distribution, and pipeline depth.

## Memory results

`memory-per-key.csv` reports the change in container process RSS divided by the actual `DBSIZE`, using a fresh server process for each data shape. It complements Recached's `used_memory` and `recached_memory_bytes`, which are logical key/value counters rather than process RSS.

RSS deltas include allocator behavior and internal indexes, but short runs also contain measurement noise and lazy initialization. Use enough keys to make the delta large relative to the baseline, repeat the run, and report medians. Do not turn one result into a universal “times more memory” claim.

## Interpreting results

- Compare medians across repeated runs, not one best result.
- Separate pipelined throughput from single-request latency.
- Treat a benchmark as evidence only for the tested commands and data shapes.
- Run with production persistence, replication, TLS, and value sizes before capacity planning.
- Use process RSS for host sizing; use Recached's logical memory counter to understand its configured eviction threshold.

Recached's product distinction is the shared server-and-browser engine. The RESP benchmarks above do not measure the browser read path at all: those reads come from local WebAssembly memory and never leave the tab. The only browser-side number here is [push latency](#browser-push-latency), which measures how quickly a server-side write reaches a subscribed client — a different question from how fast that client can read.
