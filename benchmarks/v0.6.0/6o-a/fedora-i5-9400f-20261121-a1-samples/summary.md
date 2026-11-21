## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

| cell | n | ops/s | spread | CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `prof/rc-everysec-set-P1` | 1 | 111,099 | — | 6.67 | 2.54 | 4.13 | 14,332 | 11,840 | 0.415 | 0.575 | 0.639 | 27.0 | 0.86 | 0 |
| `prof/rc-noaof-set-P1` | 1 | 148,126 | — | 4.60 | 1.16 | 3.44 | 5,453 | 5,977 | 0.343 | 0.367 | 0.415 | 26.9 | 1.19 | 0 |
| `prof/rc-everysec-incr-P1` | 1 | 111,086 | — | 6.73 | 2.53 | 4.20 | 14,933 | 12,175 | 0.423 | 0.575 | 0.631 | 28.5 | 0.86 | 0 |
| `prof/rc-everysec-hset-P1` | 1 | 105,241 | — | 7.01 | 2.69 | 4.32 | 16,605 | 12,831 | 0.439 | 0.599 | 0.655 | 25.5 | 0.82 | 0 |
| `prof/rc-everysec-set-P16` | 1 | 444,010 | — | 1.78 | 1.39 | 0.40 | 10,253 | 5,686 | 1.519 | 1.919 | 2.247 | 26.9 | 0.30 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `prof/rc-everysec-set-P1` | 0.1000 | 10.0 | 6.2 | 0.0010 |
| `prof/rc-everysec-incr-P1` | 0.0999 | 10.0 | 6.0 | 0.0010 |
| `prof/rc-everysec-hset-P1` | 0.0999 | 10.0 | 6.7 | 0.0012 |
| `prof/rc-everysec-set-P16` | 0.0062 | 161.4 | 61.2 | 0.0004 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `prof/rc-everysec-set-P1` | 1 | load=2.29 io_psi10=53.13/47.50 mem_psi10=0.18/0.18 swap_used_mib=1348 dirty_wb_kib=10432 |
| `prof/rc-noaof-set-P1` | 1 | load=3.45 io_psi10=25.49/18.13 mem_psi10=0.02/0.02 swap_used_mib=1348 dirty_wb_kib=8796 |
| `prof/rc-everysec-incr-P1` | 1 | load=3.36 io_psi10=16.02/13.15 mem_psi10=0.00/0.00 swap_used_mib=1348 dirty_wb_kib=7320 |
| `prof/rc-everysec-hset-P1` | 1 | load=3.55 io_psi10=14.63/10.10 mem_psi10=0.00/0.00 swap_used_mib=1348 dirty_wb_kib=10448 |
| `prof/rc-everysec-set-P16` | 1 | load=3.66 io_psi10=15.38/9.13 mem_psi10=0.00/0.00 swap_used_mib=1348 dirty_wb_kib=13224 |
