## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-c50-P1` | 3 | 175,426 | 2% | 3.78 | 0.96 | 2.82 | 5,573 | 5,290 | 0.271 | 0.367 | 0.399 | 27.4 | 1.19 | 0 |
| `new/rc-noaof-set-c50-P1` | 3 | 178,540 | 1% | 3.72 | 0.94 | 2.78 | 5,585 | 5,227 | 0.263 | 0.359 | 0.391 | 26.7 | 1.22 | 0 |
| `off/rc-noaof-set-c50-P1` | 3 | 165,273 | 1% | 3.95 | 0.93 | 3.01 | 4,884 | 5,316 | 0.311 | 0.327 | 0.359 | 26.6 | 1.25 | 0 |
| `redis-noaof-set-c50-P1` | 3 | 188,644 | 0% | 3.49 | 0.73 | 2.75 | 4,820 | 4,103 | 0.239 | 0.311 | 0.431 | 19.4 | 1.22 | 0 |
| `base/rc-noaof-set-c50-P16` | 3 | 1,211,876 | 1% | 0.70 | 0.47 | 0.23 | 2,928 | 2,100 | 0.623 | 0.823 | 0.887 | 27.4 | 0.70 | 0 |
| `new/rc-noaof-set-c50-P16` | 3 | 1,142,726 | 3% | 0.72 | 0.46 | 0.26 | 2,888 | 2,108 | 0.711 | 0.775 | 0.855 | 26.7 | 0.75 | 0 |
| `off/rc-noaof-set-c50-P16` | 3 | 1,153,624 | 1% | 0.71 | 0.45 | 0.26 | 2,888 | 2,106 | 0.703 | 0.767 | 0.871 | 26.7 | 0.75 | 0 |
| `redis-noaof-set-c50-P16` | 3 | 1,090,433 | 1% | 0.80 | 0.56 | 0.24 | 3,976 | 2,483 | 0.703 | 0.879 | 0.967 | 19.4 | 0.64 | 0 |
| `base/rc-noaof-mset-c50-P16` | 3 | 201,417 | 1% | 4.75 | 4.39 | 0.37 | 27,178 | 17,566 | 3.887 | 4.887 | 5.191 | 27.9 | 0.33 | 0 |
| `new/rc-noaof-mset-c50-P16` | 3 | 198,196 | 2% | 4.82 | 4.37 | 0.46 | 27,138 | 17,675 | 4.127 | 4.511 | 4.735 | 27.3 | 0.24 | 0 |
| `off/rc-noaof-mset-c50-P16` | 3 | 193,399 | 2% | 4.92 | 4.46 | 0.47 | 27,138 | 18,023 | 4.191 | 4.647 | 5.039 | 27.7 | 0.24 | 0 |
| `redis-noaof-mset-c50-P16` | 3 | 242,043 | 3% | 3.95 | 3.58 | 0.37 | 19,549 | 14,457 | 3.151 | 3.951 | 4.319 | 19.9 | 0.48 | 0 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.930** | 0.946 0.930 0.930 |
| `new/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.946** | 0.946 0.938 0.947 |
| `off/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.876** | 0.869 0.876 0.876 |
| `base/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.111** | 1.111 1.123 1.101 |
| `new/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.048** | 1.079 1.048 1.038 |
| `off/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.058** | 1.048 1.058 1.048 |
| `base/rc-noaof-mset-c50-P16` / `redis-noaof-mset-c50-P16` | **0.832** | 0.832 0.822 0.841 |
| `new/rc-noaof-mset-c50-P16` / `redis-noaof-mset-c50-P16` | **0.819** | 0.806 0.802 0.827 |
| `off/rc-noaof-mset-c50-P16` / `redis-noaof-mset-c50-P16` | **0.799** | 0.799 0.783 0.820 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` | 1 | load=1.02 io_psi10=3.47/3.12 mem_psi10=0.00/0.00 swap_used_mib=5173 dirty_wb_kib=1540 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=3.03 io_psi10=1.03/0.55 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=6644 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=3.02 io_psi10=0.43/0.35 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=2652 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 1 | load=2.21 io_psi10=0.36/0.32 mem_psi10=0.00/0.00 swap_used_mib=5173 dirty_wb_kib=2624 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 2 | load=3.04 io_psi10=0.45/0.10 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=3452 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 3 | load=2.94 io_psi10=0.28/0.25 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=3164 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P1` | 1 | load=2.50 io_psi10=1.36/0.45 mem_psi10=0.00/0.00 swap_used_mib=5173 dirty_wb_kib=4056 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P1` | 2 | load=3.10 io_psi10=0.43/0.22 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=5504 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P1` | 3 | load=2.84 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=2220 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 1 | load=3.11 io_psi10=0.18/0.14 mem_psi10=0.00/0.00 swap_used_mib=5173 dirty_wb_kib=2784 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 2 | load=1.92 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=2236 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 3 | load=3.16 io_psi10=0.87/0.42 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=9820 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 1 | load=3.11 io_psi10=7.95/5.61 mem_psi10=0.00/0.00 swap_used_mib=5173 dirty_wb_kib=32244 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 2 | load=2.08 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=2560 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 3 | load=3.24 io_psi10=0.70/0.54 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=6184 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 1 | load=3.13 io_psi10=3.20/2.38 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=16008 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 2 | load=2.01 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=2548 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 3 | load=3.12 io_psi10=0.37/0.32 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2800 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 1 | load=3.14 io_psi10=0.77/0.51 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=3996 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 2 | load=1.62 io_psi10=0.23/0.21 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=2956 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 3 | load=2.82 io_psi10=1.52/0.93 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=3528 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 1 | load=3.27 io_psi10=0.41/0.35 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=4120 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 2 | load=1.94 io_psi10=0.07/0.06 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=3596 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 3 | load=2.64 io_psi10=0.37/0.30 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=3572 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 1 | load=2.75 io_psi10=0.26/0.24 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=3500 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 2 | load=2.00 io_psi10=0.35/0.28 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=2104 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 3 | load=2.73 io_psi10=0.19/0.18 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=3192 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P16` | 1 | load=2.25 io_psi10=1.38/1.19 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=1120 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P16` | 2 | load=2.37 io_psi10=4.23/3.21 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=21364 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P16` | 3 | load=3.81 io_psi10=0.15/0.02 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2924 settle_wait_s=0 |
| `off/rc-noaof-mset-c50-P16` | 1 | load=2.01 io_psi10=0.35/0.25 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=3336 settle_wait_s=0 |
| `off/rc-noaof-mset-c50-P16` | 2 | load=1.95 io_psi10=0.90/0.81 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=2732 settle_wait_s=0 |
| `off/rc-noaof-mset-c50-P16` | 3 | load=2.88 io_psi10=0.17/0.14 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=1768 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 1 | load=2.22 io_psi10=7.80/6.03 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=20848 settle_wait_s=12 |
| `redis-noaof-mset-c50-P16` | 2 | load=2.02 io_psi10=1.09/0.79 mem_psi10=0.00/0.00 swap_used_mib=5172 dirty_wb_kib=2912 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 3 | load=2.74 io_psi10=0.90/0.71 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2892 settle_wait_s=0 |
