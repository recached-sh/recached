## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P16` | 3 | 1,408,371 | 0% | 0.57 | 0.33 | 0.24 | 2,213 | 1,607 | 0.583 | 0.615 | 0.639 | 25.3 | 0.89 | 0 |
| `fix/rc-noaof-get-c50-P16` | 3 | 1,369,788 | 1% | 0.59 | 0.34 | 0.24 | 2,313 | 1,664 | 0.591 | 0.631 | 0.655 | 25.8 | 0.88 | 0 |
| `redis-noaof-get-c50-P16` | 3 | 1,298,297 | 1% | 0.65 | 0.40 | 0.26 | 3,194 | 1,959 | 0.575 | 0.735 | 0.791 | 19.0 | 0.74 | 0 |
| `base/rc-noaof-mget-c50-P16` | 3 | 344,732 | 0% | 2.68 | 2.29 | 0.39 | 11,097 | 9,712 | 2.351 | 2.439 | 2.527 | 25.6 | 0.53 | 0 |
| `fix/rc-noaof-mget-c50-P16` | 3 | 350,779 | 2% | 2.65 | 2.27 | 0.38 | 11,350 | 9,594 | 2.327 | 2.431 | 2.527 | 25.9 | 0.53 | 0 |
| `redis-noaof-mget-c50-P16` | 3 | 322,082 | 2% | 2.96 | 2.62 | 0.34 | 13,812 | 10,886 | 2.247 | 2.639 | 2.759 | 19.4 | 0.70 | 0 |
| `base/rc-noaof-set-c50-P16` | 3 | 1,234,446 | 2% | 0.67 | 0.42 | 0.25 | 2,890 | 1,993 | 0.663 | 0.703 | 0.759 | 26.0 | 0.79 | 0 |
| `fix/rc-noaof-set-c50-P16` | 3 | 1,219,334 | 5% | 0.68 | 0.43 | 0.25 | 2,914 | 2,021 | 0.671 | 0.711 | 0.759 | 26.7 | 0.76 | 0 |
| `redis-noaof-set-c50-P16` | 3 | 1,176,028 | 1% | 0.73 | 0.56 | 0.17 | 3,965 | 2,295 | 0.639 | 0.799 | 0.863 | 19.6 | 0.67 | 0 |
| `base/rc-noaof-mset-c50-P16` | 3 | 212,675 | 1% | 4.48 | 4.08 | 0.41 | 27,092 | 16,869 | 3.847 | 3.951 | 4.151 | 26.3 | 0.25 | 0 |
| `fix/rc-noaof-mset-c50-P16` | 3 | 210,447 | 1% | 4.55 | 4.16 | 0.39 | 27,487 | 17,155 | 3.911 | 4.007 | 4.151 | 26.9 | 0.24 | 0 |
| `redis-noaof-mset-c50-P16` | 3 | 273,553 | 0% | 3.49 | 3.19 | 0.30 | 19,543 | 13,095 | 2.743 | 3.175 | 3.295 | 20.1 | 0.54 | 0 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-get-c50-P16` / `redis-noaof-get-c50-P16` | **1.085** | 1.071 1.085 1.085 |
| `fix/rc-noaof-get-c50-P16` / `redis-noaof-get-c50-P16` | **1.055** | 1.041 1.070 1.055 |
| `base/rc-noaof-mget-c50-P16` / `redis-noaof-mget-c50-P16` | **1.070** | 1.070 1.070 1.087 |
| `fix/rc-noaof-mget-c50-P16` / `redis-noaof-mget-c50-P16` | **1.089** | 1.070 1.089 1.106 |
| `base/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.050** | 1.050 1.025 1.063 |
| `fix/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.037** | 0.989 1.025 1.037 |
| `base/rc-noaof-mset-c50-P16` / `redis-noaof-mset-c50-P16` | **0.777** | 0.769 0.777 0.778 |
| `fix/rc-noaof-mset-c50-P16` / `redis-noaof-mset-c50-P16` | **0.769** | 0.761 0.769 0.769 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P16` | 1 | load=0.84 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=1444 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 2 | load=1.68 io_psi10=1.26/1.23 mem_psi10=0.00/0.00 swap_used_mib=5466 dirty_wb_kib=1912 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 3 | load=1.85 io_psi10=0.28/0.27 mem_psi10=0.00/0.00 swap_used_mib=5466 dirty_wb_kib=2068 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P16` | 1 | load=1.10 io_psi10=0.43/0.43 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=1420 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P16` | 2 | load=1.56 io_psi10=0.25/0.25 mem_psi10=0.00/0.00 swap_used_mib=5467 dirty_wb_kib=2492 settle_wait_s=0 |
| `fix/rc-noaof-get-c50-P16` | 3 | load=1.98 io_psi10=0.29/0.29 mem_psi10=0.00/0.00 swap_used_mib=5466 dirty_wb_kib=3512 settle_wait_s=0 |
| `redis-noaof-get-c50-P16` | 1 | load=1.29 io_psi10=0.36/0.36 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=3148 settle_wait_s=0 |
| `redis-noaof-get-c50-P16` | 2 | load=1.38 io_psi10=1.00/1.00 mem_psi10=0.00/0.00 swap_used_mib=5467 dirty_wb_kib=2240 settle_wait_s=0 |
| `redis-noaof-get-c50-P16` | 3 | load=2.06 io_psi10=1.15/1.15 mem_psi10=0.00/0.00 swap_used_mib=5466 dirty_wb_kib=3052 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 1 | load=1.59 io_psi10=0.88/0.88 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=1564 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 2 | load=1.53 io_psi10=0.28/0.28 mem_psi10=0.00/0.00 swap_used_mib=5467 dirty_wb_kib=1184 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 3 | load=2.20 io_psi10=0.56/0.56 mem_psi10=0.00/0.00 swap_used_mib=5466 dirty_wb_kib=3584 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P16` | 1 | load=1.61 io_psi10=0.72/0.68 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=2128 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P16` | 2 | load=1.60 io_psi10=1.13/1.12 mem_psi10=0.00/0.00 swap_used_mib=5467 dirty_wb_kib=1748 settle_wait_s=0 |
| `fix/rc-noaof-mget-c50-P16` | 3 | load=2.09 io_psi10=1.50/1.45 mem_psi10=0.00/0.00 swap_used_mib=5466 dirty_wb_kib=1852 settle_wait_s=0 |
| `redis-noaof-mget-c50-P16` | 1 | load=1.70 io_psi10=0.56/0.55 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=3156 settle_wait_s=0 |
| `redis-noaof-mget-c50-P16` | 2 | load=1.77 io_psi10=0.53/0.48 mem_psi10=0.00/0.00 swap_used_mib=5467 dirty_wb_kib=4076 settle_wait_s=0 |
| `redis-noaof-mget-c50-P16` | 3 | load=1.85 io_psi10=0.39/0.38 mem_psi10=0.00/0.00 swap_used_mib=5466 dirty_wb_kib=2864 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 1 | load=1.67 io_psi10=1.30/1.29 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=1844 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 2 | load=1.77 io_psi10=1.88/1.63 mem_psi10=0.00/0.00 swap_used_mib=5467 dirty_wb_kib=1988 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 3 | load=1.68 io_psi10=1.16/1.16 mem_psi10=0.00/0.00 swap_used_mib=5466 dirty_wb_kib=3084 settle_wait_s=0 |
| `fix/rc-noaof-set-c50-P16` | 1 | load=1.84 io_psi10=0.21/0.21 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=1716 settle_wait_s=0 |
| `fix/rc-noaof-set-c50-P16` | 2 | load=1.68 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=5467 dirty_wb_kib=3556 settle_wait_s=0 |
| `fix/rc-noaof-set-c50-P16` | 3 | load=1.77 io_psi10=1.20/1.20 mem_psi10=0.00/0.00 swap_used_mib=5466 dirty_wb_kib=3724 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 1 | load=1.90 io_psi10=0.21/0.21 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=3204 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 2 | load=1.70 io_psi10=0.14/0.14 mem_psi10=0.00/0.00 swap_used_mib=5467 dirty_wb_kib=1884 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 3 | load=1.78 io_psi10=0.99/0.96 mem_psi10=0.00/0.00 swap_used_mib=5466 dirty_wb_kib=2108 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 1 | load=1.84 io_psi10=0.78/0.78 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=3272 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 2 | load=1.87 io_psi10=0.25/0.24 mem_psi10=0.00/0.00 swap_used_mib=5467 dirty_wb_kib=1916 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 3 | load=1.70 io_psi10=0.83/0.83 mem_psi10=0.00/0.00 swap_used_mib=5466 dirty_wb_kib=2396 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P16` | 1 | load=1.82 io_psi10=0.27/0.27 mem_psi10=0.00/0.00 swap_used_mib=5468 dirty_wb_kib=1488 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P16` | 2 | load=1.89 io_psi10=0.16/0.16 mem_psi10=0.00/0.00 swap_used_mib=5467 dirty_wb_kib=3216 settle_wait_s=0 |
| `fix/rc-noaof-mset-c50-P16` | 3 | load=1.48 io_psi10=0.58/0.58 mem_psi10=0.00/0.00 swap_used_mib=5466 dirty_wb_kib=3148 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 1 | load=1.74 io_psi10=0.32/0.30 mem_psi10=0.00/0.00 swap_used_mib=5467 dirty_wb_kib=2268 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 2 | load=1.67 io_psi10=0.15/0.15 mem_psi10=0.00/0.00 swap_used_mib=5467 dirty_wb_kib=2776 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 3 | load=1.37 io_psi10=0.17/0.15 mem_psi10=0.00/0.00 swap_used_mib=5464 dirty_wb_kib=2760 settle_wait_s=0 |
