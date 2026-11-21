## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

| cell | n | ops/s | spread | CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-P1` | 3 | 148,137 | 6% | 4.51 | 1.05 | 3.44 | 5,246 | 5,931 | 0.343 | 0.367 | 0.415 | 25.8 | 1.19 | 0 |
| `redis-noaof-set-P1` | 3 | 181,802 | 2% | 3.69 | 0.72 | 2.96 | 4,806 | 3,976 | 0.247 | 0.319 | 0.463 | 19.6 | 1.22 | 0 |
| `base/rc-aofno-set-P1` | 3 | 114,273 | 6% | 6.38 | 2.29 | 4.09 | 13,903 | 11,576 | 0.407 | 0.551 | 0.615 | 25.7 | 0.89 | 0 |
| `redis-aofno-set-P1` | 3 | 148,082 | 2% | 4.43 | 1.09 | 3.36 | 8,184 | 5,782 | 0.279 | 0.383 | 0.535 | 19.7 | 1.02 | 0 |
| `base/rc-everysec-set-P1` | 3 | 114,260 | 3% | 6.42 | 2.27 | 4.14 | 13,902 | 11,557 | 0.407 | 0.559 | 0.599 | 25.8 | 0.87 | 0 |
| `redis-set-P1` | 3 | 148,082 | 4% | 4.36 | 1.13 | 3.26 | 8,175 | 5,681 | 0.279 | 0.383 | 0.535 | 24.6 | 1.01 | 0 |
| `base/rc-always-set-P1` | 3 | 330 | 7% | 23.80 | 5.80 | 17.80 | 15,826 | 25,760 | 134.015 | 191.615 | 516.863 | 23.2 | 0.01 | 0 |
| `redis-always-set-P1` | 3 | 434 | 9% | 16.40 | 2.60 | 13.80 | 9,142 | 12,904 | 100.095 | 183.295 | 400.127 | 17.4 | 0.01 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-aofno-set-P1` | 0.0999 | 10.0 | 5.3 | 0.0007 |
| `base/rc-everysec-set-P1` | 0.0999 | 10.0 | 7.1 | 0.0010 |
| `base/rc-always-set-P1` | 0.0399 | 25.1 | 21.8 | 0.3659 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-everysec-set-P1` / `redis-set-P1` | **0.772** | 0.789 0.771 0.783 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-P1` | 1 | load=0.42 io_psi10=4.11/3.89 mem_psi10=0.00/0.00 swap_used_mib=1423 dirty_wb_kib=3564 |
| `base/rc-noaof-set-P1` | 2 | load=2.11 io_psi10=58.34/54.26 mem_psi10=0.00/0.00 swap_used_mib=1376 dirty_wb_kib=6052 |
| `base/rc-noaof-set-P1` | 3 | load=1.81 io_psi10=61.71/57.23 mem_psi10=0.00/0.00 swap_used_mib=1351 dirty_wb_kib=5844 |
| `redis-noaof-set-P1` | 1 | load=1.16 io_psi10=2.79/2.46 mem_psi10=0.00/0.00 swap_used_mib=1423 dirty_wb_kib=4512 |
| `redis-noaof-set-P1` | 2 | load=2.31 io_psi10=10.51/9.76 mem_psi10=0.00/0.00 swap_used_mib=1376 dirty_wb_kib=3944 |
| `redis-noaof-set-P1` | 3 | load=2.29 io_psi10=12.93/11.98 mem_psi10=0.00/0.00 swap_used_mib=1351 dirty_wb_kib=3588 |
| `base/rc-aofno-set-P1` | 1 | load=1.27 io_psi10=3.86/3.07 mem_psi10=0.00/0.00 swap_used_mib=1423 dirty_wb_kib=3444 |
| `base/rc-aofno-set-P1` | 2 | load=2.47 io_psi10=5.89/5.39 mem_psi10=0.00/0.00 swap_used_mib=1376 dirty_wb_kib=2868 |
| `base/rc-aofno-set-P1` | 3 | load=2.93 io_psi10=4.52/4.22 mem_psi10=0.00/0.00 swap_used_mib=1351 dirty_wb_kib=2244 |
| `redis-aofno-set-P1` | 1 | load=1.92 io_psi10=7.02/6.10 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=8224 |
| `redis-aofno-set-P1` | 2 | load=2.67 io_psi10=2.13/1.99 mem_psi10=0.00/0.00 swap_used_mib=1372 dirty_wb_kib=3432 |
| `redis-aofno-set-P1` | 3 | load=3.02 io_psi10=2.73/2.52 mem_psi10=0.00/0.00 swap_used_mib=1351 dirty_wb_kib=4716 |
| `base/rc-everysec-set-P1` | 1 | load=2.17 io_psi10=8.45/4.68 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=8236 |
| `base/rc-everysec-set-P1` | 2 | load=2.54 io_psi10=6.23/3.20 mem_psi10=0.00/0.00 swap_used_mib=1371 dirty_wb_kib=6040 |
| `base/rc-everysec-set-P1` | 3 | load=3.17 io_psi10=8.04/4.35 mem_psi10=0.00/0.00 swap_used_mib=1351 dirty_wb_kib=7028 |
| `redis-set-P1` | 1 | load=2.43 io_psi10=9.43/5.02 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=8832 |
| `redis-set-P1` | 2 | load=3.18 io_psi10=6.88/2.61 mem_psi10=0.00/0.00 swap_used_mib=1371 dirty_wb_kib=5400 |
| `redis-set-P1` | 3 | load=2.99 io_psi10=6.37/2.47 mem_psi10=0.00/0.00 swap_used_mib=1351 dirty_wb_kib=6740 |
| `base/rc-always-set-P1` | 1 | load=3.00 io_psi10=14.81/7.43 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=12524 |
| `base/rc-always-set-P1` | 2 | load=3.30 io_psi10=11.76/4.45 mem_psi10=0.00/0.00 swap_used_mib=1371 dirty_wb_kib=6148 |
| `base/rc-always-set-P1` | 3 | load=3.01 io_psi10=14.29/5.25 mem_psi10=0.00/0.00 swap_used_mib=1351 dirty_wb_kib=8120 |
| `redis-always-set-P1` | 1 | load=2.64 io_psi10=66.50/62.01 mem_psi10=0.00/0.00 swap_used_mib=1376 dirty_wb_kib=5628 |
| `redis-always-set-P1` | 2 | load=1.82 io_psi10=63.61/59.26 mem_psi10=0.00/0.00 swap_used_mib=1359 dirty_wb_kib=7360 |
| `redis-always-set-P1` | 3 | load=2.12 io_psi10=62.17/57.99 mem_psi10=0.00/0.00 swap_used_mib=1350 dirty_wb_kib=6532 |
