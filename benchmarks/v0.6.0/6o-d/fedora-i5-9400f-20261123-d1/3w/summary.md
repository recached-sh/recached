## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P1` | 3 | 359,957 | 1% | 5.40 | 1.36 | 4.08 | 5,399 | 6,940 | 0.079 | 0.103 | 0.151 | 26.1 | 2.17 | 0 |
| `fix/rc-noaof-get-c50-P1` | 3 | 359,957 | 2% | 5.40 | 1.38 | 4.01 | 5,498 | 7,177 | 0.079 | 0.103 | 0.151 | 26.2 | 2.17 | 0 |
| `base/rc-noaof-mget-c50-P1` | 3 | 312,989 | 2% | 7.11 | 3.26 | 3.86 | 14,186 | 15,838 | 0.103 | 0.151 | 0.183 | 26.0 | 2.25 | 0 |
| `fix/rc-noaof-mget-c50-P1` | 3 | 313,016 | 2% | 7.16 | 3.28 | 3.85 | 14,431 | 15,949 | 0.103 | 0.151 | 0.191 | 26.2 | 2.25 | 0 |
| `base/rc-noaof-mset-c50-P1` | 3 | 257,124 | 0% | 9.50 | 5.66 | 3.83 | 29,973 | 26,450 | 0.167 | 0.311 | 0.383 | 26.5 | 1.90 | 0 |
| `fix/rc-noaof-mset-c50-P1` | 3 | 257,088 | 0% | 9.49 | 5.73 | 3.76 | 30,369 | 26,513 | 0.167 | 0.311 | 0.383 | 27.0 | 1.92 | 0 |
| `base/rc-noaof-get-c50-P16` | 3 | 3,783,580 | 1% | 0.65 | 0.39 | 0.25 | 2,222 | 1,812 | 0.199 | 0.295 | 0.335 | 26.1 | 2.29 | 0 |
| `fix/rc-noaof-get-c50-P16` | 3 | 3,684,017 | 1% | 0.66 | 0.41 | 0.26 | 2,321 | 1,881 | 0.199 | 0.303 | 0.343 | 26.5 | 2.24 | 0 |
| `base/rc-noaof-mget-c50-P16` | 3 | 874,863 | 3% | 3.22 | 2.81 | 0.41 | 11,109 | 11,525 | 0.871 | 1.351 | 1.559 | 26.1 | 1.80 | 0 |
| `fix/rc-noaof-mget-c50-P16` | 3 | 910,421 | 1% | 3.10 | 2.70 | 0.40 | 11,362 | 11,085 | 0.839 | 1.295 | 1.487 | 26.4 | 1.91 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P1` | 1 | load=0.98 io_psi10=2.73/2.73 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=4848 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 2 | load=5.32 io_psi10=0.10/0.10 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=3348 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 3 | load=5.55 io_psi10=0.36/0.35 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=2420 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 1 | load=2.64 io_psi10=1.02/0.92 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=2592 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 2 | load=4.96 io_psi10=1.26/1.26 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1996 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 3 | load=5.47 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=3348 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 1 | load=3.79 io_psi10=0.32/0.31 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1808 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 2 | load=4.66 io_psi10=0.22/0.22 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=2884 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 3 | load=5.85 io_psi10=1.10/0.42 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=2564 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 1 | load=4.28 io_psi10=0.29/0.29 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1972 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 2 | load=4.51 io_psi10=0.54/0.52 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1812 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 3 | load=5.40 io_psi10=0.58/0.26 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1864 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 1 | load=4.18 io_psi10=2.03/2.03 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1668 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 2 | load=5.11 io_psi10=1.80/1.72 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1768 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 3 | load=5.50 io_psi10=2.31/2.10 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1620 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P1` | 1 | load=4.58 io_psi10=0.50/0.50 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1844 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P1` | 2 | load=5.39 io_psi10=0.97/0.79 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=3040 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P1` | 3 | load=5.45 io_psi10=0.51/0.46 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1772 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 1 | load=4.90 io_psi10=2.18/2.18 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=3176 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 2 | load=5.14 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1788 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 3 | load=5.01 io_psi10=0.18/0.16 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=3660 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P16` | 1 | load=5.15 io_psi10=0.86/0.86 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1896 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P16` | 2 | load=5.01 io_psi10=0.16/0.15 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=2080 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P16` | 3 | load=4.98 io_psi10=0.54/0.54 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1928 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 1 | load=5.32 io_psi10=0.20/0.20 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1684 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 2 | load=4.83 io_psi10=0.05/0.04 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=2168 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 3 | load=4.64 io_psi10=0.13/0.13 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=2192 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P16` | 1 | load=4.84 io_psi10=1.48/1.48 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=3204 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P16` | 2 | load=5.18 io_psi10=0.27/0.24 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=1688 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P16` | 3 | load=4.72 io_psi10=1.36/0.54 mem_psi10=0.00/0.00 swap_used_mib=5462 dirty_wb_kib=2680 settle_wait_s=0 |
