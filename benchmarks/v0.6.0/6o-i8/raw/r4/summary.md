# r4: 3 runs, 3 valid

Runs excluded from paired ratios (invalid, a failed check, or a replica resync during a contention load):

- rep 1 raof-1840-soak h: valid=True ['restart: replica-restart: no PONG']
- rep 1 raof-1840-soak i: valid=True ['restart: replica-restart: no PONG']
- rep 1 raof-1840-soak p: valid=True ['restart: replica-restart: no PONG']

## raof-1840-soak

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 1840 | 1840 | 1.0000 | 184000 | 151.7 | 316.4 | 0.58 | 0 | 0 | 728800 | 0.00 | 1.8 | 23787 | 0.328 | 206 | 13.8 | 0.24 | 1246 | digest=eq primary=true replica=true |
| 1 | i | 1840 | 1840 | 1.0000 | 184000 | 147.3 | 125.5 | 0.23 | 0 | 0 | 883300 | 0.00 | 30.0 | 3942 | 0.055 | 195 | 14.0 | 0.21 | 673 | digest=eq primary=true replica=true |
| 1 | p | 1840 | 1840 | 1.0000 | 184000 | 144.7 | 121.4 | 0.22 | 5 | 5 | 16342400 | 0.00 | 27.4 | 3603 | 0.050 | 193 | 13.9 | 0.21 | 679 | digest=eq primary=true replica=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 54.6 | 57.6 | 57.3 | 58.2 | 0.00 | 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.4 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 57.3 | 57.3 | 57.1 | 59.5 | 50.0 | 61.5 |
| 1 | i | 31.0 | 65.3 | 65.0 | 67.3 | 0.00 | 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 65.0 64.9 64.9 64.9 64.9 65.0 65.0 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 65.0 65.0 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 64.9 65.2 65.0 65.0 65.0 65.0 65.0 65.0 65.0 64.9 64.9 64.9 64.9 65.0 65.0 65.0 | 65.0 | 64.5 | 74.1 | 58.2 | 65.7 |
| 1 | p | 54.7 | 60.8 | 58.1 | 73.4 | -0.04 | 62.7 57.7 60.7 59.9 61.3 57.8 57.9 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 57.8 | 57.8 | 57.8 | 82.4 | 50.7 | 106.4 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | – | – | – | – | – | – | – |
| p | – | – | – | – | – | – | – |

