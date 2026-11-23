## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-everysec-set-c50-P1` | 3 | 131,066 | 3% | 5.48 | 1.81 | 3.67 | 9,588 | 9,266 | 0.351 | 0.479 | 0.511 | 26.1 | 0.94 | 0 |
| `fix/rc-everysec-set-c50-P1` | 3 | 131,139 | 3% | 5.42 | 1.76 | 3.66 | 9,613 | 9,053 | 0.351 | 0.479 | 0.519 | 26.0 | 0.94 | 0 |
| `base/rc-everysec-mset-c50-P1` | 3 | 61,958 | 1% | 11.72 | 6.72 | 4.96 | 42,279 | 30,448 | 0.679 | 0.863 | 0.911 | 26.3 | 0.48 | 0 |
| `fix/rc-everysec-mset-c50-P1` | 3 | 62,465 | 2% | 11.69 | 6.75 | 4.92 | 42,651 | 30,471 | 0.679 | 0.863 | 0.911 | 26.4 | 0.48 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 7.6 | 0.0005 |
| `fix/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 7.1 | 0.0004 |
| `base/rc-everysec-mset-c50-P1` | 0.1000 | 10.0 | 34.0 | 0.0022 |
| `fix/rc-everysec-mset-c50-P1` | 0.1000 | 10.0 | 32.6 | 0.0023 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-everysec-set-c50-P1` | 1 | load=0.86 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=5462 dirty_wb_kib=820 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 2 | load=2.37 io_psi10=7.89/0.76 mem_psi10=0.00/0.00 swap_used_mib=5461 dirty_wb_kib=4168 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 3 | load=2.20 io_psi10=7.82/4.50 mem_psi10=0.00/0.00 swap_used_mib=5461 dirty_wb_kib=3968 settle_wait_s=8 |
| `fix/rc-everysec-set-c50-P1` | 1 | load=1.44 io_psi10=7.93/0.86 mem_psi10=0.00/0.00 swap_used_mib=5462 dirty_wb_kib=3236 settle_wait_s=0 |
| `fix/rc-everysec-set-c50-P1` | 2 | load=2.06 io_psi10=7.66/5.05 mem_psi10=0.00/0.00 swap_used_mib=5461 dirty_wb_kib=5860 settle_wait_s=8 |
| `fix/rc-everysec-set-c50-P1` | 3 | load=2.60 io_psi10=6.76/2.18 mem_psi10=0.00/0.00 swap_used_mib=5461 dirty_wb_kib=3108 settle_wait_s=5 |
| `base/rc-everysec-mset-c50-P1` | 1 | load=1.68 io_psi10=7.98/0.96 mem_psi10=0.00/0.00 swap_used_mib=5462 dirty_wb_kib=3736 settle_wait_s=0 |
| `base/rc-everysec-mset-c50-P1` | 2 | load=1.64 io_psi10=7.08/3.94 mem_psi10=0.00/0.00 swap_used_mib=5462 dirty_wb_kib=7932 settle_wait_s=6 |
| `base/rc-everysec-mset-c50-P1` | 3 | load=2.21 io_psi10=7.73/4.30 mem_psi10=0.00/0.00 swap_used_mib=5461 dirty_wb_kib=4788 settle_wait_s=8 |
| `fix/rc-everysec-mset-c50-P1` | 1 | load=1.69 io_psi10=7.70/4.39 mem_psi10=0.00/0.00 swap_used_mib=5462 dirty_wb_kib=7576 settle_wait_s=7 |
| `fix/rc-everysec-mset-c50-P1` | 2 | load=1.81 io_psi10=7.08/4.56 mem_psi10=0.00/0.00 swap_used_mib=5462 dirty_wb_kib=6656 settle_wait_s=9 |
| `fix/rc-everysec-mset-c50-P1` | 3 | load=1.94 io_psi10=7.16/5.00 mem_psi10=0.00/0.00 swap_used_mib=5461 dirty_wb_kib=8336 settle_wait_s=10 |
