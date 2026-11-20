## Throughput and CPU

| Cell | n | requests | rps | rps range | rps (wall) | run s | gen cores | CPU µs/op | user µs/op | sys µs/op | ctx sw/op | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| pr2/rc-inline-w2 | 5 | 1,000,000 | 128,999 | 124,984–133,298 | 128,932 | 7.8 | 1.01 | 11.80 | 4.09 | 7.73 | 0.0059 | 62 |
| pr2/rc-coalesced-w2 | 5 | 1,000,000 | 159,923 | 137,817–166,583 | 159,795 | 6.3 | 1.12 | 9.18 | 3.52 | 5.67 | 0.0033 | 65 |
| nosync/rc-inline-w4 | 5 | 1,000,000 | 129,016 | 128,932–133,316 | 128,932 | 7.8 | 1.02 | 11.60 | 4.09 | 7.58 | 0.0216 | 64 |
| nosync/rc-coalesced-w4 | 5 | 1,000,000 | 159,974 | 159,949–160,000 | 159,847 | 6.3 | 1.15 | 9.34 | 3.58 | 5.75 | 0.0166 | 67 |

## User-space hardware counters per operation (uprof; kernel excluded)

| Cell | instructions/op | range | cycles/op | IPC | branch misses/op | cache misses/op | L1d read misses/op |
|---|---:|---:|---:|---:|---:|---:|---:|
| pr2/rc-inline-w2 | 15,584 | 15,570–15,590 | 19,475 | 0.80 | 30.74 | 50.81 | 474.99 |
| pr2/rc-coalesced-w2 | 15,421 | 15,413–15,450 | 17,132 | 0.90 | 29.10 | 47.64 | 463.49 |
| nosync/rc-inline-w4 | 15,508 | 15,500–15,530 | 19,285 | 0.81 | 31.37 | 57.52 | 469.71 |
| nosync/rc-coalesced-w4 | 15,455 | 15,439–15,466 | 17,257 | 0.89 | 31.08 | 54.04 | 461.40 |

## Replication socket and replica

| Cell | frames/segment out | data segs out/op | data segs in/op (acks) | bytes out/op | replica CPU µs/op | lag p50 / p99 ms | converge | sample | replica RSS MiB |
|---|---:|---:|---:|---:|---:|---|---|---|---:|
| pr2/rc-inline-w2 | 42.95 | 0.023 | 0.022 | 87.0 | 1.22 | 0.41 / 1.82 | ok | ok | 24 |
| pr2/rc-coalesced-w2 | 24.35 | 0.041 | 0.038 | 87.0 | 1.32 | 0.36 / 1.74 | ok | ok | 28 |
| nosync/rc-inline-w4 | 59.67 | 0.017 | 0.016 | 87.0 | 1.15 | 0.45 / 4.94 | ok | ok | 24 |
| nosync/rc-coalesced-w4 | 33.45 | 0.030 | 0.027 | 87.0 | 1.22 | 0.42 / 4.19 | ok | ok | 28 |

## Latency (ms)

| Cell | p50 | p95 | p99 | p99 range | max |
|---|---:|---:|---:|---:|---:|
| pr2/rc-inline-w2 | 0.351 | 0.583 | 0.727 | 0.703–0.735 | 29.343 |
| pr2/rc-coalesced-w2 | 0.263 | 0.487 | 0.647 | 0.631–0.655 | 231.551 |
| nosync/rc-inline-w4 | 0.343 | 0.631 | 0.911 | 0.895–0.927 | 8.599 |
| nosync/rc-coalesced-w4 | 0.239 | 0.543 | 2.071 | 2.007–2.103 | 11.047 |

## AOF batching and coalescing

| Cell | write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | lock contended/cmd | lead busy/cmd | 
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| pr2/rc-inline-w2 | 0.9520 | 1.1 | 0.000 | n/a | 0 | 0.000 | 0.0829 | 0.4448 | 
| pr2/rc-coalesced-w2 | 0.3946 | 2.5 | 0.675 | 0.874 | 15,464 | 0.603 | 0.0392 | 0.1623 | 
| nosync/rc-inline-w4 | 0.8956 | 1.1 | 0.000 | n/a | 0 | 0.000 | 0.0778 | 0.4645 | 
| nosync/rc-coalesced-w4 | 0.4153 | 2.4 | 0.645 | 0.861 | 16,402 | 0.557 | 0.0417 | 0.1872 | 

## CPU by thread name (user + sys µs/op, median)

| Cell | jemalloc_bg_thd | rc-pr2 | tokio-rt-worker |
|---|---:|---:|---:|
| pr2/rc-inline-w2 | 0.00 + 0.00 | 0.00 + 0.00 | 4.09 + 7.72 |
| pr2/rc-coalesced-w2 | 0.00 + 0.00 | 0.00 + 0.00 | 3.52 + 5.66 |
| nosync/rc-inline-w4 | 0.00 + 0.00 | 0.00 + 0.00 | 4.10 + 7.59 |
| nosync/rc-coalesced-w4 | 0.00 + 0.00 | 0.00 + 0.00 | 3.58 + 5.75 |
