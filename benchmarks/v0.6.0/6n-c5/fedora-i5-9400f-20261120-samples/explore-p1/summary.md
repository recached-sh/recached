## Throughput and CPU

| Cell | n | requests | rps | rps range | run s | gen cores | CPU µs/op | user µs/op | sys µs/op | ctx sw/op | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| prof/rc-noaof-P1 | 1 | 1,000,000 | 153,822 | 153,822–153,822 | 6.5 | 1.19 | 4.15 | 1.08 | 3.07 | 0.0003 | 27 |
| prof/rc-repl-noaof-P1 | 1 | 1,000,000 | 95,229 | 95,229–95,229 | 10.5 | 0.82 | 7.31 | 2.58 | 4.73 | 0.0001 | 64 |
| prof/rc-standalone-P1 | 1 | 1,000,000 | 114,260 | 114,260–114,260 | 8.8 | 0.85 | 6.35 | 2.34 | 4.01 | 0.0006 | 27 |
| prof/rc-coalesced-norep-P1 | 1 | 1,000,000 | 105,208 | 105,208–105,208 | 9.5 | 0.82 | 6.45 | 2.45 | 4.00 | 0.0005 | 59 |
| prof/rc-coalesced-P1 | 1 | 1,000,000 | 90,893 | 90,893–90,893 | 11.0 | 0.75 | 7.67 | 2.74 | 4.93 | 0.0005 | 62 |

## User-space hardware counters per operation (uprof; kernel excluded)

| Cell | instructions/op | range | cycles/op | IPC | branch misses/op | cache misses/op | L1d read misses/op |
|---|---:|---:|---:|---:|---:|---:|---:|
| prof/rc-noaof-P1 | 5,415 | 5,415–5,415 | 5,675 | 0.95 | 14.54 | 23.22 | 193.87 |
| prof/rc-repl-noaof-P1 | 13,890 | 13,890–13,890 | 13,148 | 1.06 | 22.77 | 39.45 | 390.43 |
| prof/rc-standalone-P1 | 14,253 | 14,253–14,253 | 11,617 | 1.23 | 20.70 | 41.23 | 314.11 |
| prof/rc-coalesced-norep-P1 | 14,367 | 14,367–14,367 | 11,942 | 1.20 | 21.94 | 55.28 | 380.85 |
| prof/rc-coalesced-P1 | 15,538 | 15,538–15,538 | 13,683 | 1.14 | 27.27 | 56.31 | 429.02 |

## Replication socket and replica

| Cell | frames/segment out | data segs out/op | data segs in/op (acks) | bytes out/op | replica CPU µs/op | lag p50 / p99 ms | converge | sample | replica RSS MiB |
|---|---:|---:|---:|---:|---:|---|---|---|---:|
| prof/rc-repl-noaof-P1 | 3.22 | 0.310 | 0.207 | 87.0 | 2.70 | 0.35 / 0.69 | ok | ok | 25 |
| prof/rc-coalesced-P1 | 3.58 | 0.279 | 0.231 | 87.0 | 2.62 | 0.14 / 1.14 | ok | ok | 25 |

## Latency (ms)

| Cell | p50 | p95 | p99 | p99 range | max |
|---|---:|---:|---:|---:|---:|
| prof/rc-noaof-P1 | 0.327 | 0.343 | 0.359 | 0.359–0.359 | 2.663 |
| prof/rc-repl-noaof-P1 | 0.535 | 0.591 | 0.727 | 0.727–0.727 | 3.207 |
| prof/rc-standalone-P1 | 0.399 | 0.551 | 0.631 | 0.631–0.631 | 24.591 |
| prof/rc-coalesced-norep-P1 | 0.415 | 0.583 | 0.767 | 0.767–0.767 | 321.023 |
| prof/rc-coalesced-P1 | 0.511 | 0.719 | 0.871 | 0.871–0.871 | 256.895 |

## AOF batching and coalescing

| Cell | write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | lock contended/cmd | lead busy/cmd | 
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| prof/rc-standalone-P1 | 0.1002 | 10.0 | 0.000 | n/a | 0 | 0.000 | 0.0000 | 0.0000 | 
| prof/rc-coalesced-norep-P1 | 0.0697 | 14.3 | 0.998 | 0.932 | 73 | 0.930 | 0.0000 | 0.0000 | 
| prof/rc-coalesced-P1 | 0.0703 | 14.2 | 0.999 | 0.931 | 34 | 0.930 | 0.0000 | 0.0000 | 

## CPU by thread name (user + sys µs/op, median)

| Cell | jemalloc_bg_thd | rc-base-prof | tokio-rt-worker |
|---|---:|---:|---:|
| prof/rc-noaof-P1 | 0.00 + 0.00 | 0.00 + 0.00 | 1.08 + 3.08 |
| prof/rc-repl-noaof-P1 | 0.00 + 0.00 | 0.00 + 0.00 | 2.58 + 4.73 |
| prof/rc-standalone-P1 | 0.00 + 0.00 | 0.00 + 0.00 | 2.34 + 4.00 |
| prof/rc-coalesced-norep-P1 | 0.00 + 0.00 | 0.00 + 0.00 | 2.45 + 3.99 |
| prof/rc-coalesced-P1 | 0.00 + 0.00 | 0.00 + 0.00 | 2.74 + 4.91 |

## Ratios (rps medians)

| Ratio | value |
|---|---:|
| prof/rc-coalesced-P1 / prof/rc-standalone-P1 | 0.795 (M1 ≥ 0.85: below) |
