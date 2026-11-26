## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P1` | 5 | 179,751 | 5% | 3.64 | 0.81 | 2.81 | — | — | 0.263 | 0.359 | 0.383 | 26.5 | 1.24 | 0 |
| `g/rc-noaof-get-c50-P1` | 5 | 179,767 | 2% | 3.67 | 0.82 | 2.84 | — | — | 0.263 | 0.359 | 0.383 | 26.5 | 1.24 | 0 |
| `base/rc-noaof-mget-c50-P1` | 5 | 124,984 | 6% | 5.98 | 2.69 | 3.31 | — | — | 0.375 | 0.511 | 0.551 | 26.4 | 0.98 | 0 |
| `g/rc-noaof-mget-c50-P1` | 5 | 123,062 | 8% | 6.15 | 2.75 | 3.37 | — | — | 0.383 | 0.519 | 0.559 | 26.2 | 0.98 | 0 |
| `base/rc-noaof-set-c50-P1` | 5 | 172,036 | 3% | 3.84 | 0.94 | 2.90 | — | — | 0.271 | 0.375 | 0.399 | 27.0 | 1.21 | 0 |
| `g/rc-noaof-set-c50-P1` | 5 | 173,905 | 4% | 3.80 | 0.90 | 2.88 | — | — | 0.271 | 0.367 | 0.399 | 27.1 | 1.22 | 0 |
| `base/rc-noaof-lpush-c50-P1` | 5 | 177,770 | 2% | 3.66 | 0.78 | 2.88 | — | — | 0.263 | 0.359 | 0.391 | 269.8 | 1.21 | 0 |
| `g/rc-noaof-lpush-c50-P1` | 5 | 181,810 | 3% | 3.64 | 0.78 | 2.85 | — | — | 0.255 | 0.351 | 0.383 | 269.4 | 1.23 | 0 |
| `base/rc-noaof-mset-c50-P1` | 5 | 96,367 | 2% | 8.38 | 4.83 | 3.51 | — | — | 0.495 | 0.655 | 0.695 | 27.1 | 0.74 | 0 |
| `g/rc-noaof-mset-c50-P1` | 5 | 97,542 | 2% | 8.22 | 4.68 | 3.56 | — | — | 0.495 | 0.647 | 0.687 | 27.3 | 0.74 | 0 |
| `base/rc-noaof-incr-c50-P1` | 5 | 175,755 | 4% | 3.79 | 0.96 | 2.85 | — | — | 0.263 | 0.367 | 0.399 | 28.5 | 1.20 | 0 |
| `g/rc-noaof-incr-c50-P1` | 5 | 173,905 | 3% | 3.79 | 0.92 | 2.89 | — | — | 0.271 | 0.367 | 0.399 | 28.5 | 1.20 | 0 |
| `base/rc-noaof-get-c50-P16` | 5 | 1,269,599 | 5% | 0.64 | 0.38 | 0.26 | — | — | 0.647 | 0.687 | 0.743 | 26.7 | 0.80 | 0 |
| `base/rc-noaof-mget-c50-P16` | 5 | 322,456 | 3% | 2.89 | 2.45 | 0.44 | — | — | 2.511 | 2.783 | 2.983 | 26.9 | 0.51 | 0 |
| `base/rc-everysec-incr-c50-P1` | 5 | 127,980 | 8% | 5.56 | 1.79 | 3.77 | — | — | 0.359 | 0.495 | 0.535 | 28.8 | 0.92 | 0 |
| `base/rc-everysec-set-c50-P1` | 5 | 122,119 | 6% | 5.59 | 1.74 | 3.89 | — | — | 0.367 | 0.503 | 0.535 | 26.9 | 0.90 | 0 |
| `g/rc-noaof-get-c50-P16` | 5 | 1,269,680 | 3% | 0.64 | 0.38 | 0.26 | — | — | 0.639 | 0.687 | 0.743 | 26.6 | 0.81 | 0 |
| `g/rc-noaof-mget-c50-P16` | 5 | 317,309 | 9% | 2.93 | 2.49 | 0.44 | — | — | 2.527 | 2.911 | 3.295 | 26.8 | 0.50 | 0 |
| `g/rc-everysec-incr-c50-P1` | 5 | 126,964 | 17% | 5.56 | 1.76 | 3.78 | — | — | 0.359 | 0.495 | 0.527 | 28.6 | 0.90 | 0 |
| `g/rc-everysec-set-c50-P1` | 5 | 124,012 | 15% | 5.58 | 1.79 | 3.81 | — | — | 0.359 | 0.495 | 0.535 | 26.9 | 0.90 | 0 |
| `redis-noaof-get-c50-P1` | 5 | 192,752 | 1% | 3.41 | 0.71 | 2.69 | — | — | 0.231 | 0.295 | 0.423 | 18.9 | 1.26 | 0 |
| `redis-noaof-set-c50-P1` | 5 | 190,431 | 2% | 3.49 | 0.73 | 2.77 | — | — | 0.239 | 0.303 | 0.431 | 19.5 | 1.24 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-incr-c50-P1` | 0.1002 | 10.0 | 6.9 | 0.0008 |
| `base/rc-everysec-set-c50-P1` | 0.1002 | 10.0 | 9.2 | 0.0009 |
| `g/rc-everysec-incr-c50-P1` | 0.1005 | 9.9 | 7.8 | 0.0008 |
| `g/rc-everysec-set-c50-P1` | 0.1001 | 10.0 | 9.2 | 0.0008 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `g/rc-noaof-get-c50-P1` / `base/rc-noaof-get-c50-P1` | **1.000** | 0.977 0.989 1.011 0.989 1.011 |
| `g/rc-noaof-mget-c50-P1` / `base/rc-noaof-mget-c50-P1` | **0.985** | 1.000 0.985 1.000 0.985 0.984 |
| `g/rc-noaof-set-c50-P1` / `base/rc-noaof-set-c50-P1` | **1.011** | 1.011 1.011 0.989 1.011 1.033 |
| `g/rc-noaof-lpush-c50-P1` / `base/rc-noaof-lpush-c50-P1` | **1.023** | 1.011 1.034 1.023 1.012 1.011 |
| `g/rc-noaof-mset-c50-P1` / `base/rc-noaof-mset-c50-P1` | **1.012** | 1.024 0.988 1.012 1.024 1.012 |
| `g/rc-noaof-incr-c50-P1` / `base/rc-noaof-incr-c50-P1` | **0.989** | 1.000 1.000 1.011 0.979 1.000 |
| `g/rc-noaof-get-c50-P16` / `base/rc-noaof-get-c50-P16` | **1.000** | 0.968 1.016 1.032 1.000 1.016 |
| `g/rc-noaof-mget-c50-P16` / `base/rc-noaof-mget-c50-P16` | **0.984** | 0.940 0.961 0.984 0.992 0.992 |
| `g/rc-everysec-incr-c50-P1` / `base/rc-everysec-incr-c50-P1` | **0.992** | 0.985 0.992 0.856 1.081 1.008 |
| `g/rc-everysec-set-c50-P1` / `base/rc-everysec-set-c50-P1` | **1.016** | 0.992 1.023 0.891 1.000 1.000 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P1` | 1 | load=1.10 io_psi10=6.82/6.78 mem_psi10=0.03/0.03 swap_used_mib=8126 dirty_wb_kib=84 settle_wait_s=38 |
| `base/rc-noaof-get-c50-P1` | 2 | load=2.40 io_psi10=0.40/0.23 mem_psi10=0.00/0.00 swap_used_mib=8035 dirty_wb_kib=2984 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 3 | load=2.50 io_psi10=0.08/0.04 mem_psi10=0.02/0.00 swap_used_mib=8035 dirty_wb_kib=2068 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 4 | load=5.05 io_psi10=1.70/1.64 mem_psi10=0.05/0.04 swap_used_mib=5347 dirty_wb_kib=3192 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 5 | load=4.36 io_psi10=1.13/1.10 mem_psi10=0.00/0.00 swap_used_mib=5347 dirty_wb_kib=3184 settle_wait_s=0 |
| `g/rc-noaof-get-c50-P1` | 1 | load=1.72 io_psi10=1.44/1.43 mem_psi10=0.14/0.14 swap_used_mib=8126 dirty_wb_kib=4628 settle_wait_s=0 |
| `g/rc-noaof-get-c50-P1` | 2 | load=2.63 io_psi10=0.65/0.40 mem_psi10=0.00/0.00 swap_used_mib=8036 dirty_wb_kib=4212 settle_wait_s=0 |
| `g/rc-noaof-get-c50-P1` | 3 | load=2.43 io_psi10=0.28/0.26 mem_psi10=0.00/0.00 swap_used_mib=8035 dirty_wb_kib=3512 settle_wait_s=0 |
| `g/rc-noaof-get-c50-P1` | 4 | load=5.91 io_psi10=7.77/7.25 mem_psi10=0.71/0.63 swap_used_mib=5347 dirty_wb_kib=80032 settle_wait_s=75 |
| `g/rc-noaof-get-c50-P1` | 5 | load=3.70 io_psi10=1.70/1.40 mem_psi10=0.00/0.00 swap_used_mib=5347 dirty_wb_kib=4352 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 1 | load=1.92 io_psi10=4.95/4.71 mem_psi10=0.51/0.51 swap_used_mib=8125 dirty_wb_kib=7104 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 2 | load=2.76 io_psi10=1.02/0.86 mem_psi10=0.00/0.00 swap_used_mib=8036 dirty_wb_kib=2892 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 3 | load=2.67 io_psi10=0.14/0.14 mem_psi10=0.00/0.00 swap_used_mib=8035 dirty_wb_kib=3312 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 4 | load=7.35 io_psi10=7.85/4.92 mem_psi10=0.01/0.00 swap_used_mib=8018 dirty_wb_kib=3004 settle_wait_s=15 |
| `base/rc-noaof-mget-c50-P1` | 5 | load=3.46 io_psi10=1.15/1.08 mem_psi10=0.00/0.00 swap_used_mib=5347 dirty_wb_kib=2680 settle_wait_s=0 |
| `g/rc-noaof-mget-c50-P1` | 1 | load=2.16 io_psi10=1.26/1.22 mem_psi10=0.08/0.08 swap_used_mib=8125 dirty_wb_kib=1344 settle_wait_s=0 |
| `g/rc-noaof-mget-c50-P1` | 2 | load=2.60 io_psi10=1.31/0.61 mem_psi10=0.00/0.00 swap_used_mib=8036 dirty_wb_kib=5328 settle_wait_s=0 |
| `g/rc-noaof-mget-c50-P1` | 3 | load=2.61 io_psi10=1.69/1.51 mem_psi10=0.00/0.00 swap_used_mib=8035 dirty_wb_kib=3052 settle_wait_s=0 |
| `g/rc-noaof-mget-c50-P1` | 4 | load=3.99 io_psi10=2.04/1.72 mem_psi10=0.00/0.00 swap_used_mib=8019 dirty_wb_kib=7296 settle_wait_s=0 |
| `g/rc-noaof-mget-c50-P1` | 5 | load=3.22 io_psi10=0.62/0.57 mem_psi10=0.00/0.00 swap_used_mib=5347 dirty_wb_kib=4476 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 1 | load=2.12 io_psi10=0.25/0.24 mem_psi10=0.01/0.01 swap_used_mib=8125 dirty_wb_kib=3196 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=2.66 io_psi10=0.38/0.35 mem_psi10=0.00/0.00 swap_used_mib=8036 dirty_wb_kib=3176 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=2.92 io_psi10=0.72/0.69 mem_psi10=0.00/0.00 swap_used_mib=8034 dirty_wb_kib=3832 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 4 | load=2.97 io_psi10=0.18/0.16 mem_psi10=0.00/0.00 swap_used_mib=8019 dirty_wb_kib=2656 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 5 | load=2.80 io_psi10=1.16/1.12 mem_psi10=0.00/0.00 swap_used_mib=5347 dirty_wb_kib=3724 settle_wait_s=0 |
| `g/rc-noaof-set-c50-P1` | 1 | load=2.40 io_psi10=0.52/0.52 mem_psi10=0.00/0.00 swap_used_mib=8125 dirty_wb_kib=1500 settle_wait_s=0 |
| `g/rc-noaof-set-c50-P1` | 2 | load=2.48 io_psi10=0.13/0.11 mem_psi10=0.00/0.00 swap_used_mib=8037 dirty_wb_kib=2992 settle_wait_s=0 |
| `g/rc-noaof-set-c50-P1` | 3 | load=2.99 io_psi10=0.89/0.68 mem_psi10=0.00/0.00 swap_used_mib=8034 dirty_wb_kib=3772 settle_wait_s=0 |
| `g/rc-noaof-set-c50-P1` | 4 | load=2.97 io_psi10=0.29/0.27 mem_psi10=0.00/0.00 swap_used_mib=8019 dirty_wb_kib=3444 settle_wait_s=0 |
| `g/rc-noaof-set-c50-P1` | 5 | load=2.63 io_psi10=0.92/0.89 mem_psi10=0.00/0.00 swap_used_mib=5347 dirty_wb_kib=2708 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 1 | load=2.60 io_psi10=0.10/0.10 mem_psi10=0.00/0.00 swap_used_mib=8124 dirty_wb_kib=1372 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | load=2.66 io_psi10=0.36/0.36 mem_psi10=0.00/0.00 swap_used_mib=8037 dirty_wb_kib=2072 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | load=3.45 io_psi10=0.65/0.45 mem_psi10=0.00/0.00 swap_used_mib=8034 dirty_wb_kib=3592 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 4 | load=2.98 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=8019 dirty_wb_kib=2056 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 5 | load=2.83 io_psi10=0.67/0.63 mem_psi10=0.00/0.00 swap_used_mib=5346 dirty_wb_kib=2140 settle_wait_s=0 |
| `g/rc-noaof-lpush-c50-P1` | 1 | load=2.96 io_psi10=2.60/1.97 mem_psi10=0.00/0.00 swap_used_mib=8076 dirty_wb_kib=4484 settle_wait_s=0 |
| `g/rc-noaof-lpush-c50-P1` | 2 | load=2.56 io_psi10=0.13/0.10 mem_psi10=0.00/0.00 swap_used_mib=8037 dirty_wb_kib=4604 settle_wait_s=0 |
| `g/rc-noaof-lpush-c50-P1` | 3 | load=3.50 io_psi10=0.19/0.12 mem_psi10=0.00/0.00 swap_used_mib=8034 dirty_wb_kib=4812 settle_wait_s=0 |
| `g/rc-noaof-lpush-c50-P1` | 4 | load=3.14 io_psi10=0.37/0.34 mem_psi10=0.00/0.00 swap_used_mib=8019 dirty_wb_kib=2028 settle_wait_s=0 |
| `g/rc-noaof-lpush-c50-P1` | 5 | load=2.89 io_psi10=0.44/0.43 mem_psi10=0.18/0.18 swap_used_mib=5346 dirty_wb_kib=3572 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 1 | load=3.05 io_psi10=3.20/2.54 mem_psi10=0.00/0.00 swap_used_mib=8074 dirty_wb_kib=3468 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 2 | load=2.80 io_psi10=0.21/0.21 mem_psi10=0.00/0.00 swap_used_mib=8037 dirty_wb_kib=2148 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 3 | load=3.55 io_psi10=0.48/0.41 mem_psi10=0.00/0.00 swap_used_mib=8034 dirty_wb_kib=1944 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 4 | load=2.87 io_psi10=0.47/0.40 mem_psi10=0.00/0.00 swap_used_mib=8019 dirty_wb_kib=2660 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 5 | load=2.93 io_psi10=1.03/0.97 mem_psi10=0.01/0.01 swap_used_mib=5346 dirty_wb_kib=2308 settle_wait_s=0 |
| `g/rc-noaof-mset-c50-P1` | 1 | load=3.03 io_psi10=0.65/0.40 mem_psi10=0.00/0.00 swap_used_mib=8074 dirty_wb_kib=2876 settle_wait_s=0 |
| `g/rc-noaof-mset-c50-P1` | 2 | load=3.03 io_psi10=0.77/0.73 mem_psi10=0.00/0.00 swap_used_mib=8037 dirty_wb_kib=3124 settle_wait_s=0 |
| `g/rc-noaof-mset-c50-P1` | 3 | load=3.48 io_psi10=0.13/0.12 mem_psi10=0.00/0.00 swap_used_mib=8034 dirty_wb_kib=3460 settle_wait_s=0 |
| `g/rc-noaof-mset-c50-P1` | 4 | load=3.30 io_psi10=0.61/0.52 mem_psi10=0.00/0.00 swap_used_mib=8020 dirty_wb_kib=2852 settle_wait_s=0 |
| `g/rc-noaof-mset-c50-P1` | 5 | load=2.59 io_psi10=0.33/0.32 mem_psi10=0.00/0.00 swap_used_mib=5345 dirty_wb_kib=3208 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 1 | load=2.81 io_psi10=1.40/1.37 mem_psi10=0.00/0.00 swap_used_mib=8074 dirty_wb_kib=2696 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 2 | load=3.09 io_psi10=0.74/0.68 mem_psi10=0.00/0.00 swap_used_mib=8037 dirty_wb_kib=3008 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 3 | load=3.33 io_psi10=0.52/0.42 mem_psi10=0.00/0.00 swap_used_mib=8033 dirty_wb_kib=2876 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 4 | load=3.65 io_psi10=0.32/0.31 mem_psi10=0.00/0.00 swap_used_mib=8020 dirty_wb_kib=3200 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 5 | load=2.12 io_psi10=1.01/1.01 mem_psi10=0.00/0.00 swap_used_mib=5345 dirty_wb_kib=2252 settle_wait_s=0 |
| `g/rc-noaof-incr-c50-P1` | 1 | load=2.96 io_psi10=0.25/0.20 mem_psi10=0.00/0.00 swap_used_mib=8074 dirty_wb_kib=1972 settle_wait_s=0 |
| `g/rc-noaof-incr-c50-P1` | 2 | load=2.84 io_psi10=1.28/1.15 mem_psi10=0.00/0.00 swap_used_mib=8037 dirty_wb_kib=2876 settle_wait_s=0 |
| `g/rc-noaof-incr-c50-P1` | 3 | load=3.48 io_psi10=2.04/1.79 mem_psi10=0.00/0.00 swap_used_mib=8033 dirty_wb_kib=4556 settle_wait_s=0 |
| `g/rc-noaof-incr-c50-P1` | 4 | load=3.78 io_psi10=0.90/0.78 mem_psi10=0.00/0.00 swap_used_mib=8020 dirty_wb_kib=2456 settle_wait_s=0 |
| `g/rc-noaof-incr-c50-P1` | 5 | load=2.49 io_psi10=1.21/1.03 mem_psi10=0.00/0.00 swap_used_mib=5345 dirty_wb_kib=3928 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 1 | load=3.24 io_psi10=0.06/0.03 mem_psi10=0.00/0.00 swap_used_mib=8073 dirty_wb_kib=3000 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 2 | load=2.90 io_psi10=0.57/0.32 mem_psi10=0.00/0.00 swap_used_mib=8037 dirty_wb_kib=7352 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 3 | load=3.91 io_psi10=1.64/0.82 mem_psi10=0.00/0.00 swap_used_mib=8032 dirty_wb_kib=8384 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 4 | load=3.16 io_psi10=0.46/0.30 mem_psi10=0.00/0.00 swap_used_mib=8020 dirty_wb_kib=2596 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 5 | load=2.51 io_psi10=0.33/0.29 mem_psi10=0.00/0.00 swap_used_mib=5345 dirty_wb_kib=2660 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 1 | load=2.97 io_psi10=0.54/0.36 mem_psi10=0.00/0.00 swap_used_mib=8073 dirty_wb_kib=3444 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 2 | load=3.25 io_psi10=7.34/2.52 mem_psi10=0.00/0.00 swap_used_mib=8037 dirty_wb_kib=4692 settle_wait_s=2 |
| `base/rc-noaof-mget-c50-P16` | 3 | load=3.79 io_psi10=1.83/1.45 mem_psi10=0.00/0.00 swap_used_mib=8032 dirty_wb_kib=3376 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 4 | load=3.88 io_psi10=7.44/4.08 mem_psi10=0.00/0.00 swap_used_mib=8020 dirty_wb_kib=5728 settle_wait_s=4 |
| `base/rc-noaof-mget-c50-P16` | 5 | load=3.36 io_psi10=2.51/1.84 mem_psi10=0.00/0.00 swap_used_mib=5345 dirty_wb_kib=3292 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 1 | load=2.67 io_psi10=0.25/0.23 mem_psi10=0.00/0.00 swap_used_mib=8073 dirty_wb_kib=1712 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 2 | load=3.43 io_psi10=7.05/5.05 mem_psi10=0.00/0.00 swap_used_mib=8037 dirty_wb_kib=5776 settle_wait_s=11 |
| `base/rc-everysec-incr-c50-P1` | 3 | load=3.26 io_psi10=0.15/0.12 mem_psi10=0.00/0.00 swap_used_mib=8032 dirty_wb_kib=3492 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 4 | load=2.86 io_psi10=7.61/3.42 mem_psi10=0.00/0.00 swap_used_mib=8020 dirty_wb_kib=2944 settle_wait_s=3 |
| `base/rc-everysec-incr-c50-P1` | 5 | load=2.67 io_psi10=0.47/0.40 mem_psi10=0.00/0.00 swap_used_mib=5345 dirty_wb_kib=4344 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 1 | load=3.27 io_psi10=7.70/4.37 mem_psi10=0.00/0.00 swap_used_mib=8064 dirty_wb_kib=3340 settle_wait_s=9 |
| `base/rc-everysec-set-c50-P1` | 2 | load=2.84 io_psi10=2.29/1.71 mem_psi10=0.00/0.00 swap_used_mib=8038 dirty_wb_kib=4452 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 3 | load=3.41 io_psi10=6.34/1.37 mem_psi10=0.00/0.00 swap_used_mib=8032 dirty_wb_kib=3328 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 4 | load=2.51 io_psi10=1.47/1.28 mem_psi10=0.00/0.00 swap_used_mib=8020 dirty_wb_kib=3596 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 5 | load=2.53 io_psi10=5.23/0.85 mem_psi10=0.01/0.00 swap_used_mib=5344 dirty_wb_kib=3480 settle_wait_s=0 |
| `g/rc-noaof-get-c50-P16` | 1 | load=3.27 io_psi10=7.91/3.19 mem_psi10=0.00/0.00 swap_used_mib=8063 dirty_wb_kib=3896 settle_wait_s=7 |
| `g/rc-noaof-get-c50-P16` | 2 | load=2.99 io_psi10=2.36/1.53 mem_psi10=0.00/0.00 swap_used_mib=8038 dirty_wb_kib=8964 settle_wait_s=0 |
| `g/rc-noaof-get-c50-P16` | 3 | load=3.33 io_psi10=7.77/4.35 mem_psi10=0.00/0.00 swap_used_mib=8032 dirty_wb_kib=3816 settle_wait_s=5 |
| `g/rc-noaof-get-c50-P16` | 4 | load=2.56 io_psi10=1.49/0.86 mem_psi10=0.00/0.00 swap_used_mib=8021 dirty_wb_kib=2640 settle_wait_s=0 |
| `g/rc-noaof-get-c50-P16` | 5 | load=2.38 io_psi10=5.57/0.50 mem_psi10=0.00/0.00 swap_used_mib=5344 dirty_wb_kib=1820 settle_wait_s=0 |
| `g/rc-noaof-mget-c50-P16` | 1 | load=3.53 io_psi10=3.10/1.78 mem_psi10=0.00/0.00 swap_used_mib=8060 dirty_wb_kib=4432 settle_wait_s=0 |
| `g/rc-noaof-mget-c50-P16` | 2 | load=3.86 io_psi10=6.90/2.41 mem_psi10=0.00/0.00 swap_used_mib=8040 dirty_wb_kib=4100 settle_wait_s=2 |
| `g/rc-noaof-mget-c50-P16` | 3 | load=3.06 io_psi10=2.06/1.45 mem_psi10=0.00/0.00 swap_used_mib=8032 dirty_wb_kib=3388 settle_wait_s=0 |
| `g/rc-noaof-mget-c50-P16` | 4 | load=2.65 io_psi10=7.67/4.34 mem_psi10=0.00/0.00 swap_used_mib=8029 dirty_wb_kib=4336 settle_wait_s=6 |
| `g/rc-noaof-mget-c50-P16` | 5 | load=2.30 io_psi10=1.97/0.82 mem_psi10=0.00/0.00 swap_used_mib=5344 dirty_wb_kib=3528 settle_wait_s=0 |
| `g/rc-everysec-incr-c50-P1` | 1 | load=3.24 io_psi10=2.65/2.19 mem_psi10=0.00/0.00 swap_used_mib=8058 dirty_wb_kib=5652 settle_wait_s=0 |
| `g/rc-everysec-incr-c50-P1` | 2 | load=3.72 io_psi10=7.59/4.98 mem_psi10=0.00/0.00 swap_used_mib=8041 dirty_wb_kib=9476 settle_wait_s=12 |
| `g/rc-everysec-incr-c50-P1` | 3 | load=2.65 io_psi10=0.52/0.28 mem_psi10=0.03/0.00 swap_used_mib=8031 dirty_wb_kib=2808 settle_wait_s=0 |
| `g/rc-everysec-incr-c50-P1` | 4 | load=3.11 io_psi10=6.02/0.57 mem_psi10=0.00/0.00 swap_used_mib=8030 dirty_wb_kib=2720 settle_wait_s=0 |
| `g/rc-everysec-incr-c50-P1` | 5 | load=1.99 io_psi10=0.45/0.24 mem_psi10=0.00/0.00 swap_used_mib=5344 dirty_wb_kib=3960 settle_wait_s=0 |
| `g/rc-everysec-set-c50-P1` | 1 | load=3.17 io_psi10=6.84/3.53 mem_psi10=0.00/0.00 swap_used_mib=8050 dirty_wb_kib=5188 settle_wait_s=8 |
| `g/rc-everysec-set-c50-P1` | 2 | load=3.60 io_psi10=0.86/0.72 mem_psi10=0.00/0.00 swap_used_mib=8044 dirty_wb_kib=6852 settle_wait_s=0 |
| `g/rc-everysec-set-c50-P1` | 3 | load=2.71 io_psi10=27.80/26.26 mem_psi10=0.00/0.00 swap_used_mib=8030 dirty_wb_kib=14684 settle_wait_s=90 |
| `g/rc-everysec-set-c50-P1` | 4 | load=3.37 io_psi10=1.88/1.64 mem_psi10=0.00/0.00 swap_used_mib=8030 dirty_wb_kib=2956 settle_wait_s=0 |
| `g/rc-everysec-set-c50-P1` | 5 | load=1.90 io_psi10=5.24/0.35 mem_psi10=0.00/0.00 swap_used_mib=5344 dirty_wb_kib=5532 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 1 | load=3.11 io_psi10=7.43/5.06 mem_psi10=0.00/0.00 swap_used_mib=8050 dirty_wb_kib=5384 settle_wait_s=11 |
| `redis-noaof-get-c50-P1` | 2 | load=3.73 io_psi10=1.85/1.29 mem_psi10=0.00/0.00 swap_used_mib=8045 dirty_wb_kib=5692 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 3 | load=1.97 io_psi10=28.87/27.37 mem_psi10=0.00/0.00 swap_used_mib=8030 dirty_wb_kib=16876 settle_wait_s=90 |
| `redis-noaof-get-c50-P1` | 4 | load=2.36 io_psi10=0.69/0.58 mem_psi10=0.00/0.00 swap_used_mib=8030 dirty_wb_kib=3088 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 5 | load=2.15 io_psi10=4.54/0.89 mem_psi10=0.00/0.00 swap_used_mib=5344 dirty_wb_kib=3948 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 1 | load=3.13 io_psi10=1.40/1.14 mem_psi10=0.00/0.00 swap_used_mib=8049 dirty_wb_kib=2336 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 2 | load=3.22 io_psi10=1.74/1.11 mem_psi10=0.00/0.00 swap_used_mib=8045 dirty_wb_kib=6912 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 3 | load=2.21 io_psi10=4.67/4.34 mem_psi10=0.00/0.00 swap_used_mib=8030 dirty_wb_kib=3128 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 4 | load=2.31 io_psi10=0.71/0.67 mem_psi10=0.00/0.00 swap_used_mib=8030 dirty_wb_kib=4232 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 5 | load=2.23 io_psi10=0.97/0.54 mem_psi10=0.00/0.00 swap_used_mib=5344 dirty_wb_kib=1712 settle_wait_s=0 |
