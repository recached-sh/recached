## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `prof/rc-noaof-set-c50-P1` | 1 | 148,126 | — | 4.40 | 1.14 | 3.26 | 5,462 | 6,199 | 0.335 | 0.375 | 0.423 | 26.7 | 1.16 | 0 |
| `prof/rc-noaof-lpush-c50-P1` | 1 | 129,032 | — | 5.17 | 1.61 | 3.55 | 8,500 | 8,676 | 0.383 | 0.423 | 0.471 | 92.7 | 1.03 | 0 |
| `prof/rc-noaof-sadd-c50-P1` | 1 | 121,197 | — | 5.70 | 1.95 | 3.75 | 8,843 | 10,195 | 0.415 | 0.455 | 0.511 | 19.9 | 0.98 | 0 |
| `prof/rc-noaof-mset-c50-P1` | 1 | 81,626 | — | 9.41 | 5.11 | 4.30 | 30,681 | 23,981 | 0.607 | 0.687 | 0.743 | 27.0 | 0.73 | 0 |
| `prof/rc-noaof-get-c50-P1` | 1 | 160,000 | — | 4.00 | 0.97 | 3.04 | 4,638 | 5,188 | 0.311 | 0.343 | 0.383 | 26.4 | 1.23 | 0 |
| `prof/rc-noaof-incr-c50-P1` | 1 | 153,822 | — | 4.32 | 1.20 | 3.12 | 5,405 | 6,226 | 0.327 | 0.367 | 0.407 | 28.8 | 1.17 | 0 |
| `prof/rc-noaof-zadd-c50-P1` | 1 | 121,212 | — | 5.81 | 2.11 | 3.70 | 10,993 | 10,649 | 0.423 | 0.463 | 0.503 | 19.1 | 0.96 | 0 |
| `prof/rc-noaof-set-c1-P1` | 1 | 34,250 | — | 14.05 | 3.43 | 10.60 | 7,489 | 13,978 | 0.031 | 0.031 | 0.071 | 24.1 | 0.36 | 0 |
| `prof/rc-noaof-lpush-c1-P1` | 1 | 32,397 | — | 15.72 | 4.27 | 11.43 | 10,298 | 17,902 | 0.031 | 0.039 | 0.071 | 34.1 | 0.33 | 0 |
| `prof/rc-noaof-set-c50-P16` | 1 | 1,000,000 | — | 0.77 | 0.51 | 0.26 | 3,021 | 2,245 | 0.751 | 0.871 | 1.007 | 26.9 | 0.74 | 0 |
| `prof/rc-noaof-lpush-c50-P16` | 1 | 799,787 | — | 1.03 | 0.69 | 0.35 | 6,170 | 3,034 | 0.975 | 1.039 | 1.079 | 229.2 | 0.60 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `prof/rc-noaof-set-c50-P1` | 1 | load=0.88 io_psi10=1.43/1.37 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2200 settle_wait_s=0 |
| `prof/rc-noaof-lpush-c50-P1` | 1 | load=1.13 io_psi10=2.31/2.18 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=8140 settle_wait_s=0 |
| `prof/rc-noaof-sadd-c50-P1` | 1 | load=1.34 io_psi10=4.61/4.07 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=10920 settle_wait_s=0 |
| `prof/rc-noaof-mset-c50-P1` | 1 | load=1.60 io_psi10=5.75/5.30 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=7536 settle_wait_s=0 |
| `prof/rc-noaof-get-c50-P1` | 1 | load=2.24 io_psi10=2.68/2.32 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=7084 settle_wait_s=0 |
| `prof/rc-noaof-incr-c50-P1` | 1 | load=2.66 io_psi10=3.53/2.91 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=7012 settle_wait_s=0 |
| `prof/rc-noaof-zadd-c50-P1` | 1 | load=2.48 io_psi10=3.12/2.79 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=7040 settle_wait_s=0 |
| `prof/rc-noaof-set-c1-P1` | 1 | load=2.40 io_psi10=3.26/2.96 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=5292 settle_wait_s=0 |
| `prof/rc-noaof-lpush-c1-P1` | 1 | load=2.33 io_psi10=2.76/2.48 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=9164 settle_wait_s=0 |
| `prof/rc-noaof-set-c50-P16` | 1 | load=2.13 io_psi10=4.11/3.97 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=4416 settle_wait_s=0 |
| `prof/rc-noaof-lpush-c50-P16` | 1 | load=2.04 io_psi10=5.28/5.03 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=5348 settle_wait_s=0 |
