## Throughput and CPU

| Cell | n | requests | rps | rps range | rps (wall) | run s | gen cores | CPU µs/op | user µs/op | sys µs/op | ctx sw/op | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| pr2/rc-inline-w4 | 5 | 1,000,000 | 128,982 | 117,606–129,016 | 128,899 | 7.8 | 0.99 | 11.80 | 4.13 | 7.69 | 0.0233 | 64 |
| base/rc-coalesced-w4 | 5 | 1,000,000 | 133,298 | 114,247–142,837 | 133,191 | 7.5 | 1.00 | 10.04 | 3.43 | 6.68 | 0.0194 | 66 |
| pr2/rc-coalesced-w4 | 5 | 1,000,000 | 153,799 | 133,262–159,872 | 153,681 | 6.5 | 1.11 | 9.49 | 3.56 | 5.93 | 0.0189 | 66 |
| f1/rc-coalesced-w4 | 5 | 1,000,000 | 166,639 | 159,923–166,639 | 166,472 | 6.0 | 1.17 | 8.89 | 3.44 | 5.45 | 0.0173 | 67 |
| f2/rc-coalesced-w4 | 5 | 1,000,000 | 159,949 | 137,855–159,974 | 159,821 | 6.3 | 1.13 | 9.21 | 3.51 | 5.70 | 0.0176 | 67 |
| f1f2/rc-coalesced-w4 | 5 | 1,000,000 | 166,639 | 148,038–181,818 | 166,472 | 6.0 | 1.16 | 8.24 | 3.21 | 5.05 | 0.0151 | 66 |

## User-space hardware counters per operation (uprof; kernel excluded)

| Cell | instructions/op | range | cycles/op | IPC | branch misses/op | cache misses/op | L1d read misses/op |
|---|---:|---:|---:|---:|---:|---:|---:|
| pr2/rc-inline-w4 | 15,556 | 15,524–15,570 | 19,555 | 0.79 | 32.49 | 61.14 | 472.49 |
| base/rc-coalesced-w4 | 15,527 | 15,520–15,552 | 16,994 | 0.91 | 34.01 | 58.81 | 466.12 |
| pr2/rc-coalesced-w4 | 15,493 | 15,487–15,513 | 17,368 | 0.89 | 32.22 | 55.66 | 465.69 |
| f1/rc-coalesced-w4 | 15,500 | 15,490–15,507 | 16,709 | 0.93 | 30.09 | 54.25 | 451.25 |
| f2/rc-coalesced-w4 | 15,483 | 15,482–15,505 | 17,078 | 0.91 | 30.88 | 55.67 | 455.39 |
| f1f2/rc-coalesced-w4 | 15,509 | 15,489–15,525 | 16,084 | 0.96 | 29.93 | 53.00 | 448.80 |

## Replication socket and replica

| Cell | frames/segment out | data segs out/op | data segs in/op (acks) | bytes out/op | replica CPU µs/op | lag p50 / p99 ms | converge | sample | replica RSS MiB |
|---|---:|---:|---:|---:|---:|---|---|---|---:|
| pr2/rc-inline-w4 | 56.51 | 0.018 | 0.016 | 87.0 | 1.15 | 0.50 / 4.22 | ok | ok | 24 |
| base/rc-coalesced-w4 | 3.03 | 0.331 | 0.197 | 87.0 | 2.39 | 0.22 / 3.40 | ok | ok | 27 |
| pr2/rc-coalesced-w4 | 32.05 | 0.031 | 0.027 | 87.0 | 1.22 | 0.52 / 3.85 | ok | ok | 28 |
| f1/rc-coalesced-w4 | 31.11 | 0.032 | 0.027 | 87.0 | 1.21 | 0.48 / 4.38 | ok | ok | 28 |
| f2/rc-coalesced-w4 | 31.70 | 0.032 | 0.027 | 87.0 | 1.22 | 0.41 / 4.64 | ok | ok | 28 |
| f1f2/rc-coalesced-w4 | 27.03 | 0.037 | 0.031 | 87.0 | 1.23 | 0.41 / 3.81 | ok | ok | 28 |

## Latency (ms)

| Cell | p50 | p95 | p99 | p99 range | max |
|---|---:|---:|---:|---:|---:|
| pr2/rc-inline-w4 | 0.343 | 0.647 | 1.023 | 0.951–1.679 | 44.671 |
| base/rc-coalesced-w4 | 0.271 | 0.647 | 2.039 | 1.927–2.111 | 259.711 |
| pr2/rc-coalesced-w4 | 0.239 | 0.551 | 2.055 | 1.983–2.279 | 29.631 |
| f1/rc-coalesced-w4 | 0.223 | 0.511 | 2.039 | 2.007–2.055 | 24.927 |
| f2/rc-coalesced-w4 | 0.231 | 0.535 | 2.039 | 1.975–2.095 | 32.719 |
| f1f2/rc-coalesced-w4 | 0.199 | 0.463 | 2.247 | 2.215–2.271 | 337.919 |

## AOF batching and coalescing

| Cell | write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | lock contended/cmd | lead busy/cmd | fanout_busy_skips/cmd |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| pr2/rc-inline-w4 | 0.8881 | 1.1 | 0.000 | n/a | 0 | 0.000 | 0.0735 | 0.4747 | n/a |
| base/rc-coalesced-w4 | 0.4180 | 2.4 | 0.630 | 0.859 | 17,368 | 0.549 | 0.0430 | 0.2086 | n/a |
| pr2/rc-coalesced-w4 | 0.4294 | 2.3 | 0.629 | 0.852 | 17,931 | 0.540 | 0.0431 | 0.2040 | n/a |
| f1/rc-coalesced-w4 | 0.3427 | 2.9 | 0.652 | 0.913 | 4,581 | 0.561 | 0.0396 | 0.2560 | 0.2053 |
| f2/rc-coalesced-w4 | 0.3925 | 2.5 | 0.105 | 0.568 | 14,881 | 0.582 | 0.0414 | 0.1865 | 0.0000 |
| f1f2/rc-coalesced-w4 | 0.2146 | 4.7 | 0.077 | 0.875 | 925 | 0.697 | 0.0292 | 0.2038 | 0.1504 |

## CPU by thread name (user + sys µs/op, median)

| Cell | jemalloc_bg_thd | rc-base | rc-mw-abl3 | rc-pr2 | tokio-rt-worker |
|---|---:|---:|---:|---:|---:|
| pr2/rc-inline-w4 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 4.11 + 7.67 |
| base/rc-coalesced-w4 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 3.44 + 6.67 |
| pr2/rc-coalesced-w4 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 3.55 + 5.92 |
| f1/rc-coalesced-w4 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 3.44 + 5.45 |
| f2/rc-coalesced-w4 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 3.52 + 5.69 |
| f1f2/rc-coalesced-w4 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 3.21 + 5.05 |
