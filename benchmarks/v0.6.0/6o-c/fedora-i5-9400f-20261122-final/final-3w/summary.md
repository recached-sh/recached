## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-c50-P16` | 3 | 3,332,857 | 1% | 0.77 | 0.53 | 0.25 | 2,950 | 2,359 | 0.199 | 0.383 | 0.479 | 26.4 | 2.03 | 0 |
| `final/rc-noaof-set-c50-P16` | 3 | 3,255,360 | 1% | 0.78 | 0.51 | 0.26 | 2,897 | 2,328 | 0.231 | 0.359 | 0.407 | 27.6 | 2.03 | 0 |
| `base/rc-noaof-set-c1-P1` | 3 | 39,519 | 2% | 12.39 | 2.78 | 9.69 | 7,056 | 11,930 | 0.023 | 0.031 | 0.039 | 24.7 | 0.39 | 0 |
| `final/rc-noaof-set-c1-P1` | 3 | 39,519 | 1% | 12.11 | 2.71 | 9.40 | 6,706 | 12,400 | 0.023 | 0.023 | 0.031 | 26.2 | 0.39 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-c50-P16` | 1 | load=0.84 io_psi10=0.22/0.21 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1660 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 2 | load=3.04 io_psi10=0.55/0.53 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1688 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 3 | load=3.88 io_psi10=0.13/0.13 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1992 settle_wait_s=0 |
| `final/rc-noaof-set-c50-P16` | 1 | load=2.26 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2508 settle_wait_s=0 |
| `final/rc-noaof-set-c50-P16` | 2 | load=1.61 io_psi10=0.09/0.09 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2228 settle_wait_s=0 |
| `final/rc-noaof-set-c50-P16` | 3 | load=4.61 io_psi10=2.20/2.20 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2308 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 1 | load=2.87 io_psi10=0.66/0.64 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2076 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 2 | load=1.63 io_psi10=0.16/0.16 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1728 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 3 | load=4.92 io_psi10=0.81/0.68 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2160 settle_wait_s=0 |
| `final/rc-noaof-set-c1-P1` | 1 | load=2.42 io_psi10=0.10/0.09 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1808 settle_wait_s=0 |
| `final/rc-noaof-set-c1-P1` | 2 | load=2.05 io_psi10=0.47/0.46 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1636 settle_wait_s=0 |
| `final/rc-noaof-set-c1-P1` | 3 | load=3.58 io_psi10=0.23/0.18 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2628 settle_wait_s=0 |
