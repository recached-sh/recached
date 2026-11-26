## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-always-incrvol-c50-P1` | 3 | 373 | 8% | 20.69 | 4.67 | 15.67 | — | — | 116.607 | 183.167 | 316.927 | 15.4 | 0.01 | 0 |
| `final/rc-always-incrvol-c50-P1` | 3 | 343 | 12% | 23.95 | 8.00 | 16.00 | — | — | 141.695 | 216.959 | 341.759 | 34.1 | 0.01 | 0 |
| `comb/rc-always-incrvol-c50-P1` | 3 | 364 | 9% | 23.54 | 7.33 | 16.00 | — | — | 141.695 | 175.103 | 258.303 | 33.7 | 0.01 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-always-incrvol-c50-P1` | 0.0399 | 25.0 | 19.8 | 0.3157 |
| `final/rc-always-incrvol-c50-P1` | 0.0400 | 42.5 | 22.0 | 0.3695 |
| `comb/rc-always-incrvol-c50-P1` | 0.0399 | 42.7 | 21.7 | 0.3327 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-always-incrvol-c50-P1` | 1 | load=2.09 io_psi10=4.55/4.16 mem_psi10=0.00/0.00 swap_used_mib=8168 dirty_wb_kib=1692 settle_wait_s=0 |
| `base/rc-always-incrvol-c50-P1` | 2 | load=1.32 io_psi10=7.24/6.88 mem_psi10=0.00/0.00 swap_used_mib=8163 dirty_wb_kib=2352 settle_wait_s=24 |
| `base/rc-always-incrvol-c50-P1` | 3 | load=1.07 io_psi10=7.90/7.48 mem_psi10=0.00/0.00 swap_used_mib=8163 dirty_wb_kib=904 settle_wait_s=23 |
| `final/rc-always-incrvol-c50-P1` | 1 | load=0.94 io_psi10=7.04/7.01 mem_psi10=0.00/0.00 swap_used_mib=8168 dirty_wb_kib=3380 settle_wait_s=27 |
| `final/rc-always-incrvol-c50-P1` | 2 | load=1.78 io_psi10=6.77/6.27 mem_psi10=0.43/0.37 swap_used_mib=8163 dirty_wb_kib=1736 settle_wait_s=35 |
| `final/rc-always-incrvol-c50-P1` | 3 | load=1.15 io_psi10=7.13/6.77 mem_psi10=0.00/0.00 swap_used_mib=8163 dirty_wb_kib=2512 settle_wait_s=24 |
| `comb/rc-always-incrvol-c50-P1` | 1 | load=1.97 io_psi10=7.68/7.08 mem_psi10=0.00/0.00 swap_used_mib=8164 dirty_wb_kib=3776 settle_wait_s=21 |
| `comb/rc-always-incrvol-c50-P1` | 2 | load=1.77 io_psi10=6.94/6.36 mem_psi10=0.00/0.00 swap_used_mib=8164 dirty_wb_kib=3164 settle_wait_s=25 |
| `comb/rc-always-incrvol-c50-P1` | 3 | load=1.58 io_psi10=7.37/6.76 mem_psi10=0.00/0.00 swap_used_mib=8162 dirty_wb_kib=2372 settle_wait_s=23 |
