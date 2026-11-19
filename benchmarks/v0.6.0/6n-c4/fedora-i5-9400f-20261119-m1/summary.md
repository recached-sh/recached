## Throughput, CPU and generator

| Cell | n | requests | rps (redis-benchmark) | rps range | rps (wall clock) | run s | generator cores | CPU µs/op | ctx switches/op | syscalls/op | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| rc-coalesced P=1 w=1 | 3 | 1,000,000 | 97,532 | 95,220–97,551 | 97,485 | 10.3 | 0.79 | 7.35 | 0.001 | 0.07 | 60 |
| rc-inline P=1 w=1 | 3 | 1,000,000 | 57,941 | 56,319–57,948 | 57,924 | 17.3 | 0.57 | 13.36 | 0.001 | 1.00 | 59 |
| rc-standalone P=1 w=1 | 3 | 1,000,000 | 121,197 | 121,197–124,984 | 121,124 | 8.3 | 0.88 | 5.92 | 0.000 | 0.10 | 26 |
| rc-coalesced P=16 w=1 | 3 | 10,000,000 | 216,076 | 216,076–218,436 | 216,048 | 46.3 | 0.17 | 3.65 | 0.001 | 0.04 | 59 |
| rc-inline P=16 w=1 | 3 | 10,000,000 | 189,448 | 187,652–194,054 | 189,430 | 52.8 | 0.16 | 4.77 | 0.000 | 1.00 | 59 |
| rc-standalone P=16 w=1 | 3 | 10,000,000 | 470,146 | 387,943–499,625 | 470,035 | 21.3 | 0.29 | 1.63 | 0.000 | 0.01 | 26 |
| redis P=16 w=1 | 3 | 10,000,000 | 411,726 | 353,507–443,872 | 411,387 | 24.3 | 0.25 | 1.48 | 0.001 | 0.13 | 27 |
| valkey P=16 w=1 | 3 | 10,000,000 | 429,406 | 407,598–429,498 | 429,185 | 23.3 | 0.27 | 1.55 | 0.001 | 0.13 | 36 |
| rc-coalesced P=1 w=4 | 3 | 1,000,000 | 148,126 | 137,874–153,846 | 148,017 | 6.8 | 1.09 | 9.66 | 0.019 | 0.42 | 66 |
| rc-inline P=1 w=4 | 3 | 1,000,000 | 121,197 | 117,619–121,212 | 121,124 | 8.3 | 0.95 | 11.67 | 0.022 | 0.88 | 61 |

## AOF batching and coalescing (Recached cells with an AOF)

| Cell | AOF write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | gate yields/cmd | gate gained share |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| rc-coalesced P=1 w=1 | 0.0693 | 14.4 | 1.000 | 0.931 | 3 | 0.931 | 0.000 | n/a |
| rc-inline P=1 w=1 | 1.0003 | 1.0 | 0.000 | n/a | 0 | 0.000 | 0.000 | n/a |
| rc-standalone P=1 w=1 | 0.1000 | 10.0 | 0.000 | n/a | 0 | 0.000 | 1.000 | 1.000 |
| rc-coalesced P=16 w=1 | 0.0370 | 27.0 | 1.000 | 0.963 | 41 | 0.963 | 0.000 | n/a |
| rc-inline P=16 w=1 | 1.0001 | 1.0 | 0.000 | n/a | 0 | 0.000 | 0.000 | n/a |
| rc-standalone P=16 w=1 | 0.0062 | 160.6 | 0.000 | n/a | 0 | 0.000 | 0.062 | 1.000 |
| rc-coalesced P=1 w=4 | 0.4202 | 2.4 | 0.629 | 0.859 | 18,241 | 0.551 | 0.000 | n/a |
| rc-inline P=1 w=4 | 0.8809 | 1.1 | 0.000 | n/a | 0 | 0.000 | 0.000 | n/a |

## Latency, replica lag, convergence

| Cell | p50 ms | p95 ms | p99 ms | lag p50 / p99 ms | converge | sample | replica RSS MiB |
|---|---:|---:|---:|---|---|---|---:|
| rc-coalesced P=1 w=1 | 0.487 | 0.687 | 0.807 | 0.10 / 0.48 | ok | ok | 24 |
| rc-inline P=1 w=1 | 0.895 | 0.919 | 0.959 | 0.34 / 0.94 | ok | ok | 24 |
| rc-standalone P=1 w=1 | 0.375 | 0.503 | 0.535 | n/a | n/a | n/a | n/a |
| rc-coalesced P=16 w=1 | 3.527 | 4.495 | 4.975 | 0.10 / 0.53 | ok | ok | 24 |
| rc-inline P=16 w=1 | 4.175 | 4.279 | 4.583 | 1.57 / 3.65 | ok | ok | 24 |
| rc-standalone P=16 w=1 | 1.383 | 1.727 | 1.815 | n/a | n/a | n/a | n/a |
| redis P=16 w=1 | 1.215 | 1.575 | 4.823 | 0.68 / 2.63 | ok | ok | 20 |
| valkey P=16 w=1 | 1.271 | 1.591 | 3.855 | 0.76 / 3.55 | ok | ok | 27 |
| rc-coalesced P=1 w=4 | 0.255 | 0.599 | 1.887 | 0.14 / 3.30 | ok | ok | 27 |
| rc-inline P=1 w=4 | 0.367 | 0.807 | 1.055 | 0.29 / 2.79 | ok | ok | 24 |

## CPU by thread name (µs/op, median)

| Cell | bio_aof | bio_close_file | bio_lazy_free | bio_rdb_save | bio_tls_reload | jemalloc_bg_thd | recached-server | redis-server | tokio-rt-worker | valkey-server |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| rc-coalesced P=1 w=1 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 7.34 | 0.00 |
| rc-inline P=1 w=1 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 13.35 | 0.00 |
| rc-standalone P=1 w=1 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 5.91 | 0.00 |
| rc-coalesced P=16 w=1 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 3.65 | 0.00 |
| rc-inline P=16 w=1 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 4.76 | 0.00 |
| rc-standalone P=16 w=1 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 1.63 | 0.00 |
| redis P=16 w=1 | 0.01 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 1.47 | 0.00 | 0.00 |
| valkey P=16 w=1 | 0.01 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 1.54 |
| rc-coalesced P=1 w=4 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 9.67 | 0.00 |
| rc-inline P=1 w=4 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 11.66 | 0.00 |

## Ratios (redis-benchmark rps medians)

| Ratio | P=1 | P=16 |
|---|---:|---:|
| rc-coalesced / rc-standalone | 0.805 | 0.460 |
| rc-inline / rc-standalone | 0.478 | 0.403 |
| rc-coalesced / rc-inline | 1.683 | 1.141 |
| rc-coalesced / redis | n/a | 0.525 |
| rc-coalesced / valkey | n/a | 0.503 |
| rc-standalone / redis | n/a | 1.142 |
| rc-coalesced / rc-inline, 4 workers | 1.222 | — |

p99 at P=1: coalesced 0.807 ms vs inline 0.959 ms (-15.8%)

p99 at P=16: coalesced 4.975 ms vs inline 4.583 ms (+8.6%)

**M1 throughput: below threshold** (P=1 0.805, P=16 0.460; threshold 0.85)
