## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-lpush-c50-P1` | 3 | 180,167 | 1% | 3.68 | 0.89 | 2.78 | 6,316 | 5,012 | 0.263 | 0.351 | 0.383 | 342.7 | 1.20 | 0 |
| `c2/rc-noaof-lpush-c50-P1` | 3 | 188,658 | 3% | 3.46 | 0.77 | 2.69 | 5,637 | 4,391 | 0.247 | 0.335 | 0.367 | 343.0 | 1.24 | 0 |
| `redis-noaof-lpush-c50-P1` | 3 | 202,012 | 1% | 3.21 | 0.65 | 2.58 | 5,174 | 3,257 | 0.223 | 0.287 | 0.415 | 178.2 | 1.27 | 0 |
| `base/rc-noaof-rpush-c50-P1` | 3 | 181,805 | 2% | 3.66 | 0.87 | 2.79 | 6,380 | 4,952 | 0.263 | 0.359 | 0.383 | 327.9 | 1.20 | 0 |
| `c2/rc-noaof-rpush-c50-P1` | 3 | 190,462 | 3% | 3.45 | 0.76 | 2.69 | 5,634 | 4,343 | 0.247 | 0.335 | 0.359 | 327.9 | 1.25 | 0 |
| `redis-noaof-rpush-c50-P1` | 3 | 206,169 | 1% | 3.11 | 0.52 | 2.59 | 4,661 | 2,987 | 0.223 | 0.279 | 0.407 | 178.1 | 1.28 | 0 |
| `base/rc-noaof-sadd-c50-P1` | 3 | 180,167 | 1% | 3.75 | 1.01 | 2.74 | 6,559 | 5,615 | 0.263 | 0.359 | 0.383 | 19.2 | 1.21 | 0 |
| `c2/rc-noaof-sadd-c50-P1` | 3 | 186,902 | 3% | 3.56 | 0.88 | 2.68 | 5,800 | 5,042 | 0.255 | 0.343 | 0.367 | 19.1 | 1.24 | 0 |
| `redis-noaof-sadd-c50-P1` | 3 | 206,177 | 0% | 3.15 | 0.59 | 2.57 | 4,367 | 3,200 | 0.223 | 0.287 | 0.407 | 12.3 | 1.28 | 0 |
| `base/rc-noaof-lpush-c50-P16` | 3 | 1,249,563 | 1% | 0.67 | 0.42 | 0.26 | 3,716 | 1,945 | 0.591 | 0.767 | 0.815 | 1,568.7 | 0.65 | 0 |
| `c2/rc-noaof-lpush-c50-P16` | 3 | 1,428,326 | 3% | 0.58 | 0.33 | 0.24 | 3,039 | 1,573 | 0.511 | 0.671 | 0.711 | 1,569.0 | 0.73 | 0 |
| `base/rc-noaof-rpush-c50-P16` | 3 | 1,249,688 | 1% | 0.67 | 0.42 | 0.25 | 3,785 | 1,952 | 0.599 | 0.775 | 0.815 | 1,568.9 | 0.64 | 0 |
| `c2/rc-noaof-rpush-c50-P16` | 3 | 1,408,133 | 1% | 0.58 | 0.33 | 0.25 | 3,036 | 1,579 | 0.519 | 0.687 | 0.727 | 1,569.3 | 0.72 | 0 |
| `base/rc-noaof-sadd-c50-P16` | 3 | 1,110,815 | 1% | 0.78 | 0.56 | 0.22 | 4,015 | 2,481 | 0.695 | 0.895 | 0.943 | 19.3 | 0.61 | 0 |
| `c2/rc-noaof-sadd-c50-P16` | 3 | 1,234,263 | 1% | 0.69 | 0.46 | 0.22 | 3,261 | 2,135 | 0.615 | 0.799 | 0.855 | 19.2 | 0.67 | 0 |
| `redis-noaof-lpush-c50-P16` | 3 | 1,315,374 | 1% | 0.65 | 0.40 | 0.25 | 4,485 | 1,919 | 0.583 | 0.727 | 0.783 | 843.9 | 0.67 | 0 |
| `redis-noaof-sadd-c50-P16` | 3 | 1,388,503 | 1% | 0.61 | 0.40 | 0.21 | 3,646 | 1,791 | 0.535 | 0.703 | 0.767 | 12.2 | 0.77 | 0 |
| `base/rc-noaof-lpush-c1-P1` | 3 | 39,088 | 2% | 12.61 | 2.97 | 9.76 | 7,529 | 13,345 | 0.023 | 0.031 | 0.031 | 72.4 | 0.37 | 0 |
| `c2/rc-noaof-lpush-c1-P1` | 3 | 39,515 | 1% | 12.23 | 2.66 | 9.48 | 6,860 | 12,274 | 0.023 | 0.031 | 0.031 | 72.1 | 0.38 | 0 |
| `base/rc-everysec-lpush-c50-P1` | 3 | 132,415 | 3% | 5.41 | 1.69 | 3.71 | 10,466 | 8,877 | 0.351 | 0.479 | 0.511 | 343.4 | 0.93 | 0 |
| `c2/rc-everysec-lpush-c50-P1` | 3 | 130,705 | 3% | 5.53 | 1.80 | 3.72 | 10,645 | 9,321 | 0.359 | 0.487 | 0.519 | 342.9 | 0.92 | 0 |
| `base/rc-noaof-sadd-c1-P1` | 3 | 38,666 | 1% | 13.00 | 3.37 | 9.64 | 7,801 | 15,203 | 0.023 | 0.031 | 0.031 | 16.1 | 0.37 | 0 |
| `c2/rc-noaof-sadd-c1-P1` | 3 | 39,954 | 1% | 12.20 | 2.80 | 9.38 | 7,055 | 12,440 | 0.023 | 0.023 | 0.031 | 16.8 | 0.39 | 0 |
| `base/rc-everysec-sadd-c50-P1` | 3 | 165,284 | 5% | 4.19 | 1.25 | 2.89 | 7,928 | 6,900 | 0.287 | 0.391 | 0.423 | 19.4 | 1.12 | 0 |
| `c2/rc-everysec-sadd-c50-P1` | 3 | 169,480 | 3% | 4.09 | 1.23 | 2.87 | 8,025 | 6,863 | 0.279 | 0.383 | 0.415 | 18.8 | 1.14 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-lpush-c50-P1` | 0.0999 | 10.0 | 6.7 | 0.0004 |
| `c2/rc-everysec-lpush-c50-P1` | 0.1000 | 10.0 | 7.0 | 0.0004 |
| `base/rc-everysec-sadd-c50-P1` | 0.0028 | 2.6 | 5.6 | 0.0001 |
| `c2/rc-everysec-sadd-c50-P1` | 0.0029 | 2.6 | 4.6 | 0.0001 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.892** | 0.883 0.892 0.900 |
| `c2/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.934** | 0.942 0.925 0.934 |
| `base/rc-noaof-rpush-c50-P1` / `redis-noaof-rpush-c50-P1` | **0.882** | 0.873 0.882 0.866 |
| `c2/rc-noaof-rpush-c50-P1` / `redis-noaof-rpush-c50-P1` | **0.924** | 0.914 0.924 0.898 |
| `base/rc-noaof-sadd-c50-P1` / `redis-noaof-sadd-c50-P1` | **0.874** | 0.874 0.866 0.874 |
| `c2/rc-noaof-sadd-c50-P1` / `redis-noaof-sadd-c50-P1` | **0.907** | 0.882 0.907 0.907 |
| `base/rc-noaof-lpush-c50-P16` / `redis-noaof-lpush-c50-P16` | **0.950** | 0.949 0.950 0.950 |
| `c2/rc-noaof-lpush-c50-P16` / `redis-noaof-lpush-c50-P16` | **1.086** | 1.072 1.071 1.102 |
| `base/rc-noaof-sadd-c50-P16` / `redis-noaof-sadd-c50-P16` | **0.800** | 0.800 0.789 0.791 |
| `c2/rc-noaof-sadd-c50-P16` / `redis-noaof-sadd-c50-P16` | **0.889** | 0.889 0.877 0.900 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-lpush-c50-P1` | 1 | load=0.92 io_psi10=0.80/0.80 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2720 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | load=2.58 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2940 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | load=2.67 io_psi10=0.27/0.27 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2952 settle_wait_s=0 |
| `c2/rc-noaof-lpush-c50-P1` | 1 | load=1.64 io_psi10=0.48/0.47 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1832 settle_wait_s=0 |
| `c2/rc-noaof-lpush-c50-P1` | 2 | load=2.45 io_psi10=0.39/0.39 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=3004 settle_wait_s=0 |
| `c2/rc-noaof-lpush-c50-P1` | 3 | load=2.51 io_psi10=0.21/0.21 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2128 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 1 | load=2.17 io_psi10=0.19/0.19 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2304 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 2 | load=2.58 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2580 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 3 | load=2.39 io_psi10=0.05/0.04 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2476 settle_wait_s=0 |
| `base/rc-noaof-rpush-c50-P1` | 1 | load=2.46 io_psi10=1.29/1.29 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2388 settle_wait_s=0 |
| `base/rc-noaof-rpush-c50-P1` | 2 | load=2.60 io_psi10=0.27/0.25 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2536 settle_wait_s=0 |
| `base/rc-noaof-rpush-c50-P1` | 3 | load=2.38 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2748 settle_wait_s=0 |
| `c2/rc-noaof-rpush-c50-P1` | 1 | load=2.47 io_psi10=0.30/0.30 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1932 settle_wait_s=0 |
| `c2/rc-noaof-rpush-c50-P1` | 2 | load=2.59 io_psi10=0.34/0.16 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2072 settle_wait_s=0 |
| `c2/rc-noaof-rpush-c50-P1` | 3 | load=2.47 io_psi10=0.16/0.16 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2160 settle_wait_s=0 |
| `redis-noaof-rpush-c50-P1` | 1 | load=2.58 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1792 settle_wait_s=0 |
| `redis-noaof-rpush-c50-P1` | 2 | load=2.60 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2556 settle_wait_s=0 |
| `redis-noaof-rpush-c50-P1` | 3 | load=2.58 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2604 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 1 | load=2.42 io_psi10=0.26/0.26 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1716 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 2 | load=2.43 io_psi10=0.17/0.17 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1980 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 3 | load=2.63 io_psi10=0.22/0.22 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2108 settle_wait_s=0 |
| `c2/rc-noaof-sadd-c50-P1` | 1 | load=2.47 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1792 settle_wait_s=0 |
| `c2/rc-noaof-sadd-c50-P1` | 2 | load=2.15 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2000 settle_wait_s=0 |
| `c2/rc-noaof-sadd-c50-P1` | 3 | load=2.59 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2104 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 1 | load=2.53 io_psi10=0.00/0.00 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2148 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 2 | load=1.90 io_psi10=0.32/0.32 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2484 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 3 | load=2.48 io_psi10=0.29/0.28 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2968 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P16` | 1 | load=2.63 io_psi10=0.39/0.39 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2076 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P16` | 2 | load=1.60 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2140 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P16` | 3 | load=2.39 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2972 settle_wait_s=0 |
| `c2/rc-noaof-lpush-c50-P16` | 1 | load=2.25 io_psi10=0.17/0.17 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1876 settle_wait_s=0 |
| `c2/rc-noaof-lpush-c50-P16` | 2 | load=1.54 io_psi10=0.35/0.35 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1964 settle_wait_s=0 |
| `c2/rc-noaof-lpush-c50-P16` | 3 | load=2.13 io_psi10=0.44/0.44 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2080 settle_wait_s=0 |
| `base/rc-noaof-rpush-c50-P16` | 1 | load=2.18 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2632 settle_wait_s=0 |
| `base/rc-noaof-rpush-c50-P16` | 2 | load=1.47 io_psi10=1.27/1.27 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1936 settle_wait_s=0 |
| `base/rc-noaof-rpush-c50-P16` | 3 | load=1.96 io_psi10=0.10/0.10 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2700 settle_wait_s=0 |
| `c2/rc-noaof-rpush-c50-P16` | 1 | load=2.01 io_psi10=0.69/0.69 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2116 settle_wait_s=0 |
| `c2/rc-noaof-rpush-c50-P16` | 2 | load=1.48 io_psi10=0.31/0.29 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2268 settle_wait_s=0 |
| `c2/rc-noaof-rpush-c50-P16` | 3 | load=1.84 io_psi10=0.40/0.40 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2116 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P16` | 1 | load=2.23 io_psi10=0.22/0.22 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2188 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P16` | 2 | load=1.28 io_psi10=0.15/0.15 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1964 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P16` | 3 | load=1.74 io_psi10=0.54/0.54 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2540 settle_wait_s=0 |
| `c2/rc-noaof-sadd-c50-P16` | 1 | load=1.96 io_psi10=0.42/0.42 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2376 settle_wait_s=0 |
| `c2/rc-noaof-sadd-c50-P16` | 2 | load=1.30 io_psi10=0.56/0.56 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2436 settle_wait_s=0 |
| `c2/rc-noaof-sadd-c50-P16` | 3 | load=1.77 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2192 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P16` | 1 | load=1.90 io_psi10=0.09/0.09 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1852 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P16` | 2 | load=1.37 io_psi10=3.04/3.04 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2696 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P16` | 3 | load=1.57 io_psi10=0.67/0.67 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2456 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P16` | 1 | load=1.71 io_psi10=1.15/1.15 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2172 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P16` | 2 | load=1.48 io_psi10=0.41/0.38 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1964 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P16` | 3 | load=1.48 io_psi10=0.43/0.43 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1348 settle_wait_s=0 |
| `base/rc-noaof-lpush-c1-P1` | 1 | load=1.78 io_psi10=0.81/0.78 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1908 settle_wait_s=0 |
| `base/rc-noaof-lpush-c1-P1` | 2 | load=1.73 io_psi10=0.86/0.48 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2200 settle_wait_s=0 |
| `base/rc-noaof-lpush-c1-P1` | 3 | load=1.34 io_psi10=0.92/0.89 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1872 settle_wait_s=0 |
| `c2/rc-noaof-lpush-c1-P1` | 1 | load=1.51 io_psi10=0.17/0.17 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2384 settle_wait_s=0 |
| `c2/rc-noaof-lpush-c1-P1` | 2 | load=2.11 io_psi10=7.00/2.09 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2484 settle_wait_s=0 |
| `c2/rc-noaof-lpush-c1-P1` | 3 | load=1.29 io_psi10=0.10/0.10 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1384 settle_wait_s=0 |
| `base/rc-everysec-lpush-c50-P1` | 1 | load=1.32 io_psi10=0.51/0.51 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2036 settle_wait_s=0 |
| `base/rc-everysec-lpush-c50-P1` | 2 | load=1.90 io_psi10=5.11/0.48 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=3168 settle_wait_s=0 |
| `base/rc-everysec-lpush-c50-P1` | 3 | load=1.17 io_psi10=0.33/0.32 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2200 settle_wait_s=0 |
| `c2/rc-everysec-lpush-c50-P1` | 1 | load=2.09 io_psi10=7.95/4.21 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2444 settle_wait_s=2 |
| `c2/rc-everysec-lpush-c50-P1` | 2 | load=1.53 io_psi10=0.39/0.39 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2096 settle_wait_s=0 |
| `c2/rc-everysec-lpush-c50-P1` | 3 | load=1.65 io_psi10=4.82/0.66 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2980 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P1` | 1 | load=2.67 io_psi10=7.65/2.48 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2908 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P1` | 2 | load=1.93 io_psi10=0.13/0.13 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2584 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P1` | 3 | load=1.77 io_psi10=7.50/2.01 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=3164 settle_wait_s=0 |
| `c2/rc-noaof-sadd-c1-P1` | 1 | load=2.04 io_psi10=1.74/1.34 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2388 settle_wait_s=0 |
| `c2/rc-noaof-sadd-c1-P1` | 2 | load=2.42 io_psi10=0.21/0.16 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2240 settle_wait_s=0 |
| `c2/rc-noaof-sadd-c1-P1` | 3 | load=1.50 io_psi10=0.77/0.32 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2164 settle_wait_s=0 |
| `base/rc-everysec-sadd-c50-P1` | 1 | load=1.61 io_psi10=0.41/0.36 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1912 settle_wait_s=0 |
| `base/rc-everysec-sadd-c50-P1` | 2 | load=2.28 io_psi10=0.33/0.24 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2896 settle_wait_s=0 |
| `base/rc-everysec-sadd-c50-P1` | 3 | load=1.38 io_psi10=0.54/0.50 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2728 settle_wait_s=0 |
| `c2/rc-everysec-sadd-c50-P1` | 1 | load=2.21 io_psi10=0.35/0.18 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2280 settle_wait_s=0 |
| `c2/rc-everysec-sadd-c50-P1` | 2 | load=2.24 io_psi10=0.36/0.18 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=4448 settle_wait_s=0 |
| `c2/rc-everysec-sadd-c50-P1` | 3 | load=1.81 io_psi10=0.29/0.15 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2348 settle_wait_s=0 |
