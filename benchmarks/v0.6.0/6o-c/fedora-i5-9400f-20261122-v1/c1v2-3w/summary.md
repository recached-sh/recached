## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-c1-P1` | 5 | 39,519 | 0% | 12.52 | 2.79 | 9.62 | 7,056 | 12,251 | 0.023 | 0.031 | 0.039 | 25.4 | 0.38 | 0 |
| `new/rc-noaof-set-c1-P1` | 5 | 39,957 | 2% | 11.82 | 2.58 | 9.29 | 6,898 | 11,550 | 0.023 | 0.023 | 0.031 | 24.9 | 0.39 | 0 |
| `off/rc-noaof-set-c1-P1` | 5 | 39,519 | 1% | 11.90 | 2.69 | 9.31 | 6,887 | 12,135 | 0.023 | 0.023 | 0.031 | 25.1 | 0.39 | 0 |
| `base/rc-noaof-set-c50-P16` | 5 | 3,293,808 | 2% | 0.77 | 0.53 | 0.25 | 2,950 | 2,360 | 0.199 | 0.383 | 0.479 | 27.2 | 2.01 | 0 |
| `new/rc-noaof-set-c50-P16` | 5 | 3,218,095 | 2% | 0.78 | 0.52 | 0.26 | 2,897 | 2,341 | 0.239 | 0.359 | 0.415 | 26.4 | 1.99 | 0 |
| `off/rc-noaof-set-c50-P16` | 5 | 3,181,384 | 3% | 0.79 | 0.52 | 0.27 | 2,897 | 2,354 | 0.239 | 0.367 | 0.423 | 26.4 | 1.99 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-c1-P1` | 1 | load=0.97 io_psi10=1.01/1.01 mem_psi10=0.02/0.02 swap_used_mib=4669 dirty_wb_kib=2832 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 2 | load=2.88 io_psi10=0.23/0.23 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2256 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 3 | load=2.39 io_psi10=0.54/0.52 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1732 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 4 | load=3.11 io_psi10=0.16/0.16 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2476 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 5 | load=2.62 io_psi10=0.50/0.48 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2068 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 1 | load=0.98 io_psi10=0.76/0.76 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1772 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 2 | load=3.74 io_psi10=0.20/0.19 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2760 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 3 | load=1.99 io_psi10=0.14/0.14 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2580 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 4 | load=3.95 io_psi10=0.65/0.63 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2648 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 5 | load=2.21 io_psi10=0.14/0.14 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2004 settle_wait_s=0 |
| `off/rc-noaof-set-c1-P1` | 1 | load=0.97 io_psi10=0.21/0.21 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2016 settle_wait_s=0 |
| `off/rc-noaof-set-c1-P1` | 2 | load=4.95 io_psi10=0.62/0.46 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1572 settle_wait_s=0 |
| `off/rc-noaof-set-c1-P1` | 3 | load=1.71 io_psi10=2.48/2.48 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2660 settle_wait_s=0 |
| `off/rc-noaof-set-c1-P1` | 4 | load=5.50 io_psi10=0.25/0.12 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2568 settle_wait_s=0 |
| `off/rc-noaof-set-c1-P1` | 5 | load=1.86 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2740 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 1 | load=1.05 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2136 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 2 | load=4.73 io_psi10=0.14/0.13 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1972 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 3 | load=1.81 io_psi10=0.34/0.34 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2176 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 4 | load=5.24 io_psi10=0.53/0.50 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1840 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 5 | load=1.69 io_psi10=0.59/0.59 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2156 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 1 | load=2.20 io_psi10=0.52/0.50 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1944 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 2 | load=4.17 io_psi10=0.64/0.55 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2420 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 3 | load=3.14 io_psi10=0.18/0.18 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1708 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 4 | load=5.23 io_psi10=0.21/0.15 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2728 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 5 | load=2.54 io_psi10=1.65/1.61 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2420 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 1 | load=3.24 io_psi10=0.10/0.10 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2172 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 2 | load=3.87 io_psi10=0.14/0.14 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2084 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 3 | load=4.10 io_psi10=0.54/0.39 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2092 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 4 | load=5.17 io_psi10=0.12/0.10 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2852 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 5 | load=3.30 io_psi10=0.69/0.66 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2600 settle_wait_s=0 |
