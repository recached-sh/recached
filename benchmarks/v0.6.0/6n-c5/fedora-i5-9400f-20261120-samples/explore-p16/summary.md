## Throughput and CPU

| Cell | n | requests | rps | rps range | run s | gen cores | CPU µs/op | user µs/op | sys µs/op | ctx sw/op | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| prof/rc-repl-noaof-P16 | 1 | 10,000,000 | 449,256 | 449,256–449,256 | 22.3 | 0.35 | 1.96 | 1.48 | 0.48 | 0.0001 | 60 |
| prof/rc-coalesced-norep-P16 | 1 | 10,000,000 | 317,209 | 317,209–317,209 | 31.5 | 0.27 | 2.49 | 1.81 | 0.68 | 0.0003 | 58 |
| prof/rc-coalesced-P16 | 1 | 10,000,000 | 198,874 | 198,874–198,874 | 50.3 | 0.18 | 3.85 | 2.07 | 1.78 | 0.0005 | 60 |

## User-space hardware counters per operation (uprof; kernel excluded)

| Cell | instructions/op | range | cycles/op | IPC | branch misses/op | cache misses/op | L1d read misses/op |
|---|---:|---:|---:|---:|---:|---:|---:|
| prof/rc-repl-noaof-P16 | 11,010 | 11,010–11,010 | 6,412 | 1.72 | 2.90 | 29.70 | 61.59 |
| prof/rc-coalesced-norep-P16 | 11,904 | 11,904–11,904 | 7,562 | 1.57 | 4.70 | 31.57 | 164.33 |
| prof/rc-coalesced-P16 | 13,011 | 13,011–13,011 | 9,579 | 1.36 | 9.99 | 39.69 | 233.78 |

## Replication socket and replica

| Cell | frames/segment out | data segs out/op | data segs in/op (acks) | bytes out/op | replica CPU µs/op | lag p50 / p99 ms | converge | sample | replica RSS MiB |
|---|---:|---:|---:|---:|---:|---|---|---|---:|
| prof/rc-repl-noaof-P16 | 28.75 | 0.035 | 0.022 | 87.0 | 0.99 | 0.59 / 2.77 | ok | ok | 25 |
| prof/rc-coalesced-P16 | 3.80 | 0.263 | 0.157 | 87.0 | 1.97 | 0.11 / 2.25 | ok | ok | 24 |

## Latency (ms)

| Cell | p50 | p95 | p99 | p99 range | max |
|---|---:|---:|---:|---:|---:|
| prof/rc-repl-noaof-P16 | 1.823 | 2.015 | 2.223 | 2.223–2.223 | 5.599 |
| prof/rc-coalesced-norep-P16 | 2.183 | 2.871 | 3.303 | 3.303–3.303 | 690.175 |
| prof/rc-coalesced-P16 | 3.759 | 4.863 | 5.631 | 5.631–5.631 | 568.831 |

## AOF batching and coalescing

| Cell | write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | lock contended/cmd | lead busy/cmd | 
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| prof/rc-coalesced-norep-P16 | 0.0358 | 28.0 | 1.000 | 0.965 | 87 | 0.964 | 0.0000 | 0.0000 | 
| prof/rc-coalesced-P16 | 0.0372 | 26.9 | 1.000 | 0.963 | 87 | 0.963 | 0.0000 | 0.0000 | 

## CPU by thread name (user + sys µs/op, median)

| Cell | jemalloc_bg_thd | rc-base-prof | tokio-rt-worker |
|---|---:|---:|---:|
| prof/rc-repl-noaof-P16 | 0.00 + 0.00 | 0.00 + 0.00 | 1.48 + 0.48 |
| prof/rc-coalesced-norep-P16 | 0.00 + 0.00 | 0.00 + 0.00 | 1.81 + 0.68 |
| prof/rc-coalesced-P16 | 0.00 + 0.00 | 0.00 + 0.00 | 2.07 + 1.78 |
