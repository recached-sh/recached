## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `redis-set-c50-P1` | 3 | 157,837 | 1% | 4.08 | 1.06 | 3.03 | — | — | 0.263 | 0.359 | 0.495 | 25.3 | 1.04 | 0 |
| `redis-setpx100-c50-P1` | 3 | 122,384 | 3% | 5.32 | 2.07 | 3.23 | — | — | 0.295 | 0.415 | 0.559 | 21.0 | 0.81 | 0 |
| `redis-setpx1000-c50-P1` | 3 | 128,977 | 1% | 5.04 | 1.69 | 3.31 | — | — | 0.303 | 0.439 | 0.583 | 29.2 | 0.84 | 0 |
| `redis-incrvol-c50-P1` | 3 | 171,360 | 3% | 3.83 | 0.83 | 2.96 | — | — | 0.255 | 0.335 | 0.471 | 16.9 | 1.10 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `redis-set-c50-P1` | 1 | load=1.25 io_psi10=0.15/0.14 mem_psi10=0.00/0.00 swap_used_mib=8176 dirty_wb_kib=1536 settle_wait_s=0 |
| `redis-set-c50-P1` | 2 | load=1.77 io_psi10=7.68/5.48 mem_psi10=0.00/0.00 swap_used_mib=8175 dirty_wb_kib=276 settle_wait_s=14 |
| `redis-set-c50-P1` | 3 | load=1.84 io_psi10=7.39/2.43 mem_psi10=0.00/0.00 swap_used_mib=8175 dirty_wb_kib=5400 settle_wait_s=6 |
| `redis-setpx100-c50-P1` | 1 | load=1.63 io_psi10=7.17/1.44 mem_psi10=0.00/0.00 swap_used_mib=8175 dirty_wb_kib=6452 settle_wait_s=2 |
| `redis-setpx100-c50-P1` | 2 | load=1.96 io_psi10=6.99/4.05 mem_psi10=0.00/0.00 swap_used_mib=8175 dirty_wb_kib=124 settle_wait_s=12 |
| `redis-setpx100-c50-P1` | 3 | load=1.82 io_psi10=6.67/2.38 mem_psi10=0.00/0.00 swap_used_mib=8175 dirty_wb_kib=3040 settle_wait_s=9 |
| `redis-setpx1000-c50-P1` | 1 | load=1.86 io_psi10=7.93/6.22 mem_psi10=0.00/0.00 swap_used_mib=8175 dirty_wb_kib=1268 settle_wait_s=19 |
| `redis-setpx1000-c50-P1` | 2 | load=1.88 io_psi10=7.19/3.96 mem_psi10=0.00/0.00 swap_used_mib=8175 dirty_wb_kib=3436 settle_wait_s=9 |
| `redis-setpx1000-c50-P1` | 3 | load=1.72 io_psi10=7.25/4.20 mem_psi10=0.00/0.00 swap_used_mib=8175 dirty_wb_kib=212 settle_wait_s=13 |
| `redis-incrvol-c50-P1` | 1 | load=1.97 io_psi10=6.96/5.16 mem_psi10=0.00/0.00 swap_used_mib=8175 dirty_wb_kib=624 settle_wait_s=17 |
| `redis-incrvol-c50-P1` | 2 | load=1.87 io_psi10=7.46/4.05 mem_psi10=0.00/0.00 swap_used_mib=8175 dirty_wb_kib=3376 settle_wait_s=8 |
| `redis-incrvol-c50-P1` | 3 | load=1.56 io_psi10=7.16/4.65 mem_psi10=0.00/0.00 swap_used_mib=8175 dirty_wb_kib=276 settle_wait_s=14 |
