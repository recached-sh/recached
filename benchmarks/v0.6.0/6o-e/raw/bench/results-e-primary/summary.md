## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P1` | 5 | 181,774 | 1% | 3.63 | 0.85 | 2.78 | 4,813 | 4,764 | 0.255 | 0.351 | 0.383 | 25.3 | 1.23 | 0 |
| `e1/rc-noaof-get-c50-P1` | 5 | 181,813 | 1% | 3.60 | 0.86 | 2.76 | 4,818 | 4,796 | 0.255 | 0.351 | 0.383 | 25.3 | 1.24 | 0 |
| `fix/rc-noaof-get-c50-P1` | 5 | 181,802 | 5% | 3.61 | 0.83 | 2.77 | 4,833 | 4,647 | 0.255 | 0.351 | 0.383 | 25.4 | 1.24 | 0 |
| `redis-noaof-get-c50-P1` | 5 | 195,097 | 2% | 3.34 | 0.70 | 2.67 | 4,009 | 3,632 | 0.231 | 0.295 | 0.423 | 19.2 | 1.25 | 0 |
| `base/rc-noaof-mget-c50-P1` | 5 | 123,696 | 2% | 6.09 | 2.82 | 3.29 | 13,846 | 14,054 | 0.383 | 0.519 | 0.559 | 25.4 | 0.97 | 0 |
| `e1/rc-noaof-mget-c50-P1` | 5 | 124,974 | 1% | 6.07 | 2.83 | 3.24 | 13,852 | 14,092 | 0.375 | 0.511 | 0.551 | 25.4 | 0.97 | 0 |
| `fix/rc-noaof-mget-c50-P1` | 5 | 126,295 | 3% | 6.00 | 2.73 | 3.27 | 13,868 | 13,718 | 0.375 | 0.511 | 0.551 | 25.3 | 0.97 | 0 |
| `redis-noaof-mget-c50-P1` | 5 | 124,979 | 2% | 6.16 | 2.81 | 3.35 | 14,588 | 14,192 | 0.351 | 0.495 | 0.591 | 19.2 | 0.91 | 0 |
| `base/rc-noaof-set-c50-P1` | 5 | 176,414 | 3% | 3.80 | 0.96 | 2.82 | 5,449 | 5,386 | 0.263 | 0.367 | 0.399 | 25.9 | 1.20 | 0 |
| `fix/rc-noaof-set-c50-P1` | 5 | 179,056 | 3% | 3.69 | 0.91 | 2.78 | 5,472 | 5,106 | 0.263 | 0.359 | 0.391 | 26.1 | 1.22 | 0 |
| `redis-noaof-set-c50-P1` | 5 | 190,458 | 2% | 3.48 | 0.76 | 2.72 | 4,796 | 3,996 | 0.239 | 0.303 | 0.431 | 19.6 | 1.24 | 0 |
| `base/rc-noaof-mset-c50-P1` | 5 | 99,157 | 4% | 8.16 | 4.72 | 3.48 | 29,971 | 22,069 | 0.487 | 0.639 | 0.687 | 26.0 | 0.75 | 0 |
| `fix/rc-noaof-mset-c50-P1` | 5 | 97,545 | 2% | 8.27 | 4.81 | 3.47 | 29,991 | 22,390 | 0.487 | 0.647 | 0.687 | 26.0 | 0.73 | 0 |
| `redis-noaof-mset-c50-P1` | 5 | 117,629 | 1% | 6.71 | 3.54 | 3.13 | 20,327 | 16,420 | 0.375 | 0.535 | 0.615 | 19.9 | 0.84 | 0 |
| `base/rc-noaof-lpush-c50-P1` | 5 | 183,178 | 2% | 3.60 | 0.82 | 2.79 | 5,498 | 4,539 | 0.255 | 0.351 | 0.375 | 389.6 | 1.22 | 0 |
| `fix/rc-noaof-lpush-c50-P1` | 5 | 183,184 | 3% | 3.56 | 0.77 | 2.77 | 5,510 | 4,329 | 0.255 | 0.351 | 0.375 | 389.7 | 1.24 | 0 |
| `redis-noaof-lpush-c50-P1` | 5 | 198,334 | 2% | 3.26 | 0.61 | 2.65 | 5,147 | 3,264 | 0.231 | 0.295 | 0.423 | 211.7 | 1.25 | 0 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-get-c50-P1` / `redis-noaof-get-c50-P1` | **0.932** | 0.925 0.939 0.917 0.939 0.932 |
| `e1/rc-noaof-get-c50-P1` / `redis-noaof-get-c50-P1` | **0.932** | 0.932 0.947 0.924 0.947 0.932 |
| `fix/rc-noaof-get-c50-P1` / `redis-noaof-get-c50-P1` | **0.932** | 0.932 0.947 0.924 0.918 0.953 |
| `base/rc-noaof-mget-c50-P1` / `redis-noaof-mget-c50-P1` | **0.990** | 0.990 0.990 1.000 0.990 0.979 |
| `e1/rc-noaof-mget-c50-P1` / `redis-noaof-mget-c50-P1` | **1.000** | 0.979 1.000 1.000 1.000 0.990 |
| `fix/rc-noaof-mget-c50-P1` / `redis-noaof-mget-c50-P1` | **1.011** | 1.000 1.021 0.990 1.010 1.000 |
| `base/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.926** | 0.920 0.926 0.934 0.892 0.933 |
| `fix/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.940** | 0.940 0.940 0.955 0.905 0.947 |
| `base/rc-noaof-mset-c50-P1` / `redis-noaof-mset-c50-P1` | **0.843** | 0.843 0.836 0.802 0.835 0.843 |
| `fix/rc-noaof-mset-c50-P1` / `redis-noaof-mset-c50-P1` | **0.829** | 0.836 0.829 0.808 0.821 0.829 |
| `base/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.924** | 0.910 0.939 0.916 0.924 0.903 |
| `fix/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.924** | 0.924 0.932 0.938 0.938 0.924 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P1` | 1 | load=0.91 io_psi10=0.35/0.35 mem_psi10=0.00/0.00 swap_used_mib=5553 dirty_wb_kib=3256 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 2 | load=3.42 io_psi10=1.21/1.01 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=3192 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 3 | load=3.34 io_psi10=0.59/0.56 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2396 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 4 | load=3.38 io_psi10=0.52/0.46 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=4444 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 5 | load=3.52 io_psi10=0.62/0.42 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=4864 settle_wait_s=0 |
| `e1/rc-noaof-get-c50-P1` | 1 | load=2.07 io_psi10=0.37/0.35 mem_psi10=0.00/0.00 swap_used_mib=5553 dirty_wb_kib=2304 settle_wait_s=0 |
| `e1/rc-noaof-get-c50-P1` | 2 | load=3.21 io_psi10=0.46/0.24 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2544 settle_wait_s=0 |
| `e1/rc-noaof-get-c50-P1` | 3 | load=3.30 io_psi10=0.22/0.07 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=3312 settle_wait_s=0 |
| `e1/rc-noaof-get-c50-P1` | 4 | load=3.59 io_psi10=1.31/0.76 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3936 settle_wait_s=0 |
| `e1/rc-noaof-get-c50-P1` | 5 | load=3.52 io_psi10=0.54/0.33 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1012 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 1 | load=2.47 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=5553 dirty_wb_kib=2724 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 2 | load=2.97 io_psi10=0.23/0.21 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2756 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 3 | load=3.79 io_psi10=1.28/1.22 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=5140 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 4 | load=4.18 io_psi10=0.13/0.10 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3128 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 5 | load=3.57 io_psi10=1.25/0.85 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3384 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 1 | load=2.69 io_psi10=0.56/0.48 mem_psi10=0.00/0.00 swap_used_mib=5553 dirty_wb_kib=3092 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 2 | load=2.86 io_psi10=0.19/0.18 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2980 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 3 | load=3.41 io_psi10=2.21/1.42 mem_psi10=0.01/0.01 swap_used_mib=5551 dirty_wb_kib=3964 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 4 | load=3.89 io_psi10=1.45/1.21 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2696 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 5 | load=3.00 io_psi10=0.56/0.51 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2352 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 1 | load=3.50 io_psi10=0.43/0.25 mem_psi10=0.00/0.00 swap_used_mib=5553 dirty_wb_kib=1920 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 2 | load=2.86 io_psi10=0.26/0.23 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=3416 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 3 | load=3.03 io_psi10=0.76/0.63 mem_psi10=0.00/0.00 swap_used_mib=5551 dirty_wb_kib=2832 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 4 | load=3.55 io_psi10=0.10/0.08 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2900 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 5 | load=3.09 io_psi10=0.36/0.34 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3424 settle_wait_s=0 |
| `e1/rc-noaof-mget-c50-P1` | 1 | load=3.54 io_psi10=0.20/0.13 mem_psi10=0.00/0.00 swap_used_mib=5553 dirty_wb_kib=3616 settle_wait_s=0 |
| `e1/rc-noaof-mget-c50-P1` | 2 | load=2.82 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2336 settle_wait_s=0 |
| `e1/rc-noaof-mget-c50-P1` | 3 | load=3.24 io_psi10=0.52/0.50 mem_psi10=0.00/0.00 swap_used_mib=5551 dirty_wb_kib=3144 settle_wait_s=0 |
| `e1/rc-noaof-mget-c50-P1` | 4 | load=3.38 io_psi10=0.25/0.20 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2656 settle_wait_s=0 |
| `e1/rc-noaof-mget-c50-P1` | 5 | load=3.20 io_psi10=0.17/0.15 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3564 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 1 | load=3.05 io_psi10=0.26/0.22 mem_psi10=0.00/0.00 swap_used_mib=5553 dirty_wb_kib=2464 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 2 | load=2.84 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2260 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 3 | load=3.28 io_psi10=0.16/0.14 mem_psi10=0.00/0.00 swap_used_mib=5551 dirty_wb_kib=2476 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 4 | load=3.46 io_psi10=0.52/0.35 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2684 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 5 | load=2.96 io_psi10=0.62/0.52 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3460 settle_wait_s=0 |
| `redis-noaof-mget-c50-P1` | 1 | load=2.71 io_psi10=0.29/0.07 mem_psi10=0.00/0.00 swap_used_mib=5553 dirty_wb_kib=2252 settle_wait_s=0 |
| `redis-noaof-mget-c50-P1` | 2 | load=3.27 io_psi10=0.08/0.07 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2932 settle_wait_s=0 |
| `redis-noaof-mget-c50-P1` | 3 | load=2.93 io_psi10=0.66/0.36 mem_psi10=0.00/0.00 swap_used_mib=5551 dirty_wb_kib=3148 settle_wait_s=0 |
| `redis-noaof-mget-c50-P1` | 4 | load=3.91 io_psi10=0.13/0.10 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3632 settle_wait_s=0 |
| `redis-noaof-mget-c50-P1` | 5 | load=2.76 io_psi10=0.16/0.15 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3696 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 1 | load=2.70 io_psi10=0.20/0.06 mem_psi10=0.00/0.00 swap_used_mib=5553 dirty_wb_kib=3332 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=3.31 io_psi10=0.51/0.34 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2860 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=3.20 io_psi10=0.14/0.12 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=3400 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 4 | load=3.14 io_psi10=1.44/0.93 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2844 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 5 | load=2.84 io_psi10=0.49/0.45 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2672 settle_wait_s=0 |
| `fix/rc-noaof-set-c50-P1` | 1 | load=2.86 io_psi10=0.65/0.46 mem_psi10=0.00/0.00 swap_used_mib=5553 dirty_wb_kib=1708 settle_wait_s=0 |
| `fix/rc-noaof-set-c50-P1` | 2 | load=2.93 io_psi10=0.95/0.51 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2412 settle_wait_s=0 |
| `fix/rc-noaof-set-c50-P1` | 3 | load=3.27 io_psi10=1.52/1.08 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=6780 settle_wait_s=0 |
| `fix/rc-noaof-set-c50-P1` | 4 | load=2.68 io_psi10=0.82/0.79 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2248 settle_wait_s=0 |
| `fix/rc-noaof-set-c50-P1` | 5 | load=3.15 io_psi10=0.52/0.22 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1512 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 1 | load=3.55 io_psi10=0.76/0.45 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2612 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 2 | load=2.97 io_psi10=1.76/1.65 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=3460 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 3 | load=3.37 io_psi10=0.99/0.77 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=6744 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 4 | load=2.46 io_psi10=0.59/0.57 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3508 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 5 | load=3.00 io_psi10=0.65/0.35 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3108 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 1 | load=3.57 io_psi10=0.32/0.15 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=3432 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 2 | load=3.12 io_psi10=0.29/0.19 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2128 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 3 | load=4.21 io_psi10=0.86/0.47 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=6036 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 4 | load=2.84 io_psi10=0.62/0.35 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=4332 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 5 | load=3.02 io_psi10=0.16/0.13 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3436 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P1` | 1 | load=2.84 io_psi10=0.22/0.08 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=3232 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P1` | 2 | load=3.24 io_psi10=0.96/0.94 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=3464 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P1` | 3 | load=3.43 io_psi10=0.71/0.36 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=4992 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P1` | 4 | load=2.78 io_psi10=0.34/0.25 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=3320 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P1` | 5 | load=2.65 io_psi10=0.24/0.23 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2000 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 1 | load=2.79 io_psi10=0.15/0.05 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=3056 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 2 | load=3.68 io_psi10=0.05/0.04 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2760 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 3 | load=3.02 io_psi10=0.68/0.38 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=5440 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 4 | load=2.96 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=2996 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 5 | load=2.41 io_psi10=0.06/0.05 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2888 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 1 | load=2.90 io_psi10=0.38/0.19 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2940 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | load=3.32 io_psi10=0.11/0.10 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=13888 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | load=2.67 io_psi10=0.68/0.61 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=2928 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 4 | load=2.84 io_psi10=0.70/0.53 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=2480 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 5 | load=2.51 io_psi10=0.37/0.28 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3784 settle_wait_s=0 |
| `fix/rc-noaof-lpush-c50-P1` | 1 | load=3.07 io_psi10=0.87/0.45 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=2064 settle_wait_s=0 |
| `fix/rc-noaof-lpush-c50-P1` | 2 | load=3.10 io_psi10=0.23/0.03 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=3528 settle_wait_s=0 |
| `fix/rc-noaof-lpush-c50-P1` | 3 | load=2.89 io_psi10=0.24/0.08 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=2076 settle_wait_s=0 |
| `fix/rc-noaof-lpush-c50-P1` | 4 | load=2.77 io_psi10=0.38/0.29 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=5924 settle_wait_s=0 |
| `fix/rc-noaof-lpush-c50-P1` | 5 | load=3.67 io_psi10=0.71/0.31 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=4836 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 1 | load=2.93 io_psi10=0.53/0.16 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=3604 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 2 | load=2.84 io_psi10=0.27/0.12 mem_psi10=0.00/0.00 swap_used_mib=5552 dirty_wb_kib=1996 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 3 | load=2.84 io_psi10=0.04/0.03 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=3308 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 4 | load=2.82 io_psi10=2.04/2.03 mem_psi10=0.00/0.00 swap_used_mib=5550 dirty_wb_kib=2176 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 5 | load=3.29 io_psi10=0.77/0.41 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=4068 settle_wait_s=0 |
