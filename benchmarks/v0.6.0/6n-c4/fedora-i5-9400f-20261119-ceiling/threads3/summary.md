## Throughput, CPU and generator

| Cell | n | requests | rps (redis-benchmark) | rps range | rps (wall clock) | run s | generator cores | CPU µs/op | ctx switches/op | syscalls/op | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| rc-coalesced P=16 w=1 | 1 | 10,000,000 | 194,917 | 194,917–194,917 | 194,898 | 51.3 | 0.17 | 3.71 | 0.001 | 0.04 | 60 |
| rc-standalone P=16 w=1 | 1 | 10,000,000 | 434,329 | 434,329–434,329 | 434,235 | 23.0 | 0.30 | 1.63 | 0.000 | 0.01 | 27 |
| redis P=16 w=1 | 1 | 10,000,000 | 411,743 | 411,743–411,743 | 411,455 | 24.3 | 0.31 | 1.45 | 0.001 | 0.13 | 31 |

## AOF batching and coalescing (Recached cells with an AOF)

| Cell | AOF write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | gate yields/cmd | gate gained share |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| rc-coalesced P=16 w=1 | 0.0370 | 27.0 | 1.000 | 0.963 | 32 | 0.963 | 0.000 | n/a |
| rc-standalone P=16 w=1 | 0.0062 | 161.2 | 0.000 | n/a | 0 | 0.000 | 0.062 | 1.000 |

## Latency, replica lag, convergence

| Cell | p50 ms | p95 ms | p99 ms | lag p50 / p99 ms | converge | sample | replica RSS MiB |
|---|---:|---:|---:|---|---|---|---:|
| rc-coalesced P=16 w=1 | 3.591 | 4.575 | 5.079 | 0.10 / 0.47 | ok | ok | 26 |
| rc-standalone P=16 w=1 | 1.399 | 1.743 | 1.839 | n/a | n/a | n/a | n/a |
| redis P=16 w=1 | 1.247 | 1.599 | 4.103 | 0.58 / 3.84 | ok | ok | 20 |

## CPU by thread name (µs/op, median)

| Cell | bio_aof | bio_close_file | bio_lazy_free | jemalloc_bg_thd | rc-diag-fullfix | redis-server | tokio-rt-worker |
|---|---:|---:|---:|---:|---:|---:|---:|
| rc-coalesced P=16 w=1 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 3.71 |
| rc-standalone P=16 w=1 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 1.63 |
| redis P=16 w=1 | 0.01 | 0.00 | 0.00 | 0.00 | 0.00 | 1.44 | 0.00 |

## Ratios (redis-benchmark rps medians)

| Ratio | P=1 | P=16 |
|---|---:|---:|
| rc-coalesced / rc-standalone | n/a | 0.449 |
| rc-inline / rc-standalone | n/a | n/a |
| rc-coalesced / rc-inline | n/a | n/a |
| rc-coalesced / redis | n/a | 0.473 |
| rc-coalesced / valkey | n/a | n/a |
| rc-standalone / redis | n/a | 1.055 |
| rc-coalesced / rc-inline, 4 workers | n/a | — |

**M1 throughput: NOT VERIFIED** (a required cell is missing)
