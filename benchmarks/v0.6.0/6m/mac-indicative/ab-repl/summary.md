<details><summary>Conditions</summary>

```
date_utc: 2026-11-18T02:32:22Z
os: Darwin 24.6.0 Darwin Kernel Version 24.6.0: Sun Aug 23 20:30:56 PDT 2026; root:xnu-11417.140.69.712.69~1/RELEASE_X86_64 x86_64
cpu: Intel(R) Core(TM) i5-8259U CPU @ 2.30GHz
aof_fs: /dev/disk1s1 (macOS; indicative only)
pin: 0 server= bench=
extra_env: RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=6710
bench: redis-benchmark 7.2.5 threads=2
server.base: ~/recached-6m-bench/bin/recached-server-6m-off sha256=4d88f29b89719e654fa0b856fc691720b259f0855789f2d82995596ddebeb7af version=recached-server 0.6.0
server.recached: ~/recached-6m-bench/bin/recached-server-6m sha256=2ce378e3cfc683129b7b9e7c618feb937853a1ece3bd17e778cbf9c57980de17 version=recached-server 0.6.0
matrix: servers=[base recached] policies=[everysec] workers=[1] tests=[set] pipelines=[1 16] clients=[1 50] requests=50000 always_requests=20000 keyspace=100000 reps=1
load_average_during_run: ~176 (host rebooted 19 min earlier; other workloads busy). Counters are exact; timing is NOT evidence.
servers: base = 6M binary with RECACHED_AOF_COALESCE=off (the previous gate), recached = 6M default (coalescing on)
source: dev 08e5a7d + uncommitted 6M changes; binary sha256 2ce378e3cfc683129b7b9e7c618feb937853a1ece3bd17e778cbf9c57980de17
```
</details>

| policy | workers | test | P | clients | base rps | base µs CPU/req | base p99 ms | recached rps | recached µs CPU/req | recached p99 ms | base frames/write | base writes/M req | base waiters/write | recached frames/write | recached writes/M req | recached waiters/write |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| everysec | 1 | set | 1 | 1 | 3,382 n=1 | 111.40 | 1.647 | 4,529 n=1 | 105.00 | 0.871 | 1.0 | 1,000,000 | 1.0 | 1.0 | 1,000,000 | 1.0 |
| everysec | 1 | set | 1 | 50 | 10,384 n=1 | 58.40 | 34.687 | 16,633 n=1 | 48.80 | 8.511 | 1.0 | 1,000,000 | 1.0 | 1.0 | 1,000,000 | 1.0 |
| everysec | 1 | set | 16 | 1 | 16,589 n=1 | 31.60 | 4.215 | 39,841 n=1 | 19.00 | 0.903 | 1.0 | 1,000,000 | 1.0 | 1.0 | 1,000,000 | 1.0 |
| everysec | 1 | set | 16 | 50 | 28,490 n=1 | 27.60 | 68.351 | 49,950 n=1 | 18.60 | 56.287 | 1.0 | 1,000,000 | 1.0 | 1.0 | 1,000,000 | 1.0 |
