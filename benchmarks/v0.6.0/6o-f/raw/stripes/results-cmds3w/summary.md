## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-get-c50-P16` | 3 | 3,692,080 | 2% | 0.67 | 0.41 | 0.27 | 2,326 | 1,865 | 0.199 | 0.311 | 0.351 | 26.5 | 2.23 | 0 |
| `base/rc-everysec-set-c50-P1` | 3 | 290,824 | 2% | 7.49 | 2.50 | 5.00 | 10,584 | 12,192 | 0.143 | 0.247 | 0.311 | 27.3 | 1.92 | 0 |
| `x256/rc-noaof-get-c50-P16` | 3 | 3,691,853 | 2% | 0.67 | 0.40 | 0.26 | 2,326 | 1,873 | 0.199 | 0.303 | 0.351 | 26.0 | 2.26 | 0 |
| `x256/rc-everysec-set-c50-P1` | 3 | 290,888 | 0% | 7.39 | 2.46 | 4.92 | 10,453 | 11,876 | 0.143 | 0.239 | 0.303 | 27.3 | 1.93 | 0 |
| `x1024/rc-noaof-get-c50-P16` | 3 | 3,636,364 | 2% | 0.67 | 0.41 | 0.26 | 2,326 | 1,885 | 0.207 | 0.311 | 0.351 | 26.3 | 2.24 | 0 |
| `x1024/rc-everysec-set-c50-P1` | 3 | 293,513 | 1% | 7.40 | 2.44 | 4.96 | 10,450 | 11,934 | 0.143 | 0.239 | 0.303 | 27.2 | 1.93 | 0 |
| `x1024s/rc-noaof-get-c50-P16` | 3 | 3,636,144 | 2% | 0.67 | 0.41 | 0.26 | 2,326 | 1,876 | 0.207 | 0.311 | 0.351 | 26.6 | 2.24 | 0 |
| `x1024s/rc-everysec-set-c50-P1` | 3 | 290,867 | 3% | 7.38 | 2.47 | 4.91 | 10,453 | 12,036 | 0.143 | 0.239 | 0.303 | 27.3 | 1.93 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-set-c50-P1` | 0.5597 | 1.8 | 2.8 | 0.0134 |
| `x256/rc-everysec-set-c50-P1` | 0.5640 | 1.8 | 2.8 | 0.0134 |
| `x1024/rc-everysec-set-c50-P1` | 0.5627 | 1.8 | 2.8 | 0.0131 |
| `x1024s/rc-everysec-set-c50-P1` | 0.5631 | 1.8 | 2.8 | 0.0134 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-get-c50-P16` | 1 | load=0.90 io_psi10=1.18/1.17 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1368 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 2 | load=5.89 io_psi10=2.81/1.33 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=4424 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P16` | 3 | load=5.61 io_psi10=0.97/0.73 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2004 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 1 | load=2.04 io_psi10=0.23/0.22 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1260 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 2 | load=5.93 io_psi10=0.96/0.70 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1488 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 3 | load=5.32 io_psi10=0.70/0.66 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1868 settle_wait_s=0 |
| `x256/rc-noaof-get-c50-P16` | 1 | load=3.94 io_psi10=2.73/0.84 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=5232 settle_wait_s=0 |
| `x256/rc-noaof-get-c50-P16` | 2 | load=5.91 io_psi10=3.08/1.41 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=4360 settle_wait_s=0 |
| `x256/rc-noaof-get-c50-P16` | 3 | load=5.82 io_psi10=2.05/0.66 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=4224 settle_wait_s=0 |
| `x256/rc-everysec-set-c50-P1` | 1 | load=4.20 io_psi10=1.49/1.14 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1456 settle_wait_s=0 |
| `x256/rc-everysec-set-c50-P1` | 2 | load=6.18 io_psi10=1.45/1.15 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1504 settle_wait_s=0 |
| `x256/rc-everysec-set-c50-P1` | 3 | load=5.56 io_psi10=1.22/0.95 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1712 settle_wait_s=0 |
| `x1024/rc-noaof-get-c50-P16` | 1 | load=5.04 io_psi10=3.16/1.06 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3868 settle_wait_s=0 |
| `x1024/rc-noaof-get-c50-P16` | 2 | load=6.26 io_psi10=2.95/0.92 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=3924 settle_wait_s=0 |
| `x1024/rc-noaof-get-c50-P16` | 3 | load=5.99 io_psi10=3.05/1.21 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=4204 settle_wait_s=0 |
| `x1024/rc-everysec-set-c50-P1` | 1 | load=5.25 io_psi10=1.22/0.87 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1520 settle_wait_s=0 |
| `x1024/rc-everysec-set-c50-P1` | 2 | load=6.03 io_psi10=1.22/0.86 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1696 settle_wait_s=0 |
| `x1024/rc-everysec-set-c50-P1` | 3 | load=5.59 io_psi10=1.23/0.93 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1936 settle_wait_s=0 |
| `x1024s/rc-noaof-get-c50-P16` | 1 | load=5.86 io_psi10=2.87/1.01 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=4688 settle_wait_s=0 |
| `x1024s/rc-noaof-get-c50-P16` | 2 | load=6.39 io_psi10=3.42/1.22 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=5048 settle_wait_s=0 |
| `x1024s/rc-noaof-get-c50-P16` | 3 | load=6.07 io_psi10=2.79/0.90 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=4384 settle_wait_s=0 |
| `x1024s/rc-everysec-set-c50-P1` | 1 | load=5.90 io_psi10=1.27/0.97 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=1796 settle_wait_s=0 |
| `x1024s/rc-everysec-set-c50-P1` | 2 | load=6.18 io_psi10=2.49/0.77 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=4684 settle_wait_s=0 |
| `x1024s/rc-everysec-set-c50-P1` | 3 | load=5.80 io_psi10=1.15/0.84 mem_psi10=0.00/0.00 swap_used_mib=3577 dirty_wb_kib=2012 settle_wait_s=0 |
