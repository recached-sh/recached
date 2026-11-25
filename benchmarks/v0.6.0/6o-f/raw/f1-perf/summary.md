## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-incr-c50-P1` | 5 | 176,455 | 1% | 3.77 | 0.93 | 2.85 | 5,435 | 5,083 | 0.263 | 0.367 | 0.399 | 27.4 | 1.21 | 0 |
| `pr1/rc-noaof-incr-c50-P1` | 5 | 172,622 | 1% | 3.86 | 0.96 | 2.92 | 5,440 | 5,269 | 0.271 | 0.375 | 0.407 | 27.7 | 1.20 | 0 |
| `base/rc-noaof-set-c50-P1` | 5 | 173,898 | 4% | 3.84 | 0.95 | 2.88 | 5,473 | 5,256 | 0.271 | 0.367 | 0.399 | 26.0 | 1.21 | 0 |
| `pr1/rc-noaof-set-c50-P1` | 5 | 172,647 | 4% | 3.86 | 0.95 | 2.89 | 5,478 | 5,247 | 0.271 | 0.375 | 0.407 | 26.2 | 1.21 | 0 |
| `base/rc-noaof-hset-c50-P1` | 5 | 172,652 | 1% | 3.86 | 0.97 | 2.91 | 5,894 | 5,288 | 0.271 | 0.375 | 0.399 | 24.5 | 1.20 | 0 |
| `pr1/rc-noaof-hset-c50-P1` | 5 | 171,419 | 3% | 3.86 | 0.97 | 2.91 | 5,893 | 5,365 | 0.271 | 0.375 | 0.407 | 24.6 | 1.20 | 0 |
| `base/rc-noaof-incr-c50-P16` | 5 | 1,110,963 | 2% | 0.74 | 0.48 | 0.27 | 2,898 | 2,184 | 0.727 | 0.775 | 0.847 | 27.5 | 0.73 | 0 |
| `pr1/rc-noaof-incr-c50-P16` | 5 | 1,123,293 | 5% | 0.73 | 0.46 | 0.27 | 2,898 | 2,142 | 0.719 | 0.775 | 0.871 | 27.5 | 0.74 | 0 |
| `base/rc-everysec-incr-c50-P1` | 5 | 131,130 | 3% | 5.50 | 1.75 | 3.76 | 9,763 | 9,050 | 0.359 | 0.487 | 0.527 | 27.6 | 0.94 | 0 |
| `pr1/rc-everysec-incr-c50-P1` | 5 | 128,320 | 4% | 5.57 | 1.78 | 3.81 | 9,760 | 9,189 | 0.367 | 0.503 | 0.543 | 27.5 | 0.92 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-incr-c50-P1` | 0.1005 | 9.9 | 6.2 | 0.0003 |
| `pr1/rc-everysec-incr-c50-P1` | 0.1003 | 10.0 | 6.4 | 0.0003 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-incr-c50-P1` | 1 | load=0.78 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=3608 dirty_wb_kib=920 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 2 | load=3.14 io_psi10=0.27/0.24 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3432 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 3 | load=3.17 io_psi10=0.24/0.24 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3508 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 4 | load=5.74 io_psi10=1.36/0.99 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=7096 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 5 | load=4.92 io_psi10=0.09/0.08 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=1752 settle_wait_s=0 |
| `pr1/rc-noaof-incr-c50-P1` | 1 | load=1.90 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=3608 dirty_wb_kib=2064 settle_wait_s=0 |
| `pr1/rc-noaof-incr-c50-P1` | 2 | load=3.25 io_psi10=0.59/0.39 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=5204 settle_wait_s=0 |
| `pr1/rc-noaof-incr-c50-P1` | 3 | load=2.89 io_psi10=0.13/0.12 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=1500 settle_wait_s=0 |
| `pr1/rc-noaof-incr-c50-P1` | 4 | load=3.09 io_psi10=0.05/0.01 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=1896 settle_wait_s=0 |
| `pr1/rc-noaof-incr-c50-P1` | 5 | load=4.40 io_psi10=0.31/0.15 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3356 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 1 | load=2.87 io_psi10=3.60/1.72 mem_psi10=0.00/0.00 swap_used_mib=3608 dirty_wb_kib=2380 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=3.62 io_psi10=0.57/0.54 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=5952 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=2.81 io_psi10=0.22/0.05 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=2220 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 4 | load=3.12 io_psi10=0.14/0.13 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3412 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 5 | load=3.87 io_psi10=0.18/0.03 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=2248 settle_wait_s=0 |
| `pr1/rc-noaof-set-c50-P1` | 1 | load=2.99 io_psi10=0.60/0.32 mem_psi10=0.00/0.00 swap_used_mib=3608 dirty_wb_kib=6796 settle_wait_s=0 |
| `pr1/rc-noaof-set-c50-P1` | 2 | load=3.28 io_psi10=1.52/0.92 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=4388 settle_wait_s=0 |
| `pr1/rc-noaof-set-c50-P1` | 3 | load=2.83 io_psi10=0.62/0.46 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=2656 settle_wait_s=0 |
| `pr1/rc-noaof-set-c50-P1` | 4 | load=3.21 io_psi10=0.31/0.19 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=2616 settle_wait_s=0 |
| `pr1/rc-noaof-set-c50-P1` | 5 | load=3.43 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3072 settle_wait_s=0 |
| `base/rc-noaof-hset-c50-P1` | 1 | load=3.67 io_psi10=0.13/0.11 mem_psi10=0.00/0.00 swap_used_mib=3608 dirty_wb_kib=1944 settle_wait_s=0 |
| `base/rc-noaof-hset-c50-P1` | 2 | load=3.33 io_psi10=0.28/0.10 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=2812 settle_wait_s=0 |
| `base/rc-noaof-hset-c50-P1` | 3 | load=3.03 io_psi10=0.16/0.14 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=2548 settle_wait_s=0 |
| `base/rc-noaof-hset-c50-P1` | 4 | load=2.70 io_psi10=0.10/0.09 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3056 settle_wait_s=0 |
| `base/rc-noaof-hset-c50-P1` | 5 | load=3.14 io_psi10=0.25/0.24 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=2548 settle_wait_s=0 |
| `pr1/rc-noaof-hset-c50-P1` | 1 | load=3.28 io_psi10=0.10/0.01 mem_psi10=0.00/0.00 swap_used_mib=3608 dirty_wb_kib=2660 settle_wait_s=0 |
| `pr1/rc-noaof-hset-c50-P1` | 2 | load=3.87 io_psi10=0.17/0.09 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3228 settle_wait_s=0 |
| `pr1/rc-noaof-hset-c50-P1` | 3 | load=2.83 io_psi10=0.04/0.02 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=1888 settle_wait_s=0 |
| `pr1/rc-noaof-hset-c50-P1` | 4 | load=2.82 io_psi10=0.11/0.05 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=2452 settle_wait_s=0 |
| `pr1/rc-noaof-hset-c50-P1` | 5 | load=3.44 io_psi10=1.81/1.38 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=5472 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P16` | 1 | load=3.10 io_psi10=0.40/0.16 mem_psi10=0.00/0.00 swap_used_mib=3608 dirty_wb_kib=3148 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P16` | 2 | load=3.83 io_psi10=1.07/0.39 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3752 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P16` | 3 | load=2.64 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3580 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P16` | 4 | load=3.06 io_psi10=0.78/0.25 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=2644 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P16` | 5 | load=3.73 io_psi10=0.08/0.06 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=2456 settle_wait_s=0 |
| `pr1/rc-noaof-incr-c50-P16` | 1 | load=3.22 io_psi10=0.16/0.08 mem_psi10=0.00/0.00 swap_used_mib=3608 dirty_wb_kib=2240 settle_wait_s=0 |
| `pr1/rc-noaof-incr-c50-P16` | 2 | load=4.71 io_psi10=7.29/2.16 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=5540 settle_wait_s=0 |
| `pr1/rc-noaof-incr-c50-P16` | 3 | load=2.60 io_psi10=0.19/0.18 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=2080 settle_wait_s=0 |
| `pr1/rc-noaof-incr-c50-P16` | 4 | load=3.39 io_psi10=7.28/1.68 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3284 settle_wait_s=0 |
| `pr1/rc-noaof-incr-c50-P16` | 5 | load=3.37 io_psi10=0.09/0.09 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3036 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 1 | load=2.81 io_psi10=0.40/0.28 mem_psi10=0.00/0.00 swap_used_mib=3608 dirty_wb_kib=1800 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 2 | load=4.23 io_psi10=5.60/1.05 mem_psi10=0.02/0.02 swap_used_mib=3612 dirty_wb_kib=4944 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 3 | load=2.57 io_psi10=0.07/0.06 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=1956 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 4 | load=3.03 io_psi10=4.75/0.61 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=4104 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 5 | load=3.92 io_psi10=1.01/0.86 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3104 settle_wait_s=0 |
| `pr1/rc-everysec-incr-c50-P1` | 1 | load=3.59 io_psi10=4.05/0.30 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3712 settle_wait_s=0 |
| `pr1/rc-everysec-incr-c50-P1` | 2 | load=4.69 io_psi10=3.99/0.22 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3720 settle_wait_s=0 |
| `pr1/rc-everysec-incr-c50-P1` | 3 | load=3.05 io_psi10=5.07/0.32 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3008 settle_wait_s=0 |
| `pr1/rc-everysec-incr-c50-P1` | 4 | load=2.71 io_psi10=5.87/0.24 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3408 settle_wait_s=0 |
| `pr1/rc-everysec-incr-c50-P1` | 5 | load=3.47 io_psi10=6.48/2.32 mem_psi10=0.00/0.00 swap_used_mib=3612 dirty_wb_kib=3456 settle_wait_s=0 |
