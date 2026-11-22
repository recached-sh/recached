## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P1` | 3 | 188,651 | 2% | 3.47 | 0.78 | 2.68 | 4,858 | 4,500 | 0.255 | 0.343 | 0.367 | 25.3 | 1.26 | 0 |
| `final/rc-noaof-get-c50-P1` | 3 | 190,462 | 0% | 3.41 | 0.74 | 2.67 | 4,701 | 4,302 | 0.247 | 0.335 | 0.367 | 26.4 | 1.27 | 0 |
| `redis-noaof-get-c50-P1` | 3 | 206,177 | 0% | 3.16 | 0.58 | 2.58 | 3,956 | 3,285 | 0.223 | 0.287 | 0.407 | 19.0 | 1.27 | 0 |
| `base/rc-noaof-incr-c50-P1` | 3 | 185,178 | 2% | 3.57 | 0.88 | 2.70 | 5,525 | 5,016 | 0.255 | 0.343 | 0.375 | 27.4 | 1.24 | 0 |
| `final/rc-noaof-incr-c50-P1` | 3 | 185,151 | 2% | 3.60 | 0.89 | 2.72 | 5,382 | 5,121 | 0.255 | 0.351 | 0.375 | 29.2 | 1.22 | 0 |
| `redis-noaof-incr-c50-P1` | 3 | 206,169 | 0% | 3.15 | 0.55 | 2.59 | 4,280 | 3,210 | 0.223 | 0.279 | 0.399 | 14.0 | 1.29 | 0 |
| `base/rc-noaof-zadd-c50-P1` | 3 | 159,990 | 1% | 4.40 | 1.49 | 2.91 | 11,045 | 8,032 | 0.295 | 0.407 | 0.431 | 18.1 | 1.09 | 0 |
| `final/rc-noaof-zadd-c50-P1` | 3 | 158,720 | 2% | 4.44 | 1.52 | 2.92 | 10,874 | 8,258 | 0.303 | 0.407 | 0.431 | 19.4 | 1.09 | 0 |
| `redis-noaof-zadd-c50-P1` | 3 | 153,827 | 3% | 4.74 | 1.89 | 2.93 | 7,921 | 9,413 | 0.287 | 0.415 | 0.535 | 18.6 | 0.97 | 0 |
| `base/rc-noaof-set-c1-P1` | 3 | 39,517 | 1% | 12.07 | 2.58 | 9.50 | 6,893 | 11,691 | 0.023 | 0.023 | 0.031 | 23.4 | 0.39 | 0 |
| `final/rc-noaof-set-c1-P1` | 3 | 39,515 | 1% | 12.31 | 2.78 | 9.53 | 6,602 | 12,780 | 0.023 | 0.023 | 0.031 | 24.6 | 0.38 | 0 |
| `base/rc-noaof-set-c50-P16` | 3 | 1,281,723 | 1% | 0.66 | 0.43 | 0.23 | 2,929 | 2,002 | 0.591 | 0.775 | 0.823 | 26.1 | 0.72 | 0 |
| `final/rc-noaof-set-c50-P16` | 3 | 1,204,703 | 5% | 0.68 | 0.42 | 0.25 | 2,890 | 2,013 | 0.679 | 0.719 | 0.783 | 27.4 | 0.78 | 0 |
| `base/rc-noaof-mset-c50-P16` | 3 | 214,758 | 1% | 4.46 | 4.14 | 0.32 | 27,178 | 16,908 | 3.631 | 4.479 | 4.647 | 26.7 | 0.35 | 0 |
| `final/rc-noaof-mset-c50-P16` | 3 | 212,684 | 1% | 4.50 | 4.09 | 0.40 | 27,091 | 16,962 | 3.863 | 3.991 | 4.327 | 27.9 | 0.24 | 0 |
| `base/rc-noaof-lpush-c50-P16` | 3 | 1,265,374 | 3% | 0.67 | 0.41 | 0.25 | 3,716 | 1,927 | 0.583 | 0.759 | 0.799 | 1,568.2 | 0.64 | 0 |
| `final/rc-noaof-lpush-c50-P16` | 3 | 1,333,049 | 4% | 0.60 | 0.33 | 0.27 | 2,978 | 1,601 | 0.591 | 0.631 | 0.655 | 1,569.8 | 0.80 | 0 |
| `base/rc-noaof-sadd-c50-P16` | 3 | 1,110,815 | 1% | 0.78 | 0.56 | 0.22 | 4,015 | 2,488 | 0.695 | 0.887 | 0.935 | 18.7 | 0.61 | 0 |
| `final/rc-noaof-sadd-c50-P16` | 3 | 1,190,363 | 1% | 0.69 | 0.44 | 0.25 | 3,204 | 2,093 | 0.679 | 0.735 | 0.783 | 20.0 | 0.76 | 0 |
| `redis-noaof-set-c50-P16` | 3 | 1,175,972 | 0% | 0.74 | 0.56 | 0.17 | 3,987 | 2,311 | 0.639 | 0.799 | 0.871 | 19.4 | 0.68 | 0 |
| `redis-noaof-mset-c50-P16` | 3 | 273,463 | 1% | 3.51 | 3.21 | 0.30 | 19,554 | 13,181 | 2.759 | 3.199 | 3.391 | 20.1 | 0.54 | 0 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-get-c50-P1` / `redis-noaof-get-c50-P1` | **0.915** | 0.915 0.906 0.924 |
| `final/rc-noaof-get-c50-P1` / `redis-noaof-get-c50-P1` | **0.924** | 0.924 0.924 0.924 |
| `base/rc-noaof-incr-c50-P1` / `redis-noaof-incr-c50-P1` | **0.898** | 0.907 0.898 0.890 |
| `final/rc-noaof-incr-c50-P1` / `redis-noaof-incr-c50-P1` | **0.898** | 0.898 0.898 0.882 |
| `base/rc-noaof-zadd-c50-P1` / `redis-noaof-zadd-c50-P1` | **1.040** | 1.016 1.040 1.056 |
| `final/rc-noaof-zadd-c50-P1` / `redis-noaof-zadd-c50-P1` | **1.032** | 1.016 1.024 1.056 |
| `base/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.090** | 1.076 1.090 1.090 |
| `final/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.024** | 1.050 1.000 1.024 |
| `base/rc-noaof-mset-c50-P16` / `redis-noaof-mset-c50-P16` | **0.785** | 0.785 0.793 0.796 |
| `final/rc-noaof-mset-c50-P16` / `redis-noaof-mset-c50-P16` | **0.778** | 0.778 0.777 0.780 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P1` | 1 | load=0.72 io_psi10=0.10/0.10 mem_psi10=0.01/0.01 swap_used_mib=4668 dirty_wb_kib=1256 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 2 | load=2.44 io_psi10=0.06/0.04 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=3076 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 3 | load=2.44 io_psi10=0.37/0.37 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2656 settle_wait_s=0 |
| `final/rc-noaof-get-c50-P1` | 1 | load=1.33 io_psi10=1.48/1.48 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2392 settle_wait_s=0 |
| `final/rc-noaof-get-c50-P1` | 2 | load=2.36 io_psi10=0.11/0.09 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2892 settle_wait_s=0 |
| `final/rc-noaof-get-c50-P1` | 3 | load=2.63 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=3276 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 1 | load=1.99 io_psi10=0.35/0.35 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1860 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 2 | load=2.25 io_psi10=0.46/0.45 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2148 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 3 | load=2.54 io_psi10=0.29/0.29 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2348 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 1 | load=2.70 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1752 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 2 | load=2.17 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2544 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 3 | load=2.42 io_psi10=0.09/0.06 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2252 settle_wait_s=0 |
| `final/rc-noaof-incr-c50-P1` | 1 | load=2.43 io_psi10=0.25/0.25 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1500 settle_wait_s=0 |
| `final/rc-noaof-incr-c50-P1` | 2 | load=1.83 io_psi10=0.33/0.31 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2016 settle_wait_s=0 |
| `final/rc-noaof-incr-c50-P1` | 3 | load=2.57 io_psi10=0.28/0.28 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2340 settle_wait_s=0 |
| `redis-noaof-incr-c50-P1` | 1 | load=2.55 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2416 settle_wait_s=0 |
| `redis-noaof-incr-c50-P1` | 2 | load=2.25 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1904 settle_wait_s=0 |
| `redis-noaof-incr-c50-P1` | 3 | load=2.62 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2904 settle_wait_s=0 |
| `base/rc-noaof-zadd-c50-P1` | 1 | load=2.39 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2416 settle_wait_s=0 |
| `base/rc-noaof-zadd-c50-P1` | 2 | load=2.07 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2392 settle_wait_s=0 |
| `base/rc-noaof-zadd-c50-P1` | 3 | load=2.62 io_psi10=0.30/0.30 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2776 settle_wait_s=0 |
| `final/rc-noaof-zadd-c50-P1` | 1 | load=2.26 io_psi10=0.20/0.20 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1956 settle_wait_s=0 |
| `final/rc-noaof-zadd-c50-P1` | 2 | load=1.33 io_psi10=0.30/0.30 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2312 settle_wait_s=0 |
| `final/rc-noaof-zadd-c50-P1` | 3 | load=2.41 io_psi10=3.85/3.66 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2920 settle_wait_s=0 |
| `redis-noaof-zadd-c50-P1` | 1 | load=2.39 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2216 settle_wait_s=0 |
| `redis-noaof-zadd-c50-P1` | 2 | load=1.20 io_psi10=0.09/0.09 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2032 settle_wait_s=0 |
| `redis-noaof-zadd-c50-P1` | 3 | load=2.52 io_psi10=0.34/0.33 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2080 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 1 | load=2.63 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2100 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 2 | load=1.39 io_psi10=0.78/0.76 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2736 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 3 | load=2.29 io_psi10=0.09/0.09 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2200 settle_wait_s=0 |
| `final/rc-noaof-set-c1-P1` | 1 | load=2.07 io_psi10=0.47/0.47 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1540 settle_wait_s=0 |
| `final/rc-noaof-set-c1-P1` | 2 | load=1.51 io_psi10=0.04/0.03 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2452 settle_wait_s=0 |
| `final/rc-noaof-set-c1-P1` | 3 | load=2.01 io_psi10=0.62/0.62 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2836 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 1 | load=1.71 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1896 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 2 | load=1.52 io_psi10=0.12/0.10 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2796 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 3 | load=1.64 io_psi10=0.45/0.45 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2240 settle_wait_s=0 |
| `final/rc-noaof-set-c50-P16` | 1 | load=1.41 io_psi10=0.35/0.35 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1632 settle_wait_s=0 |
| `final/rc-noaof-set-c50-P16` | 2 | load=1.32 io_psi10=0.19/0.19 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2088 settle_wait_s=0 |
| `final/rc-noaof-set-c50-P16` | 3 | load=1.79 io_psi10=0.15/0.15 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2480 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 1 | load=1.58 io_psi10=0.10/0.10 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1692 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 2 | load=1.39 io_psi10=0.53/0.46 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2348 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 3 | load=1.78 io_psi10=0.42/0.42 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1876 settle_wait_s=0 |
| `final/rc-noaof-mset-c50-P16` | 1 | load=1.58 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2008 settle_wait_s=0 |
| `final/rc-noaof-mset-c50-P16` | 2 | load=1.47 io_psi10=0.40/0.40 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2176 settle_wait_s=0 |
| `final/rc-noaof-mset-c50-P16` | 3 | load=1.71 io_psi10=0.12/0.12 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2420 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P16` | 1 | load=1.72 io_psi10=0.31/0.29 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1752 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P16` | 2 | load=1.83 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2084 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P16` | 3 | load=1.47 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2168 settle_wait_s=0 |
| `final/rc-noaof-lpush-c50-P16` | 1 | load=1.59 io_psi10=0.46/0.46 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2704 settle_wait_s=0 |
| `final/rc-noaof-lpush-c50-P16` | 2 | load=1.77 io_psi10=0.21/0.21 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2120 settle_wait_s=0 |
| `final/rc-noaof-lpush-c50-P16` | 3 | load=1.61 io_psi10=0.74/0.74 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2312 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P16` | 1 | load=1.71 io_psi10=0.18/0.15 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2792 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P16` | 2 | load=1.76 io_psi10=0.20/0.20 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2284 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P16` | 3 | load=1.66 io_psi10=0.27/0.27 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2768 settle_wait_s=0 |
| `final/rc-noaof-sadd-c50-P16` | 1 | load=1.68 io_psi10=0.42/0.40 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2292 settle_wait_s=0 |
| `final/rc-noaof-sadd-c50-P16` | 2 | load=1.66 io_psi10=0.56/0.56 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1664 settle_wait_s=0 |
| `final/rc-noaof-sadd-c50-P16` | 3 | load=1.69 io_psi10=0.29/0.27 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2072 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 1 | load=1.79 io_psi10=0.09/0.09 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2128 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 2 | load=1.56 io_psi10=3.94/3.94 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2940 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 3 | load=1.80 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2056 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 1 | load=1.78 io_psi10=0.50/0.47 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2272 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 2 | load=1.56 io_psi10=0.21/0.20 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2032 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 3 | load=1.57 io_psi10=0.66/0.66 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2596 settle_wait_s=0 |
