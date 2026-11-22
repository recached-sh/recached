## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-c50-P1` | 2 | 307,768 | 5% | 5.39 | 1.42 | 3.97 | 5,906 | 7,470 | 0.087 | 0.275 | 0.359 | 27.1 | 1.95 | 0 |
| `new/rc-noaof-set-c50-P1` | 2 | 307,582 | 0% | 5.35 | 1.47 | 3.88 | 6,125 | 7,610 | 0.087 | 0.255 | 0.399 | 27.2 | 1.92 | 0 |
| `base/rc-noaof-set-c50-P16` | 2 | 2,909,222 | 4% | 0.82 | 0.54 | 0.28 | 2,922 | 2,441 | 0.227 | 0.415 | 0.763 | 27.1 | 1.80 | 0 |
| `new/rc-noaof-set-c50-P16` | 2 | 2,908,579 | 4% | 0.81 | 0.55 | 0.26 | 2,956 | 2,448 | 0.191 | 0.443 | 0.951 | 27.3 | 1.78 | 0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | 300,070 | 5% | 5.95 | 1.97 | 3.99 | 8,541 | 10,110 | 0.111 | 0.307 | 0.427 | 225.1 | 1.88 | 0 |
| `new/rc-noaof-lpush-c50-P1` | 2 | 307,802 | 5% | 5.38 | 1.47 | 3.91 | 6,846 | 7,660 | 0.087 | 0.251 | 0.395 | 225.7 | 1.92 | 0 |
| `base/rc-noaof-set-c1-P1` | 2 | 35,784 | 0% | 12.85 | 3.06 | 9.79 | 7,255 | 13,439 | 0.023 | 0.031 | 0.079 | 25.3 | 0.37 | 0 |
| `new/rc-noaof-set-c1-P1` | 2 | 35,268 | 3% | 13.40 | 3.08 | 10.32 | 7,114 | 13,665 | 0.023 | 0.031 | 0.079 | 26.1 | 0.36 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` | 1 | load=0.58 io_psi10=3.15/2.94 mem_psi10=0.00/0.00 swap_used_mib=7629 dirty_wb_kib=1820 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=4.69 io_psi10=0.98/0.67 mem_psi10=0.00/0.00 swap_used_mib=7625 dirty_wb_kib=1900 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 1 | load=1.41 io_psi10=1.56/1.40 mem_psi10=0.00/0.00 swap_used_mib=7628 dirty_wb_kib=3772 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 2 | load=4.46 io_psi10=1.60/1.39 mem_psi10=0.00/0.00 swap_used_mib=7625 dirty_wb_kib=2256 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 1 | load=2.35 io_psi10=2.91/2.37 mem_psi10=0.00/0.00 swap_used_mib=7628 dirty_wb_kib=2400 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 2 | load=4.15 io_psi10=1.19/0.93 mem_psi10=0.00/0.00 swap_used_mib=7625 dirty_wb_kib=2044 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 1 | load=2.72 io_psi10=1.71/1.36 mem_psi10=0.00/0.00 swap_used_mib=7628 dirty_wb_kib=2316 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 2 | load=4.17 io_psi10=1.55/1.19 mem_psi10=0.00/0.00 swap_used_mib=7625 dirty_wb_kib=3016 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 1 | load=2.86 io_psi10=0.96/0.81 mem_psi10=0.00/0.00 swap_used_mib=7628 dirty_wb_kib=2256 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | load=3.00 io_psi10=1.06/0.65 mem_psi10=0.00/0.00 swap_used_mib=7626 dirty_wb_kib=2360 settle_wait_s=0 |
| `new/rc-noaof-lpush-c50-P1` | 1 | load=3.26 io_psi10=6.44/1.72 mem_psi10=0.00/0.00 swap_used_mib=7628 dirty_wb_kib=8504 settle_wait_s=0 |
| `new/rc-noaof-lpush-c50-P1` | 2 | load=2.17 io_psi10=0.53/0.48 mem_psi10=0.00/0.00 swap_used_mib=7626 dirty_wb_kib=2044 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 1 | load=4.03 io_psi10=6.61/4.23 mem_psi10=0.00/0.00 swap_used_mib=7627 dirty_wb_kib=3832 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 2 | load=2.64 io_psi10=0.54/0.46 mem_psi10=0.00/0.00 swap_used_mib=7626 dirty_wb_kib=2776 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 1 | load=3.24 io_psi10=2.17/1.60 mem_psi10=0.00/0.00 swap_used_mib=7627 dirty_wb_kib=2472 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 2 | load=2.89 io_psi10=0.83/0.59 mem_psi10=0.00/0.00 swap_used_mib=7626 dirty_wb_kib=3564 settle_wait_s=0 |
