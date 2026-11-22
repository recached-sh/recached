## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-c50-P1` | 3 | 159,962 | 2% | 4.10 | 0.99 | 3.11 | 5,243 | 5,717 | 0.319 | 0.343 | 0.383 | 25.9 | 1.21 | 0 |
| `redis-noaof-set-c50-P1` | 3 | 190,458 | 0% | 3.46 | 0.70 | 2.75 | 4,779 | 4,019 | 0.239 | 0.311 | 0.431 | 19.4 | 1.24 | 0 |
| `valkey-noaof-set-c50-P1` | 3 | 181,802 | 0% | 3.68 | 0.82 | 2.85 | 7,444 | 4,821 | 0.247 | 0.327 | 0.447 | 16.7 | 1.18 | 0 |
| `base/rc-noaof-set-c50-P16` | 3 | 1,110,864 | 0% | 0.72 | 0.46 | 0.26 | 2,924 | 2,133 | 0.711 | 0.799 | 0.927 | 26.0 | 0.74 | 0 |
| `redis-noaof-set-c50-P16` | 3 | 1,110,371 | 5% | 0.79 | 0.55 | 0.23 | 3,980 | 2,423 | 0.687 | 0.863 | 0.943 | 19.7 | 0.65 | 0 |
| `valkey-noaof-set-c50-P16` | 3 | 999,600 | 0% | 0.88 | 0.65 | 0.23 | 6,716 | 2,775 | 0.767 | 0.927 | 0.999 | 16.8 | 0.59 | 0 |
| `base/rc-noaof-set-c1-P1` | 3 | 36,999 | 2% | 12.66 | 3.00 | 9.68 | 7,187 | 13,373 | 0.023 | 0.031 | 0.047 | 23.3 | 0.38 | 0 |
| `redis-noaof-set-c1-P1` | 3 | 35,051 | 5% | 14.31 | 3.82 | 10.56 | 8,006 | 16,597 | 0.031 | 0.031 | 0.055 | 19.1 | 0.34 | 0 |
| `valkey-noaof-set-c1-P1` | 3 | 32,753 | 2% | 15.82 | 4.78 | 11.20 | 10,180 | 20,872 | 0.031 | 0.031 | 0.063 | 16.5 | 0.33 | 0 |
| `base/rc-noaof-set-c1-P16` | 3 | 443,853 | 5% | 1.26 | 0.62 | 0.65 | 3,067 | 2,605 | 0.031 | 0.039 | 0.071 | 24.3 | 0.33 | 0 |
| `redis-noaof-set-c1-P16` | 3 | 399,521 | 5% | 1.48 | 0.77 | 0.71 | 4,185 | 3,280 | 0.039 | 0.039 | 0.071 | 19.1 | 0.30 | 0 |
| `valkey-noaof-set-c1-P16` | 3 | 363,240 | 0% | 1.66 | 0.92 | 0.72 | 6,883 | 3,804 | 0.039 | 0.047 | 0.079 | 16.6 | 0.27 | 0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | 135,584 | 5% | 5.03 | 1.54 | 3.46 | 8,238 | 8,601 | 0.375 | 0.399 | 0.439 | 162.2 | 1.07 | 0 |
| `redis-noaof-lpush-c50-P1` | 3 | 195,103 | 0% | 3.27 | 0.66 | 2.62 | 5,158 | 3,239 | 0.231 | 0.295 | 0.415 | 78.4 | 1.24 | 0 |
| `valkey-noaof-lpush-c50-P1` | 3 | 186,029 | 2% | 3.57 | 0.72 | 2.85 | 6,976 | 4,289 | 0.239 | 0.319 | 0.439 | 79.1 | 1.19 | 0 |
| `base/rc-noaof-lpush-c50-P16` | 3 | 833,056 | 4% | 1.02 | 0.69 | 0.33 | 5,978 | 3,067 | 0.951 | 1.015 | 1.047 | 345.9 | 0.60 | 0 |
| `redis-noaof-lpush-c50-P16` | 3 | 1,249,375 | 0% | 0.66 | 0.45 | 0.21 | 4,407 | 1,900 | 0.591 | 0.759 | 0.823 | 178.3 | 0.67 | 0 |
| `valkey-noaof-lpush-c50-P16` | 3 | 999,400 | 5% | 0.84 | 0.60 | 0.24 | 6,199 | 2,581 | 0.751 | 0.911 | 0.983 | 178.8 | 0.56 | 0 |
| `base/rc-noaof-lpush-c1-P1` | 3 | 34,447 | 2% | 14.51 | 4.24 | 10.26 | 10,013 | 18,612 | 0.031 | 0.031 | 0.063 | 50.8 | 0.35 | 0 |
| `redis-noaof-lpush-c1-P1` | 3 | 34,450 | 2% | 14.33 | 3.70 | 10.64 | 8,325 | 16,109 | 0.031 | 0.031 | 0.063 | 28.2 | 0.35 | 0 |
| `valkey-noaof-lpush-c1-P1` | 3 | 32,223 | 5% | 15.81 | 4.74 | 11.06 | 9,726 | 20,393 | 0.031 | 0.031 | 0.071 | 29.1 | 0.32 | 0 |
| `base/rc-noaof-lpush-c1-P16` | 3 | 380,518 | 0% | 1.59 | 0.84 | 0.74 | 6,088 | 3,587 | 0.039 | 0.047 | 0.079 | 164.1 | 0.27 | 0 |
| `redis-noaof-lpush-c1-P16` | 3 | 420,521 | 0% | 1.37 | 0.63 | 0.73 | 4,649 | 2,763 | 0.039 | 0.039 | 0.071 | 78.2 | 0.29 | 0 |
| `valkey-noaof-lpush-c1-P16` | 3 | 363,174 | 5% | 1.63 | 0.85 | 0.78 | 6,372 | 3,656 | 0.039 | 0.047 | 0.079 | 79.0 | 0.26 | 0 |
| `base/rc-noaof-sadd-c50-P1` | 3 | 124,992 | 8% | 5.51 | 1.85 | 3.67 | 8,565 | 9,935 | 0.407 | 0.431 | 0.463 | 18.6 | 1.01 | 0 |
| `redis-noaof-sadd-c50-P1` | 3 | 195,065 | 0% | 3.30 | 0.69 | 2.62 | 4,397 | 3,492 | 0.231 | 0.295 | 0.415 | 12.2 | 1.27 | 0 |
| `valkey-noaof-sadd-c50-P1` | 3 | 186,029 | 2% | 3.51 | 0.69 | 2.79 | 6,187 | 4,175 | 0.239 | 0.311 | 0.431 | 12.7 | 1.22 | 0 |
| `base/rc-noaof-sadd-c50-P16` | 3 | 714,184 | 4% | 1.19 | 0.89 | 0.30 | 6,316 | 3,891 | 1.111 | 1.207 | 1.263 | 19.9 | 0.56 | 0 |
| `redis-noaof-sadd-c50-P16` | 3 | 1,249,375 | 0% | 0.66 | 0.38 | 0.29 | 3,589 | 1,965 | 0.583 | 0.767 | 0.871 | 12.4 | 0.74 | 0 |
| `valkey-noaof-sadd-c50-P16` | 3 | 1,051,967 | 0% | 0.81 | 0.59 | 0.22 | 5,419 | 2,522 | 0.711 | 0.879 | 0.959 | 12.9 | 0.62 | 0 |
| `base/rc-noaof-sadd-c1-P1` | 3 | 34,445 | 3% | 14.76 | 4.42 | 10.22 | 10,353 | 19,430 | 0.031 | 0.031 | 0.063 | 15.5 | 0.35 | 0 |
| `redis-noaof-sadd-c1-P1` | 3 | 35,053 | 4% | 13.90 | 3.42 | 10.42 | 7,631 | 15,248 | 0.031 | 0.031 | 0.063 | 11.9 | 0.35 | 0 |
| `valkey-noaof-sadd-c1-P1` | 3 | 33,298 | 5% | 15.19 | 4.34 | 10.90 | 8,975 | 18,878 | 0.031 | 0.031 | 0.063 | 12.4 | 0.34 | 0 |
| `base/rc-noaof-sadd-c1-P16` | 3 | 347,403 | 4% | 1.79 | 1.07 | 0.70 | 6,427 | 4,552 | 0.039 | 0.047 | 0.079 | 19.1 | 0.26 | 0 |
| `redis-noaof-sadd-c1-P16` | 3 | 399,521 | 10% | 1.37 | 0.67 | 0.70 | 3,788 | 2,859 | 0.039 | 0.039 | 0.079 | 12.2 | 0.30 | 0 |
| `valkey-noaof-sadd-c1-P16` | 3 | 363,240 | 9% | 1.59 | 0.83 | 0.76 | 5,596 | 3,564 | 0.039 | 0.047 | 0.079 | 12.6 | 0.27 | 0 |
| `base/rc-noaof-mset-c50-P1` | 3 | 85,096 | 1% | 9.20 | 4.91 | 4.22 | 29,402 | 23,946 | 0.591 | 0.663 | 0.727 | 25.9 | 0.76 | 0 |
| `redis-noaof-mset-c50-P1` | 3 | 117,626 | 7% | 6.71 | 3.63 | 3.20 | 20,326 | 16,367 | 0.383 | 0.543 | 0.623 | 19.8 | 0.82 | 0 |
| `valkey-noaof-mset-c50-P1` | 3 | 115,915 | 3% | 6.82 | 3.60 | 3.08 | 39,905 | 16,587 | 0.391 | 0.551 | 0.631 | 16.8 | 0.83 | 0 |
| `base/rc-noaof-mset-c50-P16` | 3 | 194,009 | 5% | 4.88 | 4.43 | 0.47 | 27,133 | 17,876 | 4.167 | 4.623 | 4.975 | 26.4 | 0.24 | 0 |
| `redis-noaof-mset-c50-P16` | 3 | 234,973 | 9% | 4.07 | 3.67 | 0.39 | 19,551 | 14,834 | 3.231 | 4.095 | 4.567 | 20.0 | 0.48 | 0 |
| `valkey-noaof-mset-c50-P16` | 3 | 237,710 | 2% | 4.04 | 3.66 | 0.37 | 38,008 | 14,836 | 3.263 | 3.783 | 4.047 | 16.7 | 0.45 | 0 |
| `base/rc-noaof-mset-c1-P1` | 3 | 29,820 | 7% | 18.52 | 7.92 | 10.62 | 31,196 | 33,839 | 0.031 | 0.039 | 0.071 | 23.6 | 0.31 | 0 |
| `redis-noaof-mset-c1-P1` | 3 | 28,955 | 4% | 18.04 | 7.10 | 10.98 | 23,551 | 30,650 | 0.031 | 0.039 | 0.071 | 19.4 | 0.31 | 0 |
| `valkey-noaof-mset-c1-P1` | 3 | 28,542 | 3% | 19.10 | 7.70 | 11.60 | 42,664 | 33,139 | 0.031 | 0.039 | 0.071 | 16.6 | 0.30 | 0 |
| `base/rc-noaof-mset-c1-P16` | 3 | 142,714 | 7% | 5.28 | 4.48 | 0.81 | 27,244 | 17,901 | 0.103 | 0.135 | 0.159 | 24.5 | 0.17 | 0 |
| `redis-noaof-mset-c1-P16` | 3 | 163,106 | 10% | 4.51 | 3.70 | 0.82 | 19,756 | 14,877 | 0.087 | 0.119 | 0.143 | 19.4 | 0.18 | 0 |
| `valkey-noaof-mset-c1-P16` | 3 | 148,006 | 6% | 4.97 | 4.08 | 0.88 | 38,705 | 16,456 | 0.095 | 0.127 | 0.151 | 16.5 | 0.17 | 0 |
| `base/rc-noaof-get-c50-P1` | 3 | 166,653 | 2% | 3.92 | 0.92 | 3.00 | 4,505 | 5,243 | 0.303 | 0.335 | 0.367 | 25.3 | 1.25 | 0 |
| `redis-noaof-get-c50-P1` | 3 | 190,458 | 2% | 3.36 | 0.72 | 2.63 | 4,010 | 3,667 | 0.231 | 0.303 | 0.423 | 19.1 | 1.24 | 0 |
| `valkey-noaof-get-c50-P1` | 3 | 186,012 | 2% | 3.51 | 0.71 | 2.79 | 5,669 | 4,109 | 0.239 | 0.311 | 0.439 | 16.4 | 1.20 | 0 |
| `base/rc-noaof-incr-c50-P1` | 3 | 156,850 | 2% | 4.20 | 1.05 | 3.15 | 5,206 | 5,975 | 0.319 | 0.351 | 0.399 | 27.4 | 1.19 | 0 |
| `redis-noaof-incr-c50-P1` | 3 | 195,103 | 2% | 3.33 | 0.68 | 2.70 | 4,295 | 3,483 | 0.231 | 0.295 | 0.423 | 14.0 | 1.25 | 0 |
| `valkey-noaof-incr-c50-P1` | 3 | 186,012 | 5% | 3.50 | 0.70 | 2.79 | 5,538 | 4,159 | 0.239 | 0.311 | 0.431 | 14.2 | 1.22 | 0 |
| `base/rc-noaof-zadd-c50-P1` | 3 | 124,992 | 6% | 5.63 | 1.97 | 3.69 | 10,687 | 10,672 | 0.407 | 0.439 | 0.503 | 18.0 | 1.01 | 0 |
| `redis-noaof-zadd-c50-P1` | 3 | 137,902 | 7% | 5.33 | 2.12 | 3.21 | 7,945 | 11,183 | 0.311 | 0.471 | 0.575 | 18.6 | 0.90 | 0 |
| `valkey-noaof-zadd-c50-P1` | 3 | 135,566 | 7% | 5.52 | 2.15 | 3.30 | 9,567 | 11,622 | 0.319 | 0.479 | 0.583 | 16.4 | 0.89 | 0 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.840** | 0.824 0.840 0.840 |
| `base/rc-noaof-set-c50-P1` / `valkey-noaof-set-c50-P1` | **0.880** | 0.863 0.880 0.880 |
| `base/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.000** | 1.056 1.000 1.000 |
| `base/rc-noaof-set-c50-P16` / `valkey-noaof-set-c50-P16` | **1.111** | 1.111 1.112 1.111 |
| `base/rc-noaof-set-c1-P1` / `redis-noaof-set-c1-P1` | **1.056** | 1.111 1.075 1.055 |
| `base/rc-noaof-set-c1-P1` / `valkey-noaof-set-c1-P1` | **1.130** | 1.148 1.151 1.130 |
| `base/rc-noaof-set-c1-P16` / `redis-noaof-set-c1-P16` | **1.111** | 1.105 1.111 1.111 |
| `base/rc-noaof-set-c1-P16` / `valkey-noaof-set-c1-P16` | **1.222** | 1.158 1.222 1.222 |
| `base/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.695** | 0.672 0.695 0.707 |
| `base/rc-noaof-lpush-c50-P1` / `valkey-noaof-lpush-c50-P1` | **0.729** | 0.705 0.746 0.741 |
| `base/rc-noaof-lpush-c50-P16` / `redis-noaof-lpush-c50-P16` | **0.667** | 0.667 0.666 0.696 |
| `base/rc-noaof-lpush-c50-P16` / `valkey-noaof-lpush-c50-P16` | **0.834** | 0.834 0.833 0.826 |
| `base/rc-noaof-lpush-c1-P1` / `redis-noaof-lpush-c1-P1` | **1.000** | 1.000 1.000 1.000 |
| `base/rc-noaof-lpush-c1-P1` / `valkey-noaof-lpush-c1-P1` | **1.069** | 1.103 1.052 1.088 |
| `base/rc-noaof-lpush-c1-P16` / `redis-noaof-lpush-c1-P16` | **0.905** | 0.905 0.905 0.905 |
| `base/rc-noaof-lpush-c1-P16` / `valkey-noaof-lpush-c1-P16` | **1.048** | 1.000 1.048 1.048 |
| `base/rc-noaof-sadd-c50-P1` / `redis-noaof-sadd-c50-P1` | **0.641** | 0.695 0.641 0.641 |
| `base/rc-noaof-sadd-c50-P1` / `valkey-noaof-sadd-c50-P1` | **0.672** | 0.729 0.672 0.656 |
| `base/rc-noaof-sadd-c50-P16` / `redis-noaof-sadd-c50-P16` | **0.572** | 0.571 0.572 0.593 |
| `base/rc-noaof-sadd-c50-P16` / `valkey-noaof-sadd-c50-P16` | **0.679** | 0.679 0.679 0.704 |
| `base/rc-noaof-sadd-c1-P1` / `redis-noaof-sadd-c1-P1` | **0.983** | 1.000 0.966 0.982 |
| `base/rc-noaof-sadd-c1-P1` / `valkey-noaof-sadd-c1-P1` | **1.034** | 1.034 1.051 1.035 |
| `base/rc-noaof-sadd-c1-P16` / `redis-noaof-sadd-c1-P16` | **0.870** | 0.870 0.875 0.826 |
| `base/rc-noaof-sadd-c1-P16` / `valkey-noaof-sadd-c1-P16` | **0.956** | 0.956 0.958 0.913 |
| `base/rc-noaof-mset-c50-P1` / `redis-noaof-mset-c50-P1` | **0.723** | 0.723 0.755 0.710 |
| `base/rc-noaof-mset-c50-P1` / `valkey-noaof-mset-c50-P1` | **0.734** | 0.734 0.745 0.731 |
| `base/rc-noaof-mset-c50-P16` / `redis-noaof-mset-c50-P16` | **0.826** | 0.826 0.812 0.783 |
| `base/rc-noaof-mset-c50-P16` / `valkey-noaof-mset-c50-P16` | **0.816** | 0.816 0.803 0.822 |
| `base/rc-noaof-mset-c1-P1` / `redis-noaof-mset-c1-P1` | **1.030** | 1.000 0.986 1.045 |
| `base/rc-noaof-mset-c1-P1` / `valkey-noaof-mset-c1-P1` | **1.045** | 1.045 1.000 1.045 |
| `base/rc-noaof-mset-c1-P16` / `redis-noaof-mset-c1-P16` | **0.875** | 0.875 0.895 0.868 |
| `base/rc-noaof-mset-c1-P16` / `valkey-noaof-mset-c1-P16` | **0.964** | 0.964 0.947 0.962 |
| `base/rc-noaof-get-c50-P1` / `redis-noaof-get-c50-P1` | **0.875** | 0.875 0.857 0.854 |
| `base/rc-noaof-get-c50-P1` / `valkey-noaof-get-c50-P1` | **0.896** | 0.896 0.878 0.875 |
| `base/rc-noaof-incr-c50-P1` / `redis-noaof-incr-c50-P1` | **0.804** | 0.804 0.808 0.804 |
| `base/rc-noaof-incr-c50-P1` / `valkey-noaof-incr-c50-P1` | **0.843** | 0.843 0.846 0.824 |
| `base/rc-noaof-zadd-c50-P1` / `redis-noaof-zadd-c50-P1` | **0.906** | 0.866 0.937 0.859 |
| `base/rc-noaof-zadd-c50-P1` / `valkey-noaof-zadd-c50-P1` | **0.922** | 0.881 0.952 0.875 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` | 1 | load=0.58 io_psi10=1.10/1.08 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2232 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=2.01 io_psi10=1.10/1.05 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2508 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=2.16 io_psi10=1.40/1.29 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=3660 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 1 | load=1.20 io_psi10=2.34/1.52 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=3500 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 2 | load=1.83 io_psi10=3.17/3.02 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=3688 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 3 | load=2.34 io_psi10=1.70/1.50 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=4132 settle_wait_s=0 |
| `valkey-noaof-set-c50-P1` | 1 | load=1.64 io_psi10=1.47/1.15 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=3656 settle_wait_s=0 |
| `valkey-noaof-set-c50-P1` | 2 | load=1.70 io_psi10=4.45/4.44 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=1744 settle_wait_s=0 |
| `valkey-noaof-set-c50-P1` | 3 | load=2.45 io_psi10=1.46/1.25 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=1936 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 1 | load=1.71 io_psi10=1.64/1.48 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1864 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 2 | load=1.68 io_psi10=3.91/3.89 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2216 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 3 | load=2.59 io_psi10=2.97/2.72 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=4740 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 1 | load=1.73 io_psi10=1.79/1.70 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2100 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 2 | load=1.47 io_psi10=2.48/2.46 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2696 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 3 | load=2.55 io_psi10=2.52/2.23 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2684 settle_wait_s=0 |
| `valkey-noaof-set-c50-P16` | 1 | load=1.75 io_psi10=2.70/2.56 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=5836 settle_wait_s=0 |
| `valkey-noaof-set-c50-P16` | 2 | load=1.56 io_psi10=1.25/1.20 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2984 settle_wait_s=0 |
| `valkey-noaof-set-c50-P16` | 3 | load=2.58 io_psi10=4.67/4.42 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=1364 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 1 | load=1.78 io_psi10=5.09/4.75 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2128 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 2 | load=1.64 io_psi10=0.71/0.70 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2512 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 3 | load=2.49 io_psi10=6.33/6.07 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=4928 settle_wait_s=0 |
| `redis-noaof-set-c1-P1` | 1 | load=1.61 io_psi10=1.42/1.33 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2196 settle_wait_s=0 |
| `redis-noaof-set-c1-P1` | 2 | load=1.71 io_psi10=1.43/1.39 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=3468 settle_wait_s=0 |
| `redis-noaof-set-c1-P1` | 3 | load=2.24 io_psi10=2.15/1.99 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2936 settle_wait_s=0 |
| `valkey-noaof-set-c1-P1` | 1 | load=1.47 io_psi10=0.65/0.46 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=3568 settle_wait_s=0 |
| `valkey-noaof-set-c1-P1` | 2 | load=1.92 io_psi10=3.74/3.70 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=1096 settle_wait_s=0 |
| `valkey-noaof-set-c1-P1` | 3 | load=2.03 io_psi10=1.90/1.74 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=832 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P16` | 1 | load=1.60 io_psi10=0.68/0.61 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2388 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P16` | 2 | load=2.00 io_psi10=3.04/2.95 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2488 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P16` | 3 | load=2.03 io_psi10=1.19/1.09 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=3604 settle_wait_s=0 |
| `redis-noaof-set-c1-P16` | 1 | load=1.55 io_psi10=0.73/0.70 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1728 settle_wait_s=0 |
| `redis-noaof-set-c1-P16` | 2 | load=2.08 io_psi10=2.66/2.50 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=1996 settle_wait_s=0 |
| `redis-noaof-set-c1-P16` | 3 | load=1.94 io_psi10=2.64/2.41 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2480 settle_wait_s=0 |
| `valkey-noaof-set-c1-P16` | 1 | load=1.51 io_psi10=2.68/2.49 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2516 settle_wait_s=0 |
| `valkey-noaof-set-c1-P16` | 2 | load=2.18 io_psi10=1.75/1.46 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2624 settle_wait_s=0 |
| `valkey-noaof-set-c1-P16` | 3 | load=1.87 io_psi10=3.15/2.90 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=1028 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 1 | load=1.43 io_psi10=1.71/1.61 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2020 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | load=2.14 io_psi10=3.72/3.18 mem_psi10=0.01/0.01 swap_used_mib=3382 dirty_wb_kib=3172 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | load=1.65 io_psi10=2.85/2.74 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2428 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 1 | load=1.72 io_psi10=5.81/5.13 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1616 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 2 | load=2.00 io_psi10=3.08/2.54 mem_psi10=0.05/0.05 swap_used_mib=3382 dirty_wb_kib=2800 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 3 | load=2.24 io_psi10=1.15/1.12 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=1696 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P1` | 1 | load=1.93 io_psi10=6.03/5.37 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=4340 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P1` | 2 | load=1.71 io_psi10=3.48/3.11 mem_psi10=0.18/0.18 swap_used_mib=3382 dirty_wb_kib=2592 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P1` | 3 | load=2.28 io_psi10=1.57/1.40 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2296 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P16` | 1 | load=2.10 io_psi10=2.35/2.08 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2656 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P16` | 2 | load=1.68 io_psi10=3.16/3.08 mem_psi10=0.00/0.00 swap_used_mib=3379 dirty_wb_kib=2932 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P16` | 3 | load=3.30 io_psi10=1.73/1.62 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2388 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P16` | 1 | load=2.01 io_psi10=2.96/2.75 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2192 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P16` | 2 | load=1.74 io_psi10=1.67/1.53 mem_psi10=0.00/0.00 swap_used_mib=3379 dirty_wb_kib=3124 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P16` | 3 | load=3.19 io_psi10=3.61/3.45 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=3036 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P16` | 1 | load=1.93 io_psi10=1.99/1.87 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2568 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P16` | 2 | load=1.50 io_psi10=0.84/0.80 mem_psi10=0.00/0.00 swap_used_mib=3379 dirty_wb_kib=3016 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P16` | 3 | load=3.02 io_psi10=3.98/3.89 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2504 settle_wait_s=0 |
| `base/rc-noaof-lpush-c1-P1` | 1 | load=2.01 io_psi10=3.40/3.21 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2320 settle_wait_s=0 |
| `base/rc-noaof-lpush-c1-P1` | 2 | load=1.55 io_psi10=1.45/1.25 mem_psi10=0.00/0.00 swap_used_mib=3379 dirty_wb_kib=2980 settle_wait_s=0 |
| `base/rc-noaof-lpush-c1-P1` | 3 | load=2.93 io_psi10=4.73/4.56 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=1564 settle_wait_s=0 |
| `redis-noaof-lpush-c1-P1` | 1 | load=1.74 io_psi10=0.86/0.82 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=3664 settle_wait_s=0 |
| `redis-noaof-lpush-c1-P1` | 2 | load=1.62 io_psi10=2.03/1.84 mem_psi10=0.00/0.00 swap_used_mib=3379 dirty_wb_kib=3120 settle_wait_s=0 |
| `redis-noaof-lpush-c1-P1` | 3 | load=2.64 io_psi10=1.64/1.56 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=3288 settle_wait_s=0 |
| `valkey-noaof-lpush-c1-P1` | 1 | load=1.57 io_psi10=1.31/1.26 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1816 settle_wait_s=0 |
| `valkey-noaof-lpush-c1-P1` | 2 | load=1.71 io_psi10=4.40/4.10 mem_psi10=0.00/0.00 swap_used_mib=3379 dirty_wb_kib=2940 settle_wait_s=0 |
| `valkey-noaof-lpush-c1-P1` | 3 | load=2.34 io_psi10=1.85/1.54 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2988 settle_wait_s=0 |
| `base/rc-noaof-lpush-c1-P16` | 1 | load=1.42 io_psi10=7.96/7.30 mem_psi10=0.03/0.01 swap_used_mib=3384 dirty_wb_kib=2532 settle_wait_s=33 |
| `base/rc-noaof-lpush-c1-P16` | 2 | load=1.67 io_psi10=7.13/6.82 mem_psi10=0.00/0.00 swap_used_mib=3377 dirty_wb_kib=1080 settle_wait_s=70 |
| `base/rc-noaof-lpush-c1-P16` | 3 | load=2.03 io_psi10=2.28/2.20 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2944 settle_wait_s=0 |
| `redis-noaof-lpush-c1-P16` | 1 | load=1.43 io_psi10=5.46/5.10 mem_psi10=0.02/0.00 swap_used_mib=3384 dirty_wb_kib=3664 settle_wait_s=0 |
| `redis-noaof-lpush-c1-P16` | 2 | load=2.02 io_psi10=5.34/4.99 mem_psi10=0.00/0.00 swap_used_mib=3377 dirty_wb_kib=2824 settle_wait_s=0 |
| `redis-noaof-lpush-c1-P16` | 3 | load=1.94 io_psi10=4.53/4.48 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=1960 settle_wait_s=0 |
| `valkey-noaof-lpush-c1-P16` | 1 | load=1.48 io_psi10=5.10/4.82 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2192 settle_wait_s=0 |
| `valkey-noaof-lpush-c1-P16` | 2 | load=2.03 io_psi10=7.21/6.89 mem_psi10=0.00/0.00 swap_used_mib=3377 dirty_wb_kib=4560 settle_wait_s=2 |
| `valkey-noaof-lpush-c1-P16` | 3 | load=1.87 io_psi10=7.36/7.06 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2712 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 1 | load=1.44 io_psi10=3.29/3.13 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2536 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 2 | load=1.74 io_psi10=42.61/41.01 mem_psi10=0.00/0.00 swap_used_mib=3377 dirty_wb_kib=292 settle_wait_s=90 |
| `base/rc-noaof-sadd-c50-P1` | 3 | load=1.81 io_psi10=5.97/5.73 mem_psi10=0.00/0.00 swap_used_mib=3382 dirty_wb_kib=2484 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 1 | load=1.77 io_psi10=1.79/1.73 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2288 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 2 | load=1.67 io_psi10=17.60/17.09 mem_psi10=0.00/0.00 swap_used_mib=3378 dirty_wb_kib=2172 settle_wait_s=90 |
| `redis-noaof-sadd-c50-P1` | 3 | load=1.99 io_psi10=1.42/1.38 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=1884 settle_wait_s=0 |
| `valkey-noaof-sadd-c50-P1` | 1 | load=1.80 io_psi10=2.02/1.82 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2664 settle_wait_s=0 |
| `valkey-noaof-sadd-c50-P1` | 2 | load=1.56 io_psi10=30.34/29.64 mem_psi10=0.00/0.00 swap_used_mib=3378 dirty_wb_kib=1772 settle_wait_s=90 |
| `valkey-noaof-sadd-c50-P1` | 3 | load=2.05 io_psi10=2.98/2.82 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=4456 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P16` | 1 | load=1.99 io_psi10=1.67/1.54 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2724 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P16` | 2 | load=1.71 io_psi10=28.99/28.06 mem_psi10=0.00/0.00 swap_used_mib=3378 dirty_wb_kib=396 settle_wait_s=90 |
| `base/rc-noaof-sadd-c50-P16` | 3 | load=2.10 io_psi10=1.98/1.94 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=2500 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P16` | 1 | load=1.99 io_psi10=1.84/1.69 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3020 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P16` | 2 | load=1.83 io_psi10=27.89/27.33 mem_psi10=0.00/0.00 swap_used_mib=3378 dirty_wb_kib=1592 settle_wait_s=90 |
| `redis-noaof-sadd-c50-P16` | 3 | load=2.09 io_psi10=3.10/2.98 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=2576 settle_wait_s=0 |
| `valkey-noaof-sadd-c50-P16` | 1 | load=1.76 io_psi10=4.64/4.55 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2544 settle_wait_s=0 |
| `valkey-noaof-sadd-c50-P16` | 2 | load=1.82 io_psi10=40.95/39.79 mem_psi10=0.00/0.00 swap_used_mib=3378 dirty_wb_kib=3200 settle_wait_s=90 |
| `valkey-noaof-sadd-c50-P16` | 3 | load=2.00 io_psi10=7.16/6.89 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=1356 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P1` | 1 | load=1.86 io_psi10=3.76/3.60 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2848 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P1` | 2 | load=1.95 io_psi10=2.30/2.19 mem_psi10=0.00/0.00 swap_used_mib=3378 dirty_wb_kib=4744 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P1` | 3 | load=1.92 io_psi10=5.75/5.60 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=2656 settle_wait_s=0 |
| `redis-noaof-sadd-c1-P1` | 1 | load=1.66 io_psi10=1.38/1.30 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2912 settle_wait_s=0 |
| `redis-noaof-sadd-c1-P1` | 2 | load=1.69 io_psi10=7.83/7.30 mem_psi10=0.00/0.00 swap_used_mib=3378 dirty_wb_kib=4424 settle_wait_s=49 |
| `redis-noaof-sadd-c1-P1` | 3 | load=1.71 io_psi10=2.15/2.12 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=2424 settle_wait_s=0 |
| `valkey-noaof-sadd-c1-P1` | 1 | load=1.77 io_psi10=2.63/2.31 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=5084 settle_wait_s=0 |
| `valkey-noaof-sadd-c1-P1` | 2 | load=1.64 io_psi10=7.33/7.01 mem_psi10=0.00/0.00 swap_used_mib=3378 dirty_wb_kib=1272 settle_wait_s=34 |
| `valkey-noaof-sadd-c1-P1` | 3 | load=1.73 io_psi10=2.77/2.40 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=3660 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P16` | 1 | load=1.67 io_psi10=1.38/1.27 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2352 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P16` | 2 | load=1.95 io_psi10=29.38/27.49 mem_psi10=0.00/0.00 swap_used_mib=3378 dirty_wb_kib=1864 settle_wait_s=90 |
| `base/rc-noaof-sadd-c1-P16` | 3 | load=1.64 io_psi10=2.43/2.18 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=3284 settle_wait_s=0 |
| `redis-noaof-sadd-c1-P16` | 1 | load=1.72 io_psi10=0.96/0.91 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2076 settle_wait_s=0 |
| `redis-noaof-sadd-c1-P16` | 2 | load=2.23 io_psi10=2.25/1.60 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3248 settle_wait_s=0 |
| `redis-noaof-sadd-c1-P16` | 3 | load=1.59 io_psi10=1.89/1.77 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=2300 settle_wait_s=0 |
| `valkey-noaof-sadd-c1-P16` | 1 | load=1.74 io_psi10=4.52/3.86 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2812 settle_wait_s=0 |
| `valkey-noaof-sadd-c1-P16` | 2 | load=2.42 io_psi10=0.96/0.71 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3764 settle_wait_s=0 |
| `valkey-noaof-sadd-c1-P16` | 3 | load=1.72 io_psi10=4.31/4.13 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=2936 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 1 | load=1.68 io_psi10=3.52/3.22 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2832 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 2 | load=2.36 io_psi10=0.86/0.83 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3056 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 3 | load=1.66 io_psi10=6.75/6.55 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=4192 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 1 | load=2.04 io_psi10=0.87/0.73 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=4012 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 2 | load=2.48 io_psi10=0.82/0.60 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2868 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 3 | load=2.09 io_psi10=1.48/1.41 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=5732 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P1` | 1 | load=2.10 io_psi10=1.19/1.14 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2744 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P1` | 2 | load=1.94 io_psi10=0.36/0.29 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3908 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P1` | 3 | load=2.27 io_psi10=1.83/1.67 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=3588 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 1 | load=2.00 io_psi10=0.73/0.73 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3844 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 2 | load=1.72 io_psi10=0.91/0.67 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=5256 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 3 | load=2.47 io_psi10=1.81/1.73 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=3220 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 1 | load=1.69 io_psi10=0.36/0.34 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3628 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 2 | load=1.92 io_psi10=0.72/0.50 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=4356 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 3 | load=2.03 io_psi10=0.61/0.59 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=1764 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P16` | 1 | load=2.19 io_psi10=0.92/0.71 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=5236 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P16` | 2 | load=2.21 io_psi10=0.96/0.77 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=5464 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P16` | 3 | load=1.95 io_psi10=0.96/0.93 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=3048 settle_wait_s=0 |
| `base/rc-noaof-mset-c1-P1` | 1 | load=2.55 io_psi10=0.64/0.59 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2948 settle_wait_s=0 |
| `base/rc-noaof-mset-c1-P1` | 2 | load=2.60 io_psi10=0.72/0.64 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2600 settle_wait_s=0 |
| `base/rc-noaof-mset-c1-P1` | 3 | load=1.74 io_psi10=1.80/1.77 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=2884 settle_wait_s=0 |
| `redis-noaof-mset-c1-P1` | 1 | load=2.20 io_psi10=0.22/0.21 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3048 settle_wait_s=0 |
| `redis-noaof-mset-c1-P1` | 2 | load=2.46 io_psi10=0.61/0.59 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3260 settle_wait_s=0 |
| `redis-noaof-mset-c1-P1` | 3 | load=1.60 io_psi10=0.84/0.80 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=1764 settle_wait_s=0 |
| `valkey-noaof-mset-c1-P1` | 1 | load=2.33 io_psi10=0.84/0.80 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3348 settle_wait_s=0 |
| `valkey-noaof-mset-c1-P1` | 2 | load=2.86 io_psi10=0.82/0.70 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2860 settle_wait_s=0 |
| `valkey-noaof-mset-c1-P1` | 3 | load=1.62 io_psi10=2.00/1.74 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=4128 settle_wait_s=0 |
| `base/rc-noaof-mset-c1-P16` | 1 | load=2.02 io_psi10=0.47/0.43 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2928 settle_wait_s=0 |
| `base/rc-noaof-mset-c1-P16` | 2 | load=3.04 io_psi10=2.30/2.02 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3632 settle_wait_s=0 |
| `base/rc-noaof-mset-c1-P16` | 3 | load=1.50 io_psi10=0.90/0.79 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=1364 settle_wait_s=0 |
| `redis-noaof-mset-c1-P16` | 1 | load=1.94 io_psi10=0.77/0.66 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2360 settle_wait_s=0 |
| `redis-noaof-mset-c1-P16` | 2 | load=3.33 io_psi10=0.95/0.81 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2872 settle_wait_s=0 |
| `redis-noaof-mset-c1-P16` | 3 | load=1.39 io_psi10=2.03/1.97 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=3056 settle_wait_s=0 |
| `valkey-noaof-mset-c1-P16` | 1 | load=1.81 io_psi10=1.17/1.10 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2764 settle_wait_s=0 |
| `valkey-noaof-mset-c1-P16` | 2 | load=3.80 io_psi10=1.20/0.99 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3132 settle_wait_s=0 |
| `valkey-noaof-mset-c1-P16` | 3 | load=1.30 io_psi10=1.69/1.68 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=2064 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 1 | load=1.50 io_psi10=0.78/0.76 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2944 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 2 | load=3.92 io_psi10=1.00/0.91 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2660 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 3 | load=1.24 io_psi10=1.57/1.53 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=2716 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 1 | load=1.73 io_psi10=1.55/1.44 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2268 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 2 | load=3.82 io_psi10=1.31/1.21 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3532 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 3 | load=1.79 io_psi10=3.39/2.94 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=2668 settle_wait_s=0 |
| `valkey-noaof-get-c50-P1` | 1 | load=1.88 io_psi10=1.06/0.86 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3872 settle_wait_s=0 |
| `valkey-noaof-get-c50-P1` | 2 | load=3.56 io_psi10=1.06/0.93 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2484 settle_wait_s=0 |
| `valkey-noaof-get-c50-P1` | 3 | load=2.05 io_psi10=2.95/2.75 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=684 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 1 | load=2.34 io_psi10=2.36/1.99 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3312 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 2 | load=3.12 io_psi10=1.56/1.22 mem_psi10=0.01/0.00 swap_used_mib=3384 dirty_wb_kib=2916 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 3 | load=2.12 io_psi10=5.48/5.36 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=3424 settle_wait_s=0 |
| `redis-noaof-incr-c50-P1` | 1 | load=2.45 io_psi10=1.22/0.95 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2928 settle_wait_s=0 |
| `redis-noaof-incr-c50-P1` | 2 | load=3.34 io_psi10=1.71/1.42 mem_psi10=0.04/0.00 swap_used_mib=3384 dirty_wb_kib=2316 settle_wait_s=0 |
| `redis-noaof-incr-c50-P1` | 3 | load=2.31 io_psi10=2.06/2.03 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=3168 settle_wait_s=0 |
| `valkey-noaof-incr-c50-P1` | 1 | load=2.56 io_psi10=2.59/2.45 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3812 settle_wait_s=0 |
| `valkey-noaof-incr-c50-P1` | 2 | load=3.21 io_psi10=0.88/0.82 mem_psi10=0.18/0.00 swap_used_mib=3384 dirty_wb_kib=2936 settle_wait_s=0 |
| `valkey-noaof-incr-c50-P1` | 3 | load=2.27 io_psi10=2.55/2.54 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=2772 settle_wait_s=0 |
| `base/rc-noaof-zadd-c50-P1` | 1 | load=2.72 io_psi10=2.57/2.42 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3952 settle_wait_s=0 |
| `base/rc-noaof-zadd-c50-P1` | 2 | load=3.19 io_psi10=0.90/0.75 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3596 settle_wait_s=0 |
| `base/rc-noaof-zadd-c50-P1` | 3 | load=2.20 io_psi10=3.20/3.05 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=1616 settle_wait_s=0 |
| `redis-noaof-zadd-c50-P1` | 1 | load=2.72 io_psi10=1.00/0.86 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3700 settle_wait_s=0 |
| `redis-noaof-zadd-c50-P1` | 2 | load=2.59 io_psi10=1.21/1.06 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3324 settle_wait_s=0 |
| `redis-noaof-zadd-c50-P1` | 3 | load=2.29 io_psi10=1.30/1.21 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=2972 settle_wait_s=0 |
| `valkey-noaof-zadd-c50-P1` | 1 | load=2.80 io_psi10=1.19/1.08 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=2728 settle_wait_s=0 |
| `valkey-noaof-zadd-c50-P1` | 2 | load=2.48 io_psi10=0.88/0.77 mem_psi10=0.00/0.00 swap_used_mib=3384 dirty_wb_kib=3440 settle_wait_s=0 |
| `valkey-noaof-zadd-c50-P1` | 3 | load=2.61 io_psi10=2.52/2.46 mem_psi10=0.00/0.00 swap_used_mib=3381 dirty_wb_kib=2404 settle_wait_s=0 |
