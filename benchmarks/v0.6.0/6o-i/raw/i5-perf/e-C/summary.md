| cell | reps | tx/s base / p3 | ratio to base | CPU µs/tx | p99 µs | max µs | RSS MiB |
|---|---|---|---|---|---|---|---|
| set-set aof=everysec,RECACHED_AOF_FANOUT=coalesced c50 | 3/3 | 93,363 / 83,352 | 0.893 | 8.11 → 8.40 | 730 → 760 | 34,052 → 157,954 | 26 → 26 |
| set-set REPLICA=1,RECACHED_REPL_BACKLOG_BYTES=268435456 c50 | 3/3 | 82,960 / 83,003 | 1.001 | 9.39 → 9.41 | 645 → 646 | 14,317 → 14,205 | 522 → 560 |
| setn TX_N=10000 REPLICA=1 c2 | 3/3 | 106 / 99 | 0.934 | 11712.48 → 12360.14 | 23,348 → 24,434 | 27,668 → 29,143 | 86 → 127 |
| setn TX_N=100 REPLICA=1,REPLICA_AOF=1 c20 | 3/3 | 9,419 / 9,192 | 0.976 | 102.44 → 104.74 | 6,865 → 7,268 | 32,922 → 32,771 | 83 → 84 |

| replica cell | replica CPU µs/tx | lag frames at end | full syncs | queue disconnects |
|---|---|---|---|---|
| set-set REPLICA=1,RECACHED_REPL_BACKLOG_BYTES=268435456 c50 | 1.98 → 2.20 | 0 → 0 | 1 → 1 | 0 → 0 |
| setn TX_N=10000 REPLICA=1 c2 | 8069.71 → 6772.34 | 0 → 0 | 1 → 1 | 0 → 0 |
| setn TX_N=100 REPLICA=1,REPLICA_AOF=1 c20 | 63.04 → 32.18 | 0 → 3264100 | 4 → 6 | 4 → 5 |

| AOF cell | AOF bytes/tx | write(2) calls/tx | µs in write(2) per tx | fsyncs per run | µs per fsync |
|---|---|---|---|---|---|
| set-set aof=everysec,RECACHED_AOF_FANOUT=coalesced c50 | 162 → 277 | 0.100 → 0.100 | 0.83 → 2.01 | 23 → 23 | 210,812 → 267,931 |

| replica cell | replica RSS MiB |
|---|---|
| set-set REPLICA=1,RECACHED_REPL_BACKLOG_BYTES=268435456 c50 | 24 → 24 |
| setn TX_N=10000 REPLICA=1 c2 | 25 → 28 |
| setn TX_N=100 REPLICA=1,REPLICA_AOF=1 c20 | 138 → 136 |

paired ratios per rep (each label / base), and the host's foreign CPUs:
  set-set aof=everysec,RECACHED_AOF_FANOUT=coalesced c50: p3: tx/s 0.893 0.879 0.978 | CPU/tx median 1.033 [1.031-1.039]; foreign CPUs median 0.62
  set-set REPLICA=1,RECACHED_REPL_BACKLOG_BYTES=268435456 c50: p3: tx/s 0.962 1.021 1.003 | CPU/tx median 1.002 [0.977-1.040]; foreign CPUs median 0.54
  setn TX_N=10000 REPLICA=1 c2: p3: tx/s 0.934 0.925 0.925 | CPU/tx median 1.058 [1.052-1.066]; foreign CPUs median 0.08
  setn TX_N=100 REPLICA=1,REPLICA_AOF=1 c20: p3: tx/s 0.964 0.977 0.976 | CPU/tx median 1.023 [1.021-1.035]; foreign CPUs median 0.59
