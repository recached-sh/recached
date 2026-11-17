# Benchmark summary — `benchmarks/v0.6.0/6l/fedora-i5-9400f-20261117-final/E-memory`

<details><summary>Conditions</summary>

```
date_utc: 2026-11-17T15:16:37Z
host_os: Linux 7.3.3-300.fc45.x86_64 x86_64
cpu: Intel(R) Core(TM) i5-9400F CPU @ 2.90GHz
logical_cpus: 6
governor: performance
memory: 16300832 kB
numa: 1
load_average_at_start: 1.77, 1.94, 1.89
pinning: servers=0-2 bench=3-5
network: loopback (127.0.0.1)
bench_tool: redis-benchmark 7.2.5
bench_threads: 2
profile: quick reps=3 single_worker=1 workers=4 persistence=none
requests_per_run: 1000
server.baseline.binary: /home/<user>/recached-6l-bench/bin/recached-server-6k
server.baseline.sha256: a3cb62c9c3ce4336f5310764ad8d98fdd772ee774ff004f3f71e597570ebd4fe
server.baseline.version: recached-server 0.6.0
server.recached.binary: /home/<user>/recached-6l-bench/bin/recached-server-6l
server.recached.sha256: d3f569e9ccd74a09b1afcbe4f30a6fba46eac5a8bc6800b9786dd96c7fb601a1
server.recached.version: recached-server 0.6.0
server.redis.binary: /home/<user>/recached-6l-bench/bin/redis-server
server.redis.sha256: d6dd540017b76cebcf2c64f77713f783d66c477b4b9ff113a0e9e63d262b5d2e
server.redis.version: Redis server v=7.2.5 sha=00000000:0 malloc=jemalloc-5.3.0 bits=64 build=7fa7d45f1d3dc5e0
server.valkey.binary: /home/<user>/recached-6l-bench/bin/valkey-server
server.valkey.sha256: 8906188654d88a2aa4a83a9fd0d10397de4fda09f778445f5d96bd1eecc5a205
server.valkey.version: Valkey server v=9.1.0 sha=00000000:1 malloc=jemalloc-5.3.0 bits=64 build=e790ed272ac294f
rustc: rustc 1.99.1 (58cf8f9c3 2026-10-13)
recached_build: release profile from Cargo.toml (opt-level=3, lto=thin, codegen-units=1, overflow-checks=true); allocator jemalloc
rss_idle_kb_at_start.baseline: 8224
rss_idle_kb_at_start.recached: 8056
rss_idle_kb_at_start.redis: 7980
rss_idle_kb_at_start.valkey: 8652
```
</details>

## Throughput (median rps, [IQR]) and p99 latency (median ms)

| test | P | clients | size | keys | baseline rps | baseline p99 | recached rps | recached p99 | redis rps | redis p99 | valkey rps | valkey p99 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| get | 1 | 1 | 32 | 100000 | 3,968 [3,968–3,968] n=3 | 0.055 | 3,984 [3,976–3,984] n=3 | 0.071 | 3,984 [3,976–3,984] n=3 | 0.055 | 3,968 [3,968–3,976] n=3 | 0.063 |

## CPU efficiency (median server CPU-seconds per million requests)

Lower is better. Measured from the server process's own user+system time,
so it is comparable across threading models.

| test | P | clients | size | keys | baseline | recached | redis | valkey |
|---|---|---|---|---|---|---|---|---|
| get | 1 | 1 | 32 | 100000 | 10.00 | 10.00 | 10.00 | 10.00 |

## Process memory (RSS)

`bytes/key` = (RSS after load − idle RSS) / keys actually stored (DBSIZE).
RSS includes allocator overhead and fragmentation; it is not any server's
logical `used_memory` figure.

| server | kind | keys | value | idle MiB | loaded MiB | after overwrite MiB | after FLUSHDB MiB | bytes/key | retained after flush |
|---|---|---|---|---|---|---|---|---|---|
| baseline | string | 1,000,000 | 32 | 8.1 | 203.6 | 199.0 | 90.2 | 205 | 42% |
| baseline | hash | 1,000,000 | 32 | 8.0 | 372.2 | 362.9 | 109.7 | 382 | 28% |
| baseline | ttl | 1,000,000 | 32 | 8.1 | 275.3 | 260.8 | 154.1 | 280 | 55% |
| recached | string | 1,000,000 | 32 | 7.8 | 206.1 | 201.4 | 101.5 | 208 | 47% |
| recached | hash | 1,000,000 | 32 | 7.7 | 374.9 | 362.4 | 177.6 | 385 | 46% |
| recached | ttl | 1,000,000 | 32 | 8.0 | 285.6 | 265.0 | 153.1 | 291 | 52% |
| redis | string | 1,000,000 | 32 | 7.7 | 112.6 | 112.6 | 14.1 | 110 | 6% |
| redis | hash | 1,000,000 | 32 | 7.8 | 202.8 | 196.8 | 15.8 | 204 | 4% |
| redis | ttl | 1,000,000 | 32 | 7.9 | 148.0 | 148.0 | 11.1 | 147 | 2% |
| valkey | string | 1,000,000 | 32 | 8.7 | 88.7 | 88.7 | 72.9 | 84 | 80% |
| valkey | hash | 1,000,000 | 32 | 8.6 | 192.9 | 192.9 | 177.1 | 193 | 91% |
| valkey | ttl | 1,000,000 | 32 | 8.7 | 126.1 | 124.8 | 92.9 | 123 | 72% |
