<details><summary>Conditions</summary>

```
date_utc: 2026-11-18T02:26:04Z
os: Darwin 24.6.0 Darwin Kernel Version 24.6.0: Sun Aug 23 20:30:56 PDT 2026; root:xnu-11417.140.69.712.69~1/RELEASE_X86_64 x86_64
cpu: Intel(R) Core(TM) i5-8259U CPU @ 2.30GHz
aof_fs: /dev/disk1s1 (macOS; indicative only)
pin: 0 server= bench=
extra_env: none
bench: redis-benchmark 7.2.5 threads=2
server.base: ~/recached-6m-bench/bin/recached-server-6m-off sha256=4d88f29b89719e654fa0b856fc691720b259f0855789f2d82995596ddebeb7af version=recached-server 0.6.0
server.recached: ~/recached-6m-bench/bin/recached-server-6m sha256=2ce378e3cfc683129b7b9e7c618feb937853a1ece3bd17e778cbf9c57980de17 version=recached-server 0.6.0
matrix: servers=[base recached] policies=[everysec] workers=[1 4] tests=[set] pipelines=[1 16] clients=[1 2 8 50 200] requests=50000 always_requests=20000 keyspace=100000 reps=1
load_average_during_run: ~176 (host rebooted 19 min earlier; other workloads busy). Counters are exact; timing is NOT evidence.
servers: base = 6M binary with RECACHED_AOF_COALESCE=off (the previous gate), recached = 6M default (coalescing on)
source: dev 08e5a7d + uncommitted 6M changes; binary sha256 2ce378e3cfc683129b7b9e7c618feb937853a1ece3bd17e778cbf9c57980de17
```
</details>

| policy | workers | test | P | clients | base rps | base µs CPU/req | base p99 ms | recached rps | recached µs CPU/req | recached p99 ms | base frames/write | base writes/M req | base waiters/write | recached frames/write | recached writes/M req | recached waiters/write |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| everysec | 1 | set | 1 | 1 | 2,966 n=1 | 115.20 | 1.999 | 3,438 n=1 | 94.20 | 2.263 | 1.0 | 1,000,000 | 1.0 | 1.0 | 1,000,000 | 1.0 |
| everysec | 1 | set | 1 | 2 | 3,371 n=1 | 93.60 | 4.839 | 6,043 n=1 | 79.20 | 1.935 | 1.0 | 1,000,000 | 1.0 | 1.0 | 988,140 | 1.0 |
| everysec | 1 | set | 1 | 8 | 6,378 n=1 | 68.00 | 12.399 | 14,269 n=1 | 50.20 | 1.743 | 1.0 | 1,000,000 | 1.0 | 2.0 | 496,620 | 2.0 |
| everysec | 1 | set | 1 | 50 | 7,682 n=1 | 52.80 | 45.503 | 13,134 n=1 | 37.80 | 30.399 | 1.0 | 1,000,000 | 1.0 | 9.5 | 105,520 | 9.5 |
| everysec | 1 | set | 1 | 200 | 6,636 n=1 | 53.60 | 155.391 | 12,401 n=1 | 34.40 | 59.679 | 1.0 | 1,000,000 | 1.0 | 21.8 | 45,880 | 21.8 |
| everysec | 1 | set | 16 | 1 | 24,925 n=1 | 14.20 | 4.127 | 39,904 n=1 | 12.20 | 2.199 | 16.0 | 62,500 | 1.0 | 16.0 | 62,500 | 1.0 |
| everysec | 1 | set | 16 | 2 | 33,245 n=1 | 12.20 | 9.455 | 39,872 n=1 | 10.20 | 5.639 | 16.0 | 62,500 | 1.0 | 16.1 | 62,180 | 1.0 |
| everysec | 1 | set | 16 | 8 | 33,245 n=1 | 11.60 | 35.743 | 49,950 n=1 | 9.00 | 26.191 | 16.0 | 62,500 | 1.0 | 44.1 | 22,660 | 2.8 |
| everysec | 1 | set | 16 | 50 | 28,523 n=1 | 11.40 | 102.207 | 49,950 n=1 | 8.80 | 37.631 | 16.0 | 62,500 | 1.0 | 166.1 | 6,020 | 10.4 |
| everysec | 1 | set | 16 | 200 | 33,113 n=1 | 10.00 | 206.335 | 49,702 n=1 | 8.00 | 119.231 | 16.0 | 62,500 | 1.0 | 400.0 | 2,500 | 25.0 |
| everysec | 4 | set | 1 | 1 | 4,244 n=1 | 99.80 | 1.151 | 3,762 n=1 | 115.80 | 1.215 | 1.0 | 1,000,000 | 1.0 | 1.0 | 1,000,000 | 1.0 |
| everysec | 4 | set | 1 | 2 | 8,645 n=1 | 79.00 | 0.735 | 7,395 n=1 | 95.00 | 0.967 | 1.0 | 999,340 | 1.0 | 1.0 | 992,780 | 1.0 |
| everysec | 4 | set | 1 | 8 | 15,115 n=1 | 66.20 | 2.071 | 18,077 n=1 | 76.60 | 1.263 | 1.1 | 905,140 | 1.1 | 1.3 | 799,360 | 1.3 |
| everysec | 4 | set | 1 | 50 | 13,488 n=1 | 53.40 | 35.103 | 24,777 n=1 | 51.40 | 19.615 | 1.4 | 731,700 | 1.4 | 1.5 | 680,900 | 1.5 |
| everysec | 4 | set | 1 | 200 | 21,636 n=1 | 52.00 | 53.695 | 28,090 n=1 | 49.20 | 24.863 | 1.4 | 709,100 | 1.4 | 1.5 | 659,840 | 1.5 |
| everysec | 4 | set | 16 | 1 | 39,904 n=1 | 13.20 | 1.727 | 49,900 n=1 | 12.40 | 0.807 | 16.0 | 62,500 | 1.0 | 16.0 | 62,500 | 1.0 |
| everysec | 4 | set | 16 | 2 | 32,300 n=1 | 11.80 | 6.527 | 66,138 n=1 | 10.60 | 0.743 | 14.8 | 67,760 | 1.0 | 12.9 | 77,660 | 1.1 |
| everysec | 4 | set | 16 | 8 | 99,800 n=1 | 13.80 | 9.871 | 66,578 n=1 | 12.40 | 10.791 | 6.1 | 164,920 | 2.0 | 7.4 | 135,060 | 1.8 |
| everysec | 4 | set | 16 | 50 | 66,489 n=1 | 13.00 | 76.543 | 99,602 n=1 | 10.80 | 22.911 | 8.8 | 113,560 | 1.9 | 8.1 | 123,240 | 2.1 |
| everysec | 4 | set | 16 | 200 | 66,138 n=1 | 13.60 | 143.231 | 99,404 n=1 | 10.60 | 46.719 | 6.8 | 146,320 | 2.0 | 6.9 | 144,520 | 1.9 |
