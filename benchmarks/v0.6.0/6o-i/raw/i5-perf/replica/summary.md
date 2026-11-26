| cell | reps | tx/s base / i / p2 | ratio to base | CPU µs/tx | p99 µs | max µs | RSS MiB |
|---|---|---|---|---|---|---|---|
| set-set REPLICA=1 c50 | 5/5/5 | 83,854 / 83,836 / 82,168 | 1.000 / 0.980 | 9.31 → 9.33 → 9.50 | 648 → 647 → 665 | 2,973 → 3,055 → 4,032 | 59 → 72 → 66 |
| setn TX_N=100 REPLICA=1 c20 | 5/5/5 | 8,344 / 8,471 / 8,561 | 1.015 / 1.026 | 115.63 → 113.72 → 112.62 | 2,640 → 2,570 → 2,566 | 3,650 → 4,657 → 4,501 | 58 → 71 → 65 |

| replica cell | replica CPU µs/tx | lag frames at end | full syncs | queue disconnects |
|---|---|---|---|---|
| set-set REPLICA=1 c50 | 2.02 → 2.24 → 2.20 | 0 → 0 → 0 | 1 → 1 → 1 | 0 → 0 → 0 |
| setn TX_N=100 REPLICA=1 c20 | 83.18 → 72.04 → 71.89 | 0 → 0 → 0 | 1 → 1 → 1 | 0 → 0 → 0 |

paired ratios per rep (each label / base), and the host's foreign CPUs:
  set-set REPLICA=1 c50: i: 0.975 0.985 0.997 1.023 0.994; p2: 0.991 0.984 0.964 0.990 0.985; foreign CPUs median 0.58
  setn TX_N=100 REPLICA=1 c20: i: 1.016 1.009 1.018 1.015 1.015; p2: 1.027 1.019 1.028 1.026 1.023; foreign CPUs median 0.15
