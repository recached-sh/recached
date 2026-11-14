# Benchmarks

Recached publishes no cross-project performance comparison. Everything below measures Recached alone, and says nothing about how any other cache performs on this or any other host.

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

### Thread scaling

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

## Memory

Recached reports a logical key/value estimate through `INFO used_memory` and the `recached_memory_bytes` metric. That is not process RSS: it excludes allocator overhead, internal indexes and fragmentation, so it is the right number for reasoning about a configured eviction threshold and the wrong one for sizing a host.

For host sizing, measure RSS of the server process against a known `DBSIZE`, using a fresh process per data shape. Use enough keys that the delta dwarfs the baseline, repeat the run, and report medians — short runs contain lazy initialisation and allocator noise. Do not turn one result into a universal "times more memory" claim.

### Memory per key, v0.6

v0.6 removed most of Recached's per-key overhead. Every key had been copied into four separate
`String`s (six with a TTL) — the map's own plus three secondary indexes — and each entry was 72
bytes. Now the indexes share one allocation per key, the index used only by `allkeys-*` eviction
exists only under those policies, overwrites skip the index entirely, and an entry is 56 bytes.

Measured with [`scripts/bench-redis-valkey.sh`](https://github.com/recached-sh/recached/blob/main/scripts/bench-redis-valkey.sh)
(`SKIP_THROUGHPUT=1`), fresh process per data shape, median of three runs; RSS growth divided by
`DBSIZE`. One host: Intel i5-8259U, 8 GB, macOS 15.8, jemalloc defaults.

| Shape | v0.5.1 | v0.6 |
|---|---:|---:|
| 1M keys, 32-byte values | 532 B/key | 373 B/key |
| 1M keys with a TTL, 32-byte values | 637 B/key | 427 B/key |
| 100k keys, 1 KiB values | 1,398 B/key | 1,255 B/key |

RSS left behind after `FLUSHDB` of 100k 1 KiB keys fell from 99% to 38% of what loading had added.
These are Recached-only numbers on one host; re-measure on yours.

The same change measured as **live heap**, exactly, with a counting allocator linked to the engine
and every command parsed from RESP bytes as the server does
([`benchmarks/v0.6.0/hash-memory-20261114`](https://github.com/recached-sh/recached/tree/main/benchmarks/v0.6.0/hash-memory-20261114)).
This excludes allocator size classes and fragmentation, so it sits below RSS:

| 1M keys, 32-byte values | v0.5.1 | v0.6 |
|---|---:|---:|
| strings | 434.8 B/key | 282.2 B/key |
| strings with a TTL | 557.1 B/key | 351.4 B/key |
| hashes of 3 fields, after one overwrite | 1,192.8 B/key | 732.2 B/key |

The hash row includes a fix in 0.6: through 0.5.1, the first overwrite of a field in a hash whose
field table was exactly full (3, 7, 14 … fields) doubled that table, adding 260 bytes per 3-field
hash. Hash RSS figures taken on the test host before the fix were distorted by macOS memory
compression and are not published.

### The comparison harness

`scripts/bench-redis-valkey.sh` runs identical RESP workloads against Recached, Redis and Valkey —
or two Recached builds — and records throughput and latency percentiles (`redis-benchmark --csv`),
server CPU-seconds per million requests (from the kernel, comparable across threading models), and
RSS at load, after an overwrite pass and after `FLUSHDB`. Every run writes `conditions.txt` with
CPU, OS, versions, binary checksums and load average. `SINGLE=1` holds every server to one
execution worker; `PIN=1` pins server and load generator on Linux. Summarise with
`scripts/bench-summarize.py` (throughput, latency, CPU) and `scripts/bench-memory-aggregate.py`
(memory medians across repetitions).

Two traps it is built around: `redis-benchmark` times a run to roughly a quarter of a second, so
fewer than ~2M pipelined requests produce quantised rates (100,000 requests reads as exactly
199,601 or 133,156 rps) — use `REQUESTS` large enough that a run lasts many seconds; and RSS
measured after another workload is that workload's leftovers, which is why each memory scenario
starts a fresh process.

## Interpreting results

- Compare medians across repeated runs, not one best result.
- Separate pipelined throughput from single-request latency.
- Treat a benchmark as evidence only for the tested commands and data shapes.
- Run with production persistence, replication, TLS, and value sizes before capacity planning.
- Use process RSS for host sizing; use Recached's logical memory counter to understand its configured eviction threshold.

Recached's product distinction is the shared server-and-browser engine. The RESP benchmarks above do not measure the browser read path at all: those reads come from local WebAssembly memory and never leave the tab. The only browser-side number here is [push latency](#browser-push-latency), which measures how quickly a server-side write reaches a subscribed client — a different question from how fast that client can read.
