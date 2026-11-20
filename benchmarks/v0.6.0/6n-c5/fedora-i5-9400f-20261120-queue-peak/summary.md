## Throughput and CPU

| Cell | n | requests | rps | rps range | rps (wall) | run s | gen cores | CPU µs/op | user µs/op | sys µs/op | ctx sw/op | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| old/rc-repl-noaof-P16 | 3 | 10,000,000 | 454,360 | 454,339–459,622 | 454,256 | 22.0 | 0.36 | 1.92 | 1.43 | 0.48 | 0.0000 | 59 |
| new/rc-repl-noaof-P16 | 3 | 10,000,000 | 475,986 | 475,964–475,986 | 475,851 | 21.0 | 0.38 | 1.89 | 1.50 | 0.39 | 0.0000 | 59 |
| old/rc-repl-noaof-P64 | 3 | 10,000,000 | 579,341 | 570,972–596,481 | 579,139 | 17.3 | 0.20 | 1.63 | 1.45 | 0.17 | 0.0000 | 60 |
| new/rc-repl-noaof-P64 | 3 | 10,000,000 | 587,820 | 587,786–596,588 | 587,613 | 17.0 | 0.20 | 1.62 | 1.49 | 0.13 | 0.0000 | 59 |
| old/rc-coalesced-P16 | 3 | 10,000,000 | 210,389 | 209,293–214,947 | 210,362 | 47.5 | 0.18 | 3.76 | 2.02 | 1.74 | 0.0004 | 59 |
| new/rc-coalesced-P16 | 3 | 10,000,000 | 254,576 | 251,383–257,945 | 253,043 | 39.5 | 0.21 | 2.93 | 2.01 | 0.92 | 0.0003 | 59 |

## User-space hardware counters per operation (uprof; kernel excluded)

| Cell | instructions/op | range | cycles/op | IPC | branch misses/op | cache misses/op | L1d read misses/op |
|---|---:|---:|---:|---:|---:|---:|---:|
| old/rc-repl-noaof-P16 | 10,760 | 10,760–10,760 | 6,291 | 1.71 | 2.80 | 29.66 | 53.93 |
| new/rc-repl-noaof-P16 | 10,753 | 10,753–10,753 | 6,381 | 1.69 | 2.52 | 29.48 | 53.70 |
| old/rc-repl-noaof-P64 | 10,612 | 10,611–10,612 | 5,924 | 1.79 | 1.72 | 28.58 | 33.80 |
| new/rc-repl-noaof-P64 | 10,608 | 10,607–10,608 | 5,968 | 1.78 | 1.97 | 29.43 | 34.81 |
| old/rc-coalesced-P16 | 12,699 | 12,698–12,701 | 9,449 | 1.34 | 9.56 | 34.99 | 228.24 |
| new/rc-coalesced-P16 | 12,582 | 12,582–12,583 | 8,643 | 1.46 | 7.19 | 32.13 | 185.74 |

## Replication socket and replica

| Cell | frames/segment out | data segs out/op | data segs in/op (acks) | bytes out/op | replica CPU µs/op | lag p50 / p99 ms | converge | sample | replica RSS MiB |
|---|---:|---:|---:|---:|---:|---|---|---|---:|
| old/rc-repl-noaof-P16 | 28.33 | 0.035 | 0.023 | 87.0 | 0.99 | 0.58 / 1.81 | ok | ok | 24 |
| new/rc-repl-noaof-P16 | 398.55 | 0.003 | 0.002 | 87.0 | 0.82 | 1.15 / 2.59 | ok | ok | 24 |
| old/rc-repl-noaof-P64 | 85.06 | 0.012 | 0.009 | 87.0 | 0.86 | 0.35 / 4.38 | ok | ok | 24 |
| new/rc-repl-noaof-P64 | 470.92 | 0.002 | 0.002 | 87.0 | 0.81 | 1.09 / 5.29 | ok | ok | 24 |
| old/rc-coalesced-P16 | 3.82 | 0.262 | 0.160 | 87.0 | 1.93 | 0.11 / 0.67 | ok | ok | 24 |
| new/rc-coalesced-P16 | 30.12 | 0.033 | 0.032 | 87.0 | 1.15 | 0.34 / 0.78 | ok | ok | 24 |

## Latency (ms)

| Cell | p50 | p95 | p99 | p99 range | max |
|---|---:|---:|---:|---:|---:|
| old/rc-repl-noaof-P16 | 1.799 | 1.967 | 2.087 | 2.079–2.119 | 4.367 |
| new/rc-repl-noaof-P16 | 1.695 | 1.807 | 1.951 | 1.935–1.959 | 5.295 |
| old/rc-repl-noaof-P64 | 5.631 | 5.919 | 6.367 | 6.359–6.439 | 8.415 |
| new/rc-repl-noaof-P64 | 5.503 | 5.783 | 6.191 | 6.175–6.263 | 9.799 |
| old/rc-coalesced-P16 | 3.663 | 4.687 | 5.191 | 5.151–5.263 | 514.815 |
| new/rc-coalesced-P16 | 2.631 | 3.351 | 3.727 | 3.727–3.727 | 1283.071 |

## AOF batching and coalescing

| Cell | write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | lock contended/cmd | lead busy/cmd | 
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| old/rc-coalesced-P16 | 0.0370 | 27.0 | 1.000 | 0.963 | 37 | 0.963 | 0.0000 | 0.0000 | 
| new/rc-coalesced-P16 | 0.0407 | 24.6 | 0.996 | 0.963 | 1,223 | 0.959 | 0.0000 | 0.0000 | 

## CPU by thread name (user + sys µs/op, median)

| Cell | jemalloc_bg_thd | rc-pr1 | rc-pr2 | tokio-rt-worker |
|---|---:|---:|---:|---:|
| old/rc-repl-noaof-P16 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 1.43 + 0.48 |
| new/rc-repl-noaof-P16 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 1.50 + 0.39 |
| old/rc-repl-noaof-P64 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 1.45 + 0.17 |
| new/rc-repl-noaof-P64 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 1.49 + 0.13 |
| old/rc-coalesced-P16 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 2.02 + 1.74 |
| new/rc-coalesced-P16 | 0.00 + 0.00 | 0.00 + 0.00 | 0.00 + 0.00 | 2.01 + 0.92 |
