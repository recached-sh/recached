## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `bdiag/rc-noaof-set-c1-P1` | 1 | 38,979 | — | 12.58 | 2.92 | 9.65 | 7,201 | 12,870 | 0.023 | 0.031 | 0.039 | 24.6 | 0.39 | 0 |
| `cdiag/rc-noaof-set-c1-P1` | 1 | 39,952 | — | 11.94 | 2.55 | 9.40 | 7,045 | 12,026 | 0.023 | 0.023 | 0.031 | 25.4 | 0.39 | 0 |
| `bdiag/rc-noaof-set-c50-P1` | 1 | 352,900 | — | 5.56 | 1.58 | 3.98 | 6,268 | 8,021 | 0.087 | 0.111 | 0.167 | 26.6 | 2.17 | 0 |
| `cdiag/rc-noaof-set-c50-P1` | 1 | 352,858 | — | 5.56 | 1.53 | 4.03 | 6,304 | 7,885 | 0.087 | 0.111 | 0.159 | 26.6 | 2.18 | 0 |
| `bdiag/rc-noaof-set-c50-P16` | 1 | 3,242,892 | — | 0.78 | 0.53 | 0.25 | 2,953 | 2,374 | 0.199 | 0.383 | 0.479 | 26.7 | 2.02 | 0 |
| `cdiag/rc-noaof-set-c50-P16` | 1 | 3,243,243 | — | 0.77 | 0.51 | 0.26 | 2,899 | 2,300 | 0.239 | 0.359 | 0.407 | 26.9 | 1.99 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `bdiag/rc-noaof-set-c1-P1` | 1 | load=0.77 io_psi10=0.26/0.26 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1132 settle_wait_s=0 |
| `cdiag/rc-noaof-set-c1-P1` | 1 | load=0.81 io_psi10=0.40/0.40 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1932 settle_wait_s=0 |
| `bdiag/rc-noaof-set-c50-P1` | 1 | load=0.85 io_psi10=0.72/0.72 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2248 settle_wait_s=0 |
| `cdiag/rc-noaof-set-c50-P1` | 1 | load=1.56 io_psi10=3.10/3.02 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1832 settle_wait_s=0 |
| `bdiag/rc-noaof-set-c50-P16` | 1 | load=2.17 io_psi10=1.34/1.31 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1928 settle_wait_s=0 |
| `cdiag/rc-noaof-set-c50-P16` | 1 | load=2.76 io_psi10=1.66/1.65 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1980 settle_wait_s=0 |
