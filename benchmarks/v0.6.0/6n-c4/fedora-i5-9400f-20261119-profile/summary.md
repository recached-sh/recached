## Throughput, CPU and generator

| Cell | n | requests | rps (redis-benchmark) | rps range | rps (wall clock) | run s | generator cores | CPU µs/op | ctx switches/op | syscalls/op | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| rc-coalesced P=1 w=1 | 2 | 1,000,000 | 96,386 | 95,220–97,551 | 96,335 | 10.4 | 0.78 | 7.26 | 0.001 | 0.07 | 60 |
| rc-coalesced-norep P=1 w=1 | 2 | 1,000,000 | 117,640 | 117,633–117,647 | 117,564 | 8.5 | 0.92 | 6.14 | 0.001 | 0.07 | 59 |
| rc-noaof P=1 w=1 | 2 | 1,000,000 | 159,987 | 159,974–160,000 | 159,859 | 6.3 | 1.24 | 4.13 | 0.000 | 0.00 | 26 |
| rc-repl-noaof P=1 w=1 | 2 | 1,000,000 | 98,766 | 97,542–99,990 | 98,722 | 10.1 | 0.82 | 7.13 | 0.000 | 0.00 | 61 |
| rc-standalone P=1 w=1 | 2 | 1,000,000 | 117,722 | 114,247–121,197 | 117,653 | 8.5 | 0.87 | 5.99 | 0.000 | 0.10 | 26 |
| rc-coalesced P=16 w=1 | 2 | 10,000,000 | 185,657 | 163,106–208,208 | 185,637 | 54.7 | 0.16 | 3.72 | 0.001 | 0.04 | 59 |
| rc-coalesced-norep P=16 w=1 | 2 | 10,000,000 | 309,133 | 279,454–338,811 | 309,079 | 32.7 | 0.25 | 2.36 | 0.000 | 0.04 | 57 |
| rc-noaof P=16 w=1 | 2 | 10,000,000 | 1,142,596 | 1,142,596–1,142,596 | 1,141,944 | 8.8 | 0.74 | 0.72 | 0.000 | 0.00 | 26 |
| rc-repl-noaof P=16 w=1 | 2 | 10,000,000 | 464,965 | 464,943–464,986 | 464,857 | 21.5 | 0.36 | 1.90 | 0.000 | 0.00 | 59 |
| rc-standalone P=16 w=1 | 2 | 10,000,000 | 457,478 | 439,174–475,783 | 457,296 | 21.9 | 0.29 | 1.65 | 0.000 | 0.01 | 26 |

## AOF batching and coalescing (Recached cells with an AOF)

| Cell | AOF write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | gate yields/cmd | gate gained share |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| rc-coalesced P=1 w=1 | 0.0702 | 14.3 | 0.999 | 0.931 | 30 | 0.930 | 0.000 | n/a |
| rc-coalesced-norep P=1 w=1 | 0.0677 | 14.8 | 0.999 | 0.933 | 21 | 0.932 | 0.000 | n/a |
| rc-standalone P=1 w=1 | 0.0999 | 10.0 | 0.000 | n/a | 0 | 0.000 | 1.000 | 1.000 |
| rc-coalesced P=16 w=1 | 0.0370 | 27.0 | 1.000 | 0.963 | 35 | 0.963 | 0.000 | n/a |
| rc-coalesced-norep P=16 w=1 | 0.0355 | 28.2 | 1.000 | 0.965 | 26 | 0.964 | 0.000 | n/a |
| rc-standalone P=16 w=1 | 0.0062 | 160.7 | 0.000 | n/a | 0 | 0.000 | 0.062 | 1.000 |

## Latency, replica lag, convergence

| Cell | p50 ms | p95 ms | p99 ms | lag p50 / p99 ms | converge | sample | replica RSS MiB |
|---|---:|---:|---:|---|---|---|---:|
| rc-coalesced P=1 w=1 | 0.483 | 0.675 | 0.807 | 0.11 / 0.45 | ok | ok | 24 |
| rc-coalesced-norep P=1 w=1 | 0.391 | 0.535 | 0.675 | n/a | n/a | n/a | n/a |
| rc-noaof P=1 w=1 | 0.315 | 0.339 | 0.359 | n/a | n/a | n/a | n/a |
| rc-repl-noaof P=1 w=1 | 0.515 | 0.607 | 0.707 | 0.33 / 0.70 | ok | ok | 24 |
| rc-standalone P=1 w=1 | 0.379 | 0.515 | 0.555 | n/a | n/a | n/a | n/a |
| rc-coalesced P=16 w=1 | 3.591 | 4.587 | 5.127 | 0.11 / 0.65 | ok | ok | 24 |
| rc-coalesced-norep P=16 w=1 | 2.055 | 2.679 | 2.955 | n/a | n/a | n/a | n/a |
| rc-noaof P=16 w=1 | 0.707 | 0.771 | 0.879 | n/a | n/a | n/a | n/a |
| rc-repl-noaof P=16 w=1 | 1.763 | 1.923 | 2.059 | 0.57 / 1.77 | ok | ok | 24 |
| rc-standalone P=16 w=1 | 1.407 | 1.759 | 1.887 | n/a | n/a | n/a | n/a |

## CPU by thread name (µs/op, median)

| Cell | jemalloc_bg_thd | recached-server | tokio-rt-worker |
|---|---:|---:|---:|
| rc-coalesced P=1 w=1 | 0.00 | 0.00 | 7.25 |
| rc-coalesced-norep P=1 w=1 | 0.00 | 0.00 | 6.13 |
| rc-noaof P=1 w=1 | 0.00 | 0.00 | 4.13 |
| rc-repl-noaof P=1 w=1 | 0.00 | 0.00 | 7.13 |
| rc-standalone P=1 w=1 | 0.00 | 0.00 | 5.98 |
| rc-coalesced P=16 w=1 | 0.00 | 0.00 | 3.71 |
| rc-coalesced-norep P=16 w=1 | 0.00 | 0.00 | 2.36 |
| rc-noaof P=16 w=1 | 0.00 | 0.00 | 0.72 |
| rc-repl-noaof P=16 w=1 | 0.00 | 0.00 | 1.90 |
| rc-standalone P=16 w=1 | 0.00 | 0.00 | 1.65 |

## Ratios (redis-benchmark rps medians)

| Ratio | P=1 | P=16 |
|---|---:|---:|
| rc-coalesced / rc-standalone | 0.819 | 0.406 |
| rc-inline / rc-standalone | n/a | n/a |
| rc-coalesced / rc-inline | n/a | n/a |
| rc-coalesced / redis | n/a | n/a |
| rc-coalesced / valkey | n/a | n/a |
| rc-standalone / redis | n/a | n/a |
| rc-coalesced / rc-inline, 4 workers | n/a | — |

**M1 throughput: below threshold** (P=1 0.819, P=16 0.406; threshold 0.85)
