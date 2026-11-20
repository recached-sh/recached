## Throughput and CPU

| Cell | n | requests | rps | rps range | rps (wall) | run s | gen cores | CPU µs/op | user µs/op | sys µs/op | ctx sw/op | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| base/rc-noaof-P1 | 3 | 1,000,000 | 153,822 | 153,822–159,974 | 153,704 | 6.5 | 1.21 | 4.11 | 1.00 | 3.15 | 0.0007 | 26 |
| base/rc-repl-noaof-P1 | 3 | 1,000,000 | 93,006 | 90,893–95,229 | 92,963 | 10.8 | 0.81 | 7.47 | 2.52 | 4.95 | 0.0002 | 61 |
| base/rc-standalone-P1 | 3 | 1,000,000 | 108,073 | 105,230–117,633 | 108,015 | 9.3 | 0.80 | 6.17 | 2.19 | 3.97 | 0.0005 | 26 |
| base/rc-coalesced-norep-P1 | 3 | 1,000,000 | 114,273 | 111,086–117,619 | 114,207 | 8.8 | 0.90 | 6.21 | 2.23 | 4.02 | 0.0004 | 59 |
| base/rc-coalesced-P1 | 3 | 1,000,000 | 93,015 | 86,934–95,220 | 92,971 | 10.8 | 0.77 | 7.59 | 2.63 | 4.94 | 0.0005 | 61 |
| base/rc-noaof-P16 | 3 | 10,000,000 | 1,110,864 | 1,080,847–1,110,988 | 1,110,124 | 9.0 | 0.75 | 0.73 | 0.47 | 0.27 | 0.0000 | 26 |
| base/rc-repl-noaof-P16 | 3 | 10,000,000 | 459,538 | 449,256–459,559 | 459,432 | 21.8 | 0.36 | 1.91 | 1.44 | 0.48 | 0.0000 | 59 |
| base/rc-standalone-P16 | 3 | 10,000,000 | 464,770 | 454,174–470,212 | 464,641 | 21.5 | 0.30 | 1.69 | 1.32 | 0.37 | 0.0002 | 26 |
| base/rc-coalesced-norep-P16 | 3 | 10,000,000 | 327,633 | 309,799–335,942 | 327,579 | 30.5 | 0.26 | 2.42 | 1.75 | 0.67 | 0.0003 | 57 |
| base/rc-coalesced-P16 | 3 | 10,000,000 | 198,847 | 192,160–200,872 | 196,013 | 51.0 | 0.17 | 3.75 | 2.00 | 1.75 | 0.0004 | 59 |

## User-space hardware counters per operation (uprof; kernel excluded)

| Cell | instructions/op | range | cycles/op | IPC | branch misses/op | cache misses/op | L1d read misses/op |
|---|---:|---:|---:|---:|---:|---:|---:|
| base/rc-noaof-P1 | 5,234 | 5,227–5,244 | 5,557 | 0.94 | 13.96 | 31.22 | 184.75 |
| base/rc-repl-noaof-P1 | 13,567 | 13,558–13,586 | 13,545 | 1.00 | 23.63 | 49.92 | 389.64 |
| base/rc-standalone-P1 | 13,846 | 13,845–13,848 | 11,229 | 1.23 | 21.45 | 41.52 | 307.89 |
| base/rc-coalesced-norep-P1 | 13,969 | 13,969–13,969 | 11,599 | 1.20 | 20.38 | 44.85 | 369.11 |
| base/rc-coalesced-P1 | 15,123 | 15,120–15,124 | 13,728 | 1.10 | 28.18 | 51.66 | 424.99 |
| base/rc-noaof-P16 | 2,905 | 2,905–2,905 | 2,151 | 1.35 | 2.06 | 17.18 | 23.99 |
| base/rc-repl-noaof-P16 | 10,758 | 10,758–10,758 | 6,275 | 1.71 | 3.24 | 29.82 | 57.20 |
| base/rc-standalone-P16 | 9,992 | 9,992–9,993 | 5,531 | 1.81 | 2.86 | 23.54 | 56.69 |
| base/rc-coalesced-norep-P16 | 11,607 | 11,606–11,607 | 7,379 | 1.57 | 4.65 | 29.79 | 160.87 |
| base/rc-coalesced-P16 | 12,694 | 12,693–12,697 | 9,325 | 1.36 | 10.28 | 37.53 | 228.53 |

## Replication socket and replica

| Cell | frames/segment out | data segs out/op | data segs in/op (acks) | bytes out/op | replica CPU µs/op | lag p50 / p99 ms | converge | sample | replica RSS MiB |
|---|---:|---:|---:|---:|---:|---|---|---|---:|
| base/rc-repl-noaof-P1 | 3.00 | 0.333 | 0.223 | 87.0 | 2.87 | 0.35 / 1.47 | ok | ok | 24 |
| base/rc-coalesced-P1 | 3.60 | 0.278 | 0.239 | 87.0 | 2.66 | 0.12 / 0.82 | ok | ok | 24 |
| base/rc-repl-noaof-P16 | 28.69 | 0.035 | 0.022 | 87.0 | 0.99 | 0.58 / 1.85 | ok | ok | 24 |
| base/rc-coalesced-P16 | 3.81 | 0.262 | 0.159 | 87.0 | 1.93 | 0.12 / 1.75 | ok | ok | 24 |

## Latency (ms)

| Cell | p50 | p95 | p99 | p99 range | max |
|---|---:|---:|---:|---:|---:|
| base/rc-noaof-P1 | 0.319 | 0.343 | 0.375 | 0.359–0.383 | 3.263 |
| base/rc-repl-noaof-P1 | 0.535 | 0.663 | 0.767 | 0.735–0.815 | 4.511 |
| base/rc-standalone-P1 | 0.399 | 0.543 | 0.591 | 0.575–0.615 | 417.279 |
| base/rc-coalesced-norep-P1 | 0.407 | 0.559 | 0.695 | 0.687–0.711 | 45.599 |
| base/rc-coalesced-P1 | 0.511 | 0.719 | 0.863 | 0.823–0.879 | 82.815 |
| base/rc-noaof-P16 | 0.727 | 0.791 | 0.959 | 0.887–1.023 | 4.695 |
| base/rc-repl-noaof-P16 | 1.775 | 1.959 | 2.159 | 2.119–2.215 | 6.071 |
| base/rc-standalone-P16 | 1.447 | 1.807 | 1.975 | 1.943–2.055 | 410.879 |
| base/rc-coalesced-norep-P16 | 2.119 | 2.767 | 3.111 | 3.087–3.119 | 831.999 |
| base/rc-coalesced-P16 | 3.647 | 4.711 | 5.367 | 5.207–5.487 | 1116.159 |

## AOF batching and coalescing

| Cell | write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | lock contended/cmd | lead busy/cmd | 
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| base/rc-standalone-P1 | 0.0998 | 10.0 | 0.000 | n/a | 0 | 0.000 | 0.0000 | 0.0000 | 
| base/rc-coalesced-norep-P1 | 0.0676 | 14.8 | 0.999 | 0.933 | 21 | 0.932 | 0.0000 | 0.0000 | 
| base/rc-coalesced-P1 | 0.0702 | 14.3 | 0.999 | 0.931 | 27 | 0.930 | 0.0000 | 0.0000 | 
| base/rc-standalone-P16 | 0.0062 | 160.7 | 0.000 | n/a | 0 | 0.000 | 0.0000 | 0.0000 | 
| base/rc-coalesced-norep-P16 | 0.0357 | 28.0 | 1.000 | 0.965 | 67 | 0.964 | 0.0000 | 0.0000 | 
| base/rc-coalesced-P16 | 0.0371 | 26.9 | 1.000 | 0.963 | 80 | 0.963 | 0.0000 | 0.0000 | 

## CPU by thread name (user + sys µs/op, median)

| Cell | jemalloc_bg_thd | rc-base | tokio-rt-worker |
|---|---:|---:|---:|
| base/rc-noaof-P1 | 0.00 + 0.00 | 0.00 + 0.00 | 1.00 + 3.14 |
| base/rc-repl-noaof-P1 | 0.00 + 0.00 | 0.00 + 0.00 | 2.52 + 4.95 |
| base/rc-standalone-P1 | 0.00 + 0.00 | 0.00 + 0.00 | 2.18 + 3.96 |
| base/rc-coalesced-norep-P1 | 0.00 + 0.00 | 0.00 + 0.00 | 2.23 + 4.01 |
| base/rc-coalesced-P1 | 0.00 + 0.00 | 0.00 + 0.00 | 2.63 + 4.94 |
| base/rc-noaof-P16 | 0.00 + 0.00 | 0.00 + 0.00 | 0.47 + 0.27 |
| base/rc-repl-noaof-P16 | 0.00 + 0.00 | 0.00 + 0.00 | 1.44 + 0.48 |
| base/rc-standalone-P16 | 0.00 + 0.00 | 0.00 + 0.00 | 1.32 + 0.37 |
| base/rc-coalesced-norep-P16 | 0.00 + 0.00 | 0.00 + 0.00 | 1.75 + 0.66 |
| base/rc-coalesced-P16 | 0.00 + 0.00 | 0.00 + 0.00 | 2.00 + 1.75 |

## Ratios (rps medians)

| Ratio | value |
|---|---:|
| base/rc-coalesced-P1 / base/rc-standalone-P1 | 0.861 (M1 ≥ 0.85: pass) |
| base/rc-coalesced-P16 / base/rc-standalone-P16 | 0.428 (M1 ≥ 0.85: below) |
