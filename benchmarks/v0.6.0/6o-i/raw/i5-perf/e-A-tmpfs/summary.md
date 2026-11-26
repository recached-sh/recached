| cell | reps | tx/s base / i / p3 / p3nc | ratio to base | CPU µs/tx | p99 µs | max µs | RSS MiB |
|---|---|---|---|---|---|---|---|
| set-set aof=everysec c50 | 5/5/5/5 | 105,801 / 99,809 / 103,191 / 104,001 | 0.943 / 0.975 / 0.983 | 7.25 → 7.83 → 7.48 → 7.43 | 644 → 691 → 664 → 649 | 2,815 → 2,888 → 2,785 → 2,988 | 26 → 26 → 26 → 26 |
| setn TX_N=100 aof=everysec c20 | 5/5/5/5 | 10,488 / 9,955 / 10,308 / 10,419 | 0.949 / 0.983 / 0.993 | 92.07 → 97.16 → 93.72 → 92.76 | 2,784 → 2,938 → 2,835 → 2,807 | 4,328 → 4,324 → 4,058 → 4,273 | 25 → 25 → 25 → 25 |

| AOF cell | AOF bytes/tx | write(2) calls/tx | µs in write(2) per tx | fsyncs per run | µs per fsync |
|---|---|---|---|---|---|
| set-set aof=everysec c50 | 161 → 275 → 275 → 275 | 0.100 → 0.100 → 0.100 → 0.100 | 0.18 → 0.22 → 0.22 → 0.22 | 23 → 23 → 23 → 23 | 1 → 1 → 1 → 1 |
| setn TX_N=100 aof=everysec c20 | 7607 → 7727 → 7723 → 7722 | 0.166 → 0.165 → 0.165 → 0.166 | 1.97 → 1.98 → 1.98 → 1.99 | 23 → 23 → 23 → 23 | 1 → 1 → 1 → 1 |

paired ratios per rep (each label / base), and the host's foreign CPUs:
  set-set aof=everysec c50: i: tx/s 0.954 0.939 0.952 0.937 0.948 | CPU/tx median 1.073 [1.064-1.085]; p3: tx/s 0.975 0.972 0.995 0.973 0.968 | CPU/tx median 1.032 [0.996-1.035]; p3nc: tx/s 0.972 0.991 1.018 0.972 0.981 | CPU/tx median 1.026 [0.974-1.037]; foreign CPUs median 0.61
  setn TX_N=100 aof=everysec c20: i: tx/s 0.952 0.924 0.950 0.953 0.951 | CPU/tx median 1.053 [1.051-1.083]; p3: tx/s 0.981 0.960 0.989 0.984 0.983 | CPU/tx median 1.018 [1.012-1.042]; p3nc: tx/s 0.999 0.988 1.000 0.998 0.993 | CPU/tx median 1.002 [1.000-1.012]; foreign CPUs median 0.03
