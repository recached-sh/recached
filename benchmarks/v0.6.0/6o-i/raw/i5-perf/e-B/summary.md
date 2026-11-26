| cell | reps | tx/s base / i / p3 | ratio to base | CPU µs/tx | p99 µs | max µs | RSS MiB |
|---|---|---|---|---|---|---|---|
| set-set aof=everysec,REPLICA=1 c50 | 5/5/5 | 48,310 / 44,672 / 45,614 | 0.925 / 0.944 | 17.14 → 18.08 → 17.94 | 1,125 → 1,192 → 1,182 | 26,004 → 465,723 → 228,103 | 59 → 71 → 62 |
| setn TX_N=100 aof=everysec,REPLICA=1 c20 | 5/5/5 | 4,826 / 4,865 / 4,884 | 1.008 / 1.012 | 134.47 → 140.36 → 137.09 | 3,199 → 3,339 → 3,253 | 867,875 → 470,853 → 892,168 | 58 → 71 → 58 |
| set-set REPLICA=1 c50 | 5/5/5 | 83,832 / 82,375 / 83,964 | 0.983 / 1.002 | 9.26 → 9.40 → 9.16 | 646 → 661 → 644 | 2,853 → 1,616 → 2,242 | 60 → 71 → 63 |
| setn TX_N=100 REPLICA=1 c20 | 5/5/5 | 8,347 / 8,478 / 8,479 | 1.016 / 1.016 | 115.61 → 113.64 → 113.62 | 2,622 → 2,547 → 2,550 | 3,223 → 4,548 → 3,174 | 58 → 71 → 59 |

| replica cell | replica CPU µs/tx | lag frames at end | full syncs | queue disconnects |
|---|---|---|---|---|
| set-set aof=everysec,REPLICA=1 c50 | 2.03 → 2.26 → 2.26 | 0 → 0 → 0 | 1 → 1 → 1 | 0 → 0 → 0 |
| setn TX_N=100 aof=everysec,REPLICA=1 c20 | 82.65 → 73.00 → 73.15 | 0 → 0 → 0 | 1 → 1 → 1 | 0 → 0 → 0 |
| set-set REPLICA=1 c50 | 1.98 → 2.20 → 2.20 | 0 → 0 → 0 | 1 → 1 → 1 | 0 → 0 → 0 |
| setn TX_N=100 REPLICA=1 c20 | 81.02 → 71.82 → 71.85 | 0 → 0 → 0 | 1 → 1 → 1 | 0 → 0 → 0 |

| AOF cell | AOF bytes/tx | write(2) calls/tx | µs in write(2) per tx | fsyncs per run | µs per fsync |
|---|---|---|---|---|---|
| set-set aof=everysec,REPLICA=1 c50 | 173 → 289 → 289 | 1.310 → 1.336 → 1.329 | 5.31 → 6.33 → 6.13 | 25 → 25 → 25 | 178,904 → 243,001 → 227,642 |
| setn TX_N=100 aof=everysec,REPLICA=1 c20 | 7733 → 7845 → 7844 | 4.108 → 4.083 → 4.071 | 87.02 → 78.89 → 85.48 | 26 → 26 → 26 | 466,085 → 457,621 → 483,009 |

| replica cell | replica RSS MiB |
|---|---|
| set-set aof=everysec,REPLICA=1 c50 | 24 → 24 → 24 |
| setn TX_N=100 aof=everysec,REPLICA=1 c20 | 24 → 24 → 24 |
| set-set REPLICA=1 c50 | 24 → 24 → 24 |
| setn TX_N=100 REPLICA=1 c20 | 24 → 24 → 24 |

paired ratios per rep (each label / base), and the host's foreign CPUs:
  set-set aof=everysec,REPLICA=1 c50: i: tx/s 0.968 0.913 0.941 0.888 0.917 | CPU/tx median 1.058 [1.053-1.072]; p3: tx/s 0.979 0.958 0.945 0.878 0.939 | CPU/tx median 1.039 [1.033-1.088]; foreign CPUs median 0.68
  setn TX_N=100 aof=everysec,REPLICA=1 c20: i: tx/s 0.864 1.154 0.916 0.971 1.146 | CPU/tx median 1.043 [1.042-1.046]; p3: tx/s 0.849 1.036 1.054 1.027 1.004 | CPU/tx median 1.020 [1.017-1.022]; foreign CPUs median 0.18
  set-set REPLICA=1 c50: i: tx/s 0.998 0.967 0.979 0.988 0.983 | CPU/tx median 1.015 [1.003-1.037]; p3: tx/s 1.023 0.975 1.001 0.988 1.002 | CPU/tx median 0.989 [0.974-1.019]; foreign CPUs median 0.56
  setn TX_N=100 REPLICA=1 c20: i: tx/s 1.019 1.018 1.017 1.016 1.014 | CPU/tx median 0.982 [0.980-0.985]; p3: tx/s 1.021 0.993 1.020 1.009 1.017 | CPU/tx median 0.982 [0.978-1.005]; foreign CPUs median 0.14
