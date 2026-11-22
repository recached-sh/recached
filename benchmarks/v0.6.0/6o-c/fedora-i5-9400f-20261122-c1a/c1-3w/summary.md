## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-c1-P1` | 3 | 36,699 | 2% | 13.05 | 3.03 | 10.12 | 7,054 | 13,341 | 0.023 | 0.031 | 0.071 | 26.1 | 0.37 | 0 |
| `new/rc-noaof-set-c1-P1` | 3 | 36,327 | 1% | 13.33 | 3.16 | 10.24 | 7,061 | 13,670 | 0.023 | 0.031 | 0.071 | 25.5 | 0.37 | 0 |
| `off/rc-noaof-set-c1-P1` | 3 | 37,077 | 4% | 12.71 | 2.79 | 9.90 | 6,862 | 12,517 | 0.023 | 0.031 | 0.071 | 25.7 | 0.37 | 0 |
| `base/rc-noaof-set-c50-P1` | 3 | 344,748 | 4% | 5.49 | 1.51 | 3.97 | 6,195 | 7,797 | 0.087 | 0.151 | 0.279 | 27.5 | 2.11 | 0 |
| `new/rc-noaof-set-c50-P1` | 3 | 340,847 | 7% | 5.40 | 1.45 | 3.96 | 5,935 | 7,513 | 0.087 | 0.151 | 0.295 | 27.1 | 2.11 | 0 |
| `off/rc-noaof-set-c50-P1` | 3 | 344,780 | 12% | 5.41 | 1.40 | 3.99 | 5,689 | 7,263 | 0.087 | 0.151 | 0.311 | 27.5 | 2.12 | 0 |
| `base/rc-noaof-set-c50-P16` | 3 | 3,110,144 | 3% | 0.80 | 0.54 | 0.26 | 2,952 | 2,419 | 0.191 | 0.423 | 0.679 | 27.5 | 1.86 | 0 |
| `new/rc-noaof-set-c50-P16` | 3 | 3,075,842 | 4% | 0.79 | 0.53 | 0.27 | 2,898 | 2,376 | 0.231 | 0.383 | 0.559 | 27.1 | 1.92 | 0 |
| `off/rc-noaof-set-c50-P16` | 3 | 3,009,846 | 1% | 0.81 | 0.54 | 0.27 | 2,900 | 2,400 | 0.223 | 0.399 | 0.695 | 27.4 | 1.86 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-c1-P1` | 1 | load=0.99 io_psi10=0.68/0.68 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=3280 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 2 | load=3.45 io_psi10=0.51/0.45 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2420 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 3 | load=2.74 io_psi10=0.50/0.36 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=3180 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 1 | load=1.13 io_psi10=0.87/0.63 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2948 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 2 | load=4.52 io_psi10=0.30/0.26 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2656 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 3 | load=2.15 io_psi10=0.18/0.16 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2760 settle_wait_s=0 |
| `off/rc-noaof-set-c1-P1` | 1 | load=2.08 io_psi10=0.21/0.17 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=1844 settle_wait_s=0 |
| `off/rc-noaof-set-c1-P1` | 2 | load=5.90 io_psi10=1.88/1.82 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=3756 settle_wait_s=0 |
| `off/rc-noaof-set-c1-P1` | 3 | load=1.69 io_psi10=0.45/0.44 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2520 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 1 | load=2.15 io_psi10=1.61/1.54 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=3724 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=5.78 io_psi10=0.48/0.40 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=3416 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=1.60 io_psi10=0.08/0.06 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2580 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 1 | load=3.40 io_psi10=0.32/0.29 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2680 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 2 | load=5.45 io_psi10=0.88/0.27 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=3184 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 3 | load=2.63 io_psi10=3.16/3.16 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=3108 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P1` | 1 | load=3.75 io_psi10=0.19/0.10 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2404 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P1` | 2 | load=5.60 io_psi10=0.24/0.12 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=4616 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P1` | 3 | load=3.58 io_psi10=0.52/0.52 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2600 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 1 | load=4.60 io_psi10=0.34/0.32 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2808 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 2 | load=5.67 io_psi10=0.72/0.28 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2400 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 3 | load=4.38 io_psi10=1.53/1.53 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=3352 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 1 | load=5.45 io_psi10=0.16/0.14 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2264 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 2 | load=6.07 io_psi10=0.58/0.17 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=3212 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 3 | load=2.02 io_psi10=47.44/46.46 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=7824 settle_wait_s=90 |
| `off/rc-noaof-set-c50-P16` | 1 | load=6.06 io_psi10=0.17/0.09 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=3112 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 2 | load=5.99 io_psi10=0.69/0.63 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=2496 settle_wait_s=0 |
| `off/rc-noaof-set-c50-P16` | 3 | load=1.76 io_psi10=8.60/8.01 mem_psi10=0.00/0.00 swap_used_mib=5171 dirty_wb_kib=1824 settle_wait_s=90 |
