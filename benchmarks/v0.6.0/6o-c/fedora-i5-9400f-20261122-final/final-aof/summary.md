## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-everysec-set-c50-P1` | 3 | 132,192 | 2% | 5.46 | 1.79 | 3.67 | 9,900 | 9,277 | 0.359 | 0.479 | 0.519 | 25.9 | 0.94 | 0 |
| `final/rc-everysec-set-c50-P1` | 3 | 131,139 | 2% | 5.48 | 1.84 | 3.67 | 9,586 | 9,294 | 0.359 | 0.479 | 0.511 | 27.1 | 0.94 | 0 |
| `redis-set-c50-P1` | 3 | 158,372 | 0% | 4.09 | 1.03 | 3.06 | 8,170 | 5,557 | 0.263 | 0.359 | 0.487 | 21.9 | 1.03 | 0 |
| `base/rc-everysec-incr-c50-P1` | 3 | 132,218 | 2% | 5.46 | 1.82 | 3.67 | 9,968 | 9,431 | 0.359 | 0.487 | 0.519 | 27.6 | 0.94 | 0 |
| `final/rc-everysec-incr-c50-P1` | 3 | 133,324 | 3% | 5.45 | 1.81 | 3.63 | 9,652 | 9,314 | 0.359 | 0.479 | 0.511 | 28.9 | 0.96 | 0 |
| `redis-incr-c50-P1` | 3 | 175,778 | 3% | 3.74 | 0.83 | 2.93 | 7,161 | 4,616 | 0.247 | 0.335 | 0.455 | 16.0 | 1.11 | 0 |
| `base/rc-everysec-hset-c50-P1` | 3 | 126,968 | 2% | 5.64 | 1.90 | 3.73 | 10,798 | 9,759 | 0.367 | 0.495 | 0.527 | 24.4 | 0.91 | 0 |
| `final/rc-everysec-hset-c50-P1` | 3 | 130,073 | 2% | 5.51 | 1.78 | 3.72 | 10,489 | 9,150 | 0.359 | 0.487 | 0.519 | 25.6 | 0.95 | 0 |
| `redis-hset-c50-P1` | 3 | 155,292 | 3% | 4.19 | 1.14 | 3.05 | 9,688 | 5,994 | 0.263 | 0.367 | 0.495 | 26.7 | 0.99 | 0 |
| `base/rc-everysec-set-c50-P16` | 3 | 574,927 | 8% | 1.30 | 0.97 | 0.33 | 6,364 | 4,087 | 1.119 | 1.407 | 1.495 | 25.8 | 0.33 | 0 |
| `final/rc-everysec-set-c50-P16` | 3 | 558,956 | 6% | 1.28 | 0.95 | 0.33 | 6,243 | 4,025 | 1.111 | 1.391 | 1.487 | 27.5 | 0.33 | 0 |
| `redis-set-c50-P16` | 3 | 475,579 | 9% | 1.26 | 0.96 | 0.30 | 7,334 | 3,972 | 1.007 | 1.335 | 4.159 | 30.5 | 0.30 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 6.1 | 0.0004 |
| `final/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 6.0 | 0.0004 |
| `base/rc-everysec-incr-c50-P1` | 0.1000 | 10.0 | 5.7 | 0.0004 |
| `final/rc-everysec-incr-c50-P1` | 0.1000 | 10.0 | 5.6 | 0.0004 |
| `base/rc-everysec-hset-c50-P1` | 0.1000 | 10.0 | 7.1 | 0.0004 |
| `final/rc-everysec-hset-c50-P1` | 0.1000 | 10.0 | 6.7 | 0.0005 |
| `base/rc-everysec-set-c50-P16` | 0.0062 | 160.0 | 61.5 | 0.0002 |
| `final/rc-everysec-set-c50-P16` | 0.0062 | 160.3 | 74.5 | 0.0002 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-everysec-set-c50-P1` / `redis-set-c50-P1` | **0.835** | 0.842 0.828 0.835 |
| `final/rc-everysec-set-c50-P1` / `redis-set-c50-P1` | **0.828** | 0.828 0.828 0.842 |
| `base/rc-everysec-incr-c50-P1` / `redis-incr-c50-P1` | **0.752** | 0.758 0.770 0.752 |
| `final/rc-everysec-incr-c50-P1` / `redis-incr-c50-P1` | **0.758** | 0.746 0.783 0.771 |
| `base/rc-everysec-hset-c50-P1` / `redis-hset-c50-P1` | **0.818** | 0.829 0.833 0.817 |
| `final/rc-everysec-hset-c50-P1` / `redis-hset-c50-P1` | **0.838** | 0.843 0.854 0.838 |
| `base/rc-everysec-set-c50-P16` / `redis-set-c50-P16` | **1.209** | 1.184 1.205 1.209 |
| `final/rc-everysec-set-c50-P16` / `redis-set-c50-P16` | **1.175** | 1.192 1.247 1.167 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-everysec-set-c50-P1` | 1 | load=0.99 io_psi10=0.17/0.17 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=1092 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 2 | load=2.70 io_psi10=5.24/2.53 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=2320 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 3 | load=2.56 io_psi10=3.87/0.28 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=7644 settle_wait_s=0 |
| `final/rc-everysec-set-c50-P1` | 1 | load=1.29 io_psi10=2.85/0.16 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2708 settle_wait_s=0 |
| `final/rc-everysec-set-c50-P1` | 2 | load=2.61 io_psi10=5.67/2.14 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=7704 settle_wait_s=0 |
| `final/rc-everysec-set-c50-P1` | 3 | load=2.72 io_psi10=4.38/1.10 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=1096 settle_wait_s=0 |
| `redis-set-c50-P1` | 1 | load=1.69 io_psi10=3.89/0.52 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2404 settle_wait_s=0 |
| `redis-set-c50-P1` | 2 | load=2.65 io_psi10=6.02/1.65 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=2876 settle_wait_s=0 |
| `redis-set-c50-P1` | 3 | load=2.28 io_psi10=4.53/0.43 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=936 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 1 | load=1.85 io_psi10=6.39/2.09 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=6084 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 2 | load=2.51 io_psi10=4.02/0.35 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=5328 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 3 | load=2.53 io_psi10=7.00/2.94 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=4756 settle_wait_s=0 |
| `final/rc-everysec-incr-c50-P1` | 1 | load=1.90 io_psi10=3.83/0.21 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2484 settle_wait_s=0 |
| `final/rc-everysec-incr-c50-P1` | 2 | load=2.58 io_psi10=7.70/3.93 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=5788 settle_wait_s=0 |
| `final/rc-everysec-incr-c50-P1` | 3 | load=2.21 io_psi10=5.13/0.46 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=3036 settle_wait_s=0 |
| `redis-incr-c50-P1` | 1 | load=1.99 io_psi10=4.75/0.57 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2504 settle_wait_s=0 |
| `redis-incr-c50-P1` | 2 | load=2.64 io_psi10=7.63/4.50 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=2436 settle_wait_s=0 |
| `redis-incr-c50-P1` | 3 | load=2.31 io_psi10=6.28/1.85 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=3008 settle_wait_s=0 |
| `base/rc-everysec-hset-c50-P1` | 1 | load=2.12 io_psi10=4.35/1.27 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=6480 settle_wait_s=0 |
| `base/rc-everysec-hset-c50-P1` | 2 | load=2.50 io_psi10=4.61/0.87 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=2620 settle_wait_s=0 |
| `base/rc-everysec-hset-c50-P1` | 3 | load=2.40 io_psi10=5.99/1.84 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=6424 settle_wait_s=0 |
| `final/rc-everysec-hset-c50-P1` | 1 | load=2.40 io_psi10=3.62/0.28 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=5636 settle_wait_s=0 |
| `final/rc-everysec-hset-c50-P1` | 2 | load=1.61 io_psi10=6.91/2.69 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=5528 settle_wait_s=0 |
| `final/rc-everysec-hset-c50-P1` | 3 | load=2.48 io_psi10=6.62/2.27 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=2232 settle_wait_s=0 |
| `redis-hset-c50-P1` | 1 | load=2.53 io_psi10=6.27/1.91 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=2476 settle_wait_s=0 |
| `redis-hset-c50-P1` | 2 | load=1.40 io_psi10=7.88/5.78 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=3040 settle_wait_s=14 |
| `redis-hset-c50-P1` | 3 | load=2.76 io_psi10=5.35/0.53 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=4328 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P16` | 1 | load=2.49 io_psi10=7.47/5.32 mem_psi10=0.00/0.00 swap_used_mib=4668 dirty_wb_kib=372 settle_wait_s=10 |
| `base/rc-everysec-set-c50-P16` | 2 | load=1.69 io_psi10=7.32/4.95 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=3092 settle_wait_s=12 |
| `base/rc-everysec-set-c50-P16` | 3 | load=2.45 io_psi10=7.44/2.66 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=4760 settle_wait_s=0 |
| `final/rc-everysec-set-c50-P16` | 1 | load=1.73 io_psi10=6.78/4.18 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=3020 settle_wait_s=10 |
| `final/rc-everysec-set-c50-P16` | 2 | load=1.79 io_psi10=6.96/5.47 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=960 settle_wait_s=22 |
| `final/rc-everysec-set-c50-P16` | 3 | load=2.29 io_psi10=6.65/4.36 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=3512 settle_wait_s=10 |
| `redis-set-c50-P16` | 1 | load=1.85 io_psi10=7.98/4.94 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=3364 settle_wait_s=9 |
| `redis-set-c50-P16` | 2 | load=2.07 io_psi10=7.87/4.80 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=3932 settle_wait_s=15 |
| `redis-set-c50-P16` | 3 | load=2.02 io_psi10=6.65/4.49 mem_psi10=0.00/0.00 swap_used_mib=4667 dirty_wb_kib=4124 settle_wait_s=13 |
