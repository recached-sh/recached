| cell | reps | tx/s base / i | ratio to base | CPU µs/tx | p99 µs | max µs | RSS MiB |
|---|---|---|---|---|---|---|---|
| get-get noaof c50 | 5/5 | 109,135 / 108,660 | 0.996 | 6.79 → 6.89 | 556 → 554 | 3,110 → 5,608 | 25 → 25 |
| set-set noaof c50 | 5/5 | 103,418 / 101,735 | 0.984 | 7.30 → 7.35 | 590 → 584 | 3,004 → 5,194 | 26 → 26 |
| set-set aof=everysec c50 | 5/5 | 87,927 / 79,874 | 0.908 | 8.26 → 8.88 | 789 → 828 | 200,268 → 341,246 | 26 → 26 |
| set-set aof=always c50 | 5/5 | 333 / 334 | 1.003 | 24.16 → 24.44 | 383,290 → 291,733 | 625,249 → 1,324,934 | 26 → 26 |
| get-set aof=everysec c50 | 5/5 | 90,686 / 90,799 | 1.001 | 8.10 → 8.11 | 768 → 756 | 544,961 → 455,399 | 26 → 26 |
| mget-mset aof=everysec c50 | 5/5 | 38,746 / 38,152 | 0.985 | 19.23 → 19.32 | 1,581 → 1,598 | 649,870 → 430,602 | 26 → 26 |
| mset10 aof=everysec c50 | 5/5 | 43,832 / 44,892 | 1.024 | 14.10 → 14.28 | 1,231 → 1,218 | 1,202,730 → 1,203,059 | 26 → 26 |
| wide20 aof=everysec c50 | 5/5 | 24,014 / 22,140 | 0.922 | 26.66 → 28.32 | 2,152 → 2,276 | 587,975 → 1,285,488 | 26 → 26 |
| setn TX_N=100 aof=everysec c20 | 5/5 | 5,837 / 6,114 | 1.047 | 98.18 → 104.41 | 3,104 → 3,321 | 1,325,180 → 648,089 | 25 → 25 |
| setn TX_N=1000 aof=everysec c8 | 5/5 | 615 / 623 | 1.013 | 917.09 → 964.94 | 120,090 → 138,122 | 1,142,016 → 1,497,090 | 26 → 27 |
| setn TX_N=10000 aof=everysec c2 | 5/5 | 60 / 63 | 1.050 | 8970.22 → 9548.28 | 531,963 → 467,534 | 1,997,179 → 1,425,471 | 35 → 35 |
| set-set REPLICA=1 c50 | 5/5 | 81,647 / 79,623 | 0.975 | 9.49 → 9.66 | 731 → 750 | 5,136 → 9,534 | 60 → 66 |
| setn TX_N=100 REPLICA=1 c20 | 5/5 | 8,097 / 8,241 | 1.018 | 118.98 → 116.68 | 2,776 → 2,704 | 5,251 → 4,714 | 58 → 71 |

| replica cell | replica CPU µs/tx | lag frames at end | full syncs | queue disconnects |
|---|---|---|---|---|
| set-set REPLICA=1 c50 | 2.10 → 2.33 | 0 → 0 | 1 → 1 | 0 → 0 |
| setn TX_N=100 REPLICA=1 c20 | 83.48 → 73.94 | 0 → 0 | 1 → 1 | 0 → 0 |

paired ratios per rep (each label / base), and the host's foreign CPUs:
  get-get noaof c50: i: 0.946 1.008 0.984 1.010 0.998; foreign CPUs median 1.18
  set-set noaof c50: i: 1.012 0.974 1.004 1.000 0.984; foreign CPUs median 1.14
  set-set aof=everysec c50: i: 0.825 0.954 0.894 0.909 0.916; foreign CPUs median 1.20
  set-set aof=always c50: i: 0.933 0.931 1.046 0.824 1.141; foreign CPUs median 0.55
  get-set aof=everysec c50: i: 1.201 0.980 0.982 0.986 1.020; foreign CPUs median 1.13
  mget-mset aof=everysec c50: i: 1.195 0.871 0.985 0.891 0.991; foreign CPUs median 0.95
  mset10 aof=everysec c50: i: 1.024 1.069 1.013 0.943 1.031; foreign CPUs median 0.99
  wide20 aof=everysec c50: i: 0.904 0.887 1.048 0.768 0.964; foreign CPUs median 0.87
  setn TX_N=100 aof=everysec c20: i: 0.892 1.049 1.016 1.120 1.016; foreign CPUs median 0.65
  setn TX_N=1000 aof=everysec c8: i: 0.950 0.931 1.013 0.961 1.081; foreign CPUs median 0.63
  setn TX_N=10000 aof=everysec c2: i: 1.161 1.091 0.926 0.913 1.150; foreign CPUs median 0.68
  set-set REPLICA=1 c50: i: 0.968 1.003 0.976 0.998 0.986; foreign CPUs median 1.07
  setn TX_N=100 REPLICA=1 c20: i: 1.005 1.013 1.020 1.020 1.025; foreign CPUs median 0.67
