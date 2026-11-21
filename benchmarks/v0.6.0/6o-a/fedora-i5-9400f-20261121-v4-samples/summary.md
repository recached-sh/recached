## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

| cell | n | ops/s | spread | CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `baseprof/rc-everysec-set-P1` | 1 | 114,273 | — | 6.51 | 2.36 | 4.15 | 14,332 | 11,714 | 0.415 | 0.559 | 0.607 | 27.1 | 0.87 | 0 |
| `optprof/rc-everysec-set-P1` | 1 | 117,633 | — | 6.18 | 2.06 | 4.12 | 10,650 | 9,993 | 0.391 | 0.543 | 0.583 | 27.2 | 0.91 | 0 |
| `optprof/rc-everysec-hset-P1` | 1 | 114,247 | — | 6.29 | 1.98 | 4.31 | 11,518 | 10,226 | 0.407 | 0.551 | 0.599 | 25.2 | 0.88 | 0 |
| `optprof/rc-everysec-set-P16` | 1 | 605,694 | — | 1.40 | 1.02 | 0.38 | 6,572 | 4,249 | 1.207 | 1.543 | 1.815 | 26.9 | 0.38 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `baseprof/rc-everysec-set-P1` | 0.0999 | 10.0 | 6.1 | 0.0009 |
| `optprof/rc-everysec-set-P1` | 0.0998 | 10.0 | 5.8 | 0.0008 |
| `optprof/rc-everysec-hset-P1` | 0.1000 | 10.0 | 6.9 | 0.0008 |
| `optprof/rc-everysec-set-P16` | 0.0062 | 160.6 | 28.2 | 0.0003 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `baseprof/rc-everysec-set-P1` | 1 | load=3.15 io_psi10=1.27/1.14 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=5248 settle_wait_s=0 |
| `optprof/rc-everysec-set-P1` | 1 | load=2.99 io_psi10=5.97/3.37 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=3048 settle_wait_s=0 |
| `optprof/rc-everysec-hset-P1` | 1 | load=2.85 io_psi10=7.77/4.54 mem_psi10=0.00/0.00 swap_used_mib=3900 dirty_wb_kib=4668 settle_wait_s=0 |
| `optprof/rc-everysec-set-P16` | 1 | load=2.89 io_psi10=6.58/3.25 mem_psi10=0.00/0.00 swap_used_mib=3900 dirty_wb_kib=3800 settle_wait_s=0 |
