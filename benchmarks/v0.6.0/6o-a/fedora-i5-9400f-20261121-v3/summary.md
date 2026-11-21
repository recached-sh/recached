## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

| cell | n | ops/s | spread | CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-P1` | 3 | 150,932 | 2% | 4.43 | 1.06 | 3.37 | 5,248 | 5,778 | 0.335 | 0.359 | 0.391 | 26.0 | 1.22 | 0 |
| `opt/rc-noaof-set-P1` | 3 | 150,932 | 2% | 4.39 | 1.03 | 3.38 | 5,247 | 5,692 | 0.335 | 0.359 | 0.391 | 27.3 | 1.22 | 0 |
| `redis-noaof-set-P1` | 3 | 181,769 | 2% | 3.69 | 0.71 | 2.98 | 4,784 | 4,043 | 0.247 | 0.319 | 0.463 | 19.4 | 1.21 | 0 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-P1` | 1 | load=3.23 io_psi10=7.53/6.24 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=3196 settle_wait_s=28 |
| `base/rc-noaof-set-P1` | 2 | load=3.04 io_psi10=1.76/1.57 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=2012 settle_wait_s=0 |
| `base/rc-noaof-set-P1` | 3 | load=3.16 io_psi10=1.04/0.93 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=2880 settle_wait_s=0 |
| `opt/rc-noaof-set-P1` | 1 | load=3.11 io_psi10=3.05/2.69 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=2784 settle_wait_s=0 |
| `opt/rc-noaof-set-P1` | 2 | load=3.05 io_psi10=3.88/3.63 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=1968 settle_wait_s=0 |
| `opt/rc-noaof-set-P1` | 3 | load=3.12 io_psi10=2.08/1.84 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=2712 settle_wait_s=0 |
| `redis-noaof-set-P1` | 1 | load=3.16 io_psi10=3.14/2.85 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=2364 settle_wait_s=0 |
| `redis-noaof-set-P1` | 2 | load=3.27 io_psi10=2.85/2.57 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=3424 settle_wait_s=0 |
| `redis-noaof-set-P1` | 3 | load=3.26 io_psi10=1.86/1.53 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=2160 settle_wait_s=0 |
