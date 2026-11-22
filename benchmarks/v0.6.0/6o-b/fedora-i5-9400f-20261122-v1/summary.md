## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)

CPU µs/op is the server's exact on-CPU time (schedstat, kernel and inline softirq included); user/sys are /proc's tick-sampled split.

| cell | n | ops/s | spread | on-CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `base/rc-noaof-set-c50-P1` | 3 | 159,991 | 3% | 4.09 | 1.01 | 3.08 | 5,232 | 5,601 | 0.319 | 0.335 | 0.367 | 26.1 | 1.21 | 0 |
| `new/rc-noaof-set-c50-P1` | 3 | 179,072 | 1% | 3.68 | 0.90 | 2.79 | 5,582 | 5,083 | 0.263 | 0.359 | 0.391 | 26.8 | 1.22 | 0 |
| `redis-noaof-set-c50-P1` | 3 | 193,523 | 0% | 3.41 | 0.66 | 2.75 | 4,781 | 3,920 | 0.231 | 0.303 | 0.431 | 19.5 | 1.23 | 0 |
| `valkey-noaof-set-c50-P1` | 3 | 184,604 | 3% | 3.67 | 0.83 | 2.84 | 7,430 | 4,842 | 0.247 | 0.327 | 0.447 | 16.7 | 1.19 | 0 |
| `base/rc-noaof-set-c50-P16` | 3 | 1,176,263 | 4% | 0.70 | 0.45 | 0.25 | 2,908 | 2,078 | 0.695 | 0.735 | 0.799 | 26.3 | 0.75 | 0 |
| `new/rc-noaof-set-c50-P16` | 3 | 1,249,688 | 5% | 0.68 | 0.45 | 0.23 | 2,930 | 2,032 | 0.607 | 0.799 | 0.847 | 26.8 | 0.72 | 0 |
| `redis-noaof-set-c50-P16` | 3 | 1,158,950 | 4% | 0.75 | 0.56 | 0.19 | 3,976 | 2,344 | 0.655 | 0.823 | 0.887 | 19.6 | 0.67 | 0 |
| `valkey-noaof-set-c50-P16` | 3 | 1,025,168 | 1% | 0.86 | 0.65 | 0.22 | 6,718 | 2,770 | 0.751 | 0.903 | 0.959 | 16.6 | 0.60 | 0 |
| `base/rc-noaof-set-c1-P1` | 3 | 39,300 | 5% | 12.51 | 3.03 | 9.48 | 7,152 | 13,631 | 0.023 | 0.031 | 0.031 | 23.6 | 0.37 | 0 |
| `new/rc-noaof-set-c1-P1` | 3 | 39,300 | 5% | 12.30 | 2.82 | 9.47 | 6,952 | 12,761 | 0.023 | 0.031 | 0.031 | 24.1 | 0.38 | 0 |
| `redis-noaof-set-c1-P1` | 3 | 36,326 | 6% | 14.17 | 3.85 | 10.23 | 7,999 | 16,815 | 0.031 | 0.031 | 0.039 | 19.0 | 0.35 | 0 |
| `base/rc-noaof-set-c1-P16` | 3 | 463,258 | 6% | 1.20 | 0.56 | 0.65 | 3,027 | 2,461 | 0.031 | 0.039 | 0.039 | 23.4 | 0.33 | 0 |
| `new/rc-noaof-set-c1-P16` | 3 | 470,063 | 4% | 1.19 | 0.57 | 0.62 | 3,015 | 2,436 | 0.031 | 0.031 | 0.039 | 24.1 | 0.34 | 0 |
| `redis-noaof-set-c1-P16` | 3 | 420,566 | 6% | 1.42 | 0.72 | 0.70 | 4,178 | 3,185 | 0.039 | 0.039 | 0.047 | 19.1 | 0.30 | 0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | 137,931 | 2% | 4.97 | 1.50 | 3.47 | 8,239 | 8,392 | 0.367 | 0.407 | 0.431 | 224.4 | 1.10 | 0 |
| `new/rc-noaof-lpush-c50-P1` | 3 | 179,083 | 3% | 3.70 | 0.88 | 2.83 | 6,312 | 5,088 | 0.263 | 0.359 | 0.391 | 225.1 | 1.19 | 0 |
| `redis-noaof-lpush-c50-P1` | 3 | 199,973 | 2% | 3.22 | 0.60 | 2.62 | 5,106 | 3,262 | 0.223 | 0.295 | 0.415 | 111.7 | 1.26 | 0 |
| `valkey-noaof-lpush-c50-P1` | 3 | 190,464 | 2% | 3.49 | 0.69 | 2.80 | 6,956 | 4,182 | 0.239 | 0.311 | 0.439 | 112.4 | 1.19 | 0 |
| `base/rc-noaof-lpush-c50-P16` | 3 | 869,225 | 3% | 0.99 | 0.66 | 0.33 | 5,986 | 3,003 | 0.919 | 0.975 | 1.015 | 1,327.1 | 0.59 | 0 |
| `new/rc-noaof-lpush-c50-P16` | 3 | 1,230,466 | 2% | 0.68 | 0.42 | 0.26 | 3,716 | 1,932 | 0.591 | 0.775 | 0.823 | 1,327.9 | 0.64 | 0 |
| `redis-noaof-lpush-c50-P16` | 3 | 1,332,800 | 2% | 0.63 | 0.33 | 0.30 | 4,344 | 1,850 | 0.567 | 0.727 | 0.783 | 677.5 | 0.68 | 0 |
| `valkey-noaof-lpush-c50-P16` | 3 | 1,080,555 | 3% | 0.81 | 0.60 | 0.21 | 6,200 | 2,546 | 0.719 | 0.871 | 0.935 | 678.2 | 0.55 | 0 |
| `base/rc-noaof-lpush-c1-P1` | 3 | 36,882 | 4% | 14.17 | 4.12 | 10.18 | 10,013 | 18,860 | 0.031 | 0.031 | 0.031 | 57.2 | 0.35 | 0 |
| `new/rc-noaof-lpush-c1-P1` | 3 | 38,667 | 5% | 13.00 | 3.18 | 9.83 | 7,531 | 14,061 | 0.023 | 0.031 | 0.031 | 57.8 | 0.37 | 0 |
| `redis-noaof-lpush-c1-P1` | 3 | 35,778 | 4% | 14.45 | 4.00 | 10.45 | 8,325 | 17,847 | 0.031 | 0.031 | 0.039 | 31.4 | 0.34 | 0 |
| `base/rc-noaof-lpush-c1-P16` | 3 | 409,815 | 5% | 1.54 | 0.84 | 0.71 | 6,089 | 3,570 | 0.039 | 0.039 | 0.055 | 510.9 | 0.28 | 0 |
| `new/rc-noaof-lpush-c1-P16` | 3 | 470,063 | 6% | 1.24 | 0.56 | 0.68 | 3,784 | 2,475 | 0.031 | 0.039 | 0.039 | 511.6 | 0.31 | 0 |
| `redis-noaof-lpush-c1-P16` | 3 | 443,976 | 5% | 1.34 | 0.63 | 0.72 | 4,597 | 2,737 | 0.031 | 0.039 | 0.047 | 277.7 | 0.30 | 0 |
| `base/rc-noaof-sadd-c50-P1` | 3 | 131,862 | 9% | 5.24 | 1.65 | 3.59 | 8,564 | 9,327 | 0.391 | 0.423 | 0.455 | 18.9 | 1.04 | 0 |
| `new/rc-noaof-sadd-c50-P1` | 3 | 173,903 | 4% | 3.81 | 1.02 | 2.82 | 6,556 | 5,611 | 0.271 | 0.375 | 0.407 | 19.4 | 1.17 | 0 |
| `redis-noaof-sadd-c50-P1` | 3 | 199,987 | 3% | 3.25 | 0.67 | 2.58 | 4,442 | 3,457 | 0.223 | 0.295 | 0.407 | 12.2 | 1.27 | 0 |
| `valkey-noaof-sadd-c50-P1` | 3 | 193,536 | 2% | 3.38 | 0.77 | 2.59 | 6,177 | 3,988 | 0.231 | 0.303 | 0.423 | 12.9 | 1.24 | 0 |
| `base/rc-noaof-sadd-c50-P16` | 3 | 769,024 | 6% | 1.13 | 0.84 | 0.29 | 6,315 | 3,738 | 1.055 | 1.119 | 1.167 | 18.9 | 0.57 | 0 |
| `new/rc-noaof-sadd-c50-P16` | 3 | 1,080,789 | 4% | 0.79 | 0.58 | 0.22 | 4,015 | 2,530 | 0.711 | 0.911 | 0.959 | 19.5 | 0.60 | 0 |
| `redis-noaof-sadd-c50-P16` | 3 | 1,355,565 | 8% | 0.62 | 0.26 | 0.36 | 3,646 | 1,831 | 0.551 | 0.711 | 0.775 | 12.3 | 0.76 | 0 |
| `valkey-noaof-sadd-c50-P16` | 3 | 1,158,950 | 5% | 0.75 | 0.56 | 0.19 | 5,415 | 2,369 | 0.655 | 0.823 | 0.887 | 12.8 | 0.64 | 0 |
| `base/rc-noaof-sadd-c1-P1` | 3 | 36,322 | 4% | 14.22 | 4.23 | 9.97 | 10,353 | 18,834 | 0.031 | 0.031 | 0.031 | 15.8 | 0.35 | 0 |
| `new/rc-noaof-sadd-c1-P1` | 3 | 38,054 | 8% | 13.11 | 3.55 | 9.50 | 7,809 | 15,552 | 0.023 | 0.031 | 0.031 | 17.4 | 0.37 | 0 |
| `redis-noaof-sadd-c1-P1` | 3 | 36,882 | 6% | 13.76 | 3.43 | 10.23 | 7,620 | 15,609 | 0.031 | 0.031 | 0.031 | 11.9 | 0.36 | 0 |
| `base/rc-noaof-sadd-c1-P16` | 3 | 385,115 | 7% | 1.67 | 0.98 | 0.69 | 6,427 | 4,289 | 0.039 | 0.039 | 0.047 | 15.7 | 0.27 | 0 |
| `new/rc-noaof-sadd-c1-P16` | 3 | 443,976 | 7% | 1.34 | 0.70 | 0.65 | 4,094 | 3,019 | 0.031 | 0.039 | 0.039 | 16.4 | 0.31 | 0 |
| `redis-noaof-sadd-c1-P16` | 3 | 463,258 | 9% | 1.24 | 0.58 | 0.67 | 3,776 | 2,518 | 0.031 | 0.039 | 0.039 | 12.0 | 0.32 | 0 |
| `base/rc-noaof-mset-c50-P1` | 3 | 89,991 | 2% | 8.69 | 4.69 | 4.01 | 29,404 | 22,796 | 0.567 | 0.607 | 0.647 | 26.1 | 0.81 | 0 |
| `new/rc-noaof-mset-c50-P1` | 3 | 102,834 | 3% | 7.82 | 4.38 | 3.44 | 29,774 | 21,057 | 0.471 | 0.615 | 0.655 | 27.4 | 0.75 | 0 |
| `redis-noaof-mset-c50-P1` | 3 | 124,104 | 6% | 6.31 | 3.16 | 3.14 | 20,326 | 15,120 | 0.359 | 0.519 | 0.599 | 19.8 | 0.85 | 0 |
| `valkey-noaof-mset-c50-P1` | 3 | 116,114 | 3% | 6.68 | 3.61 | 3.08 | 39,899 | 16,481 | 0.383 | 0.543 | 0.607 | 17.2 | 0.82 | 0 |
| `base/rc-noaof-mset-c50-P16` | 3 | 208,609 | 5% | 4.56 | 4.14 | 0.42 | 27,133 | 17,078 | 3.911 | 4.063 | 4.207 | 26.6 | 0.24 | 0 |
| `new/rc-noaof-mset-c50-P16` | 3 | 213,965 | 4% | 4.48 | 4.13 | 0.35 | 27,177 | 16,888 | 3.655 | 4.503 | 4.663 | 27.3 | 0.33 | 0 |
| `redis-noaof-mset-c50-P16` | 3 | 269,167 | 5% | 3.58 | 3.28 | 0.33 | 19,555 | 13,334 | 2.847 | 3.279 | 3.447 | 19.9 | 0.50 | 0 |
| `valkey-noaof-mset-c50-P16` | 3 | 247,036 | 3% | 3.88 | 3.57 | 0.31 | 38,005 | 14,565 | 3.055 | 3.495 | 3.615 | 16.7 | 0.50 | 0 |
| `base/rc-noaof-mset-c1-P1` | 3 | 31,266 | 4% | 17.63 | 7.50 | 10.17 | 31,263 | 32,432 | 0.031 | 0.031 | 0.039 | 24.8 | 0.31 | 0 |
| `new/rc-noaof-mset-c1-P1` | 3 | 31,266 | 4% | 17.52 | 7.56 | 10.44 | 31,017 | 32,075 | 0.031 | 0.031 | 0.039 | 26.0 | 0.32 | 0 |
| `redis-noaof-mset-c1-P1` | 3 | 31,266 | 9% | 17.28 | 6.33 | 10.94 | 23,556 | 29,338 | 0.031 | 0.031 | 0.039 | 19.5 | 0.32 | 0 |
| `base/rc-noaof-mset-c1-P16` | 3 | 157,202 | 8% | 5.00 | 4.28 | 0.74 | 27,312 | 17,282 | 0.095 | 0.103 | 0.127 | 23.7 | 0.16 | 0 |
| `new/rc-noaof-mset-c1-P16` | 3 | 159,819 | 9% | 4.88 | 4.13 | 0.75 | 27,293 | 16,866 | 0.095 | 0.095 | 0.119 | 24.4 | 0.17 | 0 |
| `redis-noaof-mset-c1-P16` | 3 | 188,029 | 9% | 3.90 | 3.11 | 0.78 | 19,752 | 12,895 | 0.079 | 0.079 | 0.111 | 19.4 | 0.20 | 0 |
| `base/rc-noaof-get-c50-P1` | 3 | 166,657 | 4% | 3.87 | 0.86 | 3.01 | 4,505 | 4,945 | 0.303 | 0.327 | 0.351 | 25.5 | 1.26 | 0 |
| `new/rc-noaof-get-c50-P1` | 3 | 184,604 | 2% | 3.55 | 0.82 | 2.72 | 4,854 | 4,599 | 0.255 | 0.351 | 0.383 | 25.8 | 1.25 | 0 |
| `redis-noaof-get-c50-P1` | 3 | 199,987 | 2% | 3.28 | 0.68 | 2.59 | 3,986 | 3,516 | 0.231 | 0.295 | 0.415 | 19.0 | 1.25 | 0 |
| `base/rc-noaof-incr-c50-P1` | 3 | 166,657 | 3% | 3.92 | 0.98 | 2.98 | 5,183 | 5,475 | 0.303 | 0.343 | 0.367 | 27.6 | 1.26 | 0 |
| `new/rc-noaof-incr-c50-P1` | 3 | 179,062 | 4% | 3.62 | 0.89 | 2.74 | 5,541 | 4,963 | 0.263 | 0.359 | 0.391 | 28.1 | 1.19 | 0 |
| `redis-noaof-incr-c50-P1` | 3 | 199,987 | 2% | 3.24 | 0.66 | 2.59 | 4,271 | 3,373 | 0.231 | 0.295 | 0.415 | 14.0 | 1.28 | 0 |
| `base/rc-noaof-zadd-c50-P1` | 3 | 128,562 | 4% | 5.44 | 1.93 | 3.51 | 10,686 | 10,236 | 0.391 | 0.431 | 0.455 | 18.1 | 1.06 | 0 |
| `new/rc-noaof-zadd-c50-P1` | 3 | 156,508 | 0% | 4.41 | 1.43 | 2.98 | 11,047 | 7,887 | 0.303 | 0.407 | 0.439 | 18.7 | 1.09 | 0 |
| `redis-noaof-zadd-c50-P1` | 3 | 149,962 | 6% | 4.83 | 1.78 | 3.06 | 8,015 | 9,501 | 0.295 | 0.415 | 0.535 | 18.5 | 0.96 | 0 |
| `base/rc-everysec-set-c50-P1` | 3 | 129,016 | 3% | 5.58 | 1.87 | 3.71 | 10,258 | 9,609 | 0.367 | 0.495 | 0.519 | 25.9 | 0.92 | 0 |
| `new/rc-everysec-set-c50-P1` | 3 | 131,857 | 2% | 5.48 | 1.79 | 3.67 | 9,911 | 9,267 | 0.359 | 0.479 | 0.511 | 26.6 | 0.94 | 0 |
| `redis-set-c50-P1` | 3 | 153,815 | 3% | 4.11 | 1.11 | 3.01 | 8,178 | 5,735 | 0.263 | 0.367 | 0.495 | 21.5 | 1.01 | 0 |
| `base/rc-everysec-incr-c50-P1` | 3 | 129,021 | 2% | 5.62 | 1.89 | 3.73 | 10,325 | 9,695 | 0.367 | 0.495 | 0.527 | 27.6 | 0.92 | 0 |
| `new/rc-everysec-incr-c50-P1` | 3 | 133,316 | 1% | 5.40 | 1.78 | 3.68 | 9,975 | 9,164 | 0.359 | 0.479 | 0.511 | 28.3 | 0.95 | 0 |
| `redis-incr-c50-P1` | 3 | 173,873 | 3% | 3.83 | 0.88 | 2.95 | 7,164 | 4,733 | 0.255 | 0.343 | 0.471 | 15.8 | 1.12 | 0 |
| `base/rc-everysec-hset-c50-P1` | 3 | 124,990 | 2% | 5.81 | 2.00 | 3.81 | 11,145 | 10,219 | 0.375 | 0.503 | 0.527 | 24.4 | 0.90 | 0 |
| `new/rc-everysec-hset-c50-P1` | 3 | 130,423 | 2% | 5.49 | 1.82 | 3.68 | 10,799 | 9,285 | 0.359 | 0.487 | 0.511 | 25.1 | 0.94 | 0 |
| `redis-hset-c50-P1` | 3 | 151,845 | 3% | 4.27 | 1.24 | 2.98 | 9,691 | 6,225 | 0.271 | 0.375 | 0.503 | 22.1 | 0.98 | 0 |
| `base/rc-everysec-set-c50-P16` | 3 | 525,762 | 7% | 1.29 | 0.95 | 0.34 | 6,378 | 4,003 | 1.127 | 1.407 | 1.519 | 26.3 | 0.32 | 0 |
| `new/rc-everysec-set-c50-P16` | 3 | 543,670 | 9% | 1.29 | 0.96 | 0.34 | 6,364 | 4,025 | 1.127 | 1.407 | 1.495 | 26.5 | 0.32 | 0 |
| `redis-set-c50-P16` | 3 | 478,446 | 7% | 1.26 | 0.96 | 0.30 | 7,338 | 3,941 | 0.991 | 1.279 | 4.367 | 30.5 | 0.30 | 0 |

## AOF (Recached INFO persistence deltas over the timed run)

| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |
|---|---:|---:|---:|---:|
| `base/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 6.4 | 0.0004 |
| `new/rc-everysec-set-c50-P1` | 0.1000 | 10.0 | 6.4 | 0.0004 |
| `base/rc-everysec-incr-c50-P1` | 0.1000 | 10.0 | 5.8 | 0.0004 |
| `new/rc-everysec-incr-c50-P1` | 0.1000 | 10.0 | 5.5 | 0.0004 |
| `base/rc-everysec-hset-c50-P1` | 0.1000 | 10.0 | 6.2 | 0.0005 |
| `new/rc-everysec-hset-c50-P1` | 0.1000 | 10.0 | 6.2 | 0.0005 |
| `base/rc-everysec-set-c50-P16` | 0.0062 | 160.6 | 88.3 | 0.0002 |
| `new/rc-everysec-set-c50-P16` | 0.0062 | 160.5 | 79.1 | 0.0002 |

## Ratios (throughput)

| A / B | median ratio | per-repetition ratios |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.827** | 0.849 0.827 0.827 |
| `base/rc-noaof-set-c50-P1` / `valkey-noaof-set-c50-P1` | **0.867** | 0.877 0.867 0.880 |
| `new/rc-noaof-set-c50-P1` / `redis-noaof-set-c50-P1` | **0.925** | 0.925 0.925 0.912 |
| `new/rc-noaof-set-c50-P1` / `valkey-noaof-set-c50-P1` | **0.970** | 0.955 0.970 0.970 |
| `base/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.015** | 1.030 0.986 1.059 |
| `base/rc-noaof-set-c50-P16` / `valkey-noaof-set-c50-P16` | **1.147** | 1.165 1.114 1.162 |
| `new/rc-noaof-set-c50-P16` / `redis-noaof-set-c50-P16` | **1.078** | 1.095 1.046 1.125 |
| `new/rc-noaof-set-c50-P16` / `valkey-noaof-set-c50-P16` | **1.219** | 1.238 1.182 1.235 |
| `base/rc-noaof-set-c1-P1` / `redis-noaof-set-c1-P1` | **1.082** | 1.082 1.082 1.094 |
| `new/rc-noaof-set-c1-P1` / `redis-noaof-set-c1-P1` | **1.082** | 1.082 1.082 1.094 |
| `base/rc-noaof-set-c1-P16` / `redis-noaof-set-c1-P16` | **1.102** | 1.102 1.119 1.127 |
| `new/rc-noaof-set-c1-P16` / `redis-noaof-set-c1-P16` | **1.118** | 1.118 1.103 1.127 |
| `base/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.690** | 0.698 0.690 0.693 |
| `base/rc-noaof-lpush-c50-P1` / `valkey-noaof-lpush-c50-P1` | **0.724** | 0.733 0.724 0.728 |
| `new/rc-noaof-lpush-c50-P1` / `redis-noaof-lpush-c50-P1` | **0.896** | 0.895 0.896 0.884 |
| `new/rc-noaof-lpush-c50-P1` / `valkey-noaof-lpush-c50-P1` | **0.940** | 0.940 0.940 0.928 |
| `base/rc-noaof-lpush-c50-P16` / `redis-noaof-lpush-c50-P16` | **0.652** | 0.652 0.652 0.642 |
| `base/rc-noaof-lpush-c50-P16` / `valkey-noaof-lpush-c50-P16` | **0.804** | 0.804 0.804 0.800 |
| `new/rc-noaof-lpush-c50-P16` / `redis-noaof-lpush-c50-P16` | **0.923** | 0.923 0.937 0.938 |
| `new/rc-noaof-lpush-c50-P16` / `valkey-noaof-lpush-c50-P16` | **1.139** | 1.139 1.156 1.169 |
| `base/rc-noaof-lpush-c1-P1` / `redis-noaof-lpush-c1-P1` | **1.031** | 1.031 1.015 1.015 |
| `new/rc-noaof-lpush-c1-P1` / `redis-noaof-lpush-c1-P1` | **1.081** | 1.081 1.065 1.062 |
| `base/rc-noaof-lpush-c1-P16` / `redis-noaof-lpush-c1-P16` | **0.923** | 0.923 0.935 0.938 |
| `new/rc-noaof-lpush-c1-P16` / `redis-noaof-lpush-c1-P16` | **1.059** | 1.059 1.059 1.056 |
| `base/rc-noaof-sadd-c50-P1` / `redis-noaof-sadd-c50-P1` | **0.659** | 0.690 0.649 0.642 |
| `base/rc-noaof-sadd-c50-P1` / `valkey-noaof-sadd-c50-P1` | **0.681** | 0.701 0.681 0.653 |
| `new/rc-noaof-sadd-c50-P1` / `redis-noaof-sadd-c50-P1` | **0.870** | 0.882 0.855 0.859 |
| `new/rc-noaof-sadd-c50-P1` / `valkey-noaof-sadd-c50-P1` | **0.899** | 0.897 0.899 0.873 |
| `base/rc-noaof-sadd-c50-P16` / `redis-noaof-sadd-c50-P16` | **0.567** | 0.578 0.558 0.583 |
| `base/rc-noaof-sadd-c50-P16` / `valkey-noaof-sadd-c50-P16` | **0.664** | 0.676 0.664 0.676 |
| `new/rc-noaof-sadd-c50-P16` / `redis-noaof-sadd-c50-P16` | **0.797** | 0.808 0.784 0.829 |
| `new/rc-noaof-sadd-c50-P16` / `valkey-noaof-sadd-c50-P16` | **0.933** | 0.945 0.933 0.961 |
| `base/rc-noaof-sadd-c1-P1` / `redis-noaof-sadd-c1-P1` | **0.985** | 0.985 0.985 1.000 |
| `new/rc-noaof-sadd-c1-P1` / `redis-noaof-sadd-c1-P1` | **1.032** | 1.048 1.032 1.030 |
| `base/rc-noaof-sadd-c1-P16` / `redis-noaof-sadd-c1-P16` | **0.831** | 0.841 0.819 0.852 |
| `new/rc-noaof-sadd-c1-P16` / `redis-noaof-sadd-c1-P16` | **0.958** | 0.958 0.958 0.987 |
| `base/rc-noaof-mset-c50-P1` / `redis-noaof-mset-c50-P1` | **0.725** | 0.707 0.725 0.775 |
| `base/rc-noaof-mset-c50-P1` / `valkey-noaof-mset-c50-P1` | **0.775** | 0.732 0.775 0.775 |
| `new/rc-noaof-mset-c50-P1` / `redis-noaof-mset-c50-P1` | **0.829** | 0.828 0.829 0.861 |
| `new/rc-noaof-mset-c50-P1` / `valkey-noaof-mset-c50-P1` | **0.886** | 0.857 0.886 0.861 |
| `base/rc-noaof-mset-c50-P16` / `redis-noaof-mset-c50-P16` | **0.775** | 0.775 0.782 0.784 |
| `base/rc-noaof-mset-c50-P16` / `valkey-noaof-mset-c50-P16` | **0.844** | 0.844 0.843 0.825 |
| `new/rc-noaof-mset-c50-P16` / `redis-noaof-mset-c50-P16` | **0.795** | 0.802 0.795 0.811 |
| `new/rc-noaof-mset-c50-P16` / `valkey-noaof-mset-c50-P16` | **0.866** | 0.874 0.857 0.854 |
| `base/rc-noaof-mset-c1-P1` / `redis-noaof-mset-c1-P1` | **1.000** | 0.957 1.000 1.000 |
| `new/rc-noaof-mset-c1-P1` / `redis-noaof-mset-c1-P1` | **1.000** | 0.957 1.000 1.000 |
| `base/rc-noaof-mset-c1-P16` / `redis-noaof-mset-c1-P16` | **0.836** | 0.836 0.833 0.846 |
| `new/rc-noaof-mset-c1-P16` / `redis-noaof-mset-c1-P16` | **0.850** | 0.864 0.833 0.846 |
| `base/rc-noaof-get-c50-P1` / `redis-noaof-get-c50-P1` | **0.833** | 0.833 0.870 0.847 |
| `new/rc-noaof-get-c50-P1` / `redis-noaof-get-c50-P1` | **0.923** | 0.923 0.923 0.924 |
| `base/rc-noaof-incr-c50-P1` / `redis-noaof-incr-c50-P1` | **0.833** | 0.820 0.833 0.811 |
| `new/rc-noaof-incr-c50-P1` / `redis-noaof-incr-c50-P1` | **0.895** | 0.894 0.895 0.870 |
| `base/rc-noaof-zadd-c50-P1` / `redis-noaof-zadd-c50-P1` | **0.857** | 0.873 0.839 0.877 |
| `new/rc-noaof-zadd-c50-P1` / `redis-noaof-zadd-c50-P1` | **1.044** | 1.044 1.022 1.087 |
| `base/rc-everysec-set-c50-P1` / `redis-set-c50-P1` | **0.839** | 0.817 0.848 0.821 |
| `new/rc-everysec-set-c50-P1` / `redis-set-c50-P1` | **0.857** | 0.835 0.867 0.848 |
| `base/rc-everysec-incr-c50-P1` / `redis-incr-c50-P1` | **0.742** | 0.742 0.739 0.745 |
| `new/rc-everysec-incr-c50-P1` / `redis-incr-c50-P1` | **0.767** | 0.767 0.756 0.769 |
| `base/rc-everysec-hset-c50-P1` / `redis-hset-c50-P1` | **0.823** | 0.813 0.832 0.825 |
| `new/rc-everysec-hset-c50-P1` / `redis-hset-c50-P1` | **0.859** | 0.848 0.868 0.860 |
| `base/rc-everysec-set-c50-P16` / `redis-set-c50-P16` | **1.099** | 1.152 1.009 1.099 |
| `new/rc-everysec-set-c50-P16` / `redis-set-c50-P16` | **1.136** | 1.136 0.996 1.168 |

## Host pressure at each repetition's start

| cell | rep | pressure |
|---|---:|---|
| `base/rc-noaof-set-c50-P1` | 1 | load=0.19 io_psi10=2.19/2.19 mem_psi10=0.36/0.36 swap_used_mib=7688 dirty_wb_kib=1264 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 2 | load=2.32 io_psi10=1.99/1.80 mem_psi10=0.00/0.00 swap_used_mib=7645 dirty_wb_kib=2988 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P1` | 3 | load=2.29 io_psi10=0.77/0.75 mem_psi10=0.00/0.00 swap_used_mib=7645 dirty_wb_kib=1020 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 1 | load=0.72 io_psi10=1.05/1.02 mem_psi10=0.10/0.10 swap_used_mib=7687 dirty_wb_kib=1672 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 2 | load=2.11 io_psi10=1.53/1.43 mem_psi10=0.00/0.00 swap_used_mib=7645 dirty_wb_kib=3140 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P1` | 3 | load=2.18 io_psi10=1.38/1.33 mem_psi10=0.00/0.00 swap_used_mib=7645 dirty_wb_kib=4584 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 1 | load=1.24 io_psi10=1.36/1.32 mem_psi10=0.01/0.01 swap_used_mib=7687 dirty_wb_kib=2208 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 2 | load=1.95 io_psi10=1.40/1.29 mem_psi10=0.00/0.00 swap_used_mib=7645 dirty_wb_kib=688 settle_wait_s=0 |
| `redis-noaof-set-c50-P1` | 3 | load=2.36 io_psi10=0.77/0.73 mem_psi10=0.00/0.00 swap_used_mib=7645 dirty_wb_kib=5276 settle_wait_s=0 |
| `valkey-noaof-set-c50-P1` | 1 | load=1.55 io_psi10=0.36/0.35 mem_psi10=0.00/0.00 swap_used_mib=7687 dirty_wb_kib=1744 settle_wait_s=0 |
| `valkey-noaof-set-c50-P1` | 2 | load=1.83 io_psi10=1.72/1.30 mem_psi10=0.00/0.00 swap_used_mib=7645 dirty_wb_kib=3836 settle_wait_s=0 |
| `valkey-noaof-set-c50-P1` | 3 | load=2.42 io_psi10=1.49/1.37 mem_psi10=0.00/0.00 swap_used_mib=7645 dirty_wb_kib=2636 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 1 | load=1.76 io_psi10=0.16/0.16 mem_psi10=0.00/0.00 swap_used_mib=7687 dirty_wb_kib=1944 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 2 | load=1.85 io_psi10=5.96/4.54 mem_psi10=0.05/0.00 swap_used_mib=7645 dirty_wb_kib=5696 settle_wait_s=0 |
| `base/rc-noaof-set-c50-P16` | 3 | load=2.63 io_psi10=1.53/1.47 mem_psi10=0.00/0.00 swap_used_mib=7645 dirty_wb_kib=2328 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 1 | load=1.75 io_psi10=0.46/0.46 mem_psi10=0.00/0.00 swap_used_mib=7687 dirty_wb_kib=1772 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 2 | load=1.71 io_psi10=1.36/1.32 mem_psi10=0.00/0.00 swap_used_mib=7652 dirty_wb_kib=1384 settle_wait_s=0 |
| `new/rc-noaof-set-c50-P16` | 3 | load=2.49 io_psi10=1.28/1.16 mem_psi10=0.00/0.00 swap_used_mib=7645 dirty_wb_kib=688 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 1 | load=1.81 io_psi10=0.31/0.31 mem_psi10=0.00/0.00 swap_used_mib=7687 dirty_wb_kib=2120 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 2 | load=1.99 io_psi10=0.87/0.84 mem_psi10=0.00/0.00 swap_used_mib=7652 dirty_wb_kib=2268 settle_wait_s=0 |
| `redis-noaof-set-c50-P16` | 3 | load=2.50 io_psi10=1.47/1.41 mem_psi10=0.00/0.00 swap_used_mib=7645 dirty_wb_kib=3872 settle_wait_s=0 |
| `valkey-noaof-set-c50-P16` | 1 | load=1.58 io_psi10=0.15/0.15 mem_psi10=0.00/0.00 swap_used_mib=7687 dirty_wb_kib=2648 settle_wait_s=0 |
| `valkey-noaof-set-c50-P16` | 2 | load=1.64 io_psi10=1.02/1.01 mem_psi10=0.00/0.00 swap_used_mib=7652 dirty_wb_kib=2976 settle_wait_s=0 |
| `valkey-noaof-set-c50-P16` | 3 | load=2.48 io_psi10=0.72/0.71 mem_psi10=0.00/0.00 swap_used_mib=7644 dirty_wb_kib=2304 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 1 | load=1.41 io_psi10=0.52/0.52 mem_psi10=0.00/0.00 swap_used_mib=7687 dirty_wb_kib=1724 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 2 | load=1.63 io_psi10=1.68/1.64 mem_psi10=0.00/0.00 swap_used_mib=7652 dirty_wb_kib=4220 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P1` | 3 | load=2.50 io_psi10=0.94/0.91 mem_psi10=0.00/0.00 swap_used_mib=7644 dirty_wb_kib=1348 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 1 | load=1.32 io_psi10=0.13/0.13 mem_psi10=0.00/0.00 swap_used_mib=7687 dirty_wb_kib=2444 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 2 | load=1.53 io_psi10=1.55/1.51 mem_psi10=0.00/0.00 swap_used_mib=7652 dirty_wb_kib=708 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P1` | 3 | load=2.14 io_psi10=0.98/0.94 mem_psi10=0.00/0.00 swap_used_mib=7644 dirty_wb_kib=2592 settle_wait_s=0 |
| `redis-noaof-set-c1-P1` | 1 | load=1.23 io_psi10=0.91/0.87 mem_psi10=0.00/0.00 swap_used_mib=7687 dirty_wb_kib=1876 settle_wait_s=0 |
| `redis-noaof-set-c1-P1` | 2 | load=1.59 io_psi10=0.67/0.67 mem_psi10=0.00/0.00 swap_used_mib=7652 dirty_wb_kib=2028 settle_wait_s=0 |
| `redis-noaof-set-c1-P1` | 3 | load=1.90 io_psi10=1.00/1.00 mem_psi10=0.00/0.00 swap_used_mib=7644 dirty_wb_kib=2728 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P16` | 1 | load=1.27 io_psi10=7.92/7.15 mem_psi10=0.00/0.00 swap_used_mib=7671 dirty_wb_kib=2580 settle_wait_s=7 |
| `base/rc-noaof-set-c1-P16` | 2 | load=1.82 io_psi10=1.41/1.41 mem_psi10=0.00/0.00 swap_used_mib=7652 dirty_wb_kib=4084 settle_wait_s=0 |
| `base/rc-noaof-set-c1-P16` | 3 | load=1.77 io_psi10=1.55/1.39 mem_psi10=0.00/0.00 swap_used_mib=7644 dirty_wb_kib=4048 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P16` | 1 | load=1.19 io_psi10=2.36/2.23 mem_psi10=0.00/0.00 swap_used_mib=7671 dirty_wb_kib=2548 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P16` | 2 | load=2.05 io_psi10=0.63/0.63 mem_psi10=0.00/0.00 swap_used_mib=7652 dirty_wb_kib=2344 settle_wait_s=0 |
| `new/rc-noaof-set-c1-P16` | 3 | load=1.54 io_psi10=2.15/2.07 mem_psi10=0.00/0.00 swap_used_mib=7644 dirty_wb_kib=4452 settle_wait_s=0 |
| `redis-noaof-set-c1-P16` | 1 | load=1.15 io_psi10=0.53/0.51 mem_psi10=0.00/0.00 swap_used_mib=7671 dirty_wb_kib=2052 settle_wait_s=0 |
| `redis-noaof-set-c1-P16` | 2 | load=2.29 io_psi10=0.51/0.47 mem_psi10=0.00/0.00 swap_used_mib=7652 dirty_wb_kib=3316 settle_wait_s=0 |
| `redis-noaof-set-c1-P16` | 3 | load=1.50 io_psi10=1.65/1.61 mem_psi10=0.00/0.00 swap_used_mib=7644 dirty_wb_kib=4876 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 1 | load=1.19 io_psi10=0.28/0.27 mem_psi10=0.00/0.00 swap_used_mib=7671 dirty_wb_kib=1788 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 2 | load=2.12 io_psi10=1.43/1.40 mem_psi10=0.00/0.00 swap_used_mib=7652 dirty_wb_kib=1228 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P1` | 3 | load=1.51 io_psi10=1.50/1.47 mem_psi10=0.00/0.00 swap_used_mib=7644 dirty_wb_kib=4908 settle_wait_s=0 |
| `new/rc-noaof-lpush-c50-P1` | 1 | load=1.56 io_psi10=0.12/0.12 mem_psi10=0.00/0.00 swap_used_mib=7671 dirty_wb_kib=1736 settle_wait_s=0 |
| `new/rc-noaof-lpush-c50-P1` | 2 | load=1.97 io_psi10=2.06/2.06 mem_psi10=0.00/0.00 swap_used_mib=7652 dirty_wb_kib=2080 settle_wait_s=0 |
| `new/rc-noaof-lpush-c50-P1` | 3 | load=2.07 io_psi10=0.61/0.52 mem_psi10=0.00/0.00 swap_used_mib=7644 dirty_wb_kib=3032 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 1 | load=1.83 io_psi10=2.18/2.18 mem_psi10=0.00/0.00 swap_used_mib=7670 dirty_wb_kib=3160 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 2 | load=1.99 io_psi10=1.44/1.44 mem_psi10=0.00/0.00 swap_used_mib=7652 dirty_wb_kib=2948 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P1` | 3 | load=2.41 io_psi10=1.18/1.11 mem_psi10=0.00/0.00 swap_used_mib=7644 dirty_wb_kib=2404 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P1` | 1 | load=1.78 io_psi10=0.69/0.69 mem_psi10=0.00/0.00 swap_used_mib=7670 dirty_wb_kib=4388 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P1` | 2 | load=1.70 io_psi10=2.29/2.27 mem_psi10=0.00/0.00 swap_used_mib=7652 dirty_wb_kib=4012 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P1` | 3 | load=2.38 io_psi10=1.45/1.40 mem_psi10=0.00/0.00 swap_used_mib=7644 dirty_wb_kib=2824 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P16` | 1 | load=2.02 io_psi10=1.56/1.38 mem_psi10=0.00/0.00 swap_used_mib=7670 dirty_wb_kib=3556 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P16` | 2 | load=1.55 io_psi10=1.12/1.11 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=1372 settle_wait_s=0 |
| `base/rc-noaof-lpush-c50-P16` | 3 | load=2.49 io_psi10=1.76/1.67 mem_psi10=0.00/0.00 swap_used_mib=7643 dirty_wb_kib=2520 settle_wait_s=0 |
| `new/rc-noaof-lpush-c50-P16` | 1 | load=2.01 io_psi10=0.46/0.44 mem_psi10=0.00/0.00 swap_used_mib=7670 dirty_wb_kib=1944 settle_wait_s=0 |
| `new/rc-noaof-lpush-c50-P16` | 2 | load=1.49 io_psi10=0.80/0.79 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=3240 settle_wait_s=0 |
| `new/rc-noaof-lpush-c50-P16` | 3 | load=2.31 io_psi10=1.99/1.94 mem_psi10=0.00/0.00 swap_used_mib=7641 dirty_wb_kib=4284 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P16` | 1 | load=1.96 io_psi10=0.07/0.07 mem_psi10=0.00/0.00 swap_used_mib=7670 dirty_wb_kib=2028 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P16` | 2 | load=1.46 io_psi10=1.54/1.44 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=3412 settle_wait_s=0 |
| `redis-noaof-lpush-c50-P16` | 3 | load=2.29 io_psi10=1.31/1.27 mem_psi10=0.00/0.00 swap_used_mib=7641 dirty_wb_kib=4580 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P16` | 1 | load=1.83 io_psi10=0.77/0.72 mem_psi10=0.00/0.00 swap_used_mib=7670 dirty_wb_kib=2164 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P16` | 2 | load=1.57 io_psi10=0.83/0.83 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=2508 settle_wait_s=0 |
| `valkey-noaof-lpush-c50-P16` | 3 | load=2.21 io_psi10=1.79/1.70 mem_psi10=0.00/0.00 swap_used_mib=7641 dirty_wb_kib=3608 settle_wait_s=0 |
| `base/rc-noaof-lpush-c1-P1` | 1 | load=1.73 io_psi10=0.19/0.18 mem_psi10=0.00/0.00 swap_used_mib=7669 dirty_wb_kib=2596 settle_wait_s=0 |
| `base/rc-noaof-lpush-c1-P1` | 2 | load=1.64 io_psi10=1.69/1.69 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=3136 settle_wait_s=0 |
| `base/rc-noaof-lpush-c1-P1` | 3 | load=2.15 io_psi10=0.91/0.77 mem_psi10=0.00/0.00 swap_used_mib=7641 dirty_wb_kib=5192 settle_wait_s=0 |
| `new/rc-noaof-lpush-c1-P1` | 1 | load=1.52 io_psi10=0.39/0.39 mem_psi10=0.00/0.00 swap_used_mib=7669 dirty_wb_kib=1920 settle_wait_s=0 |
| `new/rc-noaof-lpush-c1-P1` | 2 | load=1.52 io_psi10=0.82/0.82 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=1540 settle_wait_s=0 |
| `new/rc-noaof-lpush-c1-P1` | 3 | load=1.90 io_psi10=1.74/1.72 mem_psi10=0.00/0.00 swap_used_mib=7641 dirty_wb_kib=2016 settle_wait_s=0 |
| `redis-noaof-lpush-c1-P1` | 1 | load=1.40 io_psi10=0.12/0.12 mem_psi10=0.00/0.00 swap_used_mib=7669 dirty_wb_kib=2056 settle_wait_s=0 |
| `redis-noaof-lpush-c1-P1` | 2 | load=1.67 io_psi10=0.63/0.63 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=2732 settle_wait_s=0 |
| `redis-noaof-lpush-c1-P1` | 3 | load=1.78 io_psi10=1.05/1.01 mem_psi10=0.00/0.00 swap_used_mib=7640 dirty_wb_kib=2704 settle_wait_s=0 |
| `base/rc-noaof-lpush-c1-P16` | 1 | load=1.28 io_psi10=1.64/1.50 mem_psi10=0.00/0.00 swap_used_mib=7669 dirty_wb_kib=1904 settle_wait_s=0 |
| `base/rc-noaof-lpush-c1-P16` | 2 | load=1.93 io_psi10=1.62/1.59 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=1296 settle_wait_s=0 |
| `base/rc-noaof-lpush-c1-P16` | 3 | load=1.68 io_psi10=2.65/2.43 mem_psi10=0.00/0.00 swap_used_mib=7639 dirty_wb_kib=3444 settle_wait_s=0 |
| `new/rc-noaof-lpush-c1-P16` | 1 | load=1.13 io_psi10=0.27/0.22 mem_psi10=0.00/0.00 swap_used_mib=7669 dirty_wb_kib=2408 settle_wait_s=0 |
| `new/rc-noaof-lpush-c1-P16` | 2 | load=1.99 io_psi10=0.69/0.69 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=2392 settle_wait_s=0 |
| `new/rc-noaof-lpush-c1-P16` | 3 | load=1.59 io_psi10=0.94/0.91 mem_psi10=0.00/0.00 swap_used_mib=7639 dirty_wb_kib=2724 settle_wait_s=0 |
| `redis-noaof-lpush-c1-P16` | 1 | load=1.09 io_psi10=0.48/0.47 mem_psi10=0.00/0.00 swap_used_mib=7669 dirty_wb_kib=2068 settle_wait_s=0 |
| `redis-noaof-lpush-c1-P16` | 2 | load=2.28 io_psi10=0.65/0.63 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=1876 settle_wait_s=0 |
| `redis-noaof-lpush-c1-P16` | 3 | load=1.49 io_psi10=1.51/1.43 mem_psi10=0.00/0.00 swap_used_mib=7639 dirty_wb_kib=4692 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 1 | load=1.22 io_psi10=0.38/0.38 mem_psi10=0.00/0.00 swap_used_mib=7669 dirty_wb_kib=2260 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 2 | load=2.12 io_psi10=0.51/0.51 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=2932 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P1` | 3 | load=1.35 io_psi10=2.33/2.00 mem_psi10=0.00/0.00 swap_used_mib=7639 dirty_wb_kib=2320 settle_wait_s=0 |
| `new/rc-noaof-sadd-c50-P1` | 1 | load=1.58 io_psi10=0.33/0.33 mem_psi10=0.00/0.00 swap_used_mib=7669 dirty_wb_kib=2328 settle_wait_s=0 |
| `new/rc-noaof-sadd-c50-P1` | 2 | load=2.09 io_psi10=2.92/2.88 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=3884 settle_wait_s=0 |
| `new/rc-noaof-sadd-c50-P1` | 3 | load=1.65 io_psi10=1.85/1.81 mem_psi10=0.00/0.00 swap_used_mib=7639 dirty_wb_kib=504 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 1 | load=1.77 io_psi10=0.62/0.62 mem_psi10=0.00/0.00 swap_used_mib=7669 dirty_wb_kib=2144 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 2 | load=1.83 io_psi10=0.43/0.43 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=2124 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P1` | 3 | load=1.87 io_psi10=2.39/2.35 mem_psi10=0.00/0.00 swap_used_mib=7639 dirty_wb_kib=2820 settle_wait_s=0 |
| `valkey-noaof-sadd-c50-P1` | 1 | load=1.97 io_psi10=0.79/0.79 mem_psi10=0.00/0.00 swap_used_mib=7669 dirty_wb_kib=2592 settle_wait_s=0 |
| `valkey-noaof-sadd-c50-P1` | 2 | load=1.79 io_psi10=0.40/0.40 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=3144 settle_wait_s=0 |
| `valkey-noaof-sadd-c50-P1` | 3 | load=1.92 io_psi10=2.53/2.10 mem_psi10=0.00/0.00 swap_used_mib=7639 dirty_wb_kib=2328 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P16` | 1 | load=2.10 io_psi10=0.20/0.20 mem_psi10=0.00/0.00 swap_used_mib=7666 dirty_wb_kib=2944 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P16` | 2 | load=1.90 io_psi10=1.89/1.89 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=1240 settle_wait_s=0 |
| `base/rc-noaof-sadd-c50-P16` | 3 | load=2.18 io_psi10=2.37/2.23 mem_psi10=0.00/0.00 swap_used_mib=7638 dirty_wb_kib=4516 settle_wait_s=0 |
| `new/rc-noaof-sadd-c50-P16` | 1 | load=1.94 io_psi10=0.48/0.48 mem_psi10=0.00/0.00 swap_used_mib=7666 dirty_wb_kib=2420 settle_wait_s=0 |
| `new/rc-noaof-sadd-c50-P16` | 2 | load=1.76 io_psi10=1.02/1.01 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=2712 settle_wait_s=0 |
| `new/rc-noaof-sadd-c50-P16` | 3 | load=1.89 io_psi10=0.97/0.92 mem_psi10=0.00/0.00 swap_used_mib=7638 dirty_wb_kib=4768 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P16` | 1 | load=1.90 io_psi10=3.23/3.23 mem_psi10=0.00/0.00 swap_used_mib=7666 dirty_wb_kib=2892 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P16` | 2 | load=1.66 io_psi10=1.36/1.33 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=4396 settle_wait_s=0 |
| `redis-noaof-sadd-c50-P16` | 3 | load=1.79 io_psi10=2.28/2.25 mem_psi10=0.00/0.00 swap_used_mib=7638 dirty_wb_kib=2936 settle_wait_s=0 |
| `valkey-noaof-sadd-c50-P16` | 1 | load=2.14 io_psi10=0.69/0.69 mem_psi10=0.00/0.00 swap_used_mib=7666 dirty_wb_kib=1980 settle_wait_s=0 |
| `valkey-noaof-sadd-c50-P16` | 2 | load=1.36 io_psi10=3.63/3.63 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=1912 settle_wait_s=0 |
| `valkey-noaof-sadd-c50-P16` | 3 | load=1.76 io_psi10=0.59/0.58 mem_psi10=0.00/0.00 swap_used_mib=7638 dirty_wb_kib=2220 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P1` | 1 | load=2.08 io_psi10=0.29/0.29 mem_psi10=0.00/0.00 swap_used_mib=7666 dirty_wb_kib=3304 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P1` | 2 | load=1.36 io_psi10=2.46/2.46 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=2640 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P1` | 3 | load=1.99 io_psi10=1.47/1.44 mem_psi10=0.00/0.00 swap_used_mib=7638 dirty_wb_kib=2968 settle_wait_s=0 |
| `new/rc-noaof-sadd-c1-P1` | 1 | load=1.83 io_psi10=0.41/0.41 mem_psi10=0.00/0.00 swap_used_mib=7664 dirty_wb_kib=2784 settle_wait_s=0 |
| `new/rc-noaof-sadd-c1-P1` | 2 | load=1.20 io_psi10=2.36/2.36 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=1924 settle_wait_s=0 |
| `new/rc-noaof-sadd-c1-P1` | 3 | load=1.78 io_psi10=1.91/1.87 mem_psi10=0.00/0.00 swap_used_mib=7638 dirty_wb_kib=4908 settle_wait_s=0 |
| `redis-noaof-sadd-c1-P1` | 1 | load=1.65 io_psi10=2.02/1.84 mem_psi10=0.00/0.00 swap_used_mib=7664 dirty_wb_kib=2568 settle_wait_s=0 |
| `redis-noaof-sadd-c1-P1` | 2 | load=1.26 io_psi10=2.91/2.91 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=2632 settle_wait_s=0 |
| `redis-noaof-sadd-c1-P1` | 3 | load=1.92 io_psi10=2.66/2.45 mem_psi10=0.00/0.00 swap_used_mib=7638 dirty_wb_kib=2980 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P16` | 1 | load=1.58 io_psi10=0.63/0.59 mem_psi10=0.00/0.00 swap_used_mib=7663 dirty_wb_kib=2936 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P16` | 2 | load=1.49 io_psi10=2.54/2.54 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=2652 settle_wait_s=0 |
| `base/rc-noaof-sadd-c1-P16` | 3 | load=1.66 io_psi10=2.39/2.30 mem_psi10=0.00/0.00 swap_used_mib=7638 dirty_wb_kib=4328 settle_wait_s=0 |
| `new/rc-noaof-sadd-c1-P16` | 1 | load=1.45 io_psi10=0.42/0.42 mem_psi10=0.00/0.00 swap_used_mib=7663 dirty_wb_kib=2140 settle_wait_s=0 |
| `new/rc-noaof-sadd-c1-P16` | 2 | load=1.69 io_psi10=0.97/0.95 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=2992 settle_wait_s=0 |
| `new/rc-noaof-sadd-c1-P16` | 3 | load=1.50 io_psi10=0.74/0.69 mem_psi10=0.00/0.00 swap_used_mib=7638 dirty_wb_kib=2844 settle_wait_s=0 |
| `redis-noaof-sadd-c1-P16` | 1 | load=1.39 io_psi10=0.05/0.05 mem_psi10=0.00/0.00 swap_used_mib=7663 dirty_wb_kib=2272 settle_wait_s=0 |
| `redis-noaof-sadd-c1-P16` | 2 | load=1.88 io_psi10=2.52/2.42 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=2412 settle_wait_s=0 |
| `redis-noaof-sadd-c1-P16` | 3 | load=1.63 io_psi10=3.63/3.39 mem_psi10=0.00/0.00 swap_used_mib=7638 dirty_wb_kib=2584 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 1 | load=1.31 io_psi10=0.59/0.55 mem_psi10=0.00/0.00 swap_used_mib=7663 dirty_wb_kib=2640 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 2 | load=1.76 io_psi10=3.57/3.46 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=3320 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P1` | 3 | load=1.45 io_psi10=3.56/3.46 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=4272 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P1` | 1 | load=1.32 io_psi10=0.25/0.24 mem_psi10=0.00/0.00 swap_used_mib=7663 dirty_wb_kib=2468 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P1` | 2 | load=1.54 io_psi10=2.19/2.11 mem_psi10=0.00/0.00 swap_used_mib=7653 dirty_wb_kib=1456 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P1` | 3 | load=1.57 io_psi10=1.74/1.64 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=3244 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 1 | load=1.51 io_psi10=1.75/1.75 mem_psi10=0.00/0.00 swap_used_mib=7663 dirty_wb_kib=2660 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 2 | load=1.45 io_psi10=1.35/1.35 mem_psi10=0.00/0.00 swap_used_mib=7654 dirty_wb_kib=2712 settle_wait_s=0 |
| `redis-noaof-mset-c50-P1` | 3 | load=1.65 io_psi10=3.22/3.12 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=2320 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P1` | 1 | load=1.52 io_psi10=2.68/2.60 mem_psi10=0.00/0.00 swap_used_mib=7662 dirty_wb_kib=2404 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P1` | 2 | load=1.53 io_psi10=0.26/0.25 mem_psi10=0.00/0.00 swap_used_mib=7654 dirty_wb_kib=3508 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P1` | 3 | load=1.63 io_psi10=3.67/3.64 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=2008 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 1 | load=1.58 io_psi10=2.23/2.20 mem_psi10=0.00/0.00 swap_used_mib=7662 dirty_wb_kib=2296 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 2 | load=1.53 io_psi10=0.87/0.87 mem_psi10=0.00/0.00 swap_used_mib=7654 dirty_wb_kib=2244 settle_wait_s=0 |
| `base/rc-noaof-mset-c50-P16` | 3 | load=1.53 io_psi10=4.20/4.02 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=1504 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P16` | 1 | load=1.41 io_psi10=0.22/0.22 mem_psi10=0.00/0.00 swap_used_mib=7662 dirty_wb_kib=2808 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P16` | 2 | load=1.47 io_psi10=0.87/0.87 mem_psi10=0.00/0.00 swap_used_mib=7654 dirty_wb_kib=3080 settle_wait_s=0 |
| `new/rc-noaof-mset-c50-P16` | 3 | load=1.58 io_psi10=0.27/0.27 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=2384 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 1 | load=1.60 io_psi10=1.14/1.14 mem_psi10=0.00/0.00 swap_used_mib=7662 dirty_wb_kib=3172 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 2 | load=1.31 io_psi10=1.22/1.22 mem_psi10=0.00/0.00 swap_used_mib=7654 dirty_wb_kib=1324 settle_wait_s=0 |
| `redis-noaof-mset-c50-P16` | 3 | load=1.48 io_psi10=1.90/1.70 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=3080 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P16` | 1 | load=1.39 io_psi10=0.32/0.32 mem_psi10=0.00/0.00 swap_used_mib=7662 dirty_wb_kib=3336 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P16` | 2 | load=1.48 io_psi10=1.71/1.66 mem_psi10=0.00/0.00 swap_used_mib=7654 dirty_wb_kib=1984 settle_wait_s=0 |
| `valkey-noaof-mset-c50-P16` | 3 | load=1.65 io_psi10=1.17/1.13 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=2536 settle_wait_s=0 |
| `base/rc-noaof-mset-c1-P1` | 1 | load=1.79 io_psi10=3.48/3.48 mem_psi10=0.00/0.00 swap_used_mib=7662 dirty_wb_kib=2280 settle_wait_s=0 |
| `base/rc-noaof-mset-c1-P1` | 2 | load=1.65 io_psi10=2.23/2.14 mem_psi10=0.00/0.00 swap_used_mib=7654 dirty_wb_kib=3556 settle_wait_s=0 |
| `base/rc-noaof-mset-c1-P1` | 3 | load=1.53 io_psi10=0.71/0.67 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=3848 settle_wait_s=0 |
| `new/rc-noaof-mset-c1-P1` | 1 | load=1.81 io_psi10=2.88/2.88 mem_psi10=0.00/0.00 swap_used_mib=7660 dirty_wb_kib=1240 settle_wait_s=0 |
| `new/rc-noaof-mset-c1-P1` | 2 | load=1.71 io_psi10=1.73/1.52 mem_psi10=0.00/0.00 swap_used_mib=7654 dirty_wb_kib=1232 settle_wait_s=0 |
| `new/rc-noaof-mset-c1-P1` | 3 | load=1.49 io_psi10=3.58/3.20 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=820 settle_wait_s=0 |
| `redis-noaof-mset-c1-P1` | 1 | load=1.76 io_psi10=2.41/2.41 mem_psi10=0.00/0.00 swap_used_mib=7660 dirty_wb_kib=2196 settle_wait_s=0 |
| `redis-noaof-mset-c1-P1` | 2 | load=1.75 io_psi10=1.19/1.11 mem_psi10=0.00/0.00 swap_used_mib=7654 dirty_wb_kib=2228 settle_wait_s=0 |
| `redis-noaof-mset-c1-P1` | 3 | load=1.49 io_psi10=3.34/3.17 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=2200 settle_wait_s=0 |
| `base/rc-noaof-mset-c1-P16` | 1 | load=1.69 io_psi10=1.44/1.44 mem_psi10=0.00/0.00 swap_used_mib=7660 dirty_wb_kib=2136 settle_wait_s=0 |
| `base/rc-noaof-mset-c1-P16` | 2 | load=1.87 io_psi10=1.37/1.36 mem_psi10=0.00/0.00 swap_used_mib=7654 dirty_wb_kib=2804 settle_wait_s=0 |
| `base/rc-noaof-mset-c1-P16` | 3 | load=1.41 io_psi10=4.83/4.76 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=2016 settle_wait_s=0 |
| `new/rc-noaof-mset-c1-P16` | 1 | load=1.50 io_psi10=1.16/1.16 mem_psi10=0.00/0.00 swap_used_mib=7660 dirty_wb_kib=2548 settle_wait_s=0 |
| `new/rc-noaof-mset-c1-P16` | 2 | load=2.22 io_psi10=2.26/2.25 mem_psi10=0.00/0.00 swap_used_mib=7654 dirty_wb_kib=2520 settle_wait_s=0 |
| `new/rc-noaof-mset-c1-P16` | 3 | load=1.32 io_psi10=1.16/1.15 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=2084 settle_wait_s=0 |
| `redis-noaof-mset-c1-P16` | 1 | load=1.39 io_psi10=0.54/0.54 mem_psi10=0.00/0.00 swap_used_mib=7660 dirty_wb_kib=2040 settle_wait_s=0 |
| `redis-noaof-mset-c1-P16` | 2 | load=2.57 io_psi10=1.45/1.38 mem_psi10=0.00/0.00 swap_used_mib=7654 dirty_wb_kib=4216 settle_wait_s=0 |
| `redis-noaof-mset-c1-P16` | 3 | load=1.43 io_psi10=1.46/1.42 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=2076 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 1 | load=1.30 io_psi10=1.60/1.60 mem_psi10=0.00/0.00 swap_used_mib=7659 dirty_wb_kib=1820 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 2 | load=2.57 io_psi10=1.24/1.24 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=5832 settle_wait_s=0 |
| `base/rc-noaof-get-c50-P1` | 3 | load=1.49 io_psi10=1.67/1.62 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=2728 settle_wait_s=0 |
| `new/rc-noaof-get-c50-P1` | 1 | load=1.65 io_psi10=1.72/1.72 mem_psi10=0.00/0.00 swap_used_mib=7659 dirty_wb_kib=2256 settle_wait_s=0 |
| `new/rc-noaof-get-c50-P1` | 2 | load=2.45 io_psi10=1.37/1.37 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=2888 settle_wait_s=0 |
| `new/rc-noaof-get-c50-P1` | 3 | load=1.79 io_psi10=1.29/1.25 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=2144 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 1 | load=2.04 io_psi10=1.05/1.05 mem_psi10=0.00/0.00 swap_used_mib=7659 dirty_wb_kib=2564 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 2 | load=2.50 io_psi10=0.15/0.14 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=2276 settle_wait_s=0 |
| `redis-noaof-get-c50-P1` | 3 | load=2.01 io_psi10=3.17/3.00 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=2260 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 1 | load=2.25 io_psi10=0.34/0.34 mem_psi10=0.00/0.00 swap_used_mib=7659 dirty_wb_kib=3188 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 2 | load=2.36 io_psi10=0.89/0.81 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=2492 settle_wait_s=0 |
| `base/rc-noaof-incr-c50-P1` | 3 | load=2.16 io_psi10=2.27/2.04 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=3960 settle_wait_s=0 |
| `new/rc-noaof-incr-c50-P1` | 1 | load=2.32 io_psi10=0.92/0.92 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=2252 settle_wait_s=0 |
| `new/rc-noaof-incr-c50-P1` | 2 | load=2.43 io_psi10=1.59/1.57 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=1132 settle_wait_s=0 |
| `new/rc-noaof-incr-c50-P1` | 3 | load=2.35 io_psi10=2.64/2.38 mem_psi10=0.00/0.00 swap_used_mib=7637 dirty_wb_kib=2944 settle_wait_s=0 |
| `redis-noaof-incr-c50-P1` | 1 | load=2.37 io_psi10=0.60/0.60 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=2592 settle_wait_s=0 |
| `redis-noaof-incr-c50-P1` | 2 | load=2.38 io_psi10=3.26/3.16 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=3244 settle_wait_s=0 |
| `redis-noaof-incr-c50-P1` | 3 | load=2.34 io_psi10=1.42/1.34 mem_psi10=0.00/0.00 swap_used_mib=7636 dirty_wb_kib=2908 settle_wait_s=0 |
| `base/rc-noaof-zadd-c50-P1` | 1 | load=2.43 io_psi10=1.39/1.17 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=3524 settle_wait_s=0 |
| `base/rc-noaof-zadd-c50-P1` | 2 | load=2.37 io_psi10=2.05/1.55 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=3104 settle_wait_s=0 |
| `base/rc-noaof-zadd-c50-P1` | 3 | load=2.55 io_psi10=1.65/1.59 mem_psi10=0.00/0.00 swap_used_mib=7636 dirty_wb_kib=2712 settle_wait_s=0 |
| `new/rc-noaof-zadd-c50-P1` | 1 | load=2.49 io_psi10=3.08/3.04 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=3056 settle_wait_s=0 |
| `new/rc-noaof-zadd-c50-P1` | 2 | load=2.26 io_psi10=3.59/2.22 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=1840 settle_wait_s=0 |
| `new/rc-noaof-zadd-c50-P1` | 3 | load=2.72 io_psi10=2.05/1.95 mem_psi10=0.00/0.00 swap_used_mib=7636 dirty_wb_kib=2224 settle_wait_s=0 |
| `redis-noaof-zadd-c50-P1` | 1 | load=2.45 io_psi10=1.32/1.31 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=2240 settle_wait_s=0 |
| `redis-noaof-zadd-c50-P1` | 2 | load=2.22 io_psi10=4.95/0.51 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=3060 settle_wait_s=0 |
| `redis-noaof-zadd-c50-P1` | 3 | load=2.56 io_psi10=2.47/2.33 mem_psi10=0.00/0.00 swap_used_mib=7636 dirty_wb_kib=1972 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 1 | load=2.57 io_psi10=1.32/1.27 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=2176 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 2 | load=1.93 io_psi10=4.25/0.77 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=3220 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P1` | 3 | load=3.03 io_psi10=2.60/2.51 mem_psi10=0.00/0.00 swap_used_mib=7636 dirty_wb_kib=2888 settle_wait_s=0 |
| `new/rc-everysec-set-c50-P1` | 1 | load=2.45 io_psi10=5.01/0.64 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=1932 settle_wait_s=0 |
| `new/rc-everysec-set-c50-P1` | 2 | load=1.99 io_psi10=6.91/4.50 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=3152 settle_wait_s=7 |
| `new/rc-everysec-set-c50-P1` | 3 | load=3.12 io_psi10=5.36/1.21 mem_psi10=0.00/0.00 swap_used_mib=7634 dirty_wb_kib=5080 settle_wait_s=0 |
| `redis-set-c50-P1` | 1 | load=2.52 io_psi10=5.33/0.98 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=3464 settle_wait_s=0 |
| `redis-set-c50-P1` | 2 | load=2.25 io_psi10=5.24/0.37 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=3224 settle_wait_s=0 |
| `redis-set-c50-P1` | 3 | load=3.18 io_psi10=6.63/1.71 mem_psi10=0.00/0.00 swap_used_mib=7634 dirty_wb_kib=2304 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 1 | load=2.38 io_psi10=6.95/3.62 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=3176 settle_wait_s=6 |
| `base/rc-everysec-incr-c50-P1` | 2 | load=2.34 io_psi10=5.32/0.80 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=3588 settle_wait_s=0 |
| `base/rc-everysec-incr-c50-P1` | 3 | load=2.68 io_psi10=7.20/4.29 mem_psi10=0.00/0.00 swap_used_mib=7634 dirty_wb_kib=5096 settle_wait_s=8 |
| `new/rc-everysec-incr-c50-P1` | 1 | load=2.34 io_psi10=5.88/1.11 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=4156 settle_wait_s=0 |
| `new/rc-everysec-incr-c50-P1` | 2 | load=2.11 io_psi10=6.55/4.33 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=3344 settle_wait_s=6 |
| `new/rc-everysec-incr-c50-P1` | 3 | load=2.60 io_psi10=6.25/1.24 mem_psi10=0.00/0.00 swap_used_mib=7634 dirty_wb_kib=2772 settle_wait_s=0 |
| `redis-incr-c50-P1` | 1 | load=2.26 io_psi10=5.60/0.48 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=3860 settle_wait_s=0 |
| `redis-incr-c50-P1` | 2 | load=2.19 io_psi10=6.28/1.06 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=2500 settle_wait_s=0 |
| `redis-incr-c50-P1` | 3 | load=2.82 io_psi10=6.13/1.13 mem_psi10=0.00/0.00 swap_used_mib=7634 dirty_wb_kib=3356 settle_wait_s=0 |
| `base/rc-everysec-hset-c50-P1` | 1 | load=2.25 io_psi10=6.54/1.30 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=3960 settle_wait_s=0 |
| `base/rc-everysec-hset-c50-P1` | 2 | load=1.94 io_psi10=6.05/0.58 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=3044 settle_wait_s=0 |
| `base/rc-everysec-hset-c50-P1` | 3 | load=2.73 io_psi10=7.73/3.04 mem_psi10=0.00/0.00 swap_used_mib=7634 dirty_wb_kib=5628 settle_wait_s=0 |
| `new/rc-everysec-hset-c50-P1` | 1 | load=2.10 io_psi10=5.62/0.72 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=4044 settle_wait_s=0 |
| `new/rc-everysec-hset-c50-P1` | 2 | load=1.84 io_psi10=7.20/3.59 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=6040 settle_wait_s=5 |
| `new/rc-everysec-hset-c50-P1` | 3 | load=3.09 io_psi10=7.22/2.98 mem_psi10=0.00/0.00 swap_used_mib=7634 dirty_wb_kib=3280 settle_wait_s=4 |
| `redis-hset-c50-P1` | 1 | load=2.19 io_psi10=5.64/1.11 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=4336 settle_wait_s=0 |
| `redis-hset-c50-P1` | 2 | load=1.47 io_psi10=7.90/5.57 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=8536 settle_wait_s=15 |
| `redis-hset-c50-P1` | 3 | load=2.96 io_psi10=6.93/1.08 mem_psi10=0.00/0.00 swap_used_mib=7633 dirty_wb_kib=5444 settle_wait_s=0 |
| `base/rc-everysec-set-c50-P16` | 1 | load=2.36 io_psi10=7.54/4.06 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=3396 settle_wait_s=6 |
| `base/rc-everysec-set-c50-P16` | 2 | load=1.75 io_psi10=7.76/5.63 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=6396 settle_wait_s=15 |
| `base/rc-everysec-set-c50-P16` | 3 | load=2.48 io_psi10=7.31/4.38 mem_psi10=0.00/0.00 swap_used_mib=7633 dirty_wb_kib=3056 settle_wait_s=9 |
| `new/rc-everysec-set-c50-P16` | 1 | load=2.05 io_psi10=7.33/5.15 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=7084 settle_wait_s=13 |
| `new/rc-everysec-set-c50-P16` | 2 | load=1.91 io_psi10=6.77/3.59 mem_psi10=0.00/0.00 swap_used_mib=7655 dirty_wb_kib=3344 settle_wait_s=18 |
| `new/rc-everysec-set-c50-P16` | 3 | load=2.58 io_psi10=6.72/5.09 mem_psi10=0.00/0.00 swap_used_mib=7633 dirty_wb_kib=7676 settle_wait_s=18 |
| `redis-set-c50-P16` | 1 | load=1.71 io_psi10=6.70/4.74 mem_psi10=0.00/0.00 swap_used_mib=7658 dirty_wb_kib=7716 settle_wait_s=15 |
| `redis-set-c50-P16` | 2 | load=2.00 io_psi10=6.65/3.65 mem_psi10=0.00/0.00 swap_used_mib=7656 dirty_wb_kib=4168 settle_wait_s=18 |
| `redis-set-c50-P16` | 3 | load=1.99 io_psi10=7.92/5.95 mem_psi10=0.00/0.00 swap_used_mib=7633 dirty_wb_kib=7532 settle_wait_s=15 |
