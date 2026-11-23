## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P1` | 5 | 191,994 | 2% | 3.40 | 0.74 | 2.66 | 4,699 | 4,289 | 0.247 | 0.335 | 0.359 | 25.7 | 1.27 | 0 |
| `fix/rc-noaof-get-c50-P1` | 5 | 191,969 | 2% | 3.42 | 0.77 | 2.65 | 4,801 | 4,392 | 0.247 | 0.335 | 0.359 | 25.5 | 1.26 | 0 |
| `redis-noaof-get-c50-P1` | 5 | 205,114 | 2% | 3.16 | 0.58 | 2.58 | 3,964 | 3,356 | 0.223 | 0.287 | 0.399 | 19.0 | 1.29 | 0 |
| `base/rc-noaof-mget-c50-P1` | 5 | 131,857 | 2% | 5.69 | 2.54 | 3.16 | 13,588 | 13,041 | 0.359 | 0.487 | 0.511 | 25.3 | 1.00 | 0 |
| `fix/rc-noaof-mget-c50-P1` | 5 | 133,321 | 1% | 5.65 | 2.46 | 3.17 | 13,842 | 12,880 | 0.359 | 0.479 | 0.503 | 25.4 | 1.01 | 0 |
| `redis-noaof-mget-c50-P1` | 5 | 136,339 | 1% | 5.60 | 2.48 | 3.09 | 14,589 | 12,497 | 0.319 | 0.463 | 0.559 | 19.0 | 0.97 | 0 |
| `base/rc-noaof-set-c50-P1` | 5 | 184,598 | 2% | 3.60 | 0.86 | 2.72 | 5,414 | 4,969 | 0.255 | 0.351 | 0.375 | 25.9 | 1.22 | 0 |
| `fix/rc-noaof-set-c50-P1` | 5 | 183,189 | 2% | 3.63 | 0.89 | 2.74 | 5,437 | 5,079 | 0.255 | 0.351 | 0.375 | 26.4 | 1.22 | 0 |
| `redis-noaof-set-c50-P1` | 5 | 199,987 | 1% | 3.30 | 0.73 | 2.57 | 4,742 | 3,736 | 0.231 | 0.295 | 0.415 | 19.6 | 1.25 | 0 |
| `base/rc-noaof-mset-c50-P1` | 5 | 104,330 | 1% | 7.72 | 4.34 | 3.39 | 29,577 | 21,031 | 0.463 | 0.607 | 0.639 | 26.2 | 0.76 | 0 |
| `fix/rc-noaof-mset-c50-P1` | 5 | 103,434 | 1% | 7.78 | 4.39 | 3.39 | 29,971 | 21,266 | 0.463 | 0.607 | 0.639 | 26.4 | 0.75 | 0 |
| `redis-noaof-mset-c50-P1` | 5 | 126,295 | 1% | 6.16 | 2.82 | 3.33 | 20,329 | 14,780 | 0.351 | 0.495 | 0.575 | 19.7 | 0.87 | 0 |
| `base/rc-noaof-lpush-c50-P1` | 5 | 188,953 | 2% | 3.46 | 0.73 | 2.72 | 5,465 | 4,269 | 0.247 | 0.335 | 0.367 | 389.7 | 1.25 | 0 |
| `fix/rc-noaof-lpush-c50-P1` | 5 | 187,488 | 2% | 3.51 | 0.77 | 2.73 | 5,491 | 4,448 | 0.255 | 0.343 | 0.367 | 390.0 | 1.23 | 0 |
| `redis-noaof-lpush-c50-P1` | 5 | 205,107 | 1% | 3.17 | 0.62 | 2.55 | 5,135 | 3,268 | 0.223 | 0.287 | 0.407 | 211.6 | 1.25 | 0 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-get-c50-P1` / `redis-noaof-get-c50-P1` | **0.936** | 0.929 0.936 0.921 0.952 0.944 |
| `fix/rc-noaof-get-c50-P1` / `redis-noaof-get-c50-P1` | **0.936** | 0.944 0.921 0.913 0.944 0.944 |
| `base/rc-noaof-mget-c50-P1` / `redis-noaof-mget-c50-P1` | **0.967** | 0.957 0.989 0.978 0.967 0.978 |
| `fix/rc-noaof-mget-c50-P1` / `redis-noaof-mget-c50-P1` | **0.978** | 0.978 0.989 0.967 0.967 0.989 |
| `base/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.923** | 0.915 0.938 0.923 0.916 0.923 |
| `fix/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.916** | 0.915 0.909 0.916 0.916 0.923 |
| `base/rc-noaof-mset-c50-P1` / `redis-noaof-mset-c50-P1` | **0.826** | 0.810 0.817 0.819 0.826 0.826 |
| `fix/rc-noaof-mset-c50-P1` / `redis-noaof-mset-c50-P1` | **0.819** | 0.817 0.810 0.819 0.819 0.819 |
| `base/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.921** | 0.914 0.921 0.929 0.921 0.944 |
| `fix/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.914** | 0.929 0.921 0.914 0.914 0.915 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P1` | 1 | load=0.75 io_psi10=4.81/4.61 mem_psi10=0.03/0.03 swap_used_mib=5667 dirty_wb_kib=1568 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 2 | load=2.39 io_psi10=0.76/0.76 mem_psi10=0.00/0.00 swap_used_mib=5472 dirty_wb_kib=472 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 3 | load=2.52 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=5472 dirty_wb_kib=504 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 4 | load=2.68 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=500 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 5 | load=2.38 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=296 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 1 | load=1.62 io_psi10=0.38/0.37 mem_psi10=0.02/0.02 swap_used_mib=5665 dirty_wb_kib=540 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 2 | load=2.06 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=5472 dirty_wb_kib=648 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 3 | load=2.65 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=5472 dirty_wb_kib=988 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 4 | load=2.56 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=564 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P1` | 5 | load=2.56 io_psi10=0.12/0.12 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=636 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 1 | load=2.11 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=5665 dirty_wb_kib=348 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 2 | load=1.97 io_psi10=0.35/0.35 mem_psi10=0.00/0.00 swap_used_mib=5472 dirty_wb_kib=364 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 3 | load=2.59 io_psi10=0.59/0.55 mem_psi10=0.00/0.00 swap_used_mib=5472 dirty_wb_kib=1376 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 4 | load=2.28 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=536 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 5 | load=2.54 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=508 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 1 | load=2.19 io_psi10=0.57/0.57 mem_psi10=0.00/0.00 swap_used_mib=5665 dirty_wb_kib=1372 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 2 | load=2.13 io_psi10=0.15/0.15 mem_psi10=0.00/0.00 swap_used_mib=5472 dirty_wb_kib=556 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 3 | load=2.59 io_psi10=0.13/0.13 mem_psi10=0.00/0.00 swap_used_mib=5471 dirty_wb_kib=732 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 4 | load=2.45 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=344 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 5 | load=2.73 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=972 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 1 | load=2.27 io_psi10=0.23/0.23 mem_psi10=0.00/0.00 swap_used_mib=5665 dirty_wb_kib=496 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 2 | load=2.43 io_psi10=0.15/0.15 mem_psi10=0.00/0.00 swap_used_mib=5472 dirty_wb_kib=408 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 3 | load=2.34 io_psi10=0.17/0.17 mem_psi10=0.00/0.00 swap_used_mib=5471 dirty_wb_kib=532 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 4 | load=2.51 io_psi10=0.00/0.00 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=592 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P1` | 5 | load=2.64 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=360 settle_wait_s=0 |
| `redis-noaof-mget-c50-P1` | 1 | load=2.23 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=5665 dirty_wb_kib=572 settle_wait_s=0 |
| `redis-noaof-mget-c50-P1` | 2 | load=2.61 io_psi10=0.21/0.21 mem_psi10=0.00/0.00 swap_used_mib=5472 dirty_wb_kib=248 settle_wait_s=0 |
| `redis-noaof-mget-c50-P1` | 3 | load=2.18 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=5471 dirty_wb_kib=436 settle_wait_s=0 |
| `redis-noaof-mget-c50-P1` | 4 | load=2.48 io_psi10=0.00/0.00 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=444 settle_wait_s=0 |
| `redis-noaof-mget-c50-P1` | 5 | load=2.51 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=580 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 1 | load=2.12 io_psi10=0.22/0.17 mem_psi10=0.00/0.00 swap_used_mib=5664 dirty_wb_kib=624 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=2.49 io_psi10=0.37/0.29 mem_psi10=0.00/0.00 swap_used_mib=5472 dirty_wb_kib=972 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=2.12 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=5471 dirty_wb_kib=636 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 4 | load=2.27 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=524 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 5 | load=2.26 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=428 settle_wait_s=0 |
| `fix/rc-noaof-set-c50-P1` | 1 | load=2.42 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=5664 dirty_wb_kib=516 settle_wait_s=0 |
| `fix/rc-noaof-set-c50-P1` | 2 | load=2.35 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=5472 dirty_wb_kib=548 settle_wait_s=0 |
| `fix/rc-noaof-set-c50-P1` | 3 | load=2.21 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=5470 dirty_wb_kib=500 settle_wait_s=0 |
| `fix/rc-noaof-set-c50-P1` | 4 | load=2.39 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=844 settle_wait_s=0 |
| `fix/rc-noaof-set-c50-P1` | 5 | load=2.50 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=620 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 1 | load=2.68 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=5664 dirty_wb_kib=896 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 2 | load=2.02 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=5476 dirty_wb_kib=496 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 3 | load=2.27 io_psi10=0.91/0.91 mem_psi10=0.00/0.00 swap_used_mib=5470 dirty_wb_kib=692 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 4 | load=2.19 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=708 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 5 | load=2.57 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=500 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 1 | load=2.62 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=5664 dirty_wb_kib=316 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 2 | load=2.28 io_psi10=0.15/0.15 mem_psi10=0.00/0.00 swap_used_mib=5476 dirty_wb_kib=1584 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 3 | load=2.43 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=5470 dirty_wb_kib=648 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 4 | load=2.22 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=1456 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 5 | load=2.47 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=508 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P1` | 1 | load=2.17 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=5664 dirty_wb_kib=724 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P1` | 2 | load=2.47 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=5662 dirty_wb_kib=620 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P1` | 3 | load=2.23 io_psi10=0.13/0.13 mem_psi10=0.00/0.00 swap_used_mib=5470 dirty_wb_kib=448 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P1` | 4 | load=2.28 io_psi10=0.00/0.00 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=380 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P1` | 5 | load=2.14 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=820 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 1 | load=1.81 io_psi10=0.09/0.08 mem_psi10=0.00/0.00 swap_used_mib=5664 dirty_wb_kib=520 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 2 | load=2.56 io_psi10=0.51/0.51 mem_psi10=0.00/0.00 swap_used_mib=5662 dirty_wb_kib=104 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 3 | load=2.20 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=5470 dirty_wb_kib=508 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 4 | load=2.59 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=180 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 5 | load=1.81 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=516 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 1 | load=1.72 io_psi10=0.31/0.31 mem_psi10=0.00/0.00 swap_used_mib=5664 dirty_wb_kib=1392 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | load=2.35 io_psi10=0.21/0.21 mem_psi10=0.00/0.00 swap_used_mib=5662 dirty_wb_kib=740 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | load=2.35 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=5470 dirty_wb_kib=444 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 4 | load=2.42 io_psi10=0.00/0.00 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=496 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 5 | load=1.78 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=1408 settle_wait_s=0 |
| `fix/rc-noaof-lpush-c50-P1` | 1 | load=2.15 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=5664 dirty_wb_kib=524 settle_wait_s=0 |
| `fix/rc-noaof-lpush-c50-P1` | 2 | load=2.51 io_psi10=0.18/0.17 mem_psi10=0.00/0.00 swap_used_mib=5663 dirty_wb_kib=504 settle_wait_s=0 |
| `fix/rc-noaof-lpush-c50-P1` | 3 | load=2.66 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=5470 dirty_wb_kib=624 settle_wait_s=0 |
| `fix/rc-noaof-lpush-c50-P1` | 4 | load=2.13 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=504 settle_wait_s=0 |
| `fix/rc-noaof-lpush-c50-P1` | 5 | load=2.12 io_psi10=0.19/0.19 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=680 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 1 | load=2.28 io_psi10=0.12/0.12 mem_psi10=0.00/0.00 swap_used_mib=5664 dirty_wb_kib=504 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 2 | load=2.61 io_psi10=1.64/1.44 mem_psi10=0.00/0.00 swap_used_mib=5663 dirty_wb_kib=672 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 3 | load=2.72 io_psi10=0.08/0.07 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=660 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 4 | load=2.56 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=5469 dirty_wb_kib=596 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 5 | load=2.03 io_psi10=0.09/0.09 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=512 settle_wait_s=0 |
