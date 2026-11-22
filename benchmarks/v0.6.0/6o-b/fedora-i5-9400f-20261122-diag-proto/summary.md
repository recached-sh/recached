## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-c50-P1` | 2 | 153,846 | 0% | 4.20 | 1.06 | 3.15 | 5,305 | 5,929 | 0.319 | 0.355 | 0.395 | 25.7 | 1.20 | 0 |
| `p0/rc-noaof-set-c50-P1` | 2 | 156,910 | 4% | 4.14 | 1.04 | 3.11 | 5,330 | 5,811 | 0.315 | 0.351 | 0.391 | 26.2 | 1.22 | 0 |
| `py/rc-noaof-set-c50-P1` | 2 | 166,542 | 0% | 3.97 | 1.09 | 2.88 | 6,051 | 5,927 | 0.271 | 0.383 | 0.427 | 26.0 | 1.17 | 0 |
| `pc/rc-noaof-set-c50-P1` | 2 | 160,245 | 8% | 4.06 | 0.98 | 3.08 | 4,965 | 5,546 | 0.311 | 0.347 | 0.387 | 25.9 | 1.24 | 0 |
| `pa/rc-noaof-set-c50-P1` | 2 | 173,837 | 0% | 3.83 | 0.98 | 2.84 | 5,684 | 5,512 | 0.267 | 0.371 | 0.411 | 26.2 | 1.19 | 0 |
| `redis-noaof-set-c50-P1` | 2 | 186,113 | 5% | 3.51 | 0.77 | 2.75 | 4,836 | 4,121 | 0.235 | 0.311 | 0.435 | 19.6 | 1.21 | 0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | 135,632 | 3% | 5.00 | 1.53 | 3.47 | 8,235 | 8,425 | 0.371 | 0.403 | 0.443 | 91.8 | 1.07 | 0 |
| `p0/rc-noaof-lpush-c50-P1` | 2 | 133,324 | 0% | 5.10 | 1.56 | 3.54 | 8,265 | 8,450 | 0.375 | 0.411 | 0.451 | 91.7 | 1.06 | 0 |
| `py/rc-noaof-lpush-c50-P1` | 2 | 156,898 | 4% | 4.29 | 1.24 | 3.05 | 8,959 | 6,816 | 0.291 | 0.403 | 0.455 | 91.9 | 1.08 | 0 |
| `pc/rc-noaof-lpush-c50-P1` | 2 | 138,076 | 7% | 4.84 | 1.40 | 3.44 | 7,897 | 7,769 | 0.363 | 0.395 | 0.435 | 91.9 | 1.09 | 0 |
| `pa/rc-noaof-lpush-c50-P1` | 2 | 159,962 | 0% | 4.24 | 1.27 | 2.98 | 8,602 | 6,681 | 0.287 | 0.399 | 0.447 | 91.8 | 1.10 | 0 |
| `redis-noaof-lpush-c50-P1` | 2 | 190,458 | 0% | 3.29 | 0.64 | 2.66 | 5,204 | 3,311 | 0.231 | 0.295 | 0.415 | 45.1 | 1.24 | 0 |
| `base/rc-noaof-set-c1-P1` | 2 | 36,324 | 0% | 12.80 | 3.02 | 9.77 | 7,313 | 13,357 | 0.023 | 0.031 | 0.067 | 24.0 | 0.37 | 0 |
| `p0/rc-noaof-set-c1-P1` | 2 | 35,790 | 3% | 12.93 | 3.17 | 9.75 | 7,344 | 13,748 | 0.023 | 0.031 | 0.071 | 23.7 | 0.37 | 0 |
| `py/rc-noaof-set-c1-P1` | 2 | 34,756 | 3% | 13.70 | 3.55 | 10.12 | 9,602 | 15,944 | 0.031 | 0.031 | 0.071 | 23.6 | 0.35 | 0 |
| `pc/rc-noaof-set-c1-P1` | 2 | 36,324 | 0% | 12.69 | 2.93 | 9.78 | 6,974 | 13,161 | 0.023 | 0.031 | 0.063 | 23.7 | 0.37 | 0 |
| `pa/rc-noaof-set-c1-P1` | 2 | 35,255 | 0% | 13.43 | 3.48 | 9.93 | 9,232 | 15,192 | 0.023 | 0.031 | 0.067 | 23.3 | 0.35 | 0 |
| `redis-noaof-set-c1-P1` | 2 | 33,773 | 3% | 14.44 | 3.80 | 10.67 | 8,027 | 16,686 | 0.031 | 0.031 | 0.071 | 18.9 | 0.35 | 0 |
| `base/rc-noaof-set-c50-P16` | 2 | 1,090,711 | 0% | 0.75 | 0.48 | 0.27 | 2,939 | 2,206 | 0.731 | 0.847 | 0.963 | 26.0 | 0.74 | 0 |
| `p0/rc-noaof-set-c50-P16` | 2 | 1,090,513 | 0% | 0.75 | 0.47 | 0.28 | 2,941 | 2,205 | 0.731 | 0.839 | 0.971 | 26.1 | 0.73 | 0 |
| `py/rc-noaof-set-c50-P16` | 2 | 1,090,513 | 0% | 0.72 | 0.48 | 0.24 | 2,982 | 2,168 | 0.639 | 0.851 | 0.943 | 25.9 | 0.71 | 0 |
| `pc/rc-noaof-set-c50-P16` | 2 | 1,090,513 | 0% | 0.74 | 0.47 | 0.26 | 2,918 | 2,185 | 0.715 | 0.831 | 0.959 | 26.0 | 0.74 | 0 |
| `pa/rc-noaof-set-c50-P16` | 2 | 1,145,016 | 10% | 0.71 | 0.48 | 0.23 | 2,959 | 2,145 | 0.635 | 0.843 | 0.939 | 26.1 | 0.71 | 0 |
| `redis-noaof-set-c50-P16` | 2 | 999,500 | 0% | 0.81 | 0.58 | 0.23 | 3,982 | 2,519 | 0.711 | 0.887 | 0.983 | 19.7 | 0.66 | 0 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.827** | 0.846 0.808 |
| `p0/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.843** | 0.846 0.840 |
| `py/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.895** | 0.916 0.874 |
| `pc/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.861** | 0.917 0.808 |
| `pa/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.934** | 0.956 0.913 |
| `base/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.712** | 0.700 0.724 |
| `p0/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.700** | 0.700 0.700 |
| `py/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.824** | 0.808 0.840 |
| `pc/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.725** | 0.700 0.750 |
| `pa/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.840** | 0.840 0.840 |
| `base/rc-noaof-set-c1-P1` / `redis-noaof-set-c1-P1` | **1.076** | 1.091 1.061 |
| `p0/rc-noaof-set-c1-P1` / `redis-noaof-set-c1-P1` | **1.060** | 1.091 1.029 |
| `py/rc-noaof-set-c1-P1` / `redis-noaof-set-c1-P1` | **1.029** | 1.059 1.000 |
| `pc/rc-noaof-set-c1-P1` / `redis-noaof-set-c1-P1` | **1.076** | 1.091 1.061 |
| `pa/rc-noaof-set-c1-P1` / `redis-noaof-set-c1-P1` | **1.044** | 1.059 1.029 |
| `base/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.091** | 1.091 1.091 |
| `p0/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.091** | 1.091 1.091 |
| `py/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.091** | 1.091 1.091 |
| `pc/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.091** | 1.091 1.091 |
| `pa/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.146** | 1.091 1.200 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` | 1 | load=1.56 io_psi10=1.04/0.82 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=7228 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=3.22 io_psi10=1.89/1.86 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2672 settle_wait_s=0 |
| `p0/rc-noaof-set-c50-P1` | 1 | load=1.68 io_psi10=2.82/2.46 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1488 settle_wait_s=0 |
| `p0/rc-noaof-set-c50-P1` | 2 | load=2.99 io_psi10=1.78/1.70 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1836 settle_wait_s=0 |
| `py/rc-noaof-set-c50-P1` | 1 | load=1.66 io_psi10=1.36/1.20 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1864 settle_wait_s=0 |
| `py/rc-noaof-set-c50-P1` | 2 | load=2.81 io_psi10=1.70/1.52 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=3400 settle_wait_s=0 |
| `pc/rc-noaof-set-c50-P1` | 1 | load=1.62 io_psi10=1.77/1.51 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2856 settle_wait_s=0 |
| `pc/rc-noaof-set-c50-P1` | 2 | load=2.68 io_psi10=1.75/1.57 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1460 settle_wait_s=0 |
| `pa/rc-noaof-set-c50-P1` | 1 | load=1.73 io_psi10=1.45/1.19 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1796 settle_wait_s=0 |
| `pa/rc-noaof-set-c50-P1` | 2 | load=2.61 io_psi10=1.77/1.64 mem_psi10=0.01/0.01 swap_used_mib=3385 dirty_wb_kib=3120 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 1 | load=1.77 io_psi10=1.17/0.78 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2624 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 2 | load=2.58 io_psi10=1.46/1.44 mem_psi10=0.02/0.02 swap_used_mib=3385 dirty_wb_kib=2636 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 1 | load=1.87 io_psi10=1.92/1.64 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1696 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | load=2.50 io_psi10=1.83/1.77 mem_psi10=0.06/0.05 swap_used_mib=3385 dirty_wb_kib=2536 settle_wait_s=0 |
| `p0/rc-noaof-lpush-c50-P1` | 1 | load=2.13 io_psi10=0.96/0.83 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2768 settle_wait_s=0 |
| `p0/rc-noaof-lpush-c50-P1` | 2 | load=2.23 io_psi10=2.09/1.96 mem_psi10=0.13/0.13 swap_used_mib=3385 dirty_wb_kib=2572 settle_wait_s=0 |
| `py/rc-noaof-lpush-c50-P1` | 1 | load=2.18 io_psi10=1.90/1.69 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2016 settle_wait_s=0 |
| `py/rc-noaof-lpush-c50-P1` | 2 | load=2.16 io_psi10=2.49/2.19 mem_psi10=0.30/0.29 swap_used_mib=3385 dirty_wb_kib=3528 settle_wait_s=0 |
| `pc/rc-noaof-lpush-c50-P1` | 1 | load=2.25 io_psi10=2.10/2.01 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1888 settle_wait_s=0 |
| `pc/rc-noaof-lpush-c50-P1` | 2 | load=2.20 io_psi10=4.49/4.17 mem_psi10=0.83/0.81 swap_used_mib=3385 dirty_wb_kib=2424 settle_wait_s=0 |
| `pa/rc-noaof-lpush-c50-P1` | 1 | load=2.66 io_psi10=1.54/1.40 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2332 settle_wait_s=0 |
| `pa/rc-noaof-lpush-c50-P1` | 2 | load=2.04 io_psi10=6.86/6.27 mem_psi10=1.52/1.48 swap_used_mib=3385 dirty_wb_kib=3168 settle_wait_s=6 |
| `redis-noaof-lpush-c50-P1` | 1 | load=2.78 io_psi10=0.99/0.92 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2196 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 2 | load=1.84 io_psi10=3.89/3.32 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2052 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 1 | load=2.80 io_psi10=2.36/2.20 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1912 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 2 | load=1.83 io_psi10=2.19/1.85 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2996 settle_wait_s=0 |
| `p0/rc-noaof-set-c1-P1` | 1 | load=2.59 io_psi10=1.11/0.96 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2056 settle_wait_s=0 |
| `p0/rc-noaof-set-c1-P1` | 2 | load=1.68 io_psi10=2.20/1.71 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=4148 settle_wait_s=0 |
| `py/rc-noaof-set-c1-P1` | 1 | load=2.42 io_psi10=0.98/0.92 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2780 settle_wait_s=0 |
| `py/rc-noaof-set-c1-P1` | 2 | load=1.81 io_psi10=2.22/2.11 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2620 settle_wait_s=0 |
| `pc/rc-noaof-set-c1-P1` | 1 | load=2.20 io_psi10=1.17/1.15 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2604 settle_wait_s=0 |
| `pc/rc-noaof-set-c1-P1` | 2 | load=1.69 io_psi10=1.43/1.37 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2576 settle_wait_s=0 |
| `pa/rc-noaof-set-c1-P1` | 1 | load=2.09 io_psi10=1.81/1.60 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=3116 settle_wait_s=0 |
| `pa/rc-noaof-set-c1-P1` | 2 | load=1.73 io_psi10=2.79/2.62 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2592 settle_wait_s=0 |
| `redis-noaof-set-c1-P1` | 1 | load=1.92 io_psi10=2.03/1.87 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2400 settle_wait_s=0 |
| `redis-noaof-set-c1-P1` | 2 | load=1.95 io_psi10=3.91/3.58 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2260 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 1 | load=1.78 io_psi10=1.14/1.00 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=3192 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 2 | load=2.04 io_psi10=2.73/2.46 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2340 settle_wait_s=0 |
| `p0/rc-noaof-set-c50-P16` | 1 | load=1.88 io_psi10=2.36/2.16 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2156 settle_wait_s=0 |
| `p0/rc-noaof-set-c50-P16` | 2 | load=2.04 io_psi10=2.75/2.57 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2212 settle_wait_s=0 |
| `py/rc-noaof-set-c50-P16` | 1 | load=1.97 io_psi10=1.73/1.60 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2416 settle_wait_s=0 |
| `py/rc-noaof-set-c50-P16` | 2 | load=2.21 io_psi10=2.77/2.51 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2668 settle_wait_s=0 |
| `pc/rc-noaof-set-c50-P16` | 1 | load=1.89 io_psi10=2.64/2.40 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1768 settle_wait_s=0 |
| `pc/rc-noaof-set-c50-P16` | 2 | load=2.23 io_psi10=3.03/2.64 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2084 settle_wait_s=0 |
| `pa/rc-noaof-set-c50-P16` | 1 | load=1.89 io_psi10=2.21/2.06 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2412 settle_wait_s=0 |
| `pa/rc-noaof-set-c50-P16` | 2 | load=2.25 io_psi10=3.86/3.49 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2184 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 1 | load=1.82 io_psi10=3.56/3.31 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2112 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 2 | load=1.75 io_psi10=2.53/2.36 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2276 settle_wait_s=0 |
