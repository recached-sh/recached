# r6b: 1 runs, 1 valid

## raof-1840-soak2

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | i | 1840 | 1840 | 1.0000 | 184000 | 147.7 | 124.8 | 0.23 | 0 | 0 | 528400 | 0.00 | 38.8 | 5097 | 0.071 | 218 | 14.0 | 0.29 | 743 | digest=eq primary=true replica=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | i | 32.7 | 67.0 | 66.7 | 66.7 | -0.00 | 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.6 66.6 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.6 66.7 66.7 66.7 66.7 66.7 66.7 66.6 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.7 66.6 66.6 | 66.7 | 66.7 | 77.3 | 58.1 | 54.1 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | – | – | – | – | – | – | – |

