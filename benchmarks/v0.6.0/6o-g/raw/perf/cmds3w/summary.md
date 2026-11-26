## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P16` | 3 | 3,477,958 | 10% | 0.68 | 0.42 | 0.27 | — | — | 0.199 | 0.319 | 0.431 | 27.2 | 2.13 | 0 |
| `base/rc-noaof-mget-c50-P16` | 3 | 878,812 | 1% | 3.18 | 2.77 | 0.42 | — | — | 0.847 | 1.335 | 1.615 | 26.5 | 1.88 | 0 |
| `base/rc-everysec-set-c50-P1` | 3 | 292,569 | 6% | 7.58 | 2.55 | 5.02 | — | — | 0.143 | 0.247 | 0.335 | 27.7 | 1.91 | 0 |
| `g/rc-noaof-get-c50-P16` | 3 | 3,477,354 | 2% | 0.69 | 0.42 | 0.27 | — | — | 0.207 | 0.319 | 0.415 | 26.3 | 2.14 | 0 |
| `g/rc-noaof-mget-c50-P16` | 3 | 878,735 | 1% | 3.20 | 2.78 | 0.42 | — | — | 0.855 | 1.351 | 1.639 | 26.2 | 1.80 | 0 |
| `g/rc-everysec-set-c50-P1` | 3 | 285,606 | 5% | 7.57 | 2.51 | 5.04 | — | — | 0.143 | 0.255 | 0.367 | 27.3 | 1.88 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-set-c50-P1` | 0.5515 | 1.8 | 2.8 | 0.0184 |
| `g/rc-everysec-set-c50-P1` | 0.5527 | 1.8 | 2.8 | 0.0186 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `g/rc-noaof-get-c50-P16` / `base/rc-noaof-get-c50-P16` | **1.000** | 1.087 0.957 1.000 |
| `g/rc-noaof-mget-c50-P16` / `base/rc-noaof-mget-c50-P16` | **1.000** | 1.011 0.989 1.000 |
| `g/rc-everysec-set-c50-P1` / `base/rc-everysec-set-c50-P1` | **0.976** | 0.942 1.036 0.988 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P16` | 1 | load=1.17 io_psi10=0.74/0.73 mem_psi10=0.00/0.00 swap_used_mib=5344 dirty_wb_kib=1608 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 2 | load=5.61 io_psi10=0.83/0.56 mem_psi10=0.00/0.00 swap_used_mib=5341 dirty_wb_kib=1776 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 3 | load=5.67 io_psi10=0.40/0.27 mem_psi10=0.00/0.00 swap_used_mib=5341 dirty_wb_kib=1688 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 1 | load=2.40 io_psi10=4.19/2.29 mem_psi10=0.16/0.00 swap_used_mib=5343 dirty_wb_kib=2476 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 2 | load=5.89 io_psi10=3.22/0.68 mem_psi10=0.09/0.00 swap_used_mib=5342 dirty_wb_kib=3440 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 3 | load=5.65 io_psi10=0.32/0.21 mem_psi10=0.00/0.00 swap_used_mib=5341 dirty_wb_kib=2944 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 1 | load=3.41 io_psi10=0.60/0.30 mem_psi10=0.01/0.00 swap_used_mib=5342 dirty_wb_kib=1696 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 2 | load=5.92 io_psi10=2.20/1.67 mem_psi10=0.00/0.00 swap_used_mib=5342 dirty_wb_kib=3360 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 3 | load=4.91 io_psi10=0.47/0.36 mem_psi10=0.00/0.00 swap_used_mib=5341 dirty_wb_kib=3360 settle_wait_s=0 |
| `g/rc-noaof-get-c50-P16` | 1 | load=4.29 io_psi10=4.05/2.50 mem_psi10=0.00/0.00 swap_used_mib=5342 dirty_wb_kib=3552 settle_wait_s=0 |
| `g/rc-noaof-get-c50-P16` | 2 | load=6.07 io_psi10=1.27/0.55 mem_psi10=0.00/0.00 swap_used_mib=5342 dirty_wb_kib=3060 settle_wait_s=0 |
| `g/rc-noaof-get-c50-P16` | 3 | load=5.37 io_psi10=2.18/0.27 mem_psi10=0.00/0.00 swap_used_mib=5341 dirty_wb_kib=2876 settle_wait_s=0 |
| `g/rc-noaof-mget-c50-P16` | 1 | load=4.34 io_psi10=1.75/1.28 mem_psi10=0.00/0.00 swap_used_mib=5342 dirty_wb_kib=3616 settle_wait_s=0 |
| `g/rc-noaof-mget-c50-P16` | 2 | load=5.82 io_psi10=3.50/0.99 mem_psi10=0.00/0.00 swap_used_mib=5342 dirty_wb_kib=4596 settle_wait_s=0 |
| `g/rc-noaof-mget-c50-P16` | 3 | load=5.03 io_psi10=1.47/0.82 mem_psi10=0.00/0.00 swap_used_mib=5341 dirty_wb_kib=2388 settle_wait_s=0 |
| `g/rc-everysec-set-c50-P1` | 1 | load=5.07 io_psi10=0.39/0.32 mem_psi10=0.00/0.00 swap_used_mib=5342 dirty_wb_kib=1876 settle_wait_s=0 |
| `g/rc-everysec-set-c50-P1` | 2 | load=5.49 io_psi10=5.57/2.52 mem_psi10=0.00/0.00 swap_used_mib=5342 dirty_wb_kib=4792 settle_wait_s=0 |
| `g/rc-everysec-set-c50-P1` | 3 | load=5.28 io_psi10=0.35/0.19 mem_psi10=0.00/0.00 swap_used_mib=5341 dirty_wb_kib=3124 settle_wait_s=0 |
