## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-c50-P1` | 3 | 357,092 | 2% | 5.52 | 1.51 | 4.01 | 6,244 | 7,769 | 0.087 | 0.111 | 0.167 | 27.0 | 2.17 | 0 |
| `c3/rc-noaof-set-c50-P1` | 3 | 361,394 | 2% | 5.47 | 1.44 | 4.03 | 6,034 | 7,413 | 0.079 | 0.103 | 0.167 | 26.6 | 2.18 | 0 |
| `base/rc-noaof-set-c1-P1` | 3 | 39,519 | 0% | 12.47 | 2.81 | 9.72 | 7,055 | 12,630 | 0.023 | 0.031 | 0.039 | 25.5 | 0.39 | 0 |
| `c3/rc-noaof-set-c1-P1` | 3 | 38,670 | 2% | 12.72 | 2.87 | 9.83 | 6,841 | 13,000 | 0.023 | 0.031 | 0.039 | 25.2 | 0.38 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` | 1 | load=0.94 io_psi10=0.94/0.94 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2068 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=2.77 io_psi10=0.06/0.06 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1804 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=3.69 io_psi10=0.47/0.47 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1760 settle_wait_s=0 |
| `c3/rc-noaof-set-c50-P1` | 1 | load=2.38 io_psi10=0.20/0.20 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1496 settle_wait_s=0 |
| `c3/rc-noaof-set-c50-P1` | 2 | load=1.53 io_psi10=0.36/0.36 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1824 settle_wait_s=0 |
| `c3/rc-noaof-set-c50-P1` | 3 | load=4.13 io_psi10=0.12/0.12 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1792 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 1 | load=3.27 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2308 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 2 | load=1.75 io_psi10=2.29/2.29 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2804 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 3 | load=4.66 io_psi10=0.11/0.11 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2232 settle_wait_s=0 |
| `c3/rc-noaof-set-c1-P1` | 1 | load=2.55 io_psi10=0.50/0.50 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1724 settle_wait_s=0 |
| `c3/rc-noaof-set-c1-P1` | 2 | load=2.02 io_psi10=0.10/0.10 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1528 settle_wait_s=0 |
| `c3/rc-noaof-set-c1-P1` | 3 | load=3.41 io_psi10=0.35/0.35 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1884 settle_wait_s=0 |
