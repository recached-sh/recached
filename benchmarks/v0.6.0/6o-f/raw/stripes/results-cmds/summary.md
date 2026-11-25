## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P1` | 5 | 188,953 | 3% | 3.49 | 0.77 | 2.72 | 4,822 | 4,411 | 0.255 | 0.343 | 0.367 | 25.1 | 1.25 | 0 |
| `x256/rc-noaof-get-c50-P1` | 5 | 184,610 | 2% | 3.56 | 0.79 | 2.77 | 4,820 | 4,499 | 0.255 | 0.351 | 0.375 | 25.4 | 1.25 | 0 |
| `x1024/rc-noaof-get-c50-P1` | 5 | 186,041 | 2% | 3.52 | 0.79 | 2.73 | 4,815 | 4,497 | 0.255 | 0.343 | 0.375 | 25.3 | 1.26 | 0 |
| `x1024s/rc-noaof-get-c50-P1` | 5 | 188,964 | 1% | 3.49 | 0.78 | 2.71 | 4,820 | 4,423 | 0.255 | 0.343 | 0.367 | 25.4 | 1.26 | 0 |
| `base/rc-noaof-set-c50-P1` | 5 | 183,189 | 5% | 3.63 | 0.86 | 2.76 | 5,460 | 4,907 | 0.263 | 0.351 | 0.375 | 26.1 | 1.24 | 0 |
| `x256/rc-noaof-set-c50-P1` | 5 | 181,802 | 2% | 3.67 | 0.87 | 2.80 | 5,458 | 4,928 | 0.263 | 0.359 | 0.383 | 25.9 | 1.23 | 0 |
| `x1024/rc-noaof-set-c50-P1` | 5 | 181,813 | 2% | 3.66 | 0.87 | 2.79 | 5,461 | 4,941 | 0.263 | 0.351 | 0.383 | 26.1 | 1.22 | 0 |
| `x1024s/rc-noaof-set-c50-P1` | 5 | 183,195 | 2% | 3.63 | 0.85 | 2.79 | 5,462 | 4,871 | 0.263 | 0.351 | 0.383 | 26.2 | 1.23 | 0 |
| `base/rc-noaof-lpush-c50-P1` | 5 | 187,488 | 3% | 3.52 | 0.76 | 2.75 | 5,504 | 4,357 | 0.255 | 0.343 | 0.367 | 389.6 | 1.24 | 0 |
| `x256/rc-noaof-lpush-c50-P1` | 5 | 184,576 | 2% | 3.56 | 0.77 | 2.79 | 5,508 | 4,369 | 0.255 | 0.351 | 0.375 | 389.7 | 1.22 | 0 |
| `x1024/rc-noaof-lpush-c50-P1` | 5 | 186,023 | 2% | 3.56 | 0.78 | 2.78 | 5,507 | 4,422 | 0.255 | 0.343 | 0.375 | 389.8 | 1.23 | 0 |
| `x1024s/rc-noaof-lpush-c50-P1` | 5 | 186,029 | 1% | 3.54 | 0.76 | 2.77 | 5,509 | 4,354 | 0.255 | 0.343 | 0.367 | 389.8 | 1.23 | 0 |
| `base/rc-noaof-mset-c50-P1` | 5 | 100,823 | 2% | 7.96 | 4.46 | 3.51 | 29,986 | 21,615 | 0.471 | 0.623 | 0.655 | 26.2 | 0.74 | 0 |
| `x256/rc-noaof-mset-c50-P1` | 5 | 100,827 | 0% | 7.98 | 4.46 | 3.51 | 29,988 | 21,665 | 0.479 | 0.631 | 0.663 | 26.0 | 0.75 | 0 |
| `x1024/rc-noaof-mset-c50-P1` | 5 | 100,823 | 2% | 7.99 | 4.49 | 3.47 | 29,989 | 21,703 | 0.479 | 0.623 | 0.663 | 26.2 | 0.75 | 0 |
| `x1024s/rc-noaof-mset-c50-P1` | 5 | 100,827 | 1% | 7.95 | 4.45 | 3.50 | 29,997 | 21,673 | 0.471 | 0.623 | 0.663 | 26.1 | 0.75 | 0 |
| `base/rc-noaof-get-c50-P16` | 5 | 1,333,262 | 4% | 0.60 | 0.35 | 0.25 | 2,317 | 1,688 | 0.607 | 0.647 | 0.679 | 25.4 | 0.87 | 0 |
| `base/rc-everysec-set-c50-P1` | 5 | 133,324 | 8% | 5.41 | 1.69 | 3.73 | 9,697 | 8,701 | 0.359 | 0.479 | 0.519 | 25.7 | 0.95 | 0 |
| `x256/rc-noaof-get-c50-P16` | 5 | 1,351,278 | 1% | 0.59 | 0.34 | 0.25 | 2,317 | 1,660 | 0.599 | 0.639 | 0.671 | 25.4 | 0.86 | 0 |
| `x256/rc-everysec-set-c50-P1` | 5 | 131,859 | 2% | 5.40 | 1.66 | 3.76 | 9,563 | 8,631 | 0.359 | 0.487 | 0.519 | 25.6 | 0.95 | 0 |
| `x1024/rc-noaof-get-c50-P16` | 5 | 1,351,278 | 1% | 0.60 | 0.35 | 0.25 | 2,317 | 1,671 | 0.607 | 0.647 | 0.679 | 25.3 | 0.86 | 0 |
| `x1024/rc-everysec-set-c50-P1` | 5 | 133,321 | 2% | 5.36 | 1.65 | 3.73 | 9,567 | 8,584 | 0.351 | 0.479 | 0.511 | 26.0 | 0.96 | 0 |
| `x1024s/rc-noaof-get-c50-P16` | 5 | 1,351,278 | 1% | 0.60 | 0.35 | 0.25 | 2,317 | 1,673 | 0.607 | 0.647 | 0.679 | 25.4 | 0.86 | 0 |
| `x1024s/rc-everysec-set-c50-P1` | 5 | 131,862 | 1% | 5.43 | 1.69 | 3.76 | 9,565 | 8,878 | 0.359 | 0.487 | 0.519 | 25.9 | 0.95 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-set-c50-P1` | 0.0999 | 10.0 | 6.4 | 0.0004 |
| `x256/rc-everysec-set-c50-P1` | 0.0999 | 10.0 | 6.1 | 0.0004 |
| `x1024/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 6.0 | 0.0004 |
| `x1024s/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 6.1 | 0.0004 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P1` | 1 | load=0.98 io_psi10=1.89/1.71 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=1204 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 2 | load=2.66 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2020 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 3 | load=2.58 io_psi10=0.12/0.12 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2680 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 4 | load=2.70 io_psi10=0.44/0.44 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2624 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 5 | load=2.76 io_psi10=0.20/0.19 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2204 settle_wait_s=0 |
| `x256/rc-noaof-get-c50-P1` | 1 | load=1.66 io_psi10=0.19/0.18 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=1224 settle_wait_s=0 |
| `x256/rc-noaof-get-c50-P1` | 2 | load=2.89 io_psi10=0.26/0.26 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1988 settle_wait_s=0 |
| `x256/rc-noaof-get-c50-P1` | 3 | load=2.47 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2140 settle_wait_s=0 |
| `x256/rc-noaof-get-c50-P1` | 4 | load=2.46 io_psi10=1.75/1.56 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2520 settle_wait_s=0 |
| `x256/rc-noaof-get-c50-P1` | 5 | load=2.78 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1008 settle_wait_s=0 |
| `x1024/rc-noaof-get-c50-P1` | 1 | load=2.04 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=2332 settle_wait_s=0 |
| `x1024/rc-noaof-get-c50-P1` | 2 | load=2.82 io_psi10=0.20/0.20 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2020 settle_wait_s=0 |
| `x1024/rc-noaof-get-c50-P1` | 3 | load=2.23 io_psi10=0.10/0.10 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1744 settle_wait_s=0 |
| `x1024/rc-noaof-get-c50-P1` | 4 | load=2.65 io_psi10=0.22/0.19 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2416 settle_wait_s=0 |
| `x1024/rc-noaof-get-c50-P1` | 5 | load=2.88 io_psi10=0.13/0.13 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2224 settle_wait_s=0 |
| `x1024s/rc-noaof-get-c50-P1` | 1 | load=2.25 io_psi10=0.16/0.16 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=1280 settle_wait_s=0 |
| `x1024s/rc-noaof-get-c50-P1` | 2 | load=2.68 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2080 settle_wait_s=0 |
| `x1024s/rc-noaof-get-c50-P1` | 3 | load=2.29 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2016 settle_wait_s=0 |
| `x1024s/rc-noaof-get-c50-P1` | 4 | load=2.75 io_psi10=0.20/0.18 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2396 settle_wait_s=0 |
| `x1024s/rc-noaof-get-c50-P1` | 5 | load=2.85 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=964 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 1 | load=2.43 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=1468 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=2.63 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2176 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=2.40 io_psi10=0.10/0.10 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2592 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 4 | load=2.67 io_psi10=0.46/0.46 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2992 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 5 | load=2.92 io_psi10=0.51/0.51 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2484 settle_wait_s=0 |
| `x256/rc-noaof-set-c50-P1` | 1 | load=2.59 io_psi10=2.35/2.35 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=176 settle_wait_s=0 |
| `x256/rc-noaof-set-c50-P1` | 2 | load=2.74 io_psi10=0.12/0.12 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1940 settle_wait_s=0 |
| `x256/rc-noaof-set-c50-P1` | 3 | load=2.40 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2444 settle_wait_s=0 |
| `x256/rc-noaof-set-c50-P1` | 4 | load=2.66 io_psi10=0.12/0.12 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2444 settle_wait_s=0 |
| `x256/rc-noaof-set-c50-P1` | 5 | load=2.91 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2028 settle_wait_s=0 |
| `x1024/rc-noaof-set-c50-P1` | 1 | load=2.59 io_psi10=0.17/0.16 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=2272 settle_wait_s=0 |
| `x1024/rc-noaof-set-c50-P1` | 2 | load=2.64 io_psi10=0.36/0.36 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2012 settle_wait_s=0 |
| `x1024/rc-noaof-set-c50-P1` | 3 | load=2.67 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2464 settle_wait_s=0 |
| `x1024/rc-noaof-set-c50-P1` | 4 | load=2.46 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1348 settle_wait_s=0 |
| `x1024/rc-noaof-set-c50-P1` | 5 | load=2.95 io_psi10=0.18/0.17 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2316 settle_wait_s=0 |
| `x1024s/rc-noaof-set-c50-P1` | 1 | load=2.64 io_psi10=0.26/0.26 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=1520 settle_wait_s=0 |
| `x1024s/rc-noaof-set-c50-P1` | 2 | load=2.49 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2080 settle_wait_s=0 |
| `x1024s/rc-noaof-set-c50-P1` | 3 | load=2.82 io_psi10=0.20/0.20 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2124 settle_wait_s=0 |
| `x1024s/rc-noaof-set-c50-P1` | 4 | load=2.51 io_psi10=0.13/0.13 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2544 settle_wait_s=0 |
| `x1024s/rc-noaof-set-c50-P1` | 5 | load=2.85 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1104 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 1 | load=2.63 io_psi10=0.00/0.00 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=1328 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | load=2.38 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1900 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | load=2.76 io_psi10=0.08/0.08 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2532 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 4 | load=2.49 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2060 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 5 | load=2.92 io_psi10=0.19/0.18 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2480 settle_wait_s=0 |
| `x256/rc-noaof-lpush-c50-P1` | 1 | load=2.39 io_psi10=0.10/0.10 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=1576 settle_wait_s=0 |
| `x256/rc-noaof-lpush-c50-P1` | 2 | load=2.47 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1852 settle_wait_s=0 |
| `x256/rc-noaof-lpush-c50-P1` | 3 | load=2.86 io_psi10=0.38/0.38 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2028 settle_wait_s=0 |
| `x256/rc-noaof-lpush-c50-P1` | 4 | load=2.50 io_psi10=0.15/0.13 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2236 settle_wait_s=0 |
| `x256/rc-noaof-lpush-c50-P1` | 5 | load=2.83 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2032 settle_wait_s=0 |
| `x1024/rc-noaof-lpush-c50-P1` | 1 | load=2.55 io_psi10=0.20/0.20 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=1396 settle_wait_s=0 |
| `x1024/rc-noaof-lpush-c50-P1` | 2 | load=2.36 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2124 settle_wait_s=0 |
| `x1024/rc-noaof-lpush-c50-P1` | 3 | load=2.82 io_psi10=0.14/0.14 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2512 settle_wait_s=0 |
| `x1024/rc-noaof-lpush-c50-P1` | 4 | load=2.19 io_psi10=0.28/0.09 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2100 settle_wait_s=0 |
| `x1024/rc-noaof-lpush-c50-P1` | 5 | load=2.84 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2432 settle_wait_s=0 |
| `x1024s/rc-noaof-lpush-c50-P1` | 1 | load=2.61 io_psi10=0.19/0.19 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=2444 settle_wait_s=0 |
| `x1024s/rc-noaof-lpush-c50-P1` | 2 | load=2.06 io_psi10=0.16/0.15 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1632 settle_wait_s=0 |
| `x1024s/rc-noaof-lpush-c50-P1` | 3 | load=2.87 io_psi10=0.13/0.07 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2696 settle_wait_s=0 |
| `x1024s/rc-noaof-lpush-c50-P1` | 4 | load=1.75 io_psi10=0.09/0.08 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2188 settle_wait_s=0 |
| `x1024s/rc-noaof-lpush-c50-P1` | 5 | load=2.80 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2616 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 1 | load=2.67 io_psi10=0.13/0.12 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=1636 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 2 | load=2.08 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2324 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 3 | load=2.64 io_psi10=0.13/0.13 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2104 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 4 | load=1.89 io_psi10=0.21/0.20 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2048 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 5 | load=2.85 io_psi10=0.17/0.17 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1120 settle_wait_s=0 |
| `x256/rc-noaof-mset-c50-P1` | 1 | load=2.64 io_psi10=0.01/0.01 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=1560 settle_wait_s=0 |
| `x256/rc-noaof-mset-c50-P1` | 2 | load=2.05 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1924 settle_wait_s=0 |
| `x256/rc-noaof-mset-c50-P1` | 3 | load=2.43 io_psi10=2.19/2.19 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2676 settle_wait_s=0 |
| `x256/rc-noaof-mset-c50-P1` | 4 | load=1.99 io_psi10=0.03/0.03 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3044 settle_wait_s=0 |
| `x256/rc-noaof-mset-c50-P1` | 5 | load=2.63 io_psi10=0.16/0.16 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2316 settle_wait_s=0 |
| `x1024/rc-noaof-mset-c50-P1` | 1 | load=2.21 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=2804 settle_wait_s=0 |
| `x1024/rc-noaof-mset-c50-P1` | 2 | load=2.30 io_psi10=0.11/0.08 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1688 settle_wait_s=0 |
| `x1024/rc-noaof-mset-c50-P1` | 3 | load=1.90 io_psi10=0.96/0.96 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2676 settle_wait_s=0 |
| `x1024/rc-noaof-mset-c50-P1` | 4 | load=2.09 io_psi10=0.14/0.12 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2036 settle_wait_s=0 |
| `x1024/rc-noaof-mset-c50-P1` | 5 | load=2.36 io_psi10=0.04/0.04 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2192 settle_wait_s=0 |
| `x1024s/rc-noaof-mset-c50-P1` | 1 | load=1.99 io_psi10=0.15/0.14 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=1492 settle_wait_s=0 |
| `x1024s/rc-noaof-mset-c50-P1` | 2 | load=2.07 io_psi10=1.15/0.42 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2348 settle_wait_s=0 |
| `x1024s/rc-noaof-mset-c50-P1` | 3 | load=1.94 io_psi10=0.12/0.12 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=776 settle_wait_s=0 |
| `x1024s/rc-noaof-mset-c50-P1` | 4 | load=2.18 io_psi10=0.98/0.51 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1936 settle_wait_s=0 |
| `x1024s/rc-noaof-mset-c50-P1` | 5 | load=2.35 io_psi10=0.29/0.28 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3652 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 1 | load=1.95 io_psi10=0.02/0.02 mem_psi10=0.00/0.00 swap_used_mib=3580 dirty_wb_kib=2204 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 2 | load=2.10 io_psi10=6.55/1.14 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3312 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 3 | load=1.89 io_psi10=1.40/1.40 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=860 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 4 | load=2.33 io_psi10=4.62/1.17 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1720 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 5 | load=2.17 io_psi10=0.44/0.34 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2668 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 1 | load=2.03 io_psi10=1.72/1.36 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=103280 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 2 | load=2.24 io_psi10=1.09/0.47 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1560 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 3 | load=1.93 io_psi10=0.69/0.63 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2976 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 4 | load=2.35 io_psi10=1.17/0.49 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1816 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 5 | load=2.15 io_psi10=0.95/0.91 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2520 settle_wait_s=0 |
| `x256/rc-noaof-get-c50-P16` | 1 | load=2.76 io_psi10=5.07/1.38 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=18580 settle_wait_s=0 |
| `x256/rc-noaof-get-c50-P16` | 2 | load=2.39 io_psi10=4.72/0.12 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3296 settle_wait_s=0 |
| `x256/rc-noaof-get-c50-P16` | 3 | load=2.15 io_psi10=3.95/0.53 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2732 settle_wait_s=0 |
| `x256/rc-noaof-get-c50-P16` | 4 | load=2.54 io_psi10=5.91/0.26 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3468 settle_wait_s=0 |
| `x256/rc-noaof-get-c50-P16` | 5 | load=2.34 io_psi10=5.52/0.31 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3884 settle_wait_s=0 |
| `x256/rc-everysec-set-c50-P1` | 1 | load=2.55 io_psi10=1.20/0.70 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1588 settle_wait_s=0 |
| `x256/rc-everysec-set-c50-P1` | 2 | load=2.39 io_psi10=1.04/0.41 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1348 settle_wait_s=0 |
| `x256/rc-everysec-set-c50-P1` | 3 | load=2.10 io_psi10=1.08/0.59 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2192 settle_wait_s=0 |
| `x256/rc-everysec-set-c50-P1` | 4 | load=2.33 io_psi10=1.86/1.30 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1904 settle_wait_s=0 |
| `x256/rc-everysec-set-c50-P1` | 5 | load=2.18 io_psi10=1.57/0.89 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2064 settle_wait_s=0 |
| `x1024/rc-noaof-get-c50-P16` | 1 | load=2.53 io_psi10=3.83/0.14 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2452 settle_wait_s=0 |
| `x1024/rc-noaof-get-c50-P16` | 2 | load=2.44 io_psi10=4.58/0.15 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3312 settle_wait_s=0 |
| `x1024/rc-noaof-get-c50-P16` | 3 | load=2.32 io_psi10=4.80/0.32 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2840 settle_wait_s=0 |
| `x1024/rc-noaof-get-c50-P16` | 4 | load=2.35 io_psi10=5.78/1.40 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3396 settle_wait_s=0 |
| `x1024/rc-noaof-get-c50-P16` | 5 | load=2.45 io_psi10=5.88/0.22 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=820 settle_wait_s=0 |
| `x1024/rc-everysec-set-c50-P1` | 1 | load=2.38 io_psi10=0.68/0.22 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1476 settle_wait_s=0 |
| `x1024/rc-everysec-set-c50-P1` | 2 | load=2.16 io_psi10=1.24/0.54 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1688 settle_wait_s=0 |
| `x1024/rc-everysec-set-c50-P1` | 3 | load=2.07 io_psi10=1.21/0.66 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2060 settle_wait_s=0 |
| `x1024/rc-everysec-set-c50-P1` | 4 | load=2.62 io_psi10=2.66/2.14 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2748 settle_wait_s=0 |
| `x1024/rc-everysec-set-c50-P1` | 5 | load=2.39 io_psi10=1.85/1.13 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2804 settle_wait_s=0 |
| `x1024s/rc-noaof-get-c50-P16` | 1 | load=2.47 io_psi10=3.62/0.00 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2728 settle_wait_s=0 |
| `x1024s/rc-noaof-get-c50-P16` | 2 | load=2.36 io_psi10=6.74/1.56 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3408 settle_wait_s=0 |
| `x1024s/rc-noaof-get-c50-P16` | 3 | load=1.99 io_psi10=5.35/0.37 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3640 settle_wait_s=0 |
| `x1024s/rc-noaof-get-c50-P16` | 4 | load=2.94 io_psi10=4.63/0.16 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3748 settle_wait_s=0 |
| `x1024s/rc-noaof-get-c50-P16` | 5 | load=2.13 io_psi10=5.18/0.23 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2444 settle_wait_s=0 |
| `x1024s/rc-everysec-set-c50-P1` | 1 | load=2.29 io_psi10=0.81/0.29 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2116 settle_wait_s=0 |
| `x1024s/rc-everysec-set-c50-P1` | 2 | load=2.52 io_psi10=4.13/0.18 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3196 settle_wait_s=0 |
| `x1024s/rc-everysec-set-c50-P1` | 3 | load=2.13 io_psi10=1.05/0.45 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2752 settle_wait_s=0 |
| `x1024s/rc-everysec-set-c50-P1` | 4 | load=2.04 io_psi10=4.33/0.44 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3544 settle_wait_s=0 |
| `x1024s/rc-everysec-set-c50-P1` | 5 | load=2.11 io_psi10=2.43/1.83 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2092 settle_wait_s=0 |
