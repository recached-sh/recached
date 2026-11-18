<details><summary>Conditions</summary>

```
date_utc: 2026-11-18T02:30:30Z
os: Darwin 24.6.0 Darwin Kernel Version 24.6.0: Sun Aug 23 20:30:56 PDT 2026; root:xnu-11417.140.69.712.69~1/RELEASE_X86_64 x86_64
cpu: Intel(R) Core(TM) i5-8259U CPU @ 2.30GHz
aof_fs: /dev/disk1s1 (macOS; indicative only)
pin: 0 server= bench=
extra_env: none
bench: redis-benchmark 7.2.5 threads=2
server.base: ~/recached-6m-bench/bin/recached-server-6m-off sha256=4d88f29b89719e654fa0b856fc691720b259f0855789f2d82995596ddebeb7af version=recached-server 0.6.0
server.recached: ~/recached-6m-bench/bin/recached-server-6m sha256=2ce378e3cfc683129b7b9e7c618feb937853a1ece3bd17e778cbf9c57980de17 version=recached-server 0.6.0
matrix: servers=[base recached] policies=[always] workers=[1] tests=[set] pipelines=[1] clients=[1 50] requests=200000 always_requests=2000 keyspace=100000 reps=1
load_average_during_run: ~176 (host rebooted 19 min earlier; other workloads busy). Counters are exact; timing is NOT evidence.
servers: base = 6M binary with RECACHED_AOF_COALESCE=off (the previous gate), recached = 6M default (coalescing on)
source: dev 08e5a7d + uncommitted 6M changes; binary sha256 2ce378e3cfc683129b7b9e7c618feb937853a1ece3bd17e778cbf9c57980de17
```
</details>

| policy | workers | test | P | clients | base rps | base µs CPU/req | base p99 ms | recached rps | recached µs CPU/req | recached p99 ms | base frames/write | base writes/M req | base waiters/write | recached frames/write | recached writes/M req | recached waiters/write |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| always | 1 | set | 1 | 1 | 49 n=1 | 430.00 | 43.775 | 45 n=1 | 510.00 | 47.967 | 1.0 | 1,000,000 | 1.0 | 1.0 | 1,000,000 | 1.0 |
| always | 1 | set | 1 | 50 | 989 n=1 | 80.00 | 140.031 | 1,130 n=1 | 60.00 | 65.311 | 24.4 | 41,000 | 24.4 | 25.0 | 40,000 | 25.0 |
