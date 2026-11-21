## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

| cell | n | ops/s | spread | CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-everysec-set-P16` | 3 | 459,411 | 11% | 1.71 | 1.33 | 0.39 | 10,010 | 5,569 | 1.455 | 1.831 | 2.119 | 26.0 | 0.30 | 0 |
| `opt/rc-everysec-set-P16` | 3 | 539,928 | 8% | 1.33 | 0.97 | 0.36 | 6,383 | 4,068 | 1.151 | 1.471 | 1.703 | 27.2 | 0.35 | 0 |
| `redis-set-P16` | 3 | 455,830 | 3% | 1.32 | 0.96 | 0.36 | 7,338 | 4,053 | 1.063 | 1.487 | 4.671 | 31.0 | 0.28 | 0 |
| `valkey-set-P16` | 3 | 475,511 | 1% | 1.38 | 1.01 | 0.37 | 10,389 | 4,329 | 1.135 | 1.447 | 3.879 | 28.5 | 0.30 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-set-P16` | 0.0062 | 161.0 | 62.4 | 0.0003 |
| `opt/rc-everysec-set-P16` | 0.0062 | 161.1 | 72.4 | 0.0003 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-everysec-set-P16` / `redis-set-P16` | **1.008** | 0.923 1.059 1.000 |
| `base/rc-everysec-set-P16` / `valkey-set-P16` | **0.966** | 0.885 1.000 0.966 |
| `opt/rc-everysec-set-P16` / `redis-set-P16` | **1.184** | 1.124 1.217 1.209 |
| `opt/rc-everysec-set-P16` / `valkey-set-P16` | **1.135** | 1.077 1.149 1.167 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-everysec-set-P16` | 1 | load=2.80 io_psi10=6.76/5.83 mem_psi10=0.00/0.00 swap_used_mib=3904 dirty_wb_kib=1564 settle_wait_s=25 |
| `base/rc-everysec-set-P16` | 2 | load=2.04 io_psi10=6.70/5.21 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=3200 settle_wait_s=19 |
| `base/rc-everysec-set-P16` | 3 | load=1.60 io_psi10=7.71/5.79 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=2436 settle_wait_s=15 |
| `opt/rc-everysec-set-P16` | 1 | load=2.43 io_psi10=7.86/5.32 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=3192 settle_wait_s=15 |
| `opt/rc-everysec-set-P16` | 2 | load=2.23 io_psi10=7.61/5.41 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=1616 settle_wait_s=24 |
| `opt/rc-everysec-set-P16` | 3 | load=1.51 io_psi10=7.24/4.64 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=3836 settle_wait_s=12 |
| `redis-set-P16` | 1 | load=1.94 io_psi10=7.21/6.11 mem_psi10=0.00/0.00 swap_used_mib=3907 dirty_wb_kib=3176 settle_wait_s=26 |
| `redis-set-P16` | 2 | load=2.41 io_psi10=7.95/6.06 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=1332 settle_wait_s=25 |
| `redis-set-P16` | 3 | load=1.90 io_psi10=7.80/4.69 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=4048 settle_wait_s=11 |
| `valkey-set-P16` | 1 | load=2.60 io_psi10=7.81/6.27 mem_psi10=0.00/0.00 swap_used_mib=3907 dirty_wb_kib=3152 settle_wait_s=29 |
| `valkey-set-P16` | 2 | load=2.26 io_psi10=7.23/5.19 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=2724 settle_wait_s=24 |
| `valkey-set-P16` | 3 | load=2.31 io_psi10=7.65/5.02 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=2512 settle_wait_s=20 |
