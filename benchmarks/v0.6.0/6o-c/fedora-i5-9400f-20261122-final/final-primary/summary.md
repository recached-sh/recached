## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-c50-P1` | 5 | 184,610 | 2% | 3.61 | 0.89 | 2.72 | 5,559 | 5,093 | 0.255 | 0.351 | 0.375 | 25.9 | 1.24 | 0 |
| `final/rc-noaof-set-c50-P1` | 5 | 184,604 | 2% | 3.59 | 0.89 | 2.72 | 5,412 | 4,981 | 0.255 | 0.351 | 0.375 | 27.5 | 1.24 | 0 |
| `redis-noaof-set-c50-P1` | 5 | 199,980 | 2% | 3.29 | 0.72 | 2.57 | 4,744 | 3,710 | 0.231 | 0.295 | 0.415 | 19.4 | 1.27 | 0 |
| `base/rc-noaof-lpush-c50-P1` | 5 | 181,813 | 4% | 3.65 | 0.86 | 2.74 | 6,309 | 4,941 | 0.263 | 0.359 | 0.383 | 389.3 | 1.21 | 0 |
| `final/rc-noaof-lpush-c50-P1` | 5 | 190,458 | 2% | 3.44 | 0.73 | 2.70 | 5,460 | 4,289 | 0.247 | 0.335 | 0.367 | 391.1 | 1.25 | 0 |
| `redis-noaof-lpush-c50-P1` | 5 | 205,114 | 1% | 3.16 | 0.59 | 2.56 | 5,139 | 3,191 | 0.223 | 0.287 | 0.407 | 211.5 | 1.26 | 0 |
| `base/rc-noaof-sadd-c50-P1` | 5 | 176,465 | 4% | 3.81 | 1.02 | 2.80 | 6,557 | 5,748 | 0.271 | 0.367 | 0.391 | 18.5 | 1.19 | 0 |
| `final/rc-noaof-sadd-c50-P1` | 5 | 183,195 | 3% | 3.64 | 0.89 | 2.74 | 5,630 | 5,127 | 0.263 | 0.351 | 0.383 | 20.2 | 1.21 | 0 |
| `redis-noaof-sadd-c50-P1` | 5 | 205,100 | 3% | 3.15 | 0.55 | 2.60 | 4,448 | 3,190 | 0.223 | 0.279 | 0.399 | 12.1 | 1.29 | 0 |
| `base/rc-noaof-mset-c50-P1` | 5 | 103,438 | 2% | 7.72 | 4.31 | 3.39 | 29,770 | 20,865 | 0.463 | 0.607 | 0.639 | 26.1 | 0.76 | 0 |
| `final/rc-noaof-mset-c50-P1` | 5 | 103,438 | 1% | 7.77 | 4.40 | 3.38 | 29,577 | 21,171 | 0.463 | 0.607 | 0.647 | 27.4 | 0.76 | 0 |
| `redis-noaof-mset-c50-P1` | 5 | 126,295 | 1% | 6.17 | 2.83 | 3.33 | 20,315 | 14,752 | 0.351 | 0.503 | 0.583 | 19.6 | 0.87 | 0 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.923** | 0.915 0.909 0.930 0.930 0.917 |
| `final/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.923** | 0.902 0.923 0.930 0.923 0.931 |
| `base/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.886** | 0.873 0.886 0.900 0.915 0.886 |
| `final/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.929** | 0.929 0.929 0.914 0.929 0.929 |
| `base/rc-noaof-sadd-c50-P1` / `redis-noaof-sadd-c50-P1` | **0.860** | 0.880 0.846 0.848 0.847 0.881 |
| `final/rc-noaof-sadd-c50-P1` / `redis-noaof-sadd-c50-P1` | **0.893** | 0.907 0.898 0.886 0.886 0.894 |
| `base/rc-noaof-mset-c50-P1` / `redis-noaof-mset-c50-P1` | **0.819** | 0.810 0.819 0.833 0.819 0.826 |
| `final/rc-noaof-mset-c50-P1` / `redis-noaof-mset-c50-P1` | **0.819** | 0.810 0.826 0.819 0.819 0.826 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` | 1 | load=0.83 io_psi10=0.28/0.28 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1976 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=2.58 io_psi10=0.09/0.09 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2204 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=2.69 io_psi10=0.17/0.17 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2464 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 4 | load=2.61 io_psi10=0.58/0.50 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1668 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 5 | load=2.84 io_psi10=0.07/0.06 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2816 settle_wait_s=0 |
| `final/rc-noaof-set-c50-P1` | 1 | load=1.79 io_psi10=0.54/0.54 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1704 settle_wait_s=0 |
| `final/rc-noaof-set-c50-P1` | 2 | load=2.67 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1736 settle_wait_s=0 |
| `final/rc-noaof-set-c50-P1` | 3 | load=2.55 io_psi10=2.07/2.07 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2292 settle_wait_s=0 |
| `final/rc-noaof-set-c50-P1` | 4 | load=2.60 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2864 settle_wait_s=0 |
| `final/rc-noaof-set-c50-P1` | 5 | load=2.56 io_psi10=0.38/0.38 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2308 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 1 | load=2.33 io_psi10=0.19/0.19 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1604 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 2 | load=2.45 io_psi10=0.19/0.17 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2308 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 3 | load=2.69 io_psi10=0.34/0.33 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1880 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 4 | load=2.66 io_psi10=0.13/0.13 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2024 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 5 | load=2.58 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2852 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 1 | load=2.46 io_psi10=0.06/0.04 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1920 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | load=2.57 io_psi10=0.39/0.02 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2512 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | load=2.81 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2400 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 4 | load=2.88 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2048 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 5 | load=2.55 io_psi10=0.18/0.18 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2140 settle_wait_s=0 |
| `final/rc-noaof-lpush-c50-P1` | 1 | load=2.40 io_psi10=0.16/0.15 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1968 settle_wait_s=0 |
| `final/rc-noaof-lpush-c50-P1` | 2 | load=2.29 io_psi10=0.29/0.29 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1824 settle_wait_s=0 |
| `final/rc-noaof-lpush-c50-P1` | 3 | load=2.84 io_psi10=1.21/1.21 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2220 settle_wait_s=0 |
| `final/rc-noaof-lpush-c50-P1` | 4 | load=2.70 io_psi10=0.26/0.26 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2224 settle_wait_s=0 |
| `final/rc-noaof-lpush-c50-P1` | 5 | load=2.59 io_psi10=0.07/0.02 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2808 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 1 | load=2.66 io_psi10=0.30/0.29 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1776 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 2 | load=2.54 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2364 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 3 | load=2.78 io_psi10=0.37/0.34 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2724 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 4 | load=2.60 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2200 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 5 | load=2.63 io_psi10=0.26/0.25 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2260 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 1 | load=2.74 io_psi10=0.34/0.34 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1844 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 2 | load=2.42 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2256 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 3 | load=2.87 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2364 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 4 | load=2.29 io_psi10=0.38/0.38 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2072 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 5 | load=2.62 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1440 settle_wait_s=0 |
| `final/rc-noaof-sadd-c50-P1` | 1 | load=2.74 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1972 settle_wait_s=0 |
| `final/rc-noaof-sadd-c50-P1` | 2 | load=2.40 io_psi10=0.55/0.54 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1712 settle_wait_s=0 |
| `final/rc-noaof-sadd-c50-P1` | 3 | load=2.71 io_psi10=0.33/0.32 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1932 settle_wait_s=0 |
| `final/rc-noaof-sadd-c50-P1` | 4 | load=2.38 io_psi10=0.20/0.19 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=3380 settle_wait_s=0 |
| `final/rc-noaof-sadd-c50-P1` | 5 | load=2.50 io_psi10=0.20/0.20 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2524 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 1 | load=2.56 io_psi10=0.21/0.21 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2548 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 2 | load=2.10 io_psi10=0.04/0.01 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2824 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 3 | load=2.84 io_psi10=0.40/0.40 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2124 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 4 | load=2.15 io_psi10=0.68/0.68 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2312 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 5 | load=2.50 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2140 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 1 | load=2.73 io_psi10=0.23/0.23 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2104 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 2 | load=2.02 io_psi10=0.34/0.33 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2288 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 3 | load=2.90 io_psi10=0.41/0.40 mem_psi10=0.09/0.08 swap_used_mib=4678 dirty_wb_kib=2436 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 4 | load=2.27 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2100 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 5 | load=2.64 io_psi10=0.20/0.20 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2944 settle_wait_s=0 |
| `final/rc-noaof-mset-c50-P1` | 1 | load=2.48 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2252 settle_wait_s=0 |
| `final/rc-noaof-mset-c50-P1` | 2 | load=2.06 io_psi10=0.45/0.45 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=2432 settle_wait_s=0 |
| `final/rc-noaof-mset-c50-P1` | 3 | load=2.47 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=4678 dirty_wb_kib=2204 settle_wait_s=0 |
| `final/rc-noaof-mset-c50-P1` | 4 | load=2.28 io_psi10=0.36/0.33 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2388 settle_wait_s=0 |
| `final/rc-noaof-mset-c50-P1` | 5 | load=2.46 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2568 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 1 | load=2.33 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=4664 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 2 | load=2.13 io_psi10=0.43/0.41 mem_psi10=0.00/0.00 swap_used_mib=4695 dirty_wb_kib=1892 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 3 | load=2.18 io_psi10=0.18/0.16 mem_psi10=0.00/0.00 swap_used_mib=4678 dirty_wb_kib=2404 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 4 | load=2.07 io_psi10=0.09/0.09 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2200 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 5 | load=2.31 io_psi10=0.24/0.24 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2824 settle_wait_s=0 |
