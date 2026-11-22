## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-lpush-c50-P1` | 3 | 139,515 | 6% | 4.95 | 1.51 | 3.44 | 8,249 | 8,318 | 0.367 | 0.383 | 0.455 | 389.3 | 1.07 | 0 |
| `new/rc-noaof-lpush-c50-P1` | 3 | 176,424 | 3% | 3.78 | 0.93 | 2.85 | 6,337 | 5,098 | 0.263 | 0.359 | 0.407 | 389.9 | 1.18 | 0 |
| `redis-noaof-lpush-c50-P1` | 3 | 196,708 | 2% | 3.31 | 0.61 | 2.67 | 5,363 | 3,391 | 0.231 | 0.295 | 0.423 | 211.6 | 1.25 | 0 |
| `valkey-noaof-lpush-c50-P1` | 3 | 184,570 | 2% | 3.59 | 0.76 | 2.83 | 6,996 | 4,386 | 0.239 | 0.319 | 0.447 | 212.0 | 1.16 | 0 |
| `base/rc-noaof-mset-c50-P1` | 3 | 86,944 | 2% | 9.07 | 4.93 | 4.08 | 29,404 | 23,867 | 0.583 | 0.639 | 0.711 | 26.3 | 0.77 | 0 |
| `new/rc-noaof-mset-c50-P1` | 3 | 98,341 | 2% | 8.19 | 4.64 | 3.50 | 29,773 | 21,944 | 0.487 | 0.647 | 0.687 | 26.9 | 0.75 | 0 |
| `redis-noaof-mset-c50-P1` | 3 | 116,482 | 1% | 6.78 | 3.64 | 3.14 | 20,332 | 16,646 | 0.383 | 0.543 | 0.623 | 19.6 | 0.84 | 0 |
| `valkey-noaof-mset-c50-P1` | 3 | 117,615 | 1% | 6.73 | 3.58 | 3.17 | 39,901 | 16,423 | 0.383 | 0.543 | 0.631 | 16.7 | 0.83 | 0 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.709** | 0.709 0.681 0.704 |
| `base/rc-noaof-lpush-c50-P1` / `valkey-noaof-lpush-c50-P1` | **0.756** | 0.744 0.714 0.762 |
| `new/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.897** | 0.878 0.912 0.896 |
| `new/rc-noaof-lpush-c50-P1` / `valkey-noaof-lpush-c50-P1` | **0.956** | 0.921 0.956 0.970 |
| `base/rc-noaof-mset-c50-P1` / `redis-noaof-mset-c50-P1` | **0.746** | 0.741 0.757 0.754 |
| `base/rc-noaof-mset-c50-P1` / `valkey-noaof-mset-c50-P1` | **0.739** | 0.734 0.757 0.739 |
| `new/rc-noaof-mset-c50-P1` / `redis-noaof-mset-c50-P1` | **0.844** | 0.844 0.858 0.852 |
| `new/rc-noaof-mset-c50-P1` / `valkey-noaof-mset-c50-P1` | **0.836** | 0.836 0.858 0.836 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-lpush-c50-P1` | 1 | load=0.86 io_psi10=0.31/0.30 mem_psi10=0.00/0.00 swap_used_mib=7622 dirty_wb_kib=1208 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | load=3.40 io_psi10=1.05/0.13 mem_psi10=0.00/0.00 swap_used_mib=7617 dirty_wb_kib=3388 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | load=3.66 io_psi10=0.34/0.07 mem_psi10=0.00/0.00 swap_used_mib=7617 dirty_wb_kib=2872 settle_wait_s=0 |
| `new/rc-noaof-lpush-c50-P1` | 1 | load=2.10 io_psi10=0.20/0.12 mem_psi10=0.00/0.00 swap_used_mib=7622 dirty_wb_kib=3144 settle_wait_s=0 |
| `new/rc-noaof-lpush-c50-P1` | 2 | load=3.41 io_psi10=0.51/0.40 mem_psi10=0.00/0.00 swap_used_mib=7617 dirty_wb_kib=2632 settle_wait_s=0 |
| `new/rc-noaof-lpush-c50-P1` | 3 | load=3.79 io_psi10=0.47/0.27 mem_psi10=0.00/0.00 swap_used_mib=7617 dirty_wb_kib=2552 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 1 | load=3.13 io_psi10=0.40/0.31 mem_psi10=0.00/0.00 swap_used_mib=7622 dirty_wb_kib=1956 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 2 | load=3.06 io_psi10=5.42/3.57 mem_psi10=0.00/0.00 swap_used_mib=7617 dirty_wb_kib=6664 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 3 | load=3.20 io_psi10=0.34/0.02 mem_psi10=0.00/0.00 swap_used_mib=7616 dirty_wb_kib=2264 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P1` | 1 | load=2.91 io_psi10=0.72/0.52 mem_psi10=0.00/0.00 swap_used_mib=7622 dirty_wb_kib=2060 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P1` | 2 | load=2.49 io_psi10=0.62/0.43 mem_psi10=0.00/0.00 swap_used_mib=7621 dirty_wb_kib=2680 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P1` | 3 | load=3.56 io_psi10=0.21/0.15 mem_psi10=0.00/0.00 swap_used_mib=7616 dirty_wb_kib=2128 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 1 | load=2.83 io_psi10=0.13/0.12 mem_psi10=0.00/0.00 swap_used_mib=7622 dirty_wb_kib=2008 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 2 | load=2.79 io_psi10=0.57/0.57 mem_psi10=0.00/0.00 swap_used_mib=7621 dirty_wb_kib=644 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 3 | load=3.47 io_psi10=0.83/0.53 mem_psi10=0.00/0.00 swap_used_mib=7616 dirty_wb_kib=3220 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P1` | 1 | load=2.96 io_psi10=0.60/0.44 mem_psi10=0.00/0.00 swap_used_mib=7621 dirty_wb_kib=4548 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P1` | 2 | load=2.39 io_psi10=0.22/0.21 mem_psi10=0.00/0.00 swap_used_mib=7621 dirty_wb_kib=2676 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P1` | 3 | load=2.86 io_psi10=0.22/0.18 mem_psi10=0.00/0.00 swap_used_mib=7616 dirty_wb_kib=3172 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 1 | load=2.52 io_psi10=0.12/0.12 mem_psi10=0.00/0.00 swap_used_mib=7621 dirty_wb_kib=6836 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 2 | load=2.53 io_psi10=1.94/1.74 mem_psi10=0.00/0.00 swap_used_mib=7621 dirty_wb_kib=2700 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 3 | load=3.42 io_psi10=0.12/0.03 mem_psi10=0.00/0.00 swap_used_mib=7616 dirty_wb_kib=2844 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P1` | 1 | load=2.91 io_psi10=1.17/0.72 mem_psi10=0.00/0.00 swap_used_mib=7621 dirty_wb_kib=4084 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P1` | 2 | load=2.86 io_psi10=0.19/0.16 mem_psi10=0.00/0.00 swap_used_mib=7621 dirty_wb_kib=1904 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P1` | 3 | load=3.54 io_psi10=0.19/0.16 mem_psi10=0.00/0.00 swap_used_mib=7616 dirty_wb_kib=3104 settle_wait_s=0 |
