## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-everysec-set-c50-P1` | 3 | 131,135 | 0% | 5.48 | 1.80 | 3.65 | 9,613 | 9,240 | 0.359 | 0.479 | 0.511 | 26.0 | 0.95 | 0 |
| `fix/rc-everysec-set-c50-P1` | 3 | 133,324 | 1% | 5.36 | 1.68 | 3.68 | 9,707 | 8,728 | 0.351 | 0.479 | 0.511 | 26.2 | 0.95 | 0 |
| `base/rc-everysec-mset-c50-P1` | 3 | 62,939 | 7% | 11.78 | 6.84 | 4.93 | 42,649 | 30,657 | 0.679 | 0.863 | 0.911 | 26.3 | 0.48 | 0 |
| `fix/rc-everysec-mset-c50-P1` | 3 | 66,094 | 8% | 11.75 | 6.82 | 4.92 | 42,732 | 30,713 | 0.679 | 0.863 | 0.911 | 26.2 | 0.50 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 6.6 | 0.0004 |
| `fix/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 6.2 | 0.0004 |
| `base/rc-everysec-mset-c50-P1` | 0.1000 | 10.0 | 31.5 | 0.0021 |
| `fix/rc-everysec-mset-c50-P1` | 0.1000 | 10.0 | 24.7 | 0.0021 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-everysec-set-c50-P1` | 1 | load=0.79 io_psi10=0.14/0.14 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=1416 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 2 | load=2.69 io_psi10=5.27/0.28 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=3508 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 3 | load=2.86 io_psi10=5.26/0.42 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=6308 settle_wait_s=0 |
| `fix/rc-everysec-set-c50-P1` | 1 | load=1.54 io_psi10=4.81/0.24 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=2260 settle_wait_s=0 |
| `fix/rc-everysec-set-c50-P1` | 2 | load=2.49 io_psi10=6.97/4.03 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2228 settle_wait_s=7 |
| `fix/rc-everysec-set-c50-P1` | 3 | load=2.76 io_psi10=6.03/1.57 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=3004 settle_wait_s=0 |
| `base/rc-everysec-mset-c50-P1` | 1 | load=2.01 io_psi10=7.33/5.39 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=2040 settle_wait_s=8 |
| `base/rc-everysec-mset-c50-P1` | 2 | load=1.76 io_psi10=7.38/5.50 mem_psi10=0.00/0.00 swap_used_mib=5553 dirty_wb_kib=2028 settle_wait_s=9 |
| `base/rc-everysec-mset-c50-P1` | 3 | load=2.91 io_psi10=7.83/2.77 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=7772 settle_wait_s=0 |
| `fix/rc-everysec-mset-c50-P1` | 1 | load=1.82 io_psi10=7.72/4.78 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2628 settle_wait_s=4 |
| `fix/rc-everysec-mset-c50-P1` | 2 | load=1.92 io_psi10=7.14/5.74 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2820 settle_wait_s=11 |
| `fix/rc-everysec-mset-c50-P1` | 3 | load=2.10 io_psi10=7.70/4.96 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2948 settle_wait_s=6 |
