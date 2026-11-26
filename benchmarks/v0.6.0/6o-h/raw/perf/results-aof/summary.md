## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-everysec-incr-c50-P1` | 5 | 131,857 | 4% | 5.49 | 1.80 | 3.74 | — | — | 0.359 | 0.487 | 0.519 | 27.4 | 0.94 | 0 |
| `h/rc-everysec-incr-c50-P1` | 5 | 129,016 | 2% | 5.61 | 1.86 | 3.76 | — | — | 0.367 | 0.495 | 0.527 | 27.6 | 0.93 | 0 |
| `base/rc-everysec-set-c50-P1` | 5 | 130,418 | 2% | 5.54 | 1.76 | 3.77 | — | — | 0.359 | 0.487 | 0.519 | 25.8 | 0.95 | 0 |
| `h/rc-everysec-set-c50-P1` | 5 | 129,010 | 1% | 5.63 | 1.88 | 3.76 | — | — | 0.367 | 0.495 | 0.527 | 26.0 | 0.93 | 0 |
| `base/rc-everysec-hset-c50-P1` | 5 | 129,016 | 2% | 5.63 | 1.80 | 3.82 | — | — | 0.367 | 0.495 | 0.527 | 24.3 | 0.93 | 0 |
| `h/rc-everysec-hset-c50-P1` | 5 | 124,990 | 2% | 5.84 | 1.97 | 3.87 | — | — | 0.375 | 0.511 | 0.535 | 24.2 | 0.90 | 0 |
| `base/rc-everysec-setpx100-c50-P1` | 5 | 117,638 | 1% | 6.34 | 2.39 | 3.95 | — | — | 0.399 | 0.543 | 0.591 | 30.9 | 0.85 | 0 |
| `h/rc-everysec-setpx100-c50-P1` | 5 | 95,223 | 2% | 8.22 | 3.94 | 4.29 | — | — | 0.495 | 0.663 | 0.711 | 31.2 | 0.70 | 0 |
| `base/rc-everysec-setpx1000-c50-P1` | 5 | 118,798 | 2% | 6.24 | 2.25 | 3.97 | — | — | 0.399 | 0.535 | 0.575 | 31.0 | 0.86 | 0 |
| `h/rc-everysec-setpx1000-c50-P1` | 5 | 105,252 | 16% | 7.25 | 3.05 | 4.17 | — | — | 0.447 | 0.599 | 0.639 | 31.3 | 0.77 | 0 |
| `base/rc-everysec-incrvol-c50-P1` | 5 | 130,423 | 2% | 5.55 | 1.82 | 3.75 | — | — | 0.359 | 0.487 | 0.519 | 27.8 | 0.94 | 0 |
| `h/rc-everysec-incrvol-c50-P1` | 5 | 127,649 | 2% | 5.70 | 1.93 | 3.77 | — | — | 0.367 | 0.503 | 0.535 | 27.8 | 0.91 | 0 |
| `base/rc-noaof-setpx100-c50-P1` | 5 | 162,153 | 1% | 4.25 | 1.31 | 2.94 | — | — | 0.287 | 0.399 | 0.471 | 30.4 | 1.13 | 0 |
| `base/rc-noaof-incrvol-c50-P1` | 5 | 179,094 | 2% | 3.71 | 0.91 | 2.78 | — | — | 0.263 | 0.359 | 0.391 | 28.1 | 1.22 | 0 |
| `h/rc-noaof-setpx100-c50-P1` | 5 | 162,153 | 3% | 4.29 | 1.35 | 2.94 | — | — | 0.287 | 0.399 | 0.471 | 30.7 | 1.11 | 0 |
| `h/rc-noaof-incrvol-c50-P1` | 5 | 179,072 | 3% | 3.73 | 0.93 | 2.81 | — | — | 0.263 | 0.367 | 0.399 | 27.9 | 1.21 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-incr-c50-P1` | 0.1000 | 10.0 | 5.6 | 0.0005 |
| `h/rc-everysec-incr-c50-P1` | 0.1000 | 10.0 | 5.7 | 0.0005 |
| `base/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 5.9 | 0.0005 |
| `h/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 6.0 | 0.0005 |
| `base/rc-everysec-hset-c50-P1` | 0.1000 | 10.0 | 6.5 | 0.0006 |
| `h/rc-everysec-hset-c50-P1` | 0.0999 | 10.0 | 6.4 | 0.0005 |
| `base/rc-everysec-setpx100-c50-P1` | 0.1000 | 10.0 | 6.7 | 0.0006 |
| `h/rc-everysec-setpx100-c50-P1` | 0.1002 | 18.2 | 8.5 | 0.0008 |
| `base/rc-everysec-setpx1000-c50-P1` | 0.1000 | 10.0 | 7.1 | 0.0006 |
| `h/rc-everysec-setpx1000-c50-P1` | 0.1001 | 13.3 | 7.5 | 0.0007 |
| `base/rc-everysec-incrvol-c50-P1` | 0.1000 | 10.0 | 5.7 | 0.0005 |
| `h/rc-everysec-incrvol-c50-P1` | 0.1000 | 10.2 | 5.9 | 0.0005 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-everysec-incr-c50-P1` | 1 | load=1.39 io_psi10=2.20/2.04 mem_psi10=0.06/0.06 swap_used_mib=8184 dirty_wb_kib=1360 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 2 | load=3.25 io_psi10=6.19/0.62 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=4072 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 3 | load=3.16 io_psi10=7.74/2.59 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=6080 settle_wait_s=2 |
| `base/rc-everysec-incr-c50-P1` | 4 | load=2.75 io_psi10=4.50/0.26 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=2568 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 5 | load=2.64 io_psi10=3.64/0.61 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=4704 settle_wait_s=0 |
| `h/rc-everysec-incr-c50-P1` | 1 | load=1.68 io_psi10=6.88/1.97 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=2204 settle_wait_s=0 |
| `h/rc-everysec-incr-c50-P1` | 2 | load=3.50 io_psi10=6.36/0.14 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=1756 settle_wait_s=0 |
| `h/rc-everysec-incr-c50-P1` | 3 | load=3.07 io_psi10=5.86/0.47 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=3028 settle_wait_s=0 |
| `h/rc-everysec-incr-c50-P1` | 4 | load=2.81 io_psi10=6.91/0.52 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=3652 settle_wait_s=0 |
| `h/rc-everysec-incr-c50-P1` | 5 | load=2.57 io_psi10=5.79/0.53 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=4308 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 1 | load=1.73 io_psi10=5.27/0.33 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=2524 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 2 | load=2.35 io_psi10=6.63/0.74 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=3808 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 3 | load=2.61 io_psi10=7.09/2.54 mem_psi10=0.01/0.01 swap_used_mib=8183 dirty_wb_kib=5060 settle_wait_s=5 |
| `base/rc-everysec-set-c50-P1` | 4 | load=2.79 io_psi10=6.89/0.84 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=4152 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 5 | load=2.64 io_psi10=6.05/2.19 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=3424 settle_wait_s=0 |
| `h/rc-everysec-set-c50-P1` | 1 | load=1.79 io_psi10=7.73/2.91 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=3700 settle_wait_s=2 |
| `h/rc-everysec-set-c50-P1` | 2 | load=2.74 io_psi10=6.50/0.36 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=3708 settle_wait_s=0 |
| `h/rc-everysec-set-c50-P1` | 3 | load=2.32 io_psi10=6.79/0.43 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2908 settle_wait_s=0 |
| `h/rc-everysec-set-c50-P1` | 4 | load=2.56 io_psi10=6.39/0.23 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=3024 settle_wait_s=0 |
| `h/rc-everysec-set-c50-P1` | 5 | load=2.64 io_psi10=4.42/0.54 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=2536 settle_wait_s=0 |
| `base/rc-everysec-hset-c50-P1` | 1 | load=2.06 io_psi10=5.06/0.46 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=2648 settle_wait_s=0 |
| `base/rc-everysec-hset-c50-P1` | 2 | load=2.38 io_psi10=6.41/0.54 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=4728 settle_wait_s=0 |
| `base/rc-everysec-hset-c50-P1` | 3 | load=2.26 io_psi10=7.74/0.32 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=3668 settle_wait_s=0 |
| `base/rc-everysec-hset-c50-P1` | 4 | load=2.52 io_psi10=7.26/0.75 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=4404 settle_wait_s=0 |
| `base/rc-everysec-hset-c50-P1` | 5 | load=2.55 io_psi10=6.43/1.83 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=4844 settle_wait_s=0 |
| `h/rc-everysec-hset-c50-P1` | 1 | load=2.27 io_psi10=6.79/1.28 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=4176 settle_wait_s=0 |
| `h/rc-everysec-hset-c50-P1` | 2 | load=2.20 io_psi10=6.50/0.36 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=3140 settle_wait_s=0 |
| `h/rc-everysec-hset-c50-P1` | 3 | load=2.23 io_psi10=4.36/0.89 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=5024 settle_wait_s=0 |
| `h/rc-everysec-hset-c50-P1` | 4 | load=2.48 io_psi10=6.88/0.82 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=4752 settle_wait_s=0 |
| `h/rc-everysec-hset-c50-P1` | 5 | load=2.86 io_psi10=6.04/1.67 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=3624 settle_wait_s=0 |
| `base/rc-everysec-setpx100-c50-P1` | 1 | load=2.36 io_psi10=5.28/0.77 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=2944 settle_wait_s=0 |
| `base/rc-everysec-setpx100-c50-P1` | 2 | load=2.05 io_psi10=6.65/2.58 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2548 settle_wait_s=3 |
| `base/rc-everysec-setpx100-c50-P1` | 3 | load=2.29 io_psi10=7.33/0.73 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=3640 settle_wait_s=0 |
| `base/rc-everysec-setpx100-c50-P1` | 4 | load=2.42 io_psi10=6.61/2.04 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=4140 settle_wait_s=4 |
| `base/rc-everysec-setpx100-c50-P1` | 5 | load=3.27 io_psi10=6.94/2.94 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=3112 settle_wait_s=0 |
| `h/rc-everysec-setpx100-c50-P1` | 1 | load=2.31 io_psi10=5.15/0.13 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=2904 settle_wait_s=0 |
| `h/rc-everysec-setpx100-c50-P1` | 2 | load=2.35 io_psi10=6.52/0.68 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2640 settle_wait_s=0 |
| `h/rc-everysec-setpx100-c50-P1` | 3 | load=2.46 io_psi10=6.66/0.13 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2808 settle_wait_s=0 |
| `h/rc-everysec-setpx100-c50-P1` | 4 | load=3.01 io_psi10=7.21/0.92 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=5148 settle_wait_s=0 |
| `h/rc-everysec-setpx100-c50-P1` | 5 | load=3.04 io_psi10=6.97/0.70 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=3692 settle_wait_s=0 |
| `base/rc-everysec-setpx1000-c50-P1` | 1 | load=2.04 io_psi10=7.25/0.38 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=2212 settle_wait_s=0 |
| `base/rc-everysec-setpx1000-c50-P1` | 2 | load=2.46 io_psi10=6.73/0.46 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2896 settle_wait_s=0 |
| `base/rc-everysec-setpx1000-c50-P1` | 3 | load=2.26 io_psi10=7.85/2.34 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2784 settle_wait_s=4 |
| `base/rc-everysec-setpx1000-c50-P1` | 4 | load=2.82 io_psi10=7.84/0.24 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2584 settle_wait_s=0 |
| `base/rc-everysec-setpx1000-c50-P1` | 5 | load=2.46 io_psi10=6.66/0.76 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=2996 settle_wait_s=2 |
| `h/rc-everysec-setpx1000-c50-P1` | 1 | load=1.87 io_psi10=6.64/0.19 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=3108 settle_wait_s=0 |
| `h/rc-everysec-setpx1000-c50-P1` | 2 | load=2.68 io_psi10=7.01/0.76 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2496 settle_wait_s=0 |
| `h/rc-everysec-setpx1000-c50-P1` | 3 | load=2.24 io_psi10=7.66/0.49 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2964 settle_wait_s=0 |
| `h/rc-everysec-setpx1000-c50-P1` | 4 | load=3.09 io_psi10=6.04/0.45 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2904 settle_wait_s=0 |
| `h/rc-everysec-setpx1000-c50-P1` | 5 | load=2.22 io_psi10=7.41/0.13 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=2692 settle_wait_s=0 |
| `base/rc-everysec-incrvol-c50-P1` | 1 | load=1.79 io_psi10=7.43/6.34 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=2972 settle_wait_s=17 |
| `base/rc-everysec-incrvol-c50-P1` | 2 | load=2.80 io_psi10=5.01/0.16 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=2704 settle_wait_s=0 |
| `base/rc-everysec-incrvol-c50-P1` | 3 | load=2.39 io_psi10=4.38/0.77 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=3724 settle_wait_s=0 |
| `base/rc-everysec-incrvol-c50-P1` | 4 | load=2.52 io_psi10=4.10/1.05 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2416 settle_wait_s=0 |
| `base/rc-everysec-incrvol-c50-P1` | 5 | load=2.51 io_psi10=7.05/0.37 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=5444 settle_wait_s=0 |
| `h/rc-everysec-incrvol-c50-P1` | 1 | load=2.23 io_psi10=7.15/0.98 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=5192 settle_wait_s=0 |
| `h/rc-everysec-incrvol-c50-P1` | 2 | load=2.40 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=1472 settle_wait_s=0 |
| `h/rc-everysec-incrvol-c50-P1` | 3 | load=2.46 io_psi10=4.13/1.02 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=3132 settle_wait_s=0 |
| `h/rc-everysec-incrvol-c50-P1` | 4 | load=2.50 io_psi10=0.14/0.14 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=1992 settle_wait_s=0 |
| `h/rc-everysec-incrvol-c50-P1` | 5 | load=2.17 io_psi10=5.97/0.39 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=3128 settle_wait_s=0 |
| `base/rc-noaof-setpx100-c50-P1` | 1 | load=2.29 io_psi10=6.75/0.53 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=2748 settle_wait_s=0 |
| `base/rc-noaof-setpx100-c50-P1` | 2 | load=2.23 io_psi10=0.44/0.41 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=1504 settle_wait_s=0 |
| `base/rc-noaof-setpx100-c50-P1` | 3 | load=2.77 io_psi10=4.14/0.65 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=3076 settle_wait_s=0 |
| `base/rc-noaof-setpx100-c50-P1` | 4 | load=2.40 io_psi10=0.89/0.88 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2172 settle_wait_s=0 |
| `base/rc-noaof-setpx100-c50-P1` | 5 | load=2.43 io_psi10=7.69/0.33 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=2840 settle_wait_s=0 |
| `base/rc-noaof-incrvol-c50-P1` | 1 | load=2.27 io_psi10=1.03/0.19 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=696 settle_wait_s=0 |
| `base/rc-noaof-incrvol-c50-P1` | 2 | load=2.32 io_psi10=0.24/0.24 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=2188 settle_wait_s=0 |
| `base/rc-noaof-incrvol-c50-P1` | 3 | load=2.59 io_psi10=1.06/0.54 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=3116 settle_wait_s=0 |
| `base/rc-noaof-incrvol-c50-P1` | 4 | load=2.65 io_psi10=0.19/0.13 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2096 settle_wait_s=0 |
| `base/rc-noaof-incrvol-c50-P1` | 5 | load=2.59 io_psi10=1.98/0.83 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=4368 settle_wait_s=0 |
| `h/rc-noaof-setpx100-c50-P1` | 1 | load=2.41 io_psi10=0.60/0.49 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=1556 settle_wait_s=0 |
| `h/rc-noaof-setpx100-c50-P1` | 2 | load=2.48 io_psi10=0.93/0.93 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=1676 settle_wait_s=0 |
| `h/rc-noaof-setpx100-c50-P1` | 3 | load=2.68 io_psi10=0.26/0.18 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=3324 settle_wait_s=0 |
| `h/rc-noaof-setpx100-c50-P1` | 4 | load=2.52 io_psi10=0.56/0.53 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=2124 settle_wait_s=0 |
| `h/rc-noaof-setpx100-c50-P1` | 5 | load=2.56 io_psi10=0.61/0.46 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=2264 settle_wait_s=0 |
| `h/rc-noaof-incrvol-c50-P1` | 1 | load=2.50 io_psi10=0.09/0.08 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=1328 settle_wait_s=0 |
| `h/rc-noaof-incrvol-c50-P1` | 2 | load=2.48 io_psi10=0.11/0.08 mem_psi10=0.00/0.00 swap_used_mib=8184 dirty_wb_kib=1964 settle_wait_s=0 |
| `h/rc-noaof-incrvol-c50-P1` | 3 | load=2.71 io_psi10=0.74/0.70 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=1476 settle_wait_s=0 |
| `h/rc-noaof-incrvol-c50-P1` | 4 | load=2.70 io_psi10=0.17/0.16 mem_psi10=0.00/0.00 swap_used_mib=8183 dirty_wb_kib=1792 settle_wait_s=0 |
| `h/rc-noaof-incrvol-c50-P1` | 5 | load=2.42 io_psi10=0.56/0.54 mem_psi10=0.00/0.00 swap_used_mib=8182 dirty_wb_kib=1784 settle_wait_s=0 |
