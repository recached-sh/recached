## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-everysec-lpush-c50-P1` | 5 | 132,223 | 2% | 5.46 | 1.67 | 3.79 | — | — | 0.359 | 0.487 | 0.519 | 268.5 | 0.94 | 0 |
| `gcap/rc-everysec-lpush-c50-P1` | 5 | 130,069 | 2% | 5.59 | 1.77 | 3.81 | — | — | 0.359 | 0.487 | 0.527 | 268.3 | 0.92 | 0 |
| `base/rc-everysec-mset-c50-P1` | 5 | 57,096 | 7% | 11.81 | 6.76 | 5.04 | — | — | 0.687 | 0.879 | 0.935 | 26.2 | 0.44 | 0 |
| `gcap/rc-everysec-mset-c50-P1` | 5 | 55,515 | 8% | 11.96 | 6.96 | 5.01 | — | — | 0.695 | 0.887 | 0.935 | 26.1 | 0.44 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-lpush-c50-P1` | 0.1000 | 10.0 | 5.8 | 0.0006 |
| `gcap/rc-everysec-lpush-c50-P1` | 0.1000 | 10.0 | 6.0 | 0.0006 |
| `base/rc-everysec-mset-c50-P1` | 0.1000 | 10.0 | 45.6 | 0.0034 |
| `gcap/rc-everysec-mset-c50-P1` | 0.1000 | 10.0 | 50.2 | 0.0034 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-everysec-lpush-c50-P1` | 1 | load=1.32 io_psi10=4.09/3.90 mem_psi10=0.33/0.33 swap_used_mib=5218 dirty_wb_kib=4052 settle_wait_s=0 |
| `base/rc-everysec-lpush-c50-P1` | 2 | load=1.95 io_psi10=7.08/0.33 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=6804 settle_wait_s=0 |
| `base/rc-everysec-lpush-c50-P1` | 3 | load=2.30 io_psi10=7.56/1.97 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=2380 settle_wait_s=2 |
| `base/rc-everysec-lpush-c50-P1` | 4 | load=2.61 io_psi10=7.43/1.04 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=1576 settle_wait_s=0 |
| `base/rc-everysec-lpush-c50-P1` | 5 | load=2.90 io_psi10=7.77/0.13 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=7388 settle_wait_s=0 |
| `gcap/rc-everysec-lpush-c50-P1` | 1 | load=2.00 io_psi10=7.91/3.28 mem_psi10=0.05/0.05 swap_used_mib=5218 dirty_wb_kib=3100 settle_wait_s=2 |
| `gcap/rc-everysec-lpush-c50-P1` | 2 | load=1.47 io_psi10=7.53/4.43 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=4364 settle_wait_s=9 |
| `gcap/rc-everysec-lpush-c50-P1` | 3 | load=2.61 io_psi10=7.07/0.15 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=9932 settle_wait_s=0 |
| `gcap/rc-everysec-lpush-c50-P1` | 4 | load=2.14 io_psi10=6.86/3.93 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=5648 settle_wait_s=11 |
| `gcap/rc-everysec-lpush-c50-P1` | 5 | load=2.88 io_psi10=7.87/0.11 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=1820 settle_wait_s=0 |
| `base/rc-everysec-mset-c50-P1` | 1 | load=1.78 io_psi10=6.86/2.63 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=916 settle_wait_s=6 |
| `base/rc-everysec-mset-c50-P1` | 2 | load=1.57 io_psi10=7.20/5.18 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=6180 settle_wait_s=13 |
| `base/rc-everysec-mset-c50-P1` | 3 | load=2.59 io_psi10=7.58/0.08 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=1648 settle_wait_s=0 |
| `base/rc-everysec-mset-c50-P1` | 4 | load=3.43 io_psi10=7.89/5.19 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=4972 settle_wait_s=11 |
| `base/rc-everysec-mset-c50-P1` | 5 | load=2.75 io_psi10=7.28/0.85 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=3676 settle_wait_s=0 |
| `gcap/rc-everysec-mset-c50-P1` | 1 | load=1.54 io_psi10=7.30/4.40 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=6132 settle_wait_s=8 |
| `gcap/rc-everysec-mset-c50-P1` | 2 | load=1.68 io_psi10=7.80/5.18 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=6756 settle_wait_s=12 |
| `gcap/rc-everysec-mset-c50-P1` | 3 | load=2.39 io_psi10=6.89/3.79 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=4248 settle_wait_s=9 |
| `gcap/rc-everysec-mset-c50-P1` | 4 | load=1.85 io_psi10=6.80/4.08 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=6148 settle_wait_s=10 |
| `gcap/rc-everysec-mset-c50-P1` | 5 | load=2.14 io_psi10=7.07/4.71 mem_psi10=0.00/0.00 swap_used_mib=5218 dirty_wb_kib=5116 settle_wait_s=11 |
