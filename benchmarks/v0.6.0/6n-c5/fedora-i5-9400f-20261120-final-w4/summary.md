## Throughput and CPU

| Cell | n | requests | rps | rps range | rps (wall) | run s | gen cores | CPU µs/op | user µs/op | sys µs/op | ctx sw/op | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| base/rc-inline-w4 | 5 | 1,000,000 | 121,197 | 117,619–121,197 | 121,124 | 8.3 | 0.95 | 11.91 | 3.82 | 8.09 | 0.0210 | 61 |
| final/rc-inline-w4 | 5 | 1,000,000 | 129,016 | 121,183–129,032 | 128,932 | 7.8 | 1.02 | 11.62 | 4.03 | 7.59 | 0.0223 | 64 |
| base/rc-coalesced-w4 | 5 | 1,000,000 | 148,126 | 148,104–148,126 | 147,995 | 6.8 | 1.09 | 9.70 | 3.31 | 6.43 | 0.0172 | 66 |
| final/rc-coalesced-w4 | 5 | 1,000,000 | 166,639 | 166,611–166,639 | 166,500 | 6.0 | 1.18 | 8.82 | 3.40 | 5.41 | 0.0158 | 66 |

## User-space hardware counters per operation (uprof; kernel excluded)

| Cell | instructions/op | range | cycles/op | IPC | branch misses/op | cache misses/op | L1d read misses/op |
|---|---:|---:|---:|---:|---:|---:|---:|
| base/rc-inline-w4 | 15,599 | 15,590–15,612 | 18,505 | 0.84 | 38.35 | 43.90 | 475.05 |
| final/rc-inline-w4 | 15,533 | 15,525–15,544 | 19,105 | 0.81 | 32.35 | 44.56 | 476.61 |
| base/rc-coalesced-w4 | 15,507 | 15,491–15,513 | 16,515 | 0.94 | 33.70 | 42.47 | 461.53 |
| final/rc-coalesced-w4 | 15,456 | 15,452–15,466 | 16,630 | 0.93 | 30.17 | 42.02 | 463.87 |

## Replication socket and replica

| Cell | frames/segment out | data segs out/op | data segs in/op (acks) | bytes out/op | replica CPU µs/op | lag p50 / p99 ms | converge | sample | replica RSS MiB |
|---|---:|---:|---:|---:|---:|---|---|---|---:|
| base/rc-inline-w4 | 3.38 | 0.296 | 0.193 | 87.0 | 2.45 | 0.33 / 2.89 | ok | ok | 24 |
| final/rc-inline-w4 | 61.23 | 0.016 | 0.016 | 87.0 | 1.12 | 0.46 / 3.65 | ok | ok | 24 |
| base/rc-coalesced-w4 | 3.14 | 0.319 | 0.192 | 87.0 | 2.31 | 0.12 / 3.04 | ok | ok | 28 |
| final/rc-coalesced-w4 | 33.20 | 0.030 | 0.027 | 87.0 | 1.20 | 0.40 / 3.77 | ok | ok | 28 |

## Latency (ms)

| Cell | p50 | p95 | p99 | p99 range | max |
|---|---:|---:|---:|---:|---:|
| base/rc-inline-w4 | 0.375 | 0.735 | 1.047 | 1.039–1.055 | 30.415 |
| final/rc-inline-w4 | 0.351 | 0.639 | 0.895 | 0.887–0.919 | 28.319 |
| base/rc-coalesced-w4 | 0.263 | 0.591 | 1.983 | 1.959–2.063 | 29.119 |
| final/rc-coalesced-w4 | 0.223 | 0.495 | 2.175 | 2.087–2.215 | 31.023 |

## AOF batching and coalescing

| Cell | write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | lock contended/cmd | lead busy/cmd | fanout_busy_skips/cmd |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| base/rc-inline-w4 | 0.8780 | 1.1 | 0.000 | n/a | 0 | 0.000 | 0.0796 | 0.5310 | n/a |
| final/rc-inline-w4 | 0.8862 | 1.1 | 0.000 | n/a | 0 | 0.000 | 0.0793 | 0.4979 | 0.0000 |
| base/rc-coalesced-w4 | 0.4233 | 2.4 | 0.626 | 0.854 | 19,378 | 0.549 | 0.0437 | 0.2177 | n/a |
| final/rc-coalesced-w4 | 0.3278 | 3.1 | 0.676 | 0.916 | 4,373 | 0.587 | 0.0390 | 0.2369 | 0.1891 |

## CPU by thread name (user + sys µs/op, median)

| Cell | jemalloc_bg_thd | rc-base | rc-final | tokio-rt-worker |
|---|---:|---:|---:|---:|
| base/rc-inline-w4 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 3.82 + 8.08 |
| final/rc-inline-w4 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 4.04 + 7.59 |
| base/rc-coalesced-w4 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 3.30 + 6.41 |
| final/rc-coalesced-w4 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 3.40 + 5.41 |
