## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P16` | 3 | 1,351,205 | 1% | 0.60 | 0.36 | 0.24 | 2,313 | 1,685 | 0.599 | 0.647 | 0.695 | 25.4 | 0.86 | 0 |
| `e1/rc-noaof-get-c50-P16` | 3 | 1,333,191 | 1% | 0.60 | 0.36 | 0.25 | 2,316 | 1,701 | 0.607 | 0.655 | 0.695 | 25.4 | 0.87 | 0 |
| `fix/rc-noaof-get-c50-P16` | 3 | 1,333,191 | 0% | 0.60 | 0.36 | 0.25 | 2,317 | 1,694 | 0.607 | 0.655 | 0.695 | 25.3 | 0.87 | 0 |
| `redis-noaof-get-c50-P16` | 3 | 1,265,374 | 1% | 0.68 | 0.47 | 0.21 | 3,192 | 2,016 | 0.599 | 0.767 | 0.831 | 19.1 | 0.73 | 0 |
| `base/rc-noaof-mget-c50-P16` | 3 | 338,891 | 0% | 2.71 | 2.31 | 0.40 | 11,350 | 9,699 | 2.375 | 2.519 | 2.695 | 25.6 | 0.54 | 0 |
| `e1/rc-noaof-mget-c50-P16` | 3 | 338,891 | 2% | 2.72 | 2.32 | 0.40 | 11,353 | 9,728 | 2.383 | 2.527 | 2.695 | 25.7 | 0.53 | 0 |
| `fix/rc-noaof-mget-c50-P16` | 3 | 338,891 | 0% | 2.72 | 2.31 | 0.40 | 11,355 | 9,668 | 2.383 | 2.527 | 2.711 | 25.8 | 0.54 | 0 |
| `redis-noaof-mget-c50-P16` | 3 | 312,051 | 0% | 3.05 | 2.70 | 0.35 | 13,813 | 11,112 | 2.327 | 2.735 | 2.951 | 19.8 | 0.66 | 0 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-get-c50-P16` / `redis-noaof-get-c50-P16` | **1.068** | 1.054 1.068 1.054 |
| `e1/rc-noaof-get-c50-P16` / `redis-noaof-get-c50-P16` | **1.054** | 1.027 1.054 1.054 |
| `fix/rc-noaof-get-c50-P16` / `redis-noaof-get-c50-P16` | **1.054** | 1.040 1.054 1.054 |
| `base/rc-noaof-mget-c50-P16` / `redis-noaof-mget-c50-P16` | **1.086** | 1.086 1.086 1.086 |
| `e1/rc-noaof-mget-c50-P16` / `redis-noaof-mget-c50-P16` | **1.086** | 1.086 1.105 1.086 |
| `fix/rc-noaof-mget-c50-P16` / `redis-noaof-mget-c50-P16` | **1.086** | 1.086 1.086 1.086 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P16` | 1 | load=0.98 io_psi10=0.16/0.16 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2268 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 2 | load=2.06 io_psi10=0.25/0.25 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2672 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 3 | load=2.11 io_psi10=0.45/0.45 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1896 settle_wait_s=0 |
| `e1/rc-noaof-get-c50-P16` | 1 | load=1.41 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2448 settle_wait_s=0 |
| `e1/rc-noaof-get-c50-P16` | 2 | load=1.90 io_psi10=0.15/0.15 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2212 settle_wait_s=0 |
| `e1/rc-noaof-get-c50-P16` | 3 | load=2.14 io_psi10=0.58/0.58 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2584 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P16` | 1 | load=1.66 io_psi10=1.71/1.39 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2024 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P16` | 2 | load=1.74 io_psi10=0.57/0.57 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2588 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P16` | 3 | load=2.04 io_psi10=0.69/0.61 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1828 settle_wait_s=0 |
| `redis-noaof-get-c50-P16` | 1 | load=1.76 io_psi10=0.35/0.26 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2952 settle_wait_s=0 |
| `redis-noaof-get-c50-P16` | 2 | load=1.58 io_psi10=0.35/0.35 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2448 settle_wait_s=0 |
| `redis-noaof-get-c50-P16` | 3 | load=2.51 io_psi10=0.18/0.17 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2636 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 1 | load=1.83 io_psi10=0.39/0.19 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3324 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 2 | load=1.75 io_psi10=0.42/0.42 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2172 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 3 | load=2.15 io_psi10=0.34/0.34 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2880 settle_wait_s=0 |
| `e1/rc-noaof-mget-c50-P16` | 1 | load=1.73 io_psi10=1.06/1.02 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1796 settle_wait_s=0 |
| `e1/rc-noaof-mget-c50-P16` | 2 | load=1.68 io_psi10=0.76/0.76 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1660 settle_wait_s=0 |
| `e1/rc-noaof-mget-c50-P16` | 3 | load=2.04 io_psi10=1.00/1.00 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1868 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P16` | 1 | load=1.73 io_psi10=0.34/0.33 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1776 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P16` | 2 | load=1.68 io_psi10=0.47/0.46 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2036 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P16` | 3 | load=1.89 io_psi10=0.48/0.43 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=2168 settle_wait_s=0 |
| `redis-noaof-mget-c50-P16` | 1 | load=1.71 io_psi10=4.22/4.21 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=5380 settle_wait_s=0 |
| `redis-noaof-mget-c50-P16` | 2 | load=1.95 io_psi10=1.11/1.07 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=3488 settle_wait_s=0 |
| `redis-noaof-mget-c50-P16` | 3 | load=1.78 io_psi10=0.81/0.80 mem_psi10=0.00/0.00 swap_used_mib=5548 dirty_wb_kib=1784 settle_wait_s=0 |
