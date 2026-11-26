| cell | reps | tx/s base / i / p3 | ratio to base | CPU µs/tx | p99 µs | max µs | RSS MiB |
|---|---|---|---|---|---|---|---|
| set-set aof=everysec c50 | 5/5/5 | 90,533 / 82,490 / 85,027 | 0.911 / 0.939 | 8.15 → 8.84 → 8.47 | 751 → 801 → 764 | 99,333 → 156,822 → 154,959 | 26 → 26 → 27 |

| AOF cell | AOF bytes/tx | write(2) calls/tx | µs in write(2) per tx | fsyncs per run | µs per fsync |
|---|---|---|---|---|---|
| set-set aof=everysec c50 | 162 → 278 → 277 | 0.100 → 0.100 → 0.100 | 0.96 → 1.52 → 1.44 | 23 → 23 → 23 | 100,843 → 117,824 → 120,524 |

paired ratios per rep (each label / base), and the host's foreign CPUs:
  set-set aof=everysec c50: i: tx/s 0.869 0.913 0.886 0.910 0.923 | CPU/tx median 1.087 [1.074-1.110]; p3: tx/s 0.934 0.928 0.956 0.946 0.919 | CPU/tx median 1.038 [1.030-1.049]; foreign CPUs median 0.84
