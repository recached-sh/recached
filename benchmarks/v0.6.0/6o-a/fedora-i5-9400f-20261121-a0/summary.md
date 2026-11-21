## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

| cell | n | ops/s | spread | CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-everysec-set-P1` | 3 | 115,935 | 3% | 6.40 | 2.29 | 4.10 | 13,903 | 11,372 | 0.399 | 0.551 | 0.607 | 25.7 | 0.88 | 0 |
| `redis-set-P1` | 3 | 137,874 | 5% | 4.38 | 1.18 | 3.19 | 8,171 | 5,793 | 0.279 | 0.383 | 0.535 | 24.7 | 0.94 | 0 |
| `valkey-set-P1` | 3 | 140,272 | 2% | 4.74 | 1.29 | 3.48 | 11,164 | 7,028 | 0.295 | 0.415 | 0.575 | 19.2 | 0.97 | 0 |
| `base/rc-everysec-incr-P1` | 3 | 114,273 | 3% | 6.46 | 2.29 | 4.17 | 14,514 | 11,736 | 0.415 | 0.567 | 0.615 | 27.3 | 0.87 | 0 |
| `redis-incr-P1` | 3 | 159,974 | 0% | 4.12 | 0.96 | 3.16 | 7,183 | 4,832 | 0.271 | 0.359 | 0.511 | 16.1 | 1.09 | 0 |
| `valkey-incr-P1` | 3 | 145,402 | 6% | 4.36 | 1.10 | 3.25 | 8,663 | 5,594 | 0.279 | 0.383 | 0.535 | 19.3 | 1.00 | 0 |
| `base/rc-everysec-hset-P1` | 3 | 109,577 | 5% | 6.64 | 2.46 | 4.18 | 16,152 | 12,266 | 0.415 | 0.567 | 0.615 | 24.0 | 0.84 | 0 |
| `redis-hset-P1` | 3 | 140,312 | 4% | 4.51 | 1.17 | 3.34 | 9,699 | 6,163 | 0.279 | 0.391 | 0.551 | 26.9 | 0.97 | 0 |
| `valkey-hset-P1` | 3 | 135,492 | 11% | 4.86 | 1.35 | 3.53 | 12,516 | 7,274 | 0.295 | 0.423 | 0.583 | 22.3 | 0.91 | 0 |
| `base/rc-everysec-set-P16` | 3 | 464,727 | 4% | 1.72 | 1.33 | 0.39 | 10,008 | 5,596 | 1.463 | 1.847 | 2.095 | 25.9 | 0.31 | 0 |
| `redis-set-P16` | 3 | 493,121 | 2% | 1.32 | 0.98 | 0.35 | 7,341 | 4,072 | 1.071 | 1.439 | 3.951 | 30.7 | 0.31 | 0 |
| `valkey-set-P16` | 3 | 469,770 | 6% | 1.39 | 1.02 | 0.36 | 10,391 | 4,373 | 1.143 | 1.463 | 3.759 | 28.4 | 0.30 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-set-P1` | 0.0999 | 10.0 | 6.4 | 0.0006 |
| `base/rc-everysec-incr-P1` | 0.0999 | 10.0 | 6.5 | 0.0008 |
| `base/rc-everysec-hset-P1` | 0.1000 | 10.0 | 7.8 | 0.0007 |
| `base/rc-everysec-set-P16` | 0.0062 | 160.9 | 56.7 | 0.0003 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-everysec-set-P1` / `redis-set-P1` | **0.841** | 0.817 0.797 0.841 |
| `base/rc-everysec-set-P1` / `valkey-set-P1` | **0.827** | 0.803 0.841 0.826 |
| `base/rc-everysec-incr-P1` / `redis-incr-P1` | **0.714** | 0.714 0.714 0.694 |
| `base/rc-everysec-incr-P1` / `valkey-incr-P1` | **0.786** | 0.743 0.786 0.764 |
| `base/rc-everysec-hset-P1` / `redis-hset-P1` | **0.781** | 0.781 0.764 0.750 |
| `base/rc-everysec-hset-P1` / `valkey-hset-P1` | **0.809** | 0.781 0.820 0.842 |
| `base/rc-everysec-set-P16` / `redis-set-P16` | **0.942** | 0.953 0.942 0.943 |
| `base/rc-everysec-set-P16` / `valkey-set-P16` | **0.989** | 0.965 1.001 0.978 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-everysec-set-P1` | 1 | load=0.41 io_psi10=5.29/5.11 mem_psi10=0.00/0.00 swap_used_mib=1424 dirty_wb_kib=3848 |
| `base/rc-everysec-set-P1` | 2 | load=3.10 io_psi10=23.88/10.82 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=7968 |
| `base/rc-everysec-set-P1` | 3 | load=3.54 io_psi10=30.38/15.47 mem_psi10=0.00/0.00 swap_used_mib=1424 dirty_wb_kib=6456 |
| `redis-set-P1` | 1 | load=1.51 io_psi10=8.78/3.99 mem_psi10=0.04/0.04 swap_used_mib=1424 dirty_wb_kib=4584 |
| `redis-set-P1` | 2 | load=2.97 io_psi10=8.90/3.06 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=6120 |
| `redis-set-P1` | 3 | load=3.71 io_psi10=10.44/4.20 mem_psi10=0.00/0.00 swap_used_mib=1424 dirty_wb_kib=6916 |
| `valkey-set-P1` | 1 | load=2.31 io_psi10=17.00/8.22 mem_psi10=0.76/0.71 swap_used_mib=1399 dirty_wb_kib=13428 |
| `valkey-set-P1` | 2 | load=2.55 io_psi10=13.38/5.40 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=6908 |
| `valkey-set-P1` | 3 | load=3.51 io_psi10=15.99/7.71 mem_psi10=0.00/0.00 swap_used_mib=1424 dirty_wb_kib=10048 |
| `base/rc-everysec-incr-P1` | 1 | load=3.37 io_psi10=12.27/6.18 mem_psi10=0.08/0.07 swap_used_mib=1399 dirty_wb_kib=7304 |
| `base/rc-everysec-incr-P1` | 2 | load=3.64 io_psi10=14.57/6.92 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=5864 |
| `base/rc-everysec-incr-P1` | 3 | load=3.86 io_psi10=14.22/7.00 mem_psi10=0.00/0.00 swap_used_mib=1424 dirty_wb_kib=5652 |
| `redis-incr-P1` | 1 | load=3.44 io_psi10=5.47/1.89 mem_psi10=0.00/0.00 swap_used_mib=1399 dirty_wb_kib=4188 |
| `redis-incr-P1` | 2 | load=3.26 io_psi10=7.64/3.09 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=6224 |
| `redis-incr-P1` | 3 | load=4.53 io_psi10=10.04/4.22 mem_psi10=0.00/0.00 swap_used_mib=1424 dirty_wb_kib=6144 |
| `valkey-incr-P1` | 1 | load=3.27 io_psi10=7.90/2.79 mem_psi10=0.00/0.00 swap_used_mib=1399 dirty_wb_kib=7192 |
| `valkey-incr-P1` | 2 | load=3.22 io_psi10=9.12/3.92 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=6204 |
| `valkey-incr-P1` | 3 | load=4.25 io_psi10=11.30/5.90 mem_psi10=0.00/0.00 swap_used_mib=1423 dirty_wb_kib=7796 |
| `base/rc-everysec-hset-P1` | 1 | load=2.94 io_psi10=11.67/5.92 mem_psi10=0.00/0.00 swap_used_mib=1399 dirty_wb_kib=7528 |
| `base/rc-everysec-hset-P1` | 2 | load=3.31 io_psi10=9.60/5.01 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=6180 |
| `base/rc-everysec-hset-P1` | 3 | load=3.88 io_psi10=13.99/6.91 mem_psi10=0.00/0.00 swap_used_mib=1423 dirty_wb_kib=9664 |
| `redis-hset-P1` | 1 | load=2.70 io_psi10=6.39/2.41 mem_psi10=0.00/0.00 swap_used_mib=1399 dirty_wb_kib=6304 |
| `redis-hset-P1` | 2 | load=3.29 io_psi10=9.16/3.92 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=5496 |
| `redis-hset-P1` | 3 | load=3.52 io_psi10=12.65/7.74 mem_psi10=0.07/0.03 swap_used_mib=1423 dirty_wb_kib=8724 |
| `valkey-hset-P1` | 1 | load=2.79 io_psi10=10.55/4.74 mem_psi10=0.00/0.00 swap_used_mib=1399 dirty_wb_kib=5292 |
| `valkey-hset-P1` | 2 | load=3.36 io_psi10=13.85/4.40 mem_psi10=0.00/0.00 swap_used_mib=1421 dirty_wb_kib=9156 |
| `valkey-hset-P1` | 3 | load=3.46 io_psi10=20.70/11.67 mem_psi10=0.00/0.00 swap_used_mib=1423 dirty_wb_kib=11512 |
| `base/rc-everysec-set-P16` | 1 | load=2.84 io_psi10=11.41/4.64 mem_psi10=0.00/0.00 swap_used_mib=1399 dirty_wb_kib=7632 |
| `base/rc-everysec-set-P16` | 2 | load=3.14 io_psi10=11.67/4.58 mem_psi10=0.00/0.00 swap_used_mib=1421 dirty_wb_kib=5616 |
| `base/rc-everysec-set-P16` | 3 | load=4.35 io_psi10=17.80/10.15 mem_psi10=0.00/0.00 swap_used_mib=1423 dirty_wb_kib=12784 |
| `redis-set-P16` | 1 | load=2.64 io_psi10=10.36/4.52 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=7872 |
| `redis-set-P16` | 2 | load=3.91 io_psi10=11.72/5.18 mem_psi10=0.00/0.00 swap_used_mib=1424 dirty_wb_kib=7328 |
| `redis-set-P16` | 3 | load=3.48 io_psi10=14.08/6.79 mem_psi10=0.00/0.00 swap_used_mib=1426 dirty_wb_kib=8080 |
| `valkey-set-P16` | 1 | load=3.03 io_psi10=25.55/11.35 mem_psi10=0.00/0.00 swap_used_mib=1422 dirty_wb_kib=6952 |
| `valkey-set-P16` | 2 | load=3.61 io_psi10=29.28/12.42 mem_psi10=0.01/0.01 swap_used_mib=1424 dirty_wb_kib=10572 |
| `valkey-set-P16` | 3 | load=2.95 io_psi10=31.10/13.45 mem_psi10=0.00/0.00 swap_used_mib=1426 dirty_wb_kib=8532 |
