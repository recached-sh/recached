## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-c50-P1` | 3 | 183,459 | 4% | 3.62 | 0.87 | 2.76 | 5,572 | 4,988 | 0.255 | 0.351 | 0.383 | 26.7 | 1.21 | 0 |
| `new/rc-noaof-set-c50-P1` | 3 | 181,792 | 1% | 3.68 | 0.91 | 2.76 | 5,598 | 5,113 | 0.263 | 0.351 | 0.383 | 26.3 | 1.22 | 0 |
| `off/rc-noaof-set-c50-P1` | 3 | 169,480 | 3% | 3.83 | 0.88 | 2.95 | 4,907 | 5,120 | 0.295 | 0.319 | 0.351 | 26.0 | 1.26 | 0 |
| `base/rc-noaof-set-c50-P4` | 3 | 576,890 | 2% | 1.26 | 0.50 | 0.77 | 3,440 | 2,640 | 0.327 | 0.447 | 0.471 | 26.7 | 1.04 | 0 |
| `new/rc-noaof-set-c50-P4` | 3 | 538,097 | 1% | 1.32 | 0.49 | 0.83 | 3,271 | 2,675 | 0.383 | 0.407 | 0.439 | 25.8 | 1.11 | 0 |
| `off/rc-noaof-set-c50-P4` | 3 | 542,967 | 1% | 1.31 | 0.49 | 0.82 | 3,271 | 2,658 | 0.375 | 0.407 | 0.431 | 26.2 | 1.11 | 0 |
| `base/rc-noaof-set-c50-P16` | 3 | 1,276,216 | 7% | 0.66 | 0.43 | 0.23 | 2,928 | 2,002 | 0.591 | 0.767 | 0.831 | 26.8 | 0.71 | 0 |
| `new/rc-noaof-set-c50-P16` | 3 | 1,164,913 | 4% | 0.71 | 0.45 | 0.26 | 2,889 | 2,076 | 0.695 | 0.759 | 0.855 | 26.1 | 0.79 | 0 |
| `off/rc-noaof-set-c50-P16` | 3 | 1,164,913 | 6% | 0.70 | 0.44 | 0.26 | 2,889 | 2,064 | 0.695 | 0.751 | 0.831 | 26.0 | 0.76 | 0 |
| `base/rc-noaof-mset-c50-P16` | 3 | 208,435 | 7% | 4.63 | 4.29 | 0.34 | 27,178 | 17,194 | 3.783 | 4.711 | 5.031 | 27.0 | 0.34 | 0 |
| `new/rc-noaof-mset-c50-P16` | 3 | 199,847 | 7% | 4.78 | 4.33 | 0.44 | 27,135 | 17,548 | 4.071 | 4.407 | 4.679 | 26.5 | 0.24 | 0 |
| `off/rc-noaof-mset-c50-P16` | 3 | 199,847 | 8% | 4.77 | 4.32 | 0.45 | 27,138 | 17,498 | 4.071 | 4.351 | 4.655 | 26.2 | 0.25 | 0 |
| `redis-noaof-set-c50-P1` | 3 | 192,300 | 6% | 3.44 | 0.71 | 2.70 | 4,782 | 3,997 | 0.231 | 0.303 | 0.423 | 19.6 | 1.24 | 0 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.954** | 0.912 0.973 0.917 |
| `new/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.945** | 0.937 0.964 0.909 |
| `off/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.881** | 0.867 0.899 0.855 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` | 1 | load=0.82 io_psi10=3.75/3.68 mem_psi10=1.13/1.13 swap_used_mib=5410 dirty_wb_kib=3228 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=2.24 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2872 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=2.42 io_psi10=0.10/0.10 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2304 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 1 | load=2.32 io_psi10=1.27/1.10 mem_psi10=0.34/0.33 swap_used_mib=5410 dirty_wb_kib=3228 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 2 | load=2.33 io_psi10=0.26/0.25 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1880 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 3 | load=2.38 io_psi10=0.26/0.25 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2708 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P1` | 1 | load=2.68 io_psi10=0.83/0.79 mem_psi10=0.02/0.02 swap_used_mib=5410 dirty_wb_kib=4088 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P1` | 2 | load=2.23 io_psi10=0.00/0.00 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1720 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P1` | 3 | load=2.55 io_psi10=0.09/0.09 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2592 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P4` | 1 | load=2.47 io_psi10=0.19/0.19 mem_psi10=0.00/0.00 swap_used_mib=5410 dirty_wb_kib=3452 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P4` | 2 | load=2.36 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2528 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P4` | 3 | load=2.52 io_psi10=0.26/0.24 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2224 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P4` | 1 | load=2.87 io_psi10=0.25/0.20 mem_psi10=0.00/0.00 swap_used_mib=5410 dirty_wb_kib=2744 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P4` | 2 | load=2.04 io_psi10=0.99/0.95 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2492 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P4` | 3 | load=2.31 io_psi10=0.59/0.59 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2064 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P4` | 1 | load=2.67 io_psi10=0.08/0.06 mem_psi10=0.00/0.00 swap_used_mib=5410 dirty_wb_kib=2044 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P4` | 2 | load=1.67 io_psi10=0.84/0.78 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2176 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P4` | 3 | load=2.40 io_psi10=0.00/0.00 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2636 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 1 | load=2.49 io_psi10=0.01/0.00 mem_psi10=0.00/0.00 swap_used_mib=5410 dirty_wb_kib=2192 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 2 | load=1.81 io_psi10=7.34/6.60 mem_psi10=0.04/0.04 swap_used_mib=4670 dirty_wb_kib=3068 settle_wait_s=20 |
| `base/rc-noaof-set-c50-P16` | 3 | load=2.46 io_psi10=0.00/0.00 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2724 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 1 | load=2.33 io_psi10=0.23/0.21 mem_psi10=0.00/0.00 swap_used_mib=5410 dirty_wb_kib=3144 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 2 | load=1.92 io_psi10=0.41/0.35 mem_psi10=0.00/0.00 swap_used_mib=5408 dirty_wb_kib=5276 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 3 | load=2.18 io_psi10=0.24/0.24 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2600 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 1 | load=2.52 io_psi10=0.63/0.33 mem_psi10=0.00/0.00 swap_used_mib=5410 dirty_wb_kib=4248 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 2 | load=1.90 io_psi10=0.49/0.43 mem_psi10=0.00/0.00 swap_used_mib=5408 dirty_wb_kib=3280 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 3 | load=2.11 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2340 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 1 | load=2.47 io_psi10=0.50/0.31 mem_psi10=0.00/0.00 swap_used_mib=5409 dirty_wb_kib=4916 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 2 | load=1.62 io_psi10=0.04/0.03 mem_psi10=0.00/0.00 swap_used_mib=5408 dirty_wb_kib=3356 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 3 | load=2.07 io_psi10=0.40/0.38 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2404 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P16` | 1 | load=2.22 io_psi10=0.44/0.42 mem_psi10=0.00/0.00 swap_used_mib=5409 dirty_wb_kib=3140 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P16` | 2 | load=2.07 io_psi10=0.19/0.17 mem_psi10=0.00/0.00 swap_used_mib=5408 dirty_wb_kib=2136 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P16` | 3 | load=1.93 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1900 settle_wait_s=0 |
| `off/rc-noaof-mset-c50-P16` | 1 | load=1.77 io_psi10=0.23/0.23 mem_psi10=0.00/0.00 swap_used_mib=5408 dirty_wb_kib=3420 settle_wait_s=0 |
| `off/rc-noaof-mset-c50-P16` | 2 | load=2.48 io_psi10=0.52/0.17 mem_psi10=0.00/0.00 swap_used_mib=5408 dirty_wb_kib=4552 settle_wait_s=0 |
| `off/rc-noaof-mset-c50-P16` | 3 | load=1.62 io_psi10=1.33/1.33 mem_psi10=0.24/0.24 swap_used_mib=4669 dirty_wb_kib=3192 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 1 | load=1.68 io_psi10=0.35/0.32 mem_psi10=0.00/0.00 swap_used_mib=5408 dirty_wb_kib=2888 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 2 | load=2.09 io_psi10=0.45/0.40 mem_psi10=0.00/0.00 swap_used_mib=5408 dirty_wb_kib=2448 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 3 | load=1.65 io_psi10=0.35/0.35 mem_psi10=0.01/0.01 swap_used_mib=4669 dirty_wb_kib=3312 settle_wait_s=0 |
