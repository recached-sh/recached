# e4b: 4 runs, 4 valid

## c1000-repl-w4-300s

| rep | label | tx/s | tx p99 ms | writer ops/s | writer p99 µs | writer max µs | primary µs/tx | primary Minstr/tx | replica µs/tx | lag max | full syncs (after prefill) | catch-up s | generator CPUs | foreign CPUs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 909.15 | 4 | 4450 | 2575 | 8466 | 1361 | 7 | 859 | 14070 | 0 | 0.00 | 0.42 | 0.49 | digest=eq |
| 1 | p | 981.30 | 3 | 4595 | 2348 | 7412 | 1285 | 7 | 720 | 8025 | 0 | 0.00 | 0.45 | 0.46 | digest=eq |
| 2 | h | 915.44 | 4 | 4506 | 2558 | 10718 | 1362 | 7 | 837 | 14785 | 0 | 0.00 | 0.42 | 0.49 | digest=eq |
| 2 | p | 980.11 | 4 | 4591 | 2355 | 8476 | 1285 | 7 | 721 | 12052 | 0 | 0.00 | 0.45 | 0.46 | digest=eq |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 61.9 | 62.2 | 60.9 | 62.4 | -0.29 | 62.3 61.9 60.5 60.7 60.6 60.8 60.5 60.7 60.6 60.6 | 60.7 | 58.2 | 62.0 | 53.6 | 24.5 |
| 1 | p | 58.3 | 71.6 | 76.6 | 80.6 | -0.02 | 75.6 77.2 76.7 78.2 76.1 76.3 77.4 76.1 76.4 76.4 | 76.5 | 58.8 | 80.5 | 69.5 | 25.3 |
| 2 | h | 58.2 | 60.8 | 60.7 | 61.1 | -0.09 | 60.8 60.8 60.9 60.9 60.8 60.7 60.5 60.5 60.5 60.7 | 60.8 | 58.2 | 60.9 | 53.5 | 24.7 |
| 2 | p | 58.8 | 69.0 | 73.2 | 76.1 | 1.04 | 70.2 70.8 71.8 73.1 73.9 74.3 73.8 74.4 74.6 75.1 | 75.1 | 59.0 | 75.8 | 65.9 | 24.7 |

Paired per rep against h (median [range]):

| label | tx_per_s | primary_cpu_us_per_tx | primary_instr_per_tx | writer_ops_per_s | w_p99_us | replica_cpu_us_per_tx | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|---|---|
| p | 1.075 [1.071–1.079] | 0.944 [0.944–0.944] | 1.019 [1.019–1.019] | 1.026 [1.019–1.033] | 0.916 [0.912–0.920] | 0.850 [0.838–0.861] | 1.232 [1.206–1.258] | 1.269 [1.245–1.292] | 1.012 [1.011–1.014] |

