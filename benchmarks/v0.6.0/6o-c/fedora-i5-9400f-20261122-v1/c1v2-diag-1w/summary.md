## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `bdiag/rc-noaof-set-c50-P1` | 1 | 181,818 | — | 3.61 | 0.88 | 2.73 | 5,605 | 5,039 | 0.255 | 0.351 | 0.375 | 26.1 | 1.23 | 0 |
| `cdiag/rc-noaof-set-c50-P1` | 1 | 186,012 | — | 3.56 | 0.85 | 2.71 | 5,626 | 4,904 | 0.255 | 0.343 | 0.367 | 25.9 | 1.26 | 0 |
| `bdiag/rc-noaof-set-c50-P16` | 1 | 1,263,025 | — | 0.67 | 0.44 | 0.23 | 2,934 | 2,060 | 0.599 | 0.783 | 0.831 | 26.1 | 0.70 | 0 |
| `cdiag/rc-noaof-set-c50-P16` | 1 | 1,170,618 | — | 0.70 | 0.44 | 0.25 | 2,895 | 2,099 | 0.687 | 0.735 | 0.831 | 25.8 | 0.76 | 0 |
| `bdiag/rc-noaof-mset-c50-P16` | 1 | 217,845 | — | 4.41 | 4.09 | 0.32 | 27,178 | 16,701 | 3.599 | 4.431 | 4.575 | 26.4 | 0.34 | 0 |
| `cdiag/rc-noaof-mset-c50-P16` | 1 | 213,258 | — | 4.49 | 4.08 | 0.42 | 27,136 | 16,889 | 3.863 | 3.975 | 4.239 | 26.5 | 0.24 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `bdiag/rc-noaof-set-c50-P1` | 1 | load=0.71 io_psi10=1.26/1.26 mem_psi10=0.07/0.07 swap_used_mib=4669 dirty_wb_kib=1516 settle_wait_s=0 |
| `cdiag/rc-noaof-set-c50-P1` | 1 | load=1.06 io_psi10=0.84/0.84 mem_psi10=0.01/0.01 swap_used_mib=4669 dirty_wb_kib=1252 settle_wait_s=0 |
| `bdiag/rc-noaof-set-c50-P16` | 1 | load=1.36 io_psi10=0.38/0.38 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2128 settle_wait_s=0 |
| `cdiag/rc-noaof-set-c50-P16` | 1 | load=1.33 io_psi10=1.71/1.64 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2844 settle_wait_s=0 |
| `bdiag/rc-noaof-mset-c50-P16` | 1 | load=1.44 io_psi10=0.79/0.76 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=2036 settle_wait_s=0 |
| `cdiag/rc-noaof-mset-c50-P16` | 1 | load=1.34 io_psi10=2.01/2.00 mem_psi10=0.00/0.00 swap_used_mib=4669 dirty_wb_kib=1276 settle_wait_s=0 |
