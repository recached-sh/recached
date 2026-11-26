## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P1` | 5 | 186,021 | 10% | 3.55 | 0.80 | 2.77 | — | — | 0.255 | 0.343 | 0.375 | 25.4 | 1.25 | 0 |
| `h/rc-noaof-get-c50-P1` | 5 | 183,883 | 21% | 3.56 | 0.81 | 2.77 | — | — | 0.255 | 0.351 | 0.383 | 25.4 | 1.24 | 0 |
| `base/rc-noaof-set-c50-P1` | 5 | 181,793 | 22% | 3.67 | 0.88 | 2.79 | — | — | 0.263 | 0.359 | 0.383 | 26.1 | 1.23 | 0 |
| `h/rc-noaof-set-c50-P1` | 5 | 179,751 | 3% | 3.71 | 0.89 | 2.82 | — | — | 0.263 | 0.359 | 0.391 | 26.2 | 1.23 | 0 |
| `base/rc-noaof-incr-c50-P1` | 5 | 181,810 | 13% | 3.64 | 0.88 | 2.77 | — | — | 0.263 | 0.351 | 0.383 | 27.4 | 1.22 | 0 |
| `h/rc-noaof-incr-c50-P1` | 5 | 179,767 | 7% | 3.69 | 0.92 | 2.81 | — | — | 0.263 | 0.359 | 0.391 | 27.7 | 1.21 | 0 |
| `base/rc-noaof-hset-c50-P1` | 5 | 177,762 | 31% | 3.74 | 0.93 | 2.82 | — | — | 0.263 | 0.359 | 0.391 | 24.4 | 1.21 | 0 |
| `h/rc-noaof-hset-c50-P1` | 5 | 177,770 | 53% | 3.76 | 0.93 | 2.83 | — | — | 0.271 | 0.367 | 0.391 | 24.6 | 1.21 | 0 |
| `base/rc-noaof-lpush-c50-P1` | 5 | 183,891 | 2% | 3.61 | 0.79 | 2.81 | — | — | 0.255 | 0.351 | 0.375 | 268.4 | 1.24 | 0 |
| `h/rc-noaof-lpush-c50-P1` | 5 | 183,900 | 20% | 3.55 | 0.77 | 2.79 | — | — | 0.255 | 0.351 | 0.375 | 268.5 | 1.23 | 0 |
| `base/rc-noaof-mset-c50-P1` | 5 | 102,548 | 57% | 7.86 | 4.38 | 3.48 | — | — | 0.471 | 0.615 | 0.655 | 26.2 | 0.75 | 0 |
| `h/rc-noaof-mset-c50-P1` | 5 | 99,980 | 4% | 7.93 | 4.38 | 3.50 | — | — | 0.479 | 0.631 | 0.671 | 26.1 | 0.75 | 0 |
| `base/rc-noaof-get-c50-P16` | 5 | 1,311,304 | 25% | 0.61 | 0.36 | 0.25 | — | — | 0.615 | 0.663 | 0.703 | 25.3 | 0.86 | 0 |
| `base/rc-noaof-incr-c50-P16` | 5 | 1,159,152 | 27% | 0.71 | 0.45 | 0.27 | — | — | 0.703 | 0.759 | 0.839 | 27.3 | 0.72 | 0 |
| `base/rc-noaof-mget-c50-P16` | 5 | 344,732 | 28% | 2.71 | 2.29 | 0.41 | — | — | 2.359 | 2.543 | 2.911 | 25.5 | 0.52 | 0 |
| `h/rc-noaof-get-c50-P16` | 5 | 1,333,156 | 5% | 0.61 | 0.35 | 0.25 | — | — | 0.615 | 0.663 | 0.719 | 25.7 | 0.84 | 0 |
| `h/rc-noaof-incr-c50-P16` | 5 | 1,159,219 | 4% | 0.71 | 0.45 | 0.26 | — | — | 0.695 | 0.751 | 0.839 | 27.5 | 0.74 | 0 |
| `h/rc-noaof-mget-c50-P16` | 5 | 344,732 | 10% | 2.69 | 2.29 | 0.41 | — | — | 2.343 | 2.543 | 2.783 | 25.9 | 0.52 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P1` | 1 | load=0.92 io_psi10=7.11/7.10 mem_psi10=2.32/2.31 swap_used_mib=8180 dirty_wb_kib=312 settle_wait_s=85 |
| `base/rc-noaof-get-c50-P1` | 2 | load=3.01 io_psi10=0.15/0.14 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=1208 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 3 | load=3.01 io_psi10=0.45/0.45 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=1900 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 4 | load=2.89 io_psi10=0.19/0.16 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=2048 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 5 | load=2.81 io_psi10=0.28/0.27 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=2824 settle_wait_s=0 |
| `h/rc-noaof-get-c50-P1` | 1 | load=2.34 io_psi10=1.12/0.93 mem_psi10=0.17/0.17 swap_used_mib=8180 dirty_wb_kib=1392 settle_wait_s=0 |
| `h/rc-noaof-get-c50-P1` | 2 | load=3.34 io_psi10=0.50/0.47 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=2976 settle_wait_s=0 |
| `h/rc-noaof-get-c50-P1` | 3 | load=3.06 io_psi10=0.36/0.33 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=3896 settle_wait_s=0 |
| `h/rc-noaof-get-c50-P1` | 4 | load=2.84 io_psi10=0.61/0.54 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=4544 settle_wait_s=0 |
| `h/rc-noaof-get-c50-P1` | 5 | load=2.87 io_psi10=0.61/0.61 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=1772 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 1 | load=3.25 io_psi10=0.77/0.32 mem_psi10=0.65/0.21 swap_used_mib=8180 dirty_wb_kib=1828 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=3.79 io_psi10=1.26/1.23 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=1488 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=3.11 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=1996 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 4 | load=2.76 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=1276 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 5 | load=2.84 io_psi10=0.29/0.22 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=2412 settle_wait_s=0 |
| `h/rc-noaof-set-c50-P1` | 1 | load=1.88 io_psi10=68.57/68.43 mem_psi10=0.00/0.00 swap_used_mib=8157 dirty_wb_kib=720 settle_wait_s=90 |
| `h/rc-noaof-set-c50-P1` | 2 | load=4.23 io_psi10=2.29/2.17 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=460 settle_wait_s=0 |
| `h/rc-noaof-set-c50-P1` | 3 | load=4.08 io_psi10=0.14/0.09 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=2700 settle_wait_s=0 |
| `h/rc-noaof-set-c50-P1` | 4 | load=2.63 io_psi10=0.36/0.36 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=2192 settle_wait_s=0 |
| `h/rc-noaof-set-c50-P1` | 5 | load=2.82 io_psi10=0.36/0.35 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=2900 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 1 | load=1.43 io_psi10=60.23/59.82 mem_psi10=0.00/0.00 swap_used_mib=8156 dirty_wb_kib=2024 settle_wait_s=90 |
| `base/rc-noaof-incr-c50-P1` | 2 | load=4.96 io_psi10=7.44/6.22 mem_psi10=0.04/0.02 swap_used_mib=8191 dirty_wb_kib=586156 settle_wait_s=10 |
| `base/rc-noaof-incr-c50-P1` | 3 | load=3.63 io_psi10=1.82/1.61 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=8916 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 4 | load=2.53 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=2484 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 5 | load=2.89 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=2480 settle_wait_s=0 |
| `h/rc-noaof-incr-c50-P1` | 1 | load=0.92 io_psi10=7.31/7.29 mem_psi10=0.00/0.00 swap_used_mib=8156 dirty_wb_kib=12 settle_wait_s=85 |
| `h/rc-noaof-incr-c50-P1` | 2 | load=6.71 io_psi10=4.54/2.71 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=416340 settle_wait_s=0 |
| `h/rc-noaof-incr-c50-P1` | 3 | load=3.63 io_psi10=7.79/5.75 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=543840 settle_wait_s=0 |
| `h/rc-noaof-incr-c50-P1` | 4 | load=2.34 io_psi10=0.47/0.46 mem_psi10=0.00/0.00 swap_used_mib=8187 dirty_wb_kib=3324 settle_wait_s=0 |
| `h/rc-noaof-incr-c50-P1` | 5 | load=2.86 io_psi10=0.69/0.67 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=3604 settle_wait_s=0 |
| `base/rc-noaof-hset-c50-P1` | 1 | load=1.37 io_psi10=0.68/0.68 mem_psi10=0.00/0.00 swap_used_mib=8156 dirty_wb_kib=1652 settle_wait_s=0 |
| `base/rc-noaof-hset-c50-P1` | 2 | load=6.89 io_psi10=7.98/0.72 mem_psi10=0.00/0.00 swap_used_mib=8185 dirty_wb_kib=361836 settle_wait_s=7 |
| `base/rc-noaof-hset-c50-P1` | 3 | load=3.76 io_psi10=3.87/3.68 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=570812 settle_wait_s=0 |
| `base/rc-noaof-hset-c50-P1` | 4 | load=2.44 io_psi10=0.26/0.24 mem_psi10=0.00/0.00 swap_used_mib=8187 dirty_wb_kib=2084 settle_wait_s=0 |
| `base/rc-noaof-hset-c50-P1` | 5 | load=2.90 io_psi10=0.46/0.46 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=3208 settle_wait_s=0 |
| `h/rc-noaof-hset-c50-P1` | 1 | load=1.78 io_psi10=0.19/0.19 mem_psi10=0.00/0.00 swap_used_mib=8156 dirty_wb_kib=2736 settle_wait_s=0 |
| `h/rc-noaof-hset-c50-P1` | 2 | load=5.21 io_psi10=0.90/0.75 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=16292 settle_wait_s=0 |
| `h/rc-noaof-hset-c50-P1` | 3 | load=3.64 io_psi10=5.50/5.35 mem_psi10=0.01/0.01 swap_used_mib=8191 dirty_wb_kib=582300 settle_wait_s=0 |
| `h/rc-noaof-hset-c50-P1` | 4 | load=2.25 io_psi10=0.67/0.65 mem_psi10=0.00/0.00 swap_used_mib=8187 dirty_wb_kib=2900 settle_wait_s=0 |
| `h/rc-noaof-hset-c50-P1` | 5 | load=2.85 io_psi10=0.20/0.18 mem_psi10=0.00/0.00 swap_used_mib=8185 dirty_wb_kib=2540 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 1 | load=2.22 io_psi10=6.84/6.34 mem_psi10=0.30/0.30 swap_used_mib=8191 dirty_wb_kib=924 settle_wait_s=36 |
| `base/rc-noaof-lpush-c50-P1` | 2 | load=6.10 io_psi10=2.73/1.24 mem_psi10=0.10/0.00 swap_used_mib=8186 dirty_wb_kib=71960 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | load=2.48 io_psi10=7.71/7.62 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=240 settle_wait_s=36 |
| `base/rc-noaof-lpush-c50-P1` | 4 | load=2.22 io_psi10=0.09/0.09 mem_psi10=0.00/0.00 swap_used_mib=8187 dirty_wb_kib=2092 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 5 | load=2.89 io_psi10=0.56/0.53 mem_psi10=0.00/0.00 swap_used_mib=8185 dirty_wb_kib=3388 settle_wait_s=0 |
| `h/rc-noaof-lpush-c50-P1` | 1 | load=2.27 io_psi10=1.30/1.26 mem_psi10=0.02/0.02 swap_used_mib=8191 dirty_wb_kib=740 settle_wait_s=0 |
| `h/rc-noaof-lpush-c50-P1` | 2 | load=6.21 io_psi10=7.74/1.11 mem_psi10=0.80/0.07 swap_used_mib=8186 dirty_wb_kib=72208 settle_wait_s=0 |
| `h/rc-noaof-lpush-c50-P1` | 3 | load=2.50 io_psi10=1.21/1.20 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=24260 settle_wait_s=0 |
| `h/rc-noaof-lpush-c50-P1` | 4 | load=2.05 io_psi10=0.41/0.40 mem_psi10=0.00/0.00 swap_used_mib=8187 dirty_wb_kib=1756 settle_wait_s=0 |
| `h/rc-noaof-lpush-c50-P1` | 5 | load=2.87 io_psi10=0.20/0.18 mem_psi10=0.00/0.00 swap_used_mib=8185 dirty_wb_kib=2556 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 1 | load=2.25 io_psi10=1.60/1.57 mem_psi10=0.19/0.17 swap_used_mib=8191 dirty_wb_kib=4548 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 2 | load=3.13 io_psi10=6.95/6.62 mem_psi10=0.12/0.10 swap_used_mib=8186 dirty_wb_kib=14764 settle_wait_s=18 |
| `base/rc-noaof-mset-c50-P1` | 3 | load=2.68 io_psi10=2.32/2.25 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=2448 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 4 | load=1.96 io_psi10=0.41/0.34 mem_psi10=0.00/0.00 swap_used_mib=8187 dirty_wb_kib=2528 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 5 | load=2.90 io_psi10=0.65/0.65 mem_psi10=0.00/0.00 swap_used_mib=8185 dirty_wb_kib=2664 settle_wait_s=0 |
| `h/rc-noaof-mset-c50-P1` | 1 | load=2.16 io_psi10=1.61/1.41 mem_psi10=0.01/0.01 swap_used_mib=8191 dirty_wb_kib=1152 settle_wait_s=0 |
| `h/rc-noaof-mset-c50-P1` | 2 | load=4.03 io_psi10=7.45/6.38 mem_psi10=0.92/0.88 swap_used_mib=8186 dirty_wb_kib=325796 settle_wait_s=21 |
| `h/rc-noaof-mset-c50-P1` | 3 | load=2.57 io_psi10=0.65/0.65 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=1884 settle_wait_s=0 |
| `h/rc-noaof-mset-c50-P1` | 4 | load=1.96 io_psi10=0.97/0.91 mem_psi10=0.00/0.00 swap_used_mib=8187 dirty_wb_kib=1844 settle_wait_s=0 |
| `h/rc-noaof-mset-c50-P1` | 5 | load=2.93 io_psi10=0.30/0.27 mem_psi10=0.00/0.00 swap_used_mib=8185 dirty_wb_kib=2380 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 1 | load=2.29 io_psi10=0.43/0.41 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=1556 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 2 | load=5.01 io_psi10=27.06/19.63 mem_psi10=0.00/0.00 swap_used_mib=8186 dirty_wb_kib=711324 settle_wait_s=90 |
| `base/rc-noaof-get-c50-P16` | 3 | load=2.35 io_psi10=0.98/0.67 mem_psi10=0.00/0.00 swap_used_mib=8191 dirty_wb_kib=2252 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 4 | load=1.85 io_psi10=1.12/1.05 mem_psi10=0.00/0.00 swap_used_mib=8187 dirty_wb_kib=4572 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 5 | load=2.71 io_psi10=0.67/0.67 mem_psi10=0.00/0.00 swap_used_mib=8185 dirty_wb_kib=1824 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P16` | 1 | load=2.22 io_psi10=0.62/0.46 mem_psi10=0.44/0.29 swap_used_mib=8191 dirty_wb_kib=660 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P16` | 2 | load=4.05 io_psi10=14.42/13.05 mem_psi10=0.09/0.07 swap_used_mib=8186 dirty_wb_kib=614732 settle_wait_s=90 |
| `base/rc-noaof-incr-c50-P16` | 3 | load=6.98 io_psi10=7.50/2.58 mem_psi10=0.35/0.13 swap_used_mib=8189 dirty_wb_kib=313820 settle_wait_s=40 |
| `base/rc-noaof-incr-c50-P16` | 4 | load=1.79 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=8187 dirty_wb_kib=2072 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P16` | 5 | load=2.62 io_psi10=0.35/0.35 mem_psi10=0.00/0.00 swap_used_mib=8185 dirty_wb_kib=2216 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 1 | load=2.15 io_psi10=1.47/1.36 mem_psi10=0.05/0.03 swap_used_mib=8190 dirty_wb_kib=2608 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 2 | load=2.38 io_psi10=6.60/5.89 mem_psi10=1.09/0.97 swap_used_mib=8187 dirty_wb_kib=630112 settle_wait_s=44 |
| `base/rc-noaof-mget-c50-P16` | 3 | load=6.60 io_psi10=4.74/4.00 mem_psi10=0.00/0.00 swap_used_mib=8189 dirty_wb_kib=630872 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 4 | load=2.07 io_psi10=1.31/1.31 mem_psi10=0.00/0.00 swap_used_mib=8188 dirty_wb_kib=2576 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 5 | load=2.45 io_psi10=1.53/1.50 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=2824 settle_wait_s=0 |
| `h/rc-noaof-get-c50-P16` | 1 | load=2.29 io_psi10=0.55/0.55 mem_psi10=0.17/0.16 swap_used_mib=8190 dirty_wb_kib=1896 settle_wait_s=0 |
| `h/rc-noaof-get-c50-P16` | 2 | load=2.31 io_psi10=2.26/1.98 mem_psi10=0.13/0.10 swap_used_mib=8187 dirty_wb_kib=1664 settle_wait_s=0 |
| `h/rc-noaof-get-c50-P16` | 3 | load=4.84 io_psi10=7.01/5.93 mem_psi10=0.02/0.00 swap_used_mib=8189 dirty_wb_kib=1280 settle_wait_s=0 |
| `h/rc-noaof-get-c50-P16` | 4 | load=2.09 io_psi10=0.11/0.08 mem_psi10=0.00/0.00 swap_used_mib=8188 dirty_wb_kib=2124 settle_wait_s=0 |
| `h/rc-noaof-get-c50-P16` | 5 | load=2.20 io_psi10=0.53/0.52 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=4256 settle_wait_s=0 |
| `h/rc-noaof-incr-c50-P16` | 1 | load=2.36 io_psi10=5.64/5.14 mem_psi10=0.29/0.16 swap_used_mib=8190 dirty_wb_kib=36220 settle_wait_s=0 |
| `h/rc-noaof-incr-c50-P16` | 2 | load=2.07 io_psi10=7.66/6.29 mem_psi10=1.03/0.78 swap_used_mib=8187 dirty_wb_kib=64576 settle_wait_s=0 |
| `h/rc-noaof-incr-c50-P16` | 3 | load=4.03 io_psi10=3.55/3.34 mem_psi10=0.00/0.00 swap_used_mib=8188 dirty_wb_kib=2492 settle_wait_s=0 |
| `h/rc-noaof-incr-c50-P16` | 4 | load=2.24 io_psi10=0.28/0.28 mem_psi10=0.00/0.00 swap_used_mib=8188 dirty_wb_kib=2184 settle_wait_s=0 |
| `h/rc-noaof-incr-c50-P16` | 5 | load=2.22 io_psi10=1.18/1.18 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=1452 settle_wait_s=0 |
| `h/rc-noaof-mget-c50-P16` | 1 | load=2.26 io_psi10=3.10/3.01 mem_psi10=0.03/0.02 swap_used_mib=8190 dirty_wb_kib=976 settle_wait_s=0 |
| `h/rc-noaof-mget-c50-P16` | 2 | load=2.19 io_psi10=0.17/0.16 mem_psi10=0.03/0.03 swap_used_mib=8187 dirty_wb_kib=1824 settle_wait_s=0 |
| `h/rc-noaof-mget-c50-P16` | 3 | load=3.45 io_psi10=1.45/1.30 mem_psi10=0.00/0.00 swap_used_mib=8188 dirty_wb_kib=2484 settle_wait_s=0 |
| `h/rc-noaof-mget-c50-P16` | 4 | load=2.74 io_psi10=0.14/0.14 mem_psi10=0.00/0.00 swap_used_mib=8188 dirty_wb_kib=3560 settle_wait_s=0 |
| `h/rc-noaof-mget-c50-P16` | 5 | load=2.17 io_psi10=0.30/0.30 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=2320 settle_wait_s=0 |
