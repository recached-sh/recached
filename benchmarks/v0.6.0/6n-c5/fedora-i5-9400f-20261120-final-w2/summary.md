## Throughput and CPU

| Cell | n | requests | rps | rps range | rps (wall) | run s | gen cores | CPU µs/op | user µs/op | sys µs/op | ctx sw/op | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| base/rc-inline-w2 | 5 | 1,000,000 | 124,984 | 121,197–128,999 | 124,906 | 8.0 | 0.99 | 11.89 | 3.77 | 8.12 | 0.0081 | 60 |
| final/rc-inline-w2 | 5 | 1,000,000 | 129,016 | 117,606–133,316 | 128,932 | 7.8 | 0.98 | 11.83 | 4.12 | 7.72 | 0.0053 | 63 |
| base/rc-coalesced-w2 | 5 | 1,000,000 | 148,082 | 133,244–148,126 | 147,929 | 6.8 | 1.11 | 9.80 | 3.27 | 6.52 | 0.0035 | 64 |
| final/rc-coalesced-w2 | 5 | 1,000,000 | 173,883 | 173,883–173,913 | 173,732 | 5.8 | 1.22 | 8.28 | 3.23 | 5.06 | 0.0021 | 65 |

## User-space hardware counters per operation (uprof; kernel excluded)

| Cell | instructions/op | range | cycles/op | IPC | branch misses/op | cache misses/op | L1d read misses/op |
|---|---:|---:|---:|---:|---:|---:|---:|
| base/rc-inline-w2 | 15,673 | 15,668–15,682 | 18,422 | 0.85 | 33.81 | 38.86 | 475.60 |
| final/rc-inline-w2 | 15,582 | 15,558–15,595 | 19,529 | 0.80 | 31.37 | 37.96 | 472.94 |
| base/rc-coalesced-w2 | 15,521 | 15,516–15,531 | 16,564 | 0.94 | 33.45 | 38.17 | 464.82 |
| final/rc-coalesced-w2 | 15,378 | 15,372–15,385 | 16,011 | 0.96 | 28.48 | 36.22 | 458.27 |

## Replication socket and replica

| Cell | frames/segment out | data segs out/op | data segs in/op (acks) | bytes out/op | replica CPU µs/op | lag p50 / p99 ms | converge | sample | replica RSS MiB |
|---|---:|---:|---:|---:|---:|---|---|---|---:|
| base/rc-inline-w2 | 2.83 | 0.354 | 0.230 | 87.0 | 2.68 | 0.08 / 0.73 | ok | ok | 28 |
| final/rc-inline-w2 | 45.34 | 0.022 | 0.022 | 87.0 | 1.13 | 0.40 / 1.34 | ok | ok | 24 |
| base/rc-coalesced-w2 | 2.29 | 0.436 | 0.279 | 87.0 | 2.94 | 0.07 / 0.59 | ok | ok | 27 |
| final/rc-coalesced-w2 | 24.19 | 0.041 | 0.040 | 87.0 | 1.26 | 0.34 / 0.78 | ok | ok | 28 |

## Latency (ms)

| Cell | p50 | p95 | p99 | p99 range | max |
|---|---:|---:|---:|---:|---:|
| base/rc-inline-w2 | 0.375 | 0.615 | 0.719 | 0.711–0.727 | 27.199 |
| final/rc-inline-w2 | 0.359 | 0.583 | 0.695 | 0.679–0.719 | 219.263 |
| base/rc-coalesced-w2 | 0.303 | 0.559 | 0.703 | 0.687–0.703 | 27.663 |
| final/rc-coalesced-w2 | 0.247 | 0.439 | 0.551 | 0.551–0.567 | 80.895 |

## AOF batching and coalescing

| Cell | write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | lock contended/cmd | lead busy/cmd | fanout_busy_skips/cmd |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| base/rc-inline-w2 | 0.9203 | 1.1 | 0.000 | n/a | 0 | 0.000 | 0.0838 | 0.5355 | n/a |
| final/rc-inline-w2 | 0.9494 | 1.1 | 0.000 | n/a | 0 | 0.000 | 0.0864 | 0.4788 | 0.0000 |
| base/rc-coalesced-w2 | 0.3910 | 2.6 | 0.667 | 0.875 | 17,246 | 0.607 | 0.0399 | 0.1793 | n/a |
| final/rc-coalesced-w2 | 0.2780 | 3.6 | 0.746 | 0.930 | 3,509 | 0.656 | 0.0340 | 0.1957 | 0.1446 |

## CPU by thread name (user + sys µs/op, median)

| Cell | jemalloc_bg_thd | rc-base | rc-final | tokio-rt-worker |
|---|---:|---:|---:|---:|
| base/rc-inline-w2 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 3.77 + 8.11 |
| final/rc-inline-w2 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 4.12 + 7.71 |
| base/rc-coalesced-w2 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 3.27 + 6.51 |
| final/rc-coalesced-w2 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 3.23 + 5.05 |
