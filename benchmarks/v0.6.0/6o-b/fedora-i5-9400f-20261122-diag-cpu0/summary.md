## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-c50-P1` | 2 | 156,910 | 4% | 4.12 | 1.04 | 3.08 | 5,298 | 5,805 | 0.319 | 0.347 | 0.387 | 27.4 | 1.20 | 0 |
| `redis-noaof-set-c50-P1` | 2 | 186,129 | 5% | 3.49 | 0.72 | 2.77 | 4,795 | 4,089 | 0.235 | 0.311 | 0.435 | 19.8 | 1.20 | 0 |
| `base/rc-noaof-set-c1-P1` | 2 | 36,324 | 0% | 12.92 | 3.12 | 9.78 | 7,311 | 13,834 | 0.023 | 0.031 | 0.063 | 25.2 | 0.37 | 0 |
| `redis-noaof-set-c1-P1` | 2 | 33,771 | 3% | 14.52 | 4.05 | 10.47 | 8,027 | 17,328 | 0.031 | 0.031 | 0.071 | 19.1 | 0.35 | 0 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.843** | 0.846 0.840 |
| `base/rc-noaof-set-c1-P1` / `redis-noaof-set-c1-P1` | **1.076** | 1.061 1.091 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` | 1 | load=0.41 io_psi10=1.15/1.08 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2032 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=1.13 io_psi10=1.30/1.26 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1796 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 1 | load=0.81 io_psi10=2.48/2.18 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=3332 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 2 | load=0.96 io_psi10=2.01/1.92 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2092 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 1 | load=0.99 io_psi10=3.06/2.77 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1948 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 2 | load=0.96 io_psi10=0.73/0.70 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2984 settle_wait_s=0 |
| `redis-noaof-set-c1-P1` | 1 | load=0.92 io_psi10=1.22/1.12 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2808 settle_wait_s=0 |
| `redis-noaof-set-c1-P1` | 2 | load=0.85 io_psi10=1.34/1.24 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2128 settle_wait_s=0 |
