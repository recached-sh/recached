## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-always-incrvol-c50-P1` | 3 | 353 | 8% | 21.14 | 4.67 | 16.00 | — | — | 149.887 | 175.231 | 258.303 | 16.5 | 0.01 | 0 |
| `comb/rc-always-incrvol-c50-P1` | 3 | 352 | 5% | 24.20 | 7.67 | 16.67 | — | — | 147.839 | 175.231 | 275.199 | 33.2 | 0.01 | 0 |
| `comb2/rc-always-incrvol-c50-P1` | 3 | 357 | 5% | 21.74 | 5.33 | 16.33 | — | — | 149.759 | 175.231 | 332.799 | 16.4 | 0.01 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-always-incrvol-c50-P1` | 0.0400 | 25.0 | 20.4 | 0.3278 |
| `comb/rc-always-incrvol-c50-P1` | 0.0399 | 42.7 | 22.3 | 0.3228 |
| `comb2/rc-always-incrvol-c50-P1` | 0.0399 | 25.1 | 20.8 | 0.3300 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-always-incrvol-c50-P1` | 1 | load=1.84 io_psi10=7.15/6.33 mem_psi10=0.00/0.00 swap_used_mib=8152 dirty_wb_kib=152 settle_wait_s=18 |
| `base/rc-always-incrvol-c50-P1` | 2 | load=1.72 io_psi10=6.86/6.46 mem_psi10=0.00/0.00 swap_used_mib=8151 dirty_wb_kib=2168 settle_wait_s=25 |
| `base/rc-always-incrvol-c50-P1` | 3 | load=1.50 io_psi10=6.87/6.48 mem_psi10=0.25/0.25 swap_used_mib=8151 dirty_wb_kib=540 settle_wait_s=32 |
| `comb/rc-always-incrvol-c50-P1` | 1 | load=1.46 io_psi10=7.58/6.78 mem_psi10=0.00/0.00 swap_used_mib=8152 dirty_wb_kib=4360 settle_wait_s=26 |
| `comb/rc-always-incrvol-c50-P1` | 2 | load=1.26 io_psi10=7.03/6.59 mem_psi10=0.00/0.00 swap_used_mib=8152 dirty_wb_kib=3392 settle_wait_s=24 |
| `comb/rc-always-incrvol-c50-P1` | 3 | load=1.90 io_psi10=7.13/6.65 mem_psi10=0.00/0.00 swap_used_mib=8151 dirty_wb_kib=2736 settle_wait_s=23 |
| `comb2/rc-always-incrvol-c50-P1` | 1 | load=1.87 io_psi10=7.51/7.06 mem_psi10=0.00/0.00 swap_used_mib=8152 dirty_wb_kib=3428 settle_wait_s=24 |
| `comb2/rc-always-incrvol-c50-P1` | 2 | load=1.85 io_psi10=7.55/7.13 mem_psi10=0.00/0.00 swap_used_mib=8152 dirty_wb_kib=492 settle_wait_s=23 |
| `comb2/rc-always-incrvol-c50-P1` | 3 | load=1.34 io_psi10=7.44/7.04 mem_psi10=0.00/0.00 swap_used_mib=8151 dirty_wb_kib=2036 settle_wait_s=24 |
