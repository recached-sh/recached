## Throughput, CPU and generator

| Cell | n | requests | rps (redis-benchmark) | rps range | rps (wall clock) | run s | generator cores | CPU µs/op | ctx switches/op | syscalls/op | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| rc-coalesced P=16 w=1 | 1 | 10,000,000 | 202,889 | 202,889–202,889 | 202,864 | 49.3 | 0.16 | 3.62 | 0.000 | 0.04 | 60 |
| rc-standalone P=16 w=1 | 1 | 10,000,000 | 493,243 | 493,243–493,243 | 493,121 | 20.3 | 0.30 | 1.61 | 0.000 | 0.01 | 27 |
| redis P=16 w=1 | 1 | 10,000,000 | 443,892 | 443,892–443,892 | 441,112 | 22.7 | 0.28 | 1.49 | 0.001 | 0.13 | 47 |

## AOF batching and coalescing (Recached cells with an AOF)

| Cell | AOF write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | gate yields/cmd | gate gained share |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| rc-coalesced P=16 w=1 | 0.0370 | 27.0 | 1.000 | 0.963 | 51 | 0.963 | 0.000 | n/a |
| rc-standalone P=16 w=1 | 0.0062 | 160.1 | 0.000 | n/a | 0 | 0.000 | 0.062 | 1.000 |

## Latency, replica lag, convergence

| Cell | p50 ms | p95 ms | p99 ms | lag p50 / p99 ms | converge | sample | replica RSS MiB |
|---|---:|---:|---:|---|---|---|---:|
| rc-coalesced P=16 w=1 | 3.503 | 4.479 | 5.007 | 0.11 / 0.53 | ok | ok | 25 |
| rc-standalone P=16 w=1 | 1.367 | 1.711 | 1.799 | n/a | n/a | n/a | n/a |
| redis P=16 w=1 | 1.231 | 1.671 | 4.071 | 0.66 / 3.71 | ok | ok | 20 |

## CPU by thread name (µs/op, median)

| Cell | bio_aof | bio_close_file | bio_lazy_free | jemalloc_bg_thd | rc-diag-fullfix | redis-server | tokio-rt-worker |
|---|---:|---:|---:|---:|---:|---:|---:|
| rc-coalesced P=16 w=1 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 3.62 |
| rc-standalone P=16 w=1 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 1.61 |
| redis P=16 w=1 | 0.01 | 0.00 | 0.00 | 0.00 | 0.00 | 1.48 | 0.00 |

## Ratios (redis-benchmark rps medians)

| Ratio | P=1 | P=16 |
|---|---:|---:|
| rc-coalesced / rc-standalone | n/a | 0.411 |
| rc-inline / rc-standalone | n/a | n/a |
| rc-coalesced / rc-inline | n/a | n/a |
| rc-coalesced / redis | n/a | 0.457 |
| rc-coalesced / valkey | n/a | n/a |
| rc-standalone / redis | n/a | 1.111 |
| rc-coalesced / rc-inline, 4 workers | n/a | — |

**M1 throughput: NOT VERIFIED** (a required cell is missing)
