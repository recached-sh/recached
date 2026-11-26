## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-always-set-c50-P1` | 5 | 365 | 4% | 18.42 | 4.20 | 14.40 | — | — | 141.695 | 175.103 | 266.751 | 23.5 | 0.01 | 0 |
| `base/rc-always-incrvol-c50-P1` | 5 | 363 | 7% | 19.09 | 4.60 | 14.60 | — | — | 149.887 | 174.975 | 241.663 | 18.1 | 0.01 | 0 |
| `h/rc-always-set-c50-P1` | 5 | 368 | 9% | 18.84 | 4.60 | 14.00 | — | — | 141.695 | 174.975 | 233.471 | 23.7 | 0.01 | 0 |
| `h/rc-always-incrvol-c50-P1` | 5 | 358 | 10% | 21.88 | 6.60 | 15.20 | — | — | 149.887 | 191.487 | 316.671 | 32.9 | 0.01 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-always-set-c50-P1` | 0.0400 | 25.0 | 13.5 | 0.2921 |
| `base/rc-always-incrvol-c50-P1` | 0.0400 | 25.0 | 12.8 | 0.2839 |
| `h/rc-always-set-c50-P1` | 0.0400 | 25.0 | 13.2 | 0.2876 |
| `h/rc-always-incrvol-c50-P1` | 0.0402 | 39.3 | 14.6 | 0.3184 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-always-set-c50-P1` | 1 | load=1.18 io_psi10=0.86/0.86 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=1252 settle_wait_s=0 |
| `base/rc-always-set-c50-P1` | 2 | load=0.76 io_psi10=7.47/7.45 mem_psi10=0.00/0.00 swap_used_mib=8181 dirty_wb_kib=2232 settle_wait_s=29 |
| `base/rc-always-set-c50-P1` | 3 | load=0.68 io_psi10=7.11/7.08 mem_psi10=0.00/0.00 swap_used_mib=8181 dirty_wb_kib=1668 settle_wait_s=30 |
| `base/rc-always-set-c50-P1` | 4 | load=0.69 io_psi10=6.63/6.59 mem_psi10=0.00/0.00 swap_used_mib=8179 dirty_wb_kib=2588 settle_wait_s=26 |
| `base/rc-always-set-c50-P1` | 5 | load=0.60 io_psi10=6.88/6.84 mem_psi10=0.00/0.00 swap_used_mib=8179 dirty_wb_kib=1600 settle_wait_s=30 |
| `base/rc-always-incrvol-c50-P1` | 1 | load=0.82 io_psi10=6.75/6.73 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=3336 settle_wait_s=29 |
| `base/rc-always-incrvol-c50-P1` | 2 | load=0.60 io_psi10=7.27/7.26 mem_psi10=0.00/0.00 swap_used_mib=8181 dirty_wb_kib=1492 settle_wait_s=34 |
| `base/rc-always-incrvol-c50-P1` | 3 | load=0.64 io_psi10=6.94/6.85 mem_psi10=0.00/0.00 swap_used_mib=8181 dirty_wb_kib=2116 settle_wait_s=29 |
| `base/rc-always-incrvol-c50-P1` | 4 | load=0.68 io_psi10=6.91/6.86 mem_psi10=0.00/0.00 swap_used_mib=8179 dirty_wb_kib=1312 settle_wait_s=27 |
| `base/rc-always-incrvol-c50-P1` | 5 | load=0.74 io_psi10=6.75/6.67 mem_psi10=0.00/0.00 swap_used_mib=8179 dirty_wb_kib=2440 settle_wait_s=29 |
| `h/rc-always-set-c50-P1` | 1 | load=0.70 io_psi10=6.95/6.92 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=1384 settle_wait_s=27 |
| `h/rc-always-set-c50-P1` | 2 | load=0.83 io_psi10=7.62/7.58 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=3272 settle_wait_s=28 |
| `h/rc-always-set-c50-P1` | 3 | load=0.80 io_psi10=7.15/7.04 mem_psi10=0.00/0.00 swap_used_mib=8180 dirty_wb_kib=1200 settle_wait_s=29 |
| `h/rc-always-set-c50-P1` | 4 | load=0.94 io_psi10=7.08/6.96 mem_psi10=0.06/0.06 swap_used_mib=8179 dirty_wb_kib=2656 settle_wait_s=26 |
| `h/rc-always-set-c50-P1` | 5 | load=0.82 io_psi10=7.25/7.15 mem_psi10=0.00/0.00 swap_used_mib=8179 dirty_wb_kib=1132 settle_wait_s=30 |
| `h/rc-always-incrvol-c50-P1` | 1 | load=0.71 io_psi10=7.56/7.55 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=3432 settle_wait_s=27 |
| `h/rc-always-incrvol-c50-P1` | 2 | load=0.90 io_psi10=6.58/6.56 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=1368 settle_wait_s=29 |
| `h/rc-always-incrvol-c50-P1` | 3 | load=0.69 io_psi10=6.95/6.93 mem_psi10=0.00/0.00 swap_used_mib=8180 dirty_wb_kib=2604 settle_wait_s=30 |
| `h/rc-always-incrvol-c50-P1` | 4 | load=0.67 io_psi10=7.77/7.74 mem_psi10=0.00/0.00 swap_used_mib=8180 dirty_wb_kib=1200 settle_wait_s=26 |
| `h/rc-always-incrvol-c50-P1` | 5 | load=0.71 io_psi10=6.84/6.76 mem_psi10=0.00/0.00 swap_used_mib=8179 dirty_wb_kib=1456 settle_wait_s=31 |
