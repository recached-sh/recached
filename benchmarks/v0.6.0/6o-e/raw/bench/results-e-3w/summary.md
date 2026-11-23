## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P1` | 3 | 359,942 | 2% | 5.39 | 1.37 | 4.03 | 5,482 | 7,031 | 0.079 | 0.111 | 0.183 | 26.4 | 2.16 | 0 |
| `fix/rc-noaof-get-c50-P1` | 3 | 359,957 | 4% | 5.38 | 1.33 | 4.05 | 5,506 | 6,961 | 0.079 | 0.111 | 0.183 | 26.0 | 2.17 | 0 |
| `base/rc-noaof-mget-c50-P1` | 3 | 313,016 | 0% | 7.16 | 3.31 | 3.84 | 14,409 | 16,090 | 0.103 | 0.167 | 0.215 | 26.1 | 2.23 | 0 |
| `fix/rc-noaof-mget-c50-P1` | 3 | 312,989 | 0% | 7.13 | 3.27 | 3.89 | 14,427 | 16,017 | 0.103 | 0.167 | 0.215 | 26.1 | 2.23 | 0 |
| `base/rc-noaof-get-c50-P16` | 3 | 3,683,822 | 1% | 0.67 | 0.41 | 0.26 | 2,322 | 1,877 | 0.199 | 0.311 | 0.351 | 26.1 | 2.21 | 0 |
| `fix/rc-noaof-get-c50-P16` | 3 | 3,636,175 | 3% | 0.67 | 0.41 | 0.26 | 2,327 | 1,893 | 0.199 | 0.311 | 0.359 | 26.3 | 2.22 | 0 |
| `base/rc-noaof-mget-c50-P16` | 3 | 895,828 | 2% | 3.13 | 2.73 | 0.40 | 11,362 | 11,211 | 0.847 | 1.311 | 1.519 | 26.1 | 1.83 | 0 |
| `fix/rc-noaof-mget-c50-P16` | 3 | 903,080 | 1% | 3.12 | 2.71 | 0.41 | 11,367 | 11,158 | 0.847 | 1.311 | 1.511 | 26.1 | 1.88 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P1` | 1 | load=0.82 io_psi10=0.35/0.35 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1436 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 2 | load=5.66 io_psi10=0.18/0.18 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=2772 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 3 | load=5.45 io_psi10=0.36/0.36 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=2416 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 1 | load=2.54 io_psi10=0.37/0.07 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2144 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 2 | load=5.48 io_psi10=0.73/0.66 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=2036 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 3 | load=5.75 io_psi10=0.25/0.14 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=2524 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 1 | load=3.73 io_psi10=0.14/0.11 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2728 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 2 | load=5.33 io_psi10=1.60/1.60 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=2260 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 3 | load=5.76 io_psi10=0.40/0.38 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=1828 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 1 | load=4.32 io_psi10=1.90/1.84 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1840 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 2 | load=5.40 io_psi10=0.45/0.42 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=2504 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 3 | load=5.81 io_psi10=0.18/0.18 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=1808 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 1 | load=4.58 io_psi10=0.79/0.78 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1604 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 2 | load=5.37 io_psi10=1.16/0.92 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=2092 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 3 | load=5.53 io_psi10=1.92/1.47 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=3080 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P16` | 1 | load=4.67 io_psi10=0.25/0.25 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2600 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P16` | 2 | load=5.02 io_psi10=0.97/0.14 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=3800 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P16` | 3 | load=5.67 io_psi10=0.73/0.64 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=1924 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 1 | load=4.83 io_psi10=0.81/0.78 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1996 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 2 | load=4.70 io_psi10=0.04/0.03 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2152 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 3 | load=5.70 io_psi10=1.81/1.79 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=2416 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P16` | 1 | load=4.68 io_psi10=1.73/0.84 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2088 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P16` | 2 | load=4.62 io_psi10=0.28/0.24 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1936 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P16` | 3 | load=5.13 io_psi10=0.24/0.23 mem_psi10=0.00/0.00 swap_used_mib=5547 dirty_wb_kib=2456 settle_wait_s=0 |
