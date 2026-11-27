# r5: 6 runs, 6 valid

## raof-1840-bounce

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 1840 | 1840 | 1.0000 | 183999 | 148.3 | 61.1 | 0.11 | 5 | 1 | 93906600 | 0.00 | 1.6 | 20971 | 0.043 | 125 | 2.0 | 0.21 | 935 | digest=eq primary=true replica=true restart=true |
| 1 | i | 1840 | 1840 | 1.0000 | 183999 | 146.0 | 32.2 | 0.06 | 4 | 0 | 93997400 | 0.00 | 21.6 | 2831 | 0.006 | 190 | 2.1 | 0.19 | 672 | digest=eq primary=true replica=true restart=true |
| 1 | p | 1840 | 1840 | 1.0000 | 183999 | 145.2 | 30.6 | 0.06 | 6 | 2 | 93813500 | 0.00 | 25.5 | 3342 | 0.007 | 203 | 2.1 | 0.21 | 805 | digest=eq primary=true replica=true restart=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 54.4 | 57.9 | 58.1 | 62.4 | 0.00 | 57.2 57.2 57.3 59.0 60.3 57.6 57.8 57.6 59.7 57.9 57.6 57.6 60.0 58.1 57.6 57.6 59.4 57.6 57.6 57.6 | 57.6 | 57.6 | 81.6 | 50.8 | 65.3 |
| 1 | i | 31.2 | 66.1 | 65.8 | 77.5 | 0.10 | 65.2 65.2 65.2 65.2 65.3 65.5 65.5 65.5 68.2 65.6 65.5 65.5 68.1 65.6 65.5 65.5 68.4 65.5 65.5 65.5 | 65.5 | 65.5 | 86.5 | 58.8 | 54.6 |
| 1 | p | 54.3 | 57.4 | 58.3 | 74.8 | 0.12 | 57.2 57.2 57.2 57.2 58.7 58.3 57.6 57.6 61.3 57.6 57.6 57.6 62.9 58.0 58.1 58.1 60.3 57.6 57.6 57.6 | 57.6 | 57.6 | 86.8 | 51.1 | 53.4 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 1.000 [1.000–1.000] | 0.984 [0.984–0.984] | 0.527 [0.527–0.527] | 13.789 [13.789–13.789] | 1.133 [1.133–1.133] | 1.241 [1.241–1.241] | 1.137 [1.137–1.137] |
| p | 1.000 [1.000–1.000] | 0.979 [0.979–0.979] | 0.501 [0.501–0.501] | 16.280 [16.280–16.280] | 1.003 [1.003–1.003] | 1.198 [1.198–1.198] | 1.000 [1.000–1.000] |

## raof-1380-bounce

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 1380 | 1380 | 1.0000 | 137999 | 146.7 | 63.8 | 0.09 | 6 | 2 | 74700700 | 0.00 | 1.8 | 23714 | 0.037 | 189 | 1.5 | 0.20 | 816 | digest=eq primary=true replica=true restart=true |
| 1 | i | 1380 | 1380 | 1.0000 | 138000 | 147.8 | 32.2 | 0.04 | 5 | 1 | 69808600 | 0.00 | 22.3 | 2927 | 0.005 | 168 | 1.6 | 0.19 | 632 | digest=eq primary=true replica=true restart=true |
| 1 | p | 1380 | 1380 | 1.0000 | 138000 | 149.1 | 33.4 | 0.05 | 4 | 0 | 70084500 | 0.00 | 24.5 | 3217 | 0.005 | 151 | 1.6 | 0.15 | 661 | digest=eq primary=true replica=true restart=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 31.3 | 57.6 | 58.7 | 72.2 | 0.09 | 57.2 57.2 57.2 57.2 61.5 58.9 57.6 57.5 63.9 57.6 57.7 57.8 64.9 58.1 57.6 57.6 61.5 58.3 57.6 57.6 | 57.6 | 57.6 | 82.1 | 51.5 | 48.6 |
| 1 | i | 31.5 | 65.8 | 66.5 | 81.1 | 0.03 | 65.5 65.5 65.5 65.5 69.3 67.0 65.8 65.8 68.5 65.8 65.7 65.8 70.9 66.2 65.7 65.7 68.0 65.8 65.8 65.8 | 66.1 | 65.7 | 95.7 | 59.1 | 49.0 |
| 1 | p | 47.7 | 57.5 | 58.0 | 71.9 | 0.07 | 57.3 57.3 57.3 57.3 59.2 57.8 57.7 57.7 59.0 57.8 57.7 57.7 58.5 57.7 57.7 57.7 60.2 57.7 57.7 58.0 | 57.7 | 57.7 | 81.3 | 50.7 | 49.5 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 1.000 [1.000–1.000] | 1.008 [1.008–1.008] | 0.505 [0.505–0.505] | 12.631 [12.631–12.631] | 1.132 [1.132–1.132] | 1.123 [1.123–1.123] | 1.141 [1.141–1.141] |
| p | 1.000 [1.000–1.000] | 1.017 [1.017–1.017] | 0.523 [0.523–0.523] | 13.883 [13.883–13.883] | 0.987 [0.987–0.987] | 0.997 [0.997–0.997] | 1.002 [1.002–1.002] |

