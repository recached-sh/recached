## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

| cell | n | ops/s | spread | CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-everysec-set-P1` | 5 | 115,929 | 3% | 6.33 | 2.28 | 4.05 | 13,904 | 11,376 | 0.399 | 0.543 | 0.591 | 25.9 | 0.89 | 0 |
| `opt/rc-everysec-set-P1` | 5 | 123,062 | 5% | 5.95 | 1.94 | 4.01 | 10,277 | 9,855 | 0.383 | 0.527 | 0.567 | 27.6 | 0.93 | 0 |
| `redis-set-P1` | 5 | 142,816 | 2% | 4.37 | 1.20 | 3.19 | 8,170 | 5,705 | 0.279 | 0.383 | 0.527 | 27.7 | 1.00 | 0 |
| `valkey-set-P1` | 5 | 140,292 | 7% | 4.68 | 1.31 | 3.38 | 11,157 | 6,861 | 0.295 | 0.407 | 0.551 | 21.1 | 0.95 | 0 |
| `base/rc-everysec-incr-P1` | 5 | 115,929 | 1% | 6.39 | 2.29 | 4.07 | 14,516 | 11,686 | 0.407 | 0.551 | 0.599 | 27.6 | 0.88 | 0 |
| `opt/rc-everysec-incr-P1` | 5 | 123,047 | 3% | 5.96 | 2.00 | 4.00 | 10,335 | 10,079 | 0.383 | 0.527 | 0.575 | 28.7 | 0.92 | 0 |
| `redis-incr-P1` | 5 | 159,949 | 6% | 4.09 | 0.94 | 3.17 | 7,177 | 4,851 | 0.271 | 0.359 | 0.503 | 17.7 | 1.08 | 0 |
| `valkey-incr-P1` | 5 | 156,801 | 4% | 4.28 | 1.14 | 3.10 | 8,663 | 5,404 | 0.271 | 0.375 | 0.519 | 16.5 | 1.06 | 0 |
| `base/rc-everysec-hset-P1` | 5 | 112,644 | 1% | 6.64 | 2.46 | 4.21 | 16,152 | 12,258 | 0.415 | 0.567 | 0.607 | 24.3 | 0.85 | 0 |
| `opt/rc-everysec-hset-P1` | 5 | 119,389 | 7% | 6.10 | 2.02 | 4.08 | 11,146 | 10,313 | 0.391 | 0.535 | 0.583 | 25.9 | 0.90 | 0 |
| `redis-hset-P1` | 5 | 140,302 | 4% | 4.55 | 1.21 | 3.35 | 9,690 | 6,420 | 0.287 | 0.391 | 0.551 | 24.9 | 0.95 | 0 |
| `valkey-hset-P1` | 5 | 140,262 | 4% | 4.73 | 1.30 | 3.42 | 12,513 | 6,980 | 0.295 | 0.407 | 0.559 | 21.0 | 0.95 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-set-P1` | 0.0999 | 10.0 | 6.1 | 0.0008 |
| `opt/rc-everysec-set-P1` | 0.0999 | 10.0 | 6.0 | 0.0008 |
| `base/rc-everysec-incr-P1` | 0.1000 | 10.0 | 5.9 | 0.0008 |
| `opt/rc-everysec-incr-P1` | 0.0999 | 10.0 | 6.2 | 0.0007 |
| `base/rc-everysec-hset-P1` | 0.1000 | 10.0 | 6.7 | 0.0008 |
| `opt/rc-everysec-hset-P1` | 0.0998 | 10.0 | 6.3 | 0.0008 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-everysec-set-P1` / `redis-set-P1` | **0.812** | 0.812 0.800 0.809 0.812 0.812 |
| `base/rc-everysec-set-P1` / `valkey-set-P1` | **0.826** | 0.870 0.843 0.824 0.826 0.812 |
| `opt/rc-everysec-set-P1` / `redis-set-P1` | **0.862** | 0.862 0.836 0.860 0.849 0.862 |
| `opt/rc-everysec-set-P1` / `valkey-set-P1` | **0.877** | 0.923 0.881 0.875 0.864 0.862 |
| `base/rc-everysec-incr-P1` / `redis-incr-P1` | **0.725** | 0.743 0.725 0.710 0.739 0.725 |
| `base/rc-everysec-incr-P1` / `valkey-incr-P1` | **0.739** | 0.729 0.754 0.725 0.754 0.739 |
| `opt/rc-everysec-incr-P1` / `redis-incr-P1` | **0.769** | 0.800 0.746 0.754 0.773 0.769 |
| `opt/rc-everysec-incr-P1` / `valkey-incr-P1` | **0.785** | 0.785 0.776 0.769 0.788 0.785 |
| `base/rc-everysec-hset-P1` / `redis-hset-P1` | **0.803** | 0.792 0.803 0.789 0.789 0.806 |
| `base/rc-everysec-hset-P1` / `valkey-hset-P1` | **0.803** | 0.806 0.817 0.789 0.803 0.792 |
| `opt/rc-everysec-hset-P1` / `redis-hset-P1` | **0.851** | 0.851 0.814 0.849 0.862 0.866 |
| `opt/rc-everysec-hset-P1` / `valkey-hset-P1` | **0.851** | 0.866 0.829 0.849 0.877 0.851 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-everysec-set-P1` | 1 | load=0.80 io_psi10=7.04/6.69 mem_psi10=2.12/1.97 swap_used_mib=4098 dirty_wb_kib=2064 settle_wait_s=0 |
| `base/rc-everysec-set-P1` | 2 | load=2.77 io_psi10=7.90/3.50 mem_psi10=0.00/0.00 swap_used_mib=4049 dirty_wb_kib=4772 settle_wait_s=0 |
| `base/rc-everysec-set-P1` | 3 | load=2.91 io_psi10=7.47/4.48 mem_psi10=0.00/0.00 swap_used_mib=3919 dirty_wb_kib=7328 settle_wait_s=6 |
| `base/rc-everysec-set-P1` | 4 | load=2.98 io_psi10=6.75/1.77 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=5032 settle_wait_s=0 |
| `base/rc-everysec-set-P1` | 5 | load=2.65 io_psi10=5.79/1.57 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=5180 settle_wait_s=0 |
| `opt/rc-everysec-set-P1` | 1 | load=1.65 io_psi10=6.87/1.82 mem_psi10=0.31/0.29 swap_used_mib=4098 dirty_wb_kib=2804 settle_wait_s=0 |
| `opt/rc-everysec-set-P1` | 2 | load=2.55 io_psi10=6.76/3.54 mem_psi10=0.00/0.00 swap_used_mib=4049 dirty_wb_kib=4424 settle_wait_s=8 |
| `opt/rc-everysec-set-P1` | 3 | load=2.80 io_psi10=6.94/2.91 mem_psi10=0.00/0.00 swap_used_mib=3919 dirty_wb_kib=5164 settle_wait_s=4 |
| `opt/rc-everysec-set-P1` | 4 | load=2.97 io_psi10=6.59/3.50 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=6940 settle_wait_s=9 |
| `opt/rc-everysec-set-P1` | 5 | load=3.20 io_psi10=7.09/2.81 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=6464 settle_wait_s=2 |
| `redis-set-P1` | 1 | load=3.37 io_psi10=7.46/3.41 mem_psi10=0.02/0.02 swap_used_mib=4073 dirty_wb_kib=5612 settle_wait_s=6 |
| `redis-set-P1` | 2 | load=3.31 io_psi10=7.38/3.87 mem_psi10=0.00/0.00 swap_used_mib=4049 dirty_wb_kib=7392 settle_wait_s=6 |
| `redis-set-P1` | 3 | load=2.43 io_psi10=6.87/4.23 mem_psi10=0.00/0.00 swap_used_mib=3918 dirty_wb_kib=6716 settle_wait_s=8 |
| `redis-set-P1` | 4 | load=2.93 io_psi10=7.30/3.95 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=7240 settle_wait_s=7 |
| `redis-set-P1` | 5 | load=3.08 io_psi10=7.01/2.18 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=5112 settle_wait_s=0 |
| `valkey-set-P1` | 1 | load=2.92 io_psi10=7.33/2.80 mem_psi10=0.04/0.04 swap_used_mib=4071 dirty_wb_kib=7148 settle_wait_s=9 |
| `valkey-set-P1` | 2 | load=3.75 io_psi10=7.32/2.12 mem_psi10=0.00/0.00 swap_used_mib=4049 dirty_wb_kib=5336 settle_wait_s=0 |
| `valkey-set-P1` | 3 | load=1.91 io_psi10=7.66/4.64 mem_psi10=0.00/0.00 swap_used_mib=3918 dirty_wb_kib=8636 settle_wait_s=9 |
| `valkey-set-P1` | 4 | load=3.12 io_psi10=6.16/2.23 mem_psi10=0.00/0.00 swap_used_mib=3916 dirty_wb_kib=4072 settle_wait_s=0 |
| `valkey-set-P1` | 5 | load=3.72 io_psi10=7.24/3.86 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=8516 settle_wait_s=9 |
| `base/rc-everysec-incr-P1` | 1 | load=2.44 io_psi10=6.63/3.91 mem_psi10=0.00/0.00 swap_used_mib=4071 dirty_wb_kib=9920 settle_wait_s=12 |
| `base/rc-everysec-incr-P1` | 2 | load=4.08 io_psi10=7.75/2.84 mem_psi10=0.00/0.00 swap_used_mib=4049 dirty_wb_kib=6512 settle_wait_s=0 |
| `base/rc-everysec-incr-P1` | 3 | load=1.92 io_psi10=7.39/4.85 mem_psi10=0.00/0.00 swap_used_mib=3918 dirty_wb_kib=5192 settle_wait_s=7 |
| `base/rc-everysec-incr-P1` | 4 | load=2.36 io_psi10=6.26/2.22 mem_psi10=0.00/0.00 swap_used_mib=3916 dirty_wb_kib=4656 settle_wait_s=0 |
| `base/rc-everysec-incr-P1` | 5 | load=3.19 io_psi10=7.42/3.43 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=5408 settle_wait_s=4 |
| `opt/rc-everysec-incr-P1` | 1 | load=2.36 io_psi10=7.21/4.07 mem_psi10=0.00/0.00 swap_used_mib=4070 dirty_wb_kib=6636 settle_wait_s=7 |
| `opt/rc-everysec-incr-P1` | 2 | load=2.89 io_psi10=7.68/4.59 mem_psi10=0.00/0.00 swap_used_mib=4051 dirty_wb_kib=7360 settle_wait_s=7 |
| `opt/rc-everysec-incr-P1` | 3 | load=1.80 io_psi10=7.21/2.22 mem_psi10=0.00/0.00 swap_used_mib=3918 dirty_wb_kib=5704 settle_wait_s=0 |
| `opt/rc-everysec-incr-P1` | 4 | load=2.18 io_psi10=7.87/5.72 mem_psi10=0.00/0.00 swap_used_mib=3916 dirty_wb_kib=6884 settle_wait_s=11 |
| `opt/rc-everysec-incr-P1` | 5 | load=3.30 io_psi10=5.51/1.11 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=4252 settle_wait_s=0 |
| `redis-incr-P1` | 1 | load=2.44 io_psi10=6.57/2.08 mem_psi10=0.00/0.00 swap_used_mib=4070 dirty_wb_kib=5516 settle_wait_s=2 |
| `redis-incr-P1` | 2 | load=2.16 io_psi10=7.72/4.66 mem_psi10=0.00/0.00 swap_used_mib=4051 dirty_wb_kib=5676 settle_wait_s=5 |
| `redis-incr-P1` | 3 | load=1.99 io_psi10=7.33/2.48 mem_psi10=0.00/0.00 swap_used_mib=3918 dirty_wb_kib=3860 settle_wait_s=0 |
| `redis-incr-P1` | 4 | load=2.17 io_psi10=7.21/3.70 mem_psi10=0.00/0.00 swap_used_mib=3917 dirty_wb_kib=6812 settle_wait_s=4 |
| `redis-incr-P1` | 5 | load=3.31 io_psi10=6.68/3.49 mem_psi10=0.00/0.00 swap_used_mib=3906 dirty_wb_kib=5544 settle_wait_s=6 |
| `valkey-incr-P1` | 1 | load=2.39 io_psi10=6.60/3.60 mem_psi10=0.00/0.00 swap_used_mib=4070 dirty_wb_kib=6072 settle_wait_s=7 |
| `valkey-incr-P1` | 2 | load=1.99 io_psi10=6.75/2.89 mem_psi10=0.00/0.00 swap_used_mib=4051 dirty_wb_kib=5296 settle_wait_s=4 |
| `valkey-incr-P1` | 3 | load=1.67 io_psi10=7.96/5.69 mem_psi10=0.00/0.00 swap_used_mib=3918 dirty_wb_kib=6820 settle_wait_s=8 |
| `valkey-incr-P1` | 4 | load=2.44 io_psi10=7.00/3.00 mem_psi10=0.00/0.00 swap_used_mib=3917 dirty_wb_kib=6416 settle_wait_s=3 |
| `valkey-incr-P1` | 5 | load=3.39 io_psi10=6.77/3.84 mem_psi10=0.00/0.00 swap_used_mib=3904 dirty_wb_kib=5784 settle_wait_s=6 |
| `base/rc-everysec-hset-P1` | 1 | load=2.55 io_psi10=7.39/2.83 mem_psi10=0.00/0.00 swap_used_mib=4070 dirty_wb_kib=6904 settle_wait_s=2 |
| `base/rc-everysec-hset-P1` | 2 | load=2.13 io_psi10=7.60/3.47 mem_psi10=0.00/0.00 swap_used_mib=4052 dirty_wb_kib=5072 settle_wait_s=2 |
| `base/rc-everysec-hset-P1` | 3 | load=1.62 io_psi10=6.58/4.80 mem_psi10=0.00/0.00 swap_used_mib=3918 dirty_wb_kib=4844 settle_wait_s=8 |
| `base/rc-everysec-hset-P1` | 4 | load=2.28 io_psi10=7.95/4.57 mem_psi10=0.00/0.00 swap_used_mib=3917 dirty_wb_kib=4312 settle_wait_s=3 |
| `base/rc-everysec-hset-P1` | 5 | load=3.38 io_psi10=7.07/3.64 mem_psi10=0.00/0.00 swap_used_mib=3904 dirty_wb_kib=5328 settle_wait_s=4 |
| `opt/rc-everysec-hset-P1` | 1 | load=3.01 io_psi10=7.54/2.41 mem_psi10=0.00/0.00 swap_used_mib=4070 dirty_wb_kib=3812 settle_wait_s=4 |
| `opt/rc-everysec-hset-P1` | 2 | load=1.80 io_psi10=7.05/6.29 mem_psi10=0.00/0.00 swap_used_mib=4068 dirty_wb_kib=2780 settle_wait_s=33 |
| `opt/rc-everysec-hset-P1` | 3 | load=1.52 io_psi10=7.37/3.12 mem_psi10=0.00/0.00 swap_used_mib=3918 dirty_wb_kib=5500 settle_wait_s=2 |
| `opt/rc-everysec-hset-P1` | 4 | load=2.01 io_psi10=7.92/5.51 mem_psi10=0.00/0.00 swap_used_mib=3917 dirty_wb_kib=6932 settle_wait_s=10 |
| `opt/rc-everysec-hset-P1` | 5 | load=3.27 io_psi10=7.14/1.93 mem_psi10=0.00/0.00 swap_used_mib=3904 dirty_wb_kib=5776 settle_wait_s=0 |
| `redis-hset-P1` | 1 | load=2.66 io_psi10=7.60/1.94 mem_psi10=0.00/0.00 swap_used_mib=4069 dirty_wb_kib=4524 settle_wait_s=3 |
| `redis-hset-P1` | 2 | load=2.83 io_psi10=6.85/3.42 mem_psi10=0.00/0.00 swap_used_mib=4068 dirty_wb_kib=7736 settle_wait_s=8 |
| `redis-hset-P1` | 3 | load=2.08 io_psi10=7.54/3.85 mem_psi10=0.00/0.00 swap_used_mib=3918 dirty_wb_kib=4816 settle_wait_s=6 |
| `redis-hset-P1` | 4 | load=2.09 io_psi10=7.35/5.01 mem_psi10=0.00/0.00 swap_used_mib=3918 dirty_wb_kib=5860 settle_wait_s=10 |
| `redis-hset-P1` | 5 | load=3.84 io_psi10=6.84/3.61 mem_psi10=0.00/0.00 swap_used_mib=3904 dirty_wb_kib=5928 settle_wait_s=6 |
| `valkey-hset-P1` | 1 | load=2.61 io_psi10=7.22/3.56 mem_psi10=0.00/0.00 swap_used_mib=4069 dirty_wb_kib=6912 settle_wait_s=10 |
| `valkey-hset-P1` | 2 | load=2.68 io_psi10=7.09/3.97 mem_psi10=0.02/0.00 swap_used_mib=4068 dirty_wb_kib=4756 settle_wait_s=9 |
| `valkey-hset-P1` | 3 | load=3.29 io_psi10=7.73/4.39 mem_psi10=0.00/0.00 swap_used_mib=3918 dirty_wb_kib=7408 settle_wait_s=10 |
| `valkey-hset-P1` | 4 | load=2.64 io_psi10=7.32/4.96 mem_psi10=0.00/0.00 swap_used_mib=3918 dirty_wb_kib=5592 settle_wait_s=9 |
| `valkey-hset-P1` | 5 | load=3.86 io_psi10=6.73/3.71 mem_psi10=0.00/0.00 swap_used_mib=3904 dirty_wb_kib=8820 settle_wait_s=11 |
