## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-c1-P1` | 3 | 36,328 | 0% | 12.83 | 3.02 | 9.87 | 7,167 | 13,138 | 0.023 | 0.031 | 0.071 | 25.1 | 0.37 | 0 |
| `new/rc-noaof-set-c1-P1` | 3 | 35,786 | 5% | 13.18 | 2.91 | 10.22 | 7,025 | 12,732 | 0.023 | 0.031 | 0.079 | 25.1 | 0.37 | 0 |
| `off/rc-noaof-set-c1-P1` | 3 | 35,785 | 3% | 12.77 | 2.99 | 9.74 | 6,826 | 13,230 | 0.023 | 0.031 | 0.079 | 25.4 | 0.37 | 0 |
| `base/rc-noaof-set-c50-P1` | 3 | 299,890 | 2% | 5.33 | 1.38 | 3.95 | 5,825 | 7,262 | 0.087 | 0.311 | 0.391 | 26.6 | 1.90 | 0 |
| `new/rc-noaof-set-c50-P1` | 3 | 307,608 | 6% | 5.28 | 1.41 | 3.88 | 6,081 | 7,334 | 0.087 | 0.263 | 0.415 | 27.2 | 1.90 | 0 |
| `off/rc-noaof-set-c50-P1` | 3 | 295,034 | 13% | 5.23 | 1.35 | 3.88 | 5,471 | 7,064 | 0.087 | 0.311 | 0.383 | 27.1 | 1.89 | 0 |
| `base/rc-noaof-set-c50-P16` | 3 | 2,924,832 | 6% | 0.82 | 0.54 | 0.28 | 2,918 | 2,444 | 0.223 | 0.439 | 0.895 | 26.5 | 1.75 | 0 |
| `new/rc-noaof-set-c50-P16` | 3 | 2,924,832 | 2% | 0.82 | 0.55 | 0.27 | 2,955 | 2,463 | 0.175 | 0.463 | 1.183 | 27.1 | 1.70 | 0 |
| `off/rc-noaof-set-c50-P16` | 3 | 2,890,312 | 2% | 0.81 | 0.53 | 0.28 | 2,898 | 2,414 | 0.223 | 0.439 | 0.943 | 27.3 | 1.75 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-c1-P1` | 1 | load=0.98 io_psi10=0.52/0.46 mem_psi10=0.00/0.00 swap_used_mib=7609 dirty_wb_kib=2728 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 2 | load=3.28 io_psi10=0.33/0.31 mem_psi10=0.00/0.00 swap_used_mib=7598 dirty_wb_kib=2028 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 3 | load=2.81 io_psi10=0.38/0.37 mem_psi10=0.00/0.00 swap_used_mib=7598 dirty_wb_kib=2852 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 1 | load=1.11 io_psi10=0.43/0.40 mem_psi10=0.00/0.00 swap_used_mib=7609 dirty_wb_kib=1732 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 2 | load=4.50 io_psi10=0.68/0.37 mem_psi10=0.00/0.00 swap_used_mib=7598 dirty_wb_kib=2936 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 3 | load=2.77 io_psi10=0.17/0.16 mem_psi10=0.00/0.00 swap_used_mib=7598 dirty_wb_kib=1916 settle_wait_s=0 |
| `off/rc-noaof-set-c1-P1` | 1 | load=1.78 io_psi10=1.46/0.81 mem_psi10=0.00/0.00 swap_used_mib=7609 dirty_wb_kib=2432 settle_wait_s=0 |
| `off/rc-noaof-set-c1-P1` | 2 | load=6.75 io_psi10=2.25/0.03 mem_psi10=0.00/0.00 swap_used_mib=7598 dirty_wb_kib=4044 settle_wait_s=0 |
| `off/rc-noaof-set-c1-P1` | 3 | load=2.28 io_psi10=0.24/0.13 mem_psi10=0.00/0.00 swap_used_mib=7598 dirty_wb_kib=3460 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 1 | load=1.68 io_psi10=0.76/0.54 mem_psi10=0.00/0.00 swap_used_mib=7609 dirty_wb_kib=2772 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=6.76 io_psi10=0.21/0.16 mem_psi10=0.00/0.00 swap_used_mib=7598 dirty_wb_kib=3480 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=2.11 io_psi10=2.30/1.44 mem_psi10=0.00/0.00 swap_used_mib=7597 dirty_wb_kib=4212 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 1 | load=3.42 io_psi10=0.35/0.12 mem_psi10=0.00/0.00 swap_used_mib=7609 dirty_wb_kib=2152 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 2 | load=7.01 io_psi10=0.21/0.01 mem_psi10=0.00/0.00 swap_used_mib=7599 dirty_wb_kib=2992 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 3 | load=4.17 io_psi10=0.59/0.34 mem_psi10=0.00/0.00 swap_used_mib=7597 dirty_wb_kib=5744 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P1` | 1 | load=5.00 io_psi10=2.00/0.17 mem_psi10=0.29/0.00 swap_used_mib=7606 dirty_wb_kib=6012 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P1` | 2 | load=6.09 io_psi10=0.45/0.27 mem_psi10=0.00/0.00 swap_used_mib=7599 dirty_wb_kib=2264 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P1` | 3 | load=5.25 io_psi10=0.32/0.18 mem_psi10=0.00/0.00 swap_used_mib=7597 dirty_wb_kib=3440 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 1 | load=5.91 io_psi10=0.80/0.24 mem_psi10=0.01/0.00 swap_used_mib=7606 dirty_wb_kib=2824 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 2 | load=6.21 io_psi10=0.54/0.31 mem_psi10=0.00/0.00 swap_used_mib=7599 dirty_wb_kib=2900 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 3 | load=5.83 io_psi10=0.59/0.21 mem_psi10=0.00/0.00 swap_used_mib=7597 dirty_wb_kib=4240 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 1 | load=5.82 io_psi10=0.20/0.10 mem_psi10=0.00/0.00 swap_used_mib=7606 dirty_wb_kib=1988 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 2 | load=6.19 io_psi10=0.24/0.06 mem_psi10=0.00/0.00 swap_used_mib=7599 dirty_wb_kib=3100 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 3 | load=6.08 io_psi10=0.37/0.23 mem_psi10=0.00/0.00 swap_used_mib=7597 dirty_wb_kib=2980 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 1 | load=6.22 io_psi10=0.22/0.05 mem_psi10=0.00/0.00 swap_used_mib=7600 dirty_wb_kib=3448 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 2 | load=6.08 io_psi10=0.34/0.29 mem_psi10=0.02/0.02 swap_used_mib=7599 dirty_wb_kib=2372 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 3 | load=6.32 io_psi10=0.92/0.64 mem_psi10=0.00/0.00 swap_used_mib=7597 dirty_wb_kib=4556 settle_wait_s=0 |
