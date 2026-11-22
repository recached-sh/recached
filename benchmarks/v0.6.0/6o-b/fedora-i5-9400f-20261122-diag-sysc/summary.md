## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-lpush-c50-P1` | 1 | 133,298 | — | 5.12 | 1.68 | 3.45 | — | — | 0.375 | 0.407 | 0.439 | 93.0 | 1.05 | 0 |
| `redis-noaof-lpush-c50-P1` | 1 | 190,367 | — | 3.40 | 0.62 | 2.77 | — | — | 0.231 | 0.303 | 0.431 | 45.2 | 1.23 | 0 |
| `base/rc-noaof-sadd-c50-P1` | 1 | 129,032 | — | 5.40 | 1.77 | 3.63 | — | — | 0.391 | 0.423 | 0.455 | 21.2 | 1.03 | 0 |
| `redis-noaof-sadd-c50-P1` | 1 | 190,476 | — | 3.40 | 0.71 | 2.69 | — | — | 0.231 | 0.303 | 0.431 | 12.5 | 1.23 | 0 |
| `base/rc-noaof-mset-c50-P1` | 1 | 85,085 | — | 9.14 | 4.95 | 4.19 | — | — | 0.591 | 0.647 | 0.695 | 27.4 | 0.74 | 0 |
| `redis-noaof-mset-c50-P1` | 1 | 114,247 | — | 6.86 | 3.85 | 3.01 | — | — | 0.391 | 0.551 | 0.631 | 19.9 | 0.82 | 0 |
| `base/rc-noaof-set-c1-P1` | 1 | 42,808 | — | 9.30 | 2.77 | 6.57 | — | — | 0.023 | 0.023 | 0.047 | 25.0 | 0.43 | 0 |
| `redis-noaof-set-c1-P1` | 1 | 38,665 | — | 11.04 | 3.57 | 7.47 | — | — | 0.023 | 0.031 | 0.055 | 19.1 | 0.40 | 0 |
| `base/rc-noaof-set-c50-P16` | 1 | 1,090,513 | — | 0.76 | 0.47 | 0.28 | — | — | 0.743 | 0.847 | 0.951 | 26.8 | 0.72 | 0 |
| `redis-noaof-set-c50-P16` | 1 | 999,667 | — | 0.82 | 0.61 | 0.21 | — | — | 0.727 | 0.895 | 0.975 | 19.9 | 0.64 | 0 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.700** | 0.700 |
| `base/rc-noaof-sadd-c50-P1` / `redis-noaof-sadd-c50-P1` | **0.677** | 0.677 |
| `base/rc-noaof-mset-c50-P1` / `redis-noaof-mset-c50-P1` | **0.745** | 0.745 |
| `base/rc-noaof-set-c1-P1` / `redis-noaof-set-c1-P1` | **1.107** | 1.107 |
| `base/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.091** | 1.091 |

## System calls per operation (sysc.so cells; ns include inline softirq work)

### `base/rc-noaof-lpush-c50-P1` rep 1

| syscall | calls/op | bytes or events/call | EAGAIN/op | zero/op | ns/call | ns/op |
|---|---:|---:|---:|---:|---:|---:|
| write | 0.0000 | 8.0 | 0.0000 | 0.0000 | 1124 | 0 |
| recv | 1.0001 | 66.0 | 0.0001 | 0.0001 | 575 | 576 |
| send | 1.0000 | 9.1 | 0.0000 | 0.0000 | 4458 | 4458 |
| epoll_wait | 0.0205 | 38.8 | 0.0000 | 0.0000 | 16005 | 327 |
| epoll_wait_t0 | 0.0167 | 12.4 | 0.0000 | 0.0001 | 1709 | 29 |
| epoll_ctl | 0.0001 | 0.0 | 0.0000 | 0.0001 | 1022 | 0 |
| accept4 | 0.0001 | 33.7 | 0.0001 | 0.0000 | 2479 | 0 |
| **all** | 2.0375 | | | | 2645 | 5390 |

### `redis-noaof-lpush-c50-P1` rep 1

| syscall | calls/op | bytes or events/call | EAGAIN/op | zero/op | ns/call | ns/op |
|---|---:|---:|---:|---:|---:|---:|
| read | 1.0001 | 66.0 | 0.0000 | 0.0001 | 548 | 548 |
| write | 1.0000 | 9.1 | 0.0000 | 0.0000 | 3631 | 3631 |
| epoll_wait | 0.0397 | 25.2 | 0.0000 | 0.0000 | 8592 | 341 |
| epoll_wait_t0 | 0.0000 | 39.5 | 0.0000 | 0.0000 | 5097 | 0 |
| epoll_ctl | 0.0001 | 0.0 | 0.0000 | 0.0001 | 910 | 0 |
| accept4 | 0.0001 | 30.1 | 0.0001 | 0.0000 | 2292 | 0 |
| **all** | 2.0400 | | | | 2215 | 4520 |

### `base/rc-noaof-sadd-c50-P1` rep 1

| syscall | calls/op | bytes or events/call | EAGAIN/op | zero/op | ns/call | ns/op |
|---|---:|---:|---:|---:|---:|---:|
| write | 0.0000 | 8.0 | 0.0000 | 0.0000 | 906 | 0 |
| recv | 1.0001 | 52.0 | 0.0001 | 0.0001 | 580 | 580 |
| send | 1.0000 | 4.0 | 0.0000 | 0.0000 | 4514 | 4514 |
| epoll_wait | 0.0179 | 44.3 | 0.0000 | 0.0000 | 47301 | 846 |
| epoll_wait_t0 | 0.0167 | 12.4 | 0.0000 | 0.0000 | 1738 | 29 |
| epoll_ctl | 0.0001 | 0.0 | 0.0000 | 0.0001 | 957 | 0 |
| accept4 | 0.0001 | 33.7 | 0.0001 | 0.0000 | 2461 | 0 |
| **all** | 2.0349 | | | | 2933 | 5969 |

### `redis-noaof-sadd-c50-P1` rep 1

| syscall | calls/op | bytes or events/call | EAGAIN/op | zero/op | ns/call | ns/op |
|---|---:|---:|---:|---:|---:|---:|
| read | 1.0001 | 52.0 | 0.0000 | 0.0001 | 520 | 520 |
| write | 1.0000 | 4.0 | 0.0000 | 0.0000 | 3618 | 3618 |
| epoll_wait | 0.0361 | 27.7 | 0.0000 | 0.0000 | 9191 | 332 |
| epoll_wait_t0 | 0.0000 | 39.8 | 0.0000 | 0.0000 | 5069 | 0 |
| epoll_ctl | 0.0001 | 0.0 | 0.0000 | 0.0001 | 921 | 0 |
| accept4 | 0.0001 | 30.1 | 0.0000 | 0.0000 | 2345 | 0 |
| **all** | 2.0365 | | | | 2195 | 4470 |

### `base/rc-noaof-mset-c50-P1` rep 1

| syscall | calls/op | bytes or events/call | EAGAIN/op | zero/op | ns/call | ns/op |
|---|---:|---:|---:|---:|---:|---:|
| write | 0.0000 | 8.0 | 0.0000 | 0.0000 | 1037 | 0 |
| recv | 1.0001 | 635.0 | 0.0001 | 0.0001 | 602 | 602 |
| send | 1.0000 | 5.0 | 0.0000 | 0.0000 | 4800 | 4800 |
| epoll_wait | 0.0168 | 48.3 | 0.0000 | 0.0000 | 34513 | 579 |
| epoll_wait_t0 | 0.0167 | 11.4 | 0.0000 | 0.0000 | 1644 | 27 |
| epoll_ctl | 0.0001 | 0.0 | 0.0000 | 0.0001 | 940 | 0 |
| futex | 0.0000 | 0.0 | 0.0000 | 0.0000 | 9999719235 | 10000 |
| accept4 | 0.0001 | 33.7 | 0.0001 | 0.0000 | 2699 | 0 |
| **all** | 2.0338 | | | | 7872 | 16009 |

### `redis-noaof-mset-c50-P1` rep 1

| syscall | calls/op | bytes or events/call | EAGAIN/op | zero/op | ns/call | ns/op |
|---|---:|---:|---:|---:|---:|---:|
| read | 1.0001 | 635.0 | 0.0000 | 0.0001 | 607 | 608 |
| write | 1.0000 | 5.0 | 0.0000 | 0.0000 | 3589 | 3589 |
| epoll_wait | 0.0303 | 33.0 | 0.0000 | 0.0000 | 14884 | 451 |
| epoll_wait_t0 | 0.0000 | 33.7 | 0.0000 | 0.0000 | 4733 | 0 |
| epoll_ctl | 0.0001 | 0.0 | 0.0000 | 0.0001 | 951 | 0 |
| accept4 | 0.0001 | 30.1 | 0.0000 | 0.0000 | 2479 | 0 |
| **all** | 2.0307 | | | | 2289 | 4648 |

### `base/rc-noaof-set-c1-P1` rep 1

| syscall | calls/op | bytes or events/call | EAGAIN/op | zero/op | ns/call | ns/op |
|---|---:|---:|---:|---:|---:|---:|
| write | 0.0000 | 8.0 | 0.0000 | 0.0000 | 1037 | 0 |
| recv | 1.0000 | 75.0 | 0.0000 | 0.0000 | 747 | 747 |
| send | 1.0000 | 5.0 | 0.0000 | 0.0000 | 5804 | 5804 |
| epoll_wait | 1.0001 | 1.0 | 0.0000 | 0.0000 | 15730 | 15731 |
| epoll_wait_t0 | 0.0328 | 1.0 | 0.0000 | 0.0328 | 221 | 7 |
| epoll_ctl | 0.0000 | 0.0 | 0.0000 | 0.0000 | 2071 | 0 |
| accept4 | 0.0000 | 11.0 | 0.0000 | 0.0000 | 5295 | 0 |
| **all** | 3.0330 | | | | 7349 | 22289 |

### `redis-noaof-set-c1-P1` rep 1

| syscall | calls/op | bytes or events/call | EAGAIN/op | zero/op | ns/call | ns/op |
|---|---:|---:|---:|---:|---:|---:|
| read | 1.0003 | 75.1 | 0.0000 | 0.0000 | 943 | 943 |
| write | 1.0000 | 5.0 | 0.0000 | 0.0000 | 6639 | 6639 |
| epoll_wait | 1.0000 | 1.0 | 0.0000 | 0.0000 | 14674 | 14675 |
| epoll_wait_t0 | 0.0001 | 0.0 | 0.0000 | 0.0001 | 589 | 0 |
| epoll_ctl | 0.0000 | 0.0 | 0.0000 | 0.0000 | 1747 | 0 |
| accept4 | 0.0000 | 7.0 | 0.0000 | 0.0000 | 5458 | 0 |
| **all** | 3.0005 | | | | 7418 | 22257 |

### `base/rc-noaof-set-c50-P16` rep 1

| syscall | calls/op | bytes or events/call | EAGAIN/op | zero/op | ns/call | ns/op |
|---|---:|---:|---:|---:|---:|---:|
| write | 0.0000 | 8.0 | 0.0000 | 0.0000 | 1007 | 0 |
| recv | 0.0625 | 1200.0 | 0.0000 | 0.0000 | 640 | 40 |
| send | 0.0625 | 80.0 | 0.0000 | 0.0000 | 4760 | 298 |
| epoll_wait | 0.0011 | 47.2 | 0.0000 | 0.0000 | 196296 | 211 |
| epoll_wait_t0 | 0.0010 | 11.2 | 0.0000 | 0.0000 | 1610 | 2 |
| epoll_ctl | 0.0000 | 0.0 | 0.0000 | 0.0000 | 890 | 0 |
| accept4 | 0.0000 | 33.7 | 0.0000 | 0.0000 | 2420 | 0 |
| **all** | 0.1272 | | | | 4327 | 551 |

### `redis-noaof-set-c50-P16` rep 1

| syscall | calls/op | bytes or events/call | EAGAIN/op | zero/op | ns/call | ns/op |
|---|---:|---:|---:|---:|---:|---:|
| read | 0.0625 | 1199.8 | 0.0000 | 0.0000 | 627 | 39 |
| write | 0.0625 | 80.0 | 0.0000 | 0.0000 | 3690 | 231 |
| epoll_wait | 0.0022 | 27.8 | 0.0000 | 0.0000 | 47983 | 108 |
| epoll_wait_t0 | 0.0000 | 28.6 | 0.0000 | 0.0000 | 4460 | 0 |
| epoll_ctl | 0.0000 | 0.0 | 0.0000 | 0.0000 | 926 | 0 |
| accept4 | 0.0000 | 30.1 | 0.0000 | 0.0000 | 2404 | 0 |
| **all** | 0.1274 | | | | 2967 | 378 |


## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-lpush-c50-P1` | 1 | load=2.11 io_psi10=6.50/6.00 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=3472 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 1 | load=2.10 io_psi10=4.30/3.87 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1640 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 1 | load=2.09 io_psi10=3.21/2.85 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=3552 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 1 | load=2.16 io_psi10=2.68/2.47 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2076 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 1 | load=2.30 io_psi10=1.71/1.59 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2652 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 1 | load=2.52 io_psi10=0.93/0.90 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2276 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 1 | load=2.43 io_psi10=0.67/0.65 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=2712 settle_wait_s=0 |
| `redis-noaof-set-c1-P1` | 1 | load=2.32 io_psi10=1.41/1.41 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1912 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 1 | load=2.26 io_psi10=0.72/0.71 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=1556 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 1 | load=2.16 io_psi10=3.20/3.05 mem_psi10=0.00/0.00 swap_used_mib=3385 dirty_wb_kib=3192 settle_wait_s=0 |
