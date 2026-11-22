## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `prof/rc-noaof-set-c50-P1` | 1 | 166,667 | — | 4.04 | 1.06 | 2.98 | 5,816 | 5,735 | 0.279 | 0.391 | 0.431 | 27.0 | 1.16 | 0 |
| `prof/rc-noaof-lpush-c50-P1` | 1 | 166,611 | — | 3.92 | 1.04 | 2.88 | 6,512 | 5,202 | 0.271 | 0.375 | 0.423 | 92.5 | 1.14 | 0 |
| `prof/rc-noaof-sadd-c50-P1` | 1 | 159,923 | — | 4.15 | 1.25 | 2.90 | 6,790 | 6,333 | 0.279 | 0.391 | 0.431 | 20.5 | 1.12 | 0 |
| `prof/rc-noaof-mset-c50-P1` | 1 | 92,980 | — | 8.63 | 5.14 | 3.50 | 31,056 | 22,968 | 0.511 | 0.679 | 0.743 | 26.4 | 0.70 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `prof/rc-noaof-set-c50-P1` | 1 | load=0.86 io_psi10=0.74/0.64 mem_psi10=0.00/0.00 swap_used_mib=7614 dirty_wb_kib=1804 settle_wait_s=0 |
| `prof/rc-noaof-lpush-c50-P1` | 1 | load=1.04 io_psi10=0.89/0.66 mem_psi10=0.00/0.00 swap_used_mib=7611 dirty_wb_kib=2352 settle_wait_s=0 |
| `prof/rc-noaof-sadd-c50-P1` | 1 | load=1.35 io_psi10=2.83/2.41 mem_psi10=0.00/0.00 swap_used_mib=7611 dirty_wb_kib=5360 settle_wait_s=0 |
| `prof/rc-noaof-mset-c50-P1` | 1 | load=1.75 io_psi10=3.47/2.90 mem_psi10=0.00/0.00 swap_used_mib=7611 dirty_wb_kib=2020 settle_wait_s=0 |
