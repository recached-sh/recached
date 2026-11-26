## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P1` | 5 | 186,003 | 3% | 3.51 | 0.78 | 2.73 | — | — | 0.255 | 0.343 | 0.367 | 25.1 | 1.24 | 0 |
| `gcap/rc-noaof-get-c50-P1` | 5 | 183,900 | 2% | 3.58 | 0.81 | 2.77 | — | — | 0.255 | 0.351 | 0.375 | 25.2 | 1.24 | 0 |
| `base/rc-noaof-mget-c50-P1` | 5 | 131,130 | 3% | 5.66 | 2.43 | 3.23 | — | — | 0.359 | 0.487 | 0.511 | 25.1 | 1.00 | 0 |
| `gcap/rc-noaof-mget-c50-P1` | 5 | 131,139 | 0% | 5.71 | 2.46 | 3.23 | — | — | 0.359 | 0.487 | 0.519 | 25.1 | 1.00 | 0 |
| `base/rc-noaof-set-c50-P1` | 5 | 179,767 | 3% | 3.67 | 0.88 | 2.81 | — | — | 0.263 | 0.359 | 0.383 | 26.0 | 1.22 | 0 |
| `gcap/rc-noaof-set-c50-P1` | 5 | 181,802 | 2% | 3.68 | 0.89 | 2.79 | — | — | 0.263 | 0.351 | 0.383 | 26.1 | 1.22 | 0 |
| `base/rc-noaof-incr-c50-P1` | 5 | 181,810 | 2% | 3.63 | 0.86 | 2.75 | — | — | 0.263 | 0.351 | 0.383 | 27.7 | 1.22 | 0 |
| `gcap/rc-noaof-incr-c50-P1` | 5 | 181,802 | 2% | 3.66 | 0.89 | 2.77 | — | — | 0.263 | 0.351 | 0.383 | 27.5 | 1.23 | 0 |
| `base/rc-noaof-get-c50-P16` | 5 | 1,333,244 | 2% | 0.60 | 0.35 | 0.25 | — | — | 0.607 | 0.647 | 0.679 | 25.4 | 0.86 | 0 |
| `base/rc-noaof-mget-c50-P16` | 5 | 350,766 | 1% | 2.66 | 2.26 | 0.40 | — | — | 2.335 | 2.447 | 2.663 | 25.7 | 0.52 | 0 |
| `base/rc-everysec-incr-c50-P1` | 5 | 131,139 | 2% | 5.44 | 1.70 | 3.73 | — | — | 0.359 | 0.487 | 0.519 | 27.5 | 0.95 | 0 |
| `base/rc-everysec-set-c50-P1` | 5 | 131,130 | 3% | 5.45 | 1.69 | 3.76 | — | — | 0.359 | 0.487 | 0.519 | 25.9 | 0.94 | 0 |
| `gcap/rc-noaof-get-c50-P16` | 5 | 1,355,840 | 2% | 0.60 | 0.35 | 0.25 | — | — | 0.607 | 0.647 | 0.679 | 25.2 | 0.87 | 0 |
| `gcap/rc-noaof-mget-c50-P16` | 5 | 347,741 | 0% | 2.66 | 2.25 | 0.41 | — | — | 2.335 | 2.447 | 2.647 | 26.0 | 0.53 | 0 |
| `gcap/rc-everysec-incr-c50-P1` | 5 | 130,073 | 3% | 5.54 | 1.77 | 3.76 | — | — | 0.359 | 0.487 | 0.519 | 27.4 | 0.93 | 0 |
| `gcap/rc-everysec-set-c50-P1` | 5 | 130,073 | 3% | 5.55 | 1.79 | 3.77 | — | — | 0.359 | 0.487 | 0.527 | 25.8 | 0.93 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-incr-c50-P1` | 0.0999 | 10.0 | 5.7 | 0.0005 |
| `base/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 6.7 | 0.0006 |
| `gcap/rc-everysec-incr-c50-P1` | 0.1000 | 10.0 | 5.9 | 0.0005 |
| `gcap/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 6.3 | 0.0005 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P1` | 1 | load=1.65 io_psi10=1.05/1.04 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2056 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 2 | load=2.67 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2304 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 3 | load=2.71 io_psi10=0.34/0.34 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3312 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 4 | load=2.30 io_psi10=1.23/1.23 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2296 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 5 | load=2.34 io_psi10=0.33/0.33 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2956 settle_wait_s=0 |
| `gcap/rc-noaof-get-c50-P1` | 1 | load=1.97 io_psi10=0.38/0.31 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1976 settle_wait_s=0 |
| `gcap/rc-noaof-get-c50-P1` | 2 | load=2.49 io_psi10=0.19/0.19 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1472 settle_wait_s=0 |
| `gcap/rc-noaof-get-c50-P1` | 3 | load=2.79 io_psi10=0.09/0.09 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1548 settle_wait_s=0 |
| `gcap/rc-noaof-get-c50-P1` | 4 | load=2.22 io_psi10=0.93/0.92 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2028 settle_wait_s=0 |
| `gcap/rc-noaof-get-c50-P1` | 5 | load=2.46 io_psi10=1.39/1.21 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2444 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 1 | load=2.32 io_psi10=0.21/0.21 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2796 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 2 | load=2.54 io_psi10=0.30/0.30 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3124 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 3 | load=2.68 io_psi10=0.30/0.27 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1872 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 4 | load=2.18 io_psi10=1.73/1.67 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=5884 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P1` | 5 | load=2.47 io_psi10=0.49/0.47 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1472 settle_wait_s=0 |
| `gcap/rc-noaof-mget-c50-P1` | 1 | load=2.40 io_psi10=0.23/0.23 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1516 settle_wait_s=0 |
| `gcap/rc-noaof-mget-c50-P1` | 2 | load=2.61 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2100 settle_wait_s=0 |
| `gcap/rc-noaof-mget-c50-P1` | 3 | load=2.68 io_psi10=0.14/0.14 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3200 settle_wait_s=0 |
| `gcap/rc-noaof-mget-c50-P1` | 4 | load=2.13 io_psi10=1.84/1.84 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2152 settle_wait_s=0 |
| `gcap/rc-noaof-mget-c50-P1` | 5 | load=2.20 io_psi10=0.78/0.73 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=128 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 1 | load=2.17 io_psi10=0.13/0.13 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1972 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=2.51 io_psi10=0.28/0.28 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1564 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=2.61 io_psi10=0.38/0.38 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2144 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 4 | load=2.33 io_psi10=0.32/0.32 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2328 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 5 | load=2.38 io_psi10=2.93/2.88 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3800 settle_wait_s=0 |
| `gcap/rc-noaof-set-c50-P1` | 1 | load=2.10 io_psi10=0.44/0.44 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2796 settle_wait_s=0 |
| `gcap/rc-noaof-set-c50-P1` | 2 | load=2.32 io_psi10=0.02/0.01 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3500 settle_wait_s=0 |
| `gcap/rc-noaof-set-c50-P1` | 3 | load=2.44 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=188 settle_wait_s=0 |
| `gcap/rc-noaof-set-c50-P1` | 4 | load=2.28 io_psi10=2.09/2.09 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3452 settle_wait_s=0 |
| `gcap/rc-noaof-set-c50-P1` | 5 | load=2.44 io_psi10=0.41/0.40 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2044 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 1 | load=2.41 io_psi10=0.17/0.15 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1532 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 2 | load=2.19 io_psi10=0.21/0.18 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1996 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 3 | load=2.48 io_psi10=0.12/0.12 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3068 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 4 | load=2.09 io_psi10=0.17/0.16 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2212 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 5 | load=2.50 io_psi10=0.57/0.53 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=4336 settle_wait_s=0 |
| `gcap/rc-noaof-incr-c50-P1` | 1 | load=2.58 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2872 settle_wait_s=0 |
| `gcap/rc-noaof-incr-c50-P1` | 2 | load=1.97 io_psi10=0.15/0.10 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1988 settle_wait_s=0 |
| `gcap/rc-noaof-incr-c50-P1` | 3 | load=2.50 io_psi10=0.27/0.25 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1920 settle_wait_s=0 |
| `gcap/rc-noaof-incr-c50-P1` | 4 | load=1.95 io_psi10=0.58/0.49 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2336 settle_wait_s=0 |
| `gcap/rc-noaof-incr-c50-P1` | 5 | load=2.57 io_psi10=0.20/0.19 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2840 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 1 | load=2.61 io_psi10=0.25/0.25 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2652 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 2 | load=1.96 io_psi10=0.56/0.30 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3112 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 3 | load=2.65 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1688 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 4 | load=1.93 io_psi10=0.46/0.24 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2548 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 5 | load=2.52 io_psi10=0.50/0.50 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3880 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 1 | load=2.47 io_psi10=0.22/0.22 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1580 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 2 | load=2.27 io_psi10=6.09/0.40 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1052 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 3 | load=2.50 io_psi10=0.49/0.49 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2124 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 4 | load=2.08 io_psi10=7.21/2.77 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1828 settle_wait_s=0 |
| `base/rc-noaof-mget-c50-P16` | 5 | load=2.48 io_psi10=3.38/3.38 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2600 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 1 | load=2.12 io_psi10=0.15/0.14 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3388 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 2 | load=2.24 io_psi10=5.75/0.62 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3364 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 3 | load=2.02 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1992 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 4 | load=2.15 io_psi10=3.80/0.18 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1436 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 5 | load=2.24 io_psi10=0.34/0.34 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2660 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 1 | load=2.35 io_psi10=5.79/0.11 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=4624 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 2 | load=2.06 io_psi10=0.62/0.51 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1368 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 3 | load=2.09 io_psi10=5.20/0.13 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3428 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 4 | load=2.23 io_psi10=2.11/2.07 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2248 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 5 | load=2.43 io_psi10=5.74/0.46 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=4844 settle_wait_s=0 |
| `gcap/rc-noaof-get-c50-P16` | 1 | load=2.61 io_psi10=6.30/0.30 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3116 settle_wait_s=0 |
| `gcap/rc-noaof-get-c50-P16` | 2 | load=2.08 io_psi10=0.41/0.10 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1584 settle_wait_s=0 |
| `gcap/rc-noaof-get-c50-P16` | 3 | load=2.30 io_psi10=6.75/1.86 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2320 settle_wait_s=0 |
| `gcap/rc-noaof-get-c50-P16` | 4 | load=2.13 io_psi10=0.47/0.25 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2816 settle_wait_s=0 |
| `gcap/rc-noaof-get-c50-P16` | 5 | load=2.40 io_psi10=7.67/3.47 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=4804 settle_wait_s=4 |
| `gcap/rc-noaof-mget-c50-P16` | 1 | load=2.63 io_psi10=1.63/0.32 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1624 settle_wait_s=0 |
| `gcap/rc-noaof-mget-c50-P16` | 2 | load=2.25 io_psi10=6.88/0.78 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3336 settle_wait_s=0 |
| `gcap/rc-noaof-mget-c50-P16` | 3 | load=2.13 io_psi10=1.78/0.80 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3456 settle_wait_s=0 |
| `gcap/rc-noaof-mget-c50-P16` | 4 | load=2.39 io_psi10=5.64/1.05 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2504 settle_wait_s=0 |
| `gcap/rc-noaof-mget-c50-P16` | 5 | load=2.16 io_psi10=1.63/0.79 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1680 settle_wait_s=0 |
| `gcap/rc-everysec-incr-c50-P1` | 1 | load=2.25 io_psi10=0.12/0.04 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1968 settle_wait_s=0 |
| `gcap/rc-everysec-incr-c50-P1` | 2 | load=2.53 io_psi10=7.79/2.58 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1364 settle_wait_s=2 |
| `gcap/rc-everysec-incr-c50-P1` | 3 | load=1.97 io_psi10=0.83/0.78 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=1908 settle_wait_s=0 |
| `gcap/rc-everysec-incr-c50-P1` | 4 | load=2.24 io_psi10=4.97/0.69 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3288 settle_wait_s=0 |
| `gcap/rc-everysec-incr-c50-P1` | 5 | load=1.91 io_psi10=0.22/0.18 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2364 settle_wait_s=0 |
| `gcap/rc-everysec-set-c50-P1` | 1 | load=2.27 io_psi10=5.71/0.47 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3164 settle_wait_s=0 |
| `gcap/rc-everysec-set-c50-P1` | 2 | load=2.12 io_psi10=5.68/0.17 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=3628 settle_wait_s=0 |
| `gcap/rc-everysec-set-c50-P1` | 3 | load=2.11 io_psi10=3.92/0.22 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=4076 settle_wait_s=0 |
| `gcap/rc-everysec-set-c50-P1` | 4 | load=2.33 io_psi10=7.56/2.06 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=2300 settle_wait_s=0 |
| `gcap/rc-everysec-set-c50-P1` | 5 | load=2.27 io_psi10=4.67/0.12 mem_psi10=0.00/0.00 swap_used_mib=5190 dirty_wb_kib=7956 settle_wait_s=0 |
