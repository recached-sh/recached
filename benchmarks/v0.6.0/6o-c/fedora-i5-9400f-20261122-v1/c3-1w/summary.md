## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P1` | 3 | 186,909 | 2% | 3.50 | 0.80 | 2.69 | 4,849 | 4,591 | 0.255 | 0.343 | 0.375 | 26.1 | 1.25 | 0 |
| `c3/rc-noaof-get-c50-P1` | 3 | 192,300 | 3% | 3.38 | 0.73 | 2.66 | 4,654 | 4,312 | 0.247 | 0.335 | 0.359 | 25.2 | 1.28 | 0 |
| `base/rc-noaof-set-c50-P1` | 3 | 183,473 | 3% | 3.62 | 0.89 | 2.73 | 5,562 | 5,041 | 0.255 | 0.351 | 0.375 | 26.4 | 1.24 | 0 |
| `c3/rc-noaof-set-c50-P1` | 3 | 185,178 | 2% | 3.59 | 0.86 | 2.72 | 5,365 | 4,930 | 0.255 | 0.351 | 0.375 | 26.1 | 1.24 | 0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | 181,805 | 1% | 3.65 | 0.87 | 2.79 | 6,316 | 4,945 | 0.263 | 0.351 | 0.383 | 342.8 | 1.19 | 0 |
| `c3/rc-noaof-lpush-c50-P1` | 3 | 185,171 | 1% | 3.58 | 0.84 | 2.74 | 6,112 | 4,796 | 0.255 | 0.351 | 0.375 | 342.5 | 1.21 | 0 |
| `base/rc-noaof-sadd-c50-P1` | 3 | 176,966 | 3% | 3.79 | 1.04 | 2.81 | 6,555 | 5,774 | 0.271 | 0.367 | 0.391 | 19.3 | 1.20 | 0 |
| `c3/rc-noaof-sadd-c50-P1` | 3 | 180,167 | 1% | 3.71 | 0.97 | 2.75 | 6,358 | 5,452 | 0.263 | 0.359 | 0.383 | 18.9 | 1.21 | 0 |
| `base/rc-noaof-mset-c50-P1` | 3 | 104,149 | 1% | 7.73 | 4.32 | 3.39 | 29,773 | 20,777 | 0.463 | 0.607 | 0.639 | 26.7 | 0.76 | 0 |
| `c3/rc-noaof-mset-c50-P1` | 3 | 105,245 | 1% | 7.65 | 4.34 | 3.30 | 29,569 | 20,785 | 0.455 | 0.599 | 0.639 | 26.2 | 0.78 | 0 |
| `base/rc-noaof-set-c50-P16` | 3 | 1,281,788 | 0% | 0.66 | 0.43 | 0.23 | 2,929 | 1,993 | 0.591 | 0.767 | 0.823 | 26.4 | 0.72 | 0 |
| `c3/rc-noaof-set-c50-P16` | 3 | 1,281,788 | 1% | 0.66 | 0.43 | 0.23 | 2,911 | 1,998 | 0.591 | 0.767 | 0.823 | 26.6 | 0.72 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P1` | 1 | load=0.79 io_psi10=0.10/0.10 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1416 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 2 | load=2.62 io_psi10=0.29/0.28 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2096 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 3 | load=2.45 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1988 settle_wait_s=0 |
| `c3/rc-noaof-get-c50-P1` | 1 | load=1.31 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2244 settle_wait_s=0 |
| `c3/rc-noaof-get-c50-P1` | 2 | load=2.52 io_psi10=0.00/0.00 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2540 settle_wait_s=0 |
| `c3/rc-noaof-get-c50-P1` | 3 | load=2.55 io_psi10=0.22/0.22 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2068 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 1 | load=1.60 io_psi10=0.27/0.27 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1724 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=2.63 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2052 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=2.63 io_psi10=0.31/0.31 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2852 settle_wait_s=0 |
| `c3/rc-noaof-set-c50-P1` | 1 | load=2.08 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1816 settle_wait_s=0 |
| `c3/rc-noaof-set-c50-P1` | 2 | load=2.50 io_psi10=0.24/0.24 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1668 settle_wait_s=0 |
| `c3/rc-noaof-set-c50-P1` | 3 | load=2.78 io_psi10=0.17/0.17 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2580 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 1 | load=2.31 io_psi10=0.00/0.00 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2960 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | load=2.42 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2060 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | load=2.58 io_psi10=0.10/0.10 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2292 settle_wait_s=0 |
| `c3/rc-noaof-lpush-c50-P1` | 1 | load=2.36 io_psi10=0.70/0.67 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2564 settle_wait_s=0 |
| `c3/rc-noaof-lpush-c50-P1` | 2 | load=2.57 io_psi10=0.18/0.18 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1964 settle_wait_s=0 |
| `c3/rc-noaof-lpush-c50-P1` | 3 | load=2.43 io_psi10=2.74/2.74 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2624 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 1 | load=2.31 io_psi10=0.16/0.16 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1832 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 2 | load=2.33 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2664 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 3 | load=2.43 io_psi10=0.23/0.22 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2588 settle_wait_s=0 |
| `c3/rc-noaof-sadd-c50-P1` | 1 | load=2.48 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2528 settle_wait_s=0 |
| `c3/rc-noaof-sadd-c50-P1` | 2 | load=2.04 io_psi10=0.15/0.15 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2740 settle_wait_s=0 |
| `c3/rc-noaof-sadd-c50-P1` | 3 | load=2.51 io_psi10=1.84/1.84 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2588 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 1 | load=2.45 io_psi10=0.28/0.28 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2096 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 2 | load=1.95 io_psi10=0.24/0.23 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2484 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 3 | load=2.59 io_psi10=0.21/0.21 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1976 settle_wait_s=0 |
| `c3/rc-noaof-mset-c50-P1` | 1 | load=1.89 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2040 settle_wait_s=0 |
| `c3/rc-noaof-mset-c50-P1` | 2 | load=1.95 io_psi10=0.46/0.46 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2124 settle_wait_s=0 |
| `c3/rc-noaof-mset-c50-P1` | 3 | load=2.52 io_psi10=0.09/0.09 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1896 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 1 | load=1.94 io_psi10=0.39/0.39 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1936 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 2 | load=1.80 io_psi10=0.23/0.23 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1844 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 3 | load=2.45 io_psi10=0.31/0.31 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2080 settle_wait_s=0 |
| `c3/rc-noaof-set-c50-P16` | 1 | load=1.95 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1900 settle_wait_s=0 |
| `c3/rc-noaof-set-c50-P16` | 2 | load=1.91 io_psi10=0.62/0.60 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1484 settle_wait_s=0 |
| `c3/rc-noaof-set-c50-P16` | 3 | load=1.97 io_psi10=0.26/0.26 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2068 settle_wait_s=0 |
