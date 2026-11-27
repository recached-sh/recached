# r3: 30 runs, 30 valid

Runs excluded from paired ratios (invalid, a failed check, or a replica resync during a contention load):

- rep 1 tmpfs-9200 i: valid=True ['replica not converged within drain window', 'restart: replica-restart: no PONG']
- rep 1 tmpfs-9200 p: valid=True ['replica not converged within drain window', 'restart: replica-restart: no PONG']
- rep 2 tmpfs-9200 i: valid=True ['restart: replica-restart: no PONG']
- rep 2 tmpfs-9200 p: valid=True ['restart: replica-restart: no PONG']

## noaof-2300

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 2300 | 2300 | 1.0000 | 229995 | 142.6 | 101.2 | 0.23 | 0 | 0 | 800 | 0.00 | – | – | – | – | – | 0.03 | 524 | digest=eq primary=true replica=true |
| 1 | i | 2300 | 2300 | 1.0000 | 229997 | 145.3 | 91.1 | 0.21 | 0 | 0 | 500 | 0.00 | – | – | – | – | – | 0.02 | 529 | digest=eq primary=true replica=true |
| 1 | p | 2300 | 2300 | 1.0000 | 229994 | 145.7 | 91.8 | 0.21 | 0 | 0 | 500 | 0.00 | – | – | – | – | – | 0.01 | 527 | digest=eq primary=true replica=true |
| 2 | h | 2300 | 2300 | 1.0000 | 229998 | 143.4 | 102.1 | 0.23 | 0 | 0 | 600 | 0.00 | – | – | – | – | – | 0.02 | 496 | digest=eq primary=true replica=true |
| 2 | i | 2300 | 2300 | 1.0000 | 229993 | 145.2 | 89.8 | 0.21 | 0 | 0 | 600 | 0.00 | – | – | – | – | – | 0.09 | 434 | digest=eq primary=true replica=true |
| 2 | p | 2300 | 2300 | 1.0000 | 230001 | 145.5 | 91.0 | 0.21 | 0 | 0 | 500 | 0.00 | – | – | – | – | – | 0.01 | 519 | digest=eq primary=true replica=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 31.2 | 57.4 | 57.0 | 57.4 | -0.01 | 57.0 57.0 57.0 57.0 | 57.0 | 57.0 | 59.3 | 50.0 | 24.6 |
| 1 | i | 70.8 | 65.5 | 65.2 | 65.2 | -0.00 | 65.2 65.2 65.2 65.2 | 65.2 | 65.2 | 75.9 | 58.1 | 24.8 |
| 1 | p | 31.1 | 57.4 | 57.1 | 57.4 | -0.02 | 57.1 57.1 57.1 57.1 | 57.1 | 57.1 | 59.4 | 50.2 | 24.6 |
| 2 | h | 31.6 | 57.7 | 57.3 | 57.7 | -0.01 | 57.4 57.3 57.3 57.3 | 57.3 | 57.3 | 59.5 | 49.9 | 24.8 |
| 2 | i | 31.3 | 65.5 | 65.2 | 65.6 | -0.03 | 65.2 65.2 65.2 65.2 | 65.2 | 65.2 | 75.8 | 58.1 | 24.6 |
| 2 | p | 56.7 | 57.6 | 57.4 | 57.6 | -0.01 | 57.4 57.4 57.4 57.4 | 57.4 | 57.4 | 59.6 | 50.1 | 24.9 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 1.000 [1.000–1.000] | 1.016 [1.012–1.019] | 0.890 [0.880–0.901] | – | 1.140 [1.137–1.143] | 1.135 [1.135–1.135] | 1.140 [1.137–1.143] |
| p | 1.000 [1.000–1.000] | 1.018 [1.014–1.022] | 0.899 [0.891–0.907] | – | 1.001 [1.000–1.002] | 0.998 [0.997–0.999] | 1.001 [1.000–1.002] |

## noaof-9200

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 9200 | 8104 | 0.8809 | 810415 | 119.0 | 84.0 | 0.68 | 0 | 0 | 5964 | 0.00 | – | – | – | – | – | 0.01 | 15815440 | digest=eq primary=true replica=true |
| 1 | i | 9200 | 8158 | 0.8868 | 815820 | 117.9 | 74.4 | 0.61 | 0 | 0 | 8800 | 0.00 | – | – | – | – | – | 0.01 | 15146441 | digest=eq primary=true replica=true |
| 1 | p | 9200 | 8030 | 0.8728 | 802977 | 119.7 | 75.1 | 0.60 | 0 | 0 | 26600 | 0.00 | – | – | – | – | – | 0.13 | 16817139 | digest=eq primary=true replica=true |
| 2 | h | 9200 | 8097 | 0.8801 | 809676 | 119.2 | 84.1 | 0.68 | 0 | 0 | 6264 | 0.00 | – | – | – | – | – | 0.01 | 15928813 | digest=eq primary=true replica=true |
| 2 | i | 9200 | 8088 | 0.8792 | 808831 | 119.0 | 74.7 | 0.60 | 0 | 0 | 7000 | 0.00 | – | – | – | – | – | 0.00 | 16106275 | digest=eq primary=true replica=true |
| 2 | p | 9200 | 8080 | 0.8783 | 808026 | 119.2 | 74.7 | 0.60 | 0 | 0 | 6100 | 0.00 | – | – | – | – | – | 0.01 | 16236062 | digest=eq primary=true replica=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 31.4 | 57.7 | 57.3 | 57.7 | -0.05 | 57.4 57.3 57.3 57.3 | 57.3 | 57.2 | 59.5 | 50.1 | 25.0 |
| 1 | i | 31.5 | 66.0 | 65.8 | 66.0 | -0.14 | 65.9 65.9 65.8 65.7 | 65.7 | 65.3 | 74.8 | 58.5 | 27.1 |
| 1 | p | 60.0 | 57.9 | 57.8 | 57.9 | -0.06 | 57.8 57.8 57.7 57.7 | 57.8 | 57.6 | 60.0 | 50.4 | 27.0 |
| 2 | h | 59.7 | 57.8 | 57.5 | 57.8 | -0.07 | 57.6 57.5 57.5 57.5 | 57.5 | 57.4 | 59.8 | 50.1 | 25.3 |
| 2 | i | 76.5 | 66.0 | 65.9 | 66.0 | -0.12 | 66.0 66.0 65.9 65.8 | 65.8 | 65.6 | 76.3 | 58.6 | 24.9 |
| 2 | p | 31.7 | 58.0 | 57.9 | 58.0 | -0.03 | 57.9 57.9 57.9 57.9 | 57.9 | 57.6 | 60.2 | 50.4 | 26.1 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 1.003 [0.999–1.007] | 0.994 [0.990–0.998] | 0.887 [0.885–0.889] | – | 1.147 [1.146–1.148] | 1.143 [1.142–1.144] | 1.141 [1.141–1.142] |
| p | 0.994 [0.991–0.998] | 1.003 [1.000–1.006] | 0.891 [0.888–0.894] | – | 1.007 [1.006–1.008] | 1.003 [1.002–1.003] | 1.005 [1.003–1.007] |

## tmpfs-2300

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 2300 | 2300 | 1.0000 | 229995 | 142.8 | 199.8 | 0.46 | 0 | 0 | 1500 | 0.00 | 0.4 | 5547 | 0.096 | 0 | 17.2 | 0.01 | 536 | digest=eq primary=true replica=true restart=true |
| 1 | i | 2300 | 2300 | 1.0000 | 229998 | 145.2 | 107.1 | 0.25 | 0 | 0 | 200 | 0.00 | 4.9 | 646 | 0.011 | 0 | 17.5 | 0.02 | 453 | digest=eq primary=true replica=true restart=true |
| 1 | p | 2300 | 2300 | 1.0000 | 229995 | 145.3 | 104.9 | 0.24 | 0 | 0 | 500 | 0.00 | 5.2 | 685 | 0.012 | 0 | 17.5 | 0.02 | 524 | digest=eq primary=true replica=true restart=true |
| 2 | h | 2300 | 2300 | 1.0000 | 229993 | 143.1 | 196.5 | 0.45 | 0 | 0 | 1000 | 0.00 | 0.4 | 5592 | 0.096 | 0 | 17.2 | 0.01 | 478 | digest=eq primary=true replica=true restart=true |
| 2 | i | 2300 | 2300 | 1.0000 | 229996 | 144.6 | 108.1 | 0.25 | 0 | 0 | 200 | 0.00 | 5.1 | 675 | 0.012 | 0 | 17.5 | 0.00 | 520 | digest=eq primary=true replica=true restart=true |
| 2 | p | 2300 | 2300 | 1.0000 | 229995 | 145.0 | 103.7 | 0.24 | 0 | 0 | 500 | 0.00 | 4.9 | 647 | 0.011 | 0 | 17.5 | 0.05 | 454 | digest=eq primary=true replica=true restart=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 56.4 | 57.5 | 57.1 | 57.5 | -0.01 | 57.1 57.1 57.1 57.1 | 57.1 | 56.9 | 59.5 | 50.0 | 24.6 |
| 1 | i | 71.2 | 65.9 | 65.6 | 65.6 | -0.00 | 65.6 65.6 65.6 65.6 | 65.6 | 65.6 | 76.3 | 58.2 | 24.5 |
| 1 | p | 31.4 | 57.6 | 57.4 | 57.6 | -0.01 | 57.4 57.4 57.4 57.4 | 57.4 | 57.4 | 59.6 | 50.1 | 24.9 |
| 2 | h | 31.5 | 57.8 | 57.4 | 57.8 | -0.01 | 57.4 57.4 57.4 57.4 | 57.4 | 57.4 | 59.7 | 50.0 | 25.0 |
| 2 | i | 31.1 | 65.4 | 65.1 | 65.1 | -0.00 | 65.1 65.1 65.1 65.1 | 65.1 | 65.1 | 74.2 | 58.2 | 24.7 |
| 2 | p | 56.8 | 57.6 | 57.4 | 57.6 | -0.00 | 57.4 57.4 57.4 57.4 | 57.4 | 57.4 | 59.8 | 50.1 | 25.3 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 1.000 [1.000–1.000] | 1.014 [1.011–1.017] | 0.543 [0.536–0.550] | 12.043 [11.831–12.254] | 1.142 [1.135–1.148] | 1.134 [1.127–1.140] | 1.144 [1.136–1.152] |
| p | 1.000 [1.000–1.000] | 1.015 [1.013–1.017] | 0.526 [0.525–0.528] | 12.139 [11.751–12.528] | 1.003 [1.001–1.005] | 0.999 [0.997–1.001] | 1.005 [1.001–1.008] |

## tmpfs-9200

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 9200 | 8637 | 0.9388 | 863706 | 111.7 | 82.0 | 0.71 | 21 | 21 | 2807500 | 1.75 | 0.4 | 5085 | 0.156 | 0 | 30.6 | 0.02 | 20651216 | digest=eq primary=true replica=true restart=true |
| 1 | i | 9200 | 8327 | 0.9051 | 832733 | 115.6 | 66.9 | 0.56 | 109 | 1 | 114663300 | – | 3.5 | 456 | 0.022 | 0 | 47.8 | 0.03 | 15781929 | digest=eq primary=true replica=true |
| 1 | p | 9200 | 8172 | 0.8883 | 817225 | 117.9 | 79.3 | 0.65 | 102 | 1 | 116439700 | – | 3.4 | 598 | 0.029 | 0 | 47.8 | 0.16 | 15754811 | digest=eq primary=true replica=true |
| 2 | h | 9200 | 8636 | 0.9387 | 863572 | 111.7 | 81.9 | 0.71 | 21 | 21 | 120295300 | 2.00 | 0.4 | 4929 | 0.152 | 0 | 30.8 | 0.55 | 21039426 | digest=eq primary=true replica=true restart=true |
| 2 | i | 9200 | 8186 | 0.8899 | 818655 | 117.6 | 92.5 | 0.76 | 0 | 0 | 11800 | 0.00 | 3.3 | 701 | 0.033 | 0 | 47.7 | 0.02 | 14732865 | digest=eq primary=true replica=true |
| 2 | p | 9200 | 7984 | 0.8679 | 798403 | 120.5 | 89.1 | 0.71 | 0 | 0 | 130700 | 0.00 | 3.3 | 664 | 0.032 | 0 | 47.9 | 0.05 | 17643670 | digest=eq primary=true replica=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 31.3 | 69.8 | 64.9 | 74.7 | -1.22 | 65.2 65.6 65.8 62.9 | 61.7 | 59.1 | 85.5 | 57.7 | 116.7 |
| 1 | i | 76.5 | 66.0 | 68.6 | 82.4 | 6.44 | 66.0 65.9 65.9 76.7 | 74.8 | 75.9 | 103.8 | 61.2 | 47.1 |
| 1 | p | 31.5 | 57.8 | 58.5 | 73.1 | 2.05 | 57.8 57.8 57.7 60.9 | 68.1 | 63.2 | 86.7 | 51.2 | 49.1 |
| 2 | h | 31.6 | 60.4 | 65.5 | 80.9 | 0.44 | 65.0 65.8 65.7 65.3 | 60.8 | 61.0 | 86.5 | 58.0 | 111.7 |
| 2 | i | 31.3 | 65.8 | 65.7 | 65.8 | -0.04 | 65.7 65.7 65.7 65.6 | 65.9 | 64.9 | 76.0 | 58.6 | 27.0 |
| 2 | p | 31.4 | 57.6 | 57.6 | 58.6 | -0.00 | 57.6 57.6 57.5 57.6 | 57.6 | 57.6 | 59.8 | 50.3 | 31.0 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | – | – | – | – | – | – | – |
| p | – | – | – | – | – | – | – |

## raof-2300-diag

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 2300 | 2300 | 1.0000 | 229998 | 146.9 | 332.0 | 0.76 | 0 | 0 | 891300 | 0.00 | 1.8 | 23445 | 0.404 | 184 | 17.2 | 0.20 | 843 | digest=eq primary=true replica=true restart=true |
| 1 | i | 2300 | 2300 | 1.0000 | 229994 | 143.6 | 114.4 | 0.26 | 0 | 0 | 600 | 0.00 | 27.6 | 3626 | 0.063 | 180 | 17.5 | 0.21 | 611 | digest=eq primary=true replica=true restart=true |
| 1 | p | 2300 | 2300 | 1.0000 | 229999 | 143.7 | 112.8 | 0.26 | 1 | 1 | 2255400 | 0.00 | 28.4 | 3733 | 0.065 | 159 | 17.5 | 0.17 | 725 | digest=eq primary=true replica=true restart=true |
| 2 | h | 2300 | 2300 | 1.0000 | 229997 | 147.5 | 329.8 | 0.76 | 0 | 0 | 1122400 | 0.00 | 1.7 | 23218 | 0.400 | 196 | 17.2 | 0.22 | 846 | digest=eq primary=true replica=true restart=true |
| 2 | i | 2300 | 2300 | 1.0000 | 229994 | 144.0 | 115.1 | 0.26 | 0 | 0 | 347400 | 0.00 | 44.5 | 5849 | 0.102 | 223 | 17.5 | 0.26 | 721 | digest=eq primary=true replica=true restart=true |
| 2 | p | 2300 | 2300 | 1.0000 | 229990 | 143.1 | 111.3 | 0.26 | 0 | 0 | 3900 | 0.00 | 38.9 | 5111 | 0.089 | 193 | 17.5 | 0.26 | 849 | digest=eq primary=true replica=true restart=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 32.3 | 58.4 | 58.2 | 59.1 | 0.07 | 58.1 58.1 58.6 58.1 | 58.1 | 58.0 | 60.3 | 50.4 | 55.1 |
| 1 | i | 32.2 | 66.6 | 66.2 | 66.3 | 0.01 | 66.2 66.3 66.2 66.3 | 66.3 | 66.3 | 76.9 | 58.4 | 37.8 |
| 1 | p | 32.1 | 69.6 | 58.4 | 65.5 | -0.17 | 58.6 58.4 58.4 58.4 | 58.4 | 58.4 | 85.1 | 50.7 | 45.2 |
| 2 | h | 32.1 | 58.4 | 58.0 | 58.4 | -0.01 | 58.0 58.0 58.0 58.0 | 58.0 | 58.0 | 60.3 | 50.2 | 56.6 |
| 2 | i | 32.1 | 66.4 | 66.1 | 66.1 | 0.00 | 66.1 66.1 66.1 66.1 | 66.1 | 66.1 | 76.7 | 58.5 | 56.3 |
| 2 | p | 57.4 | 58.3 | 58.1 | 58.3 | 0.00 | 58.1 58.1 58.1 58.1 | 58.1 | 58.1 | 60.3 | 50.4 | 39.9 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 1.000 [1.000–1.000] | 0.977 [0.976–0.978] | 0.347 [0.345–0.349] | 20.639 [15.701–25.577] | 1.139 [1.138–1.140] | 1.127 [1.121–1.133] | 1.142 [1.140–1.143] |
| p | 1.000 [1.000–1.000] | 0.974 [0.970–0.978] | 0.339 [0.337–0.340] | 19.256 [16.162–22.350] | 1.003 [1.002–1.004] | 1.053 [0.998–1.108] | 1.004 [1.002–1.007] |

Diag (primary, measured window; times in µs (waiters_at_release: a count): mean / upper bound of the log2 bucket holding the 99th percentile / max; n per s):

| rep | label | stream_hold_g | stream_hold_s16 | stream_wait_g | stream_wait_s | stream_free_s | stripe_wait_big | stripe_hold_big | stripe_wait_small | stripe_hold_small | stripe_wait_all | stripe_hold_all | stripe_gap | waiters_at_release | gate_close | tx_apply | tx_log |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | – | 0.1 / 0 / 8 (14375/s) | – | – | – / 0 / 0 (230000/s) | 2.4 / 4 / 19 (2300/s) | 89.2 / 131 / 7446 (2300/s) | – | – | – | – | – | – | 0.1 / 0 / 7 (2300/s) | 64.3 / 131 / 7353 (2300/s) | 13.9 / 33 / 2181 (2300/s) |
| 1 | i | 10.0 / 16 / 55 (2300/s) | – | – | – | – | 2.3 / 4 / 16 (2300/s) | 87.8 / 131 / 1423 (2300/s) | – | – | – | – | – | – | 0.1 / 0 / 6 (2300/s) | 63.0 / 131 / 312 (2300/s) | 12.9 / 16 / 58 (2300/s) |
| 1 | p | 9.3 / 16 / 228 (2300/s) | – | – | – | – | 2.3 / 4 / 20 (2300/s) | 87.9 / 131 / 670 (2300/s) | – | – | – | – | – | – | 0.1 / 0 / 7 (2300/s) | 63.2 / 131 / 530 (2300/s) | 12.7 / 16 / 232 (2300/s) |
| 2 | h | – | 0.1 / 0 / 21 (14375/s) | – | – | – / 0 / 0 (230000/s) | 2.4 / 4 / 14 (2300/s) | 89.0 / 131 / 2313 (2300/s) | – | – | – | – | – | – | 0.1 / 0 / 6 (2300/s) | 64.1 / 131 / 2278 (2300/s) | 13.9 / 33 / 39 (2300/s) |
| 2 | i | 10.0 / 16 / 91 (2300/s) | – | – | – | – | 2.3 / 4 / 23 (2300/s) | 88.4 / 131 / 326 (2300/s) | – | – | – | – | – | – | 0.1 / 0 / 5 (2300/s) | 63.4 / 131 / 208 (2300/s) | 13.0 / 16 / 94 (2300/s) |
| 2 | p | 9.3 / 16 / 2172 (2300/s) | – | – | – | – | 2.3 / 4 / 21 (2300/s) | 87.3 / 131 / 2281 (2300/s) | – | – | – | – | – | – | 0.1 / 0 / 5 (2300/s) | 63.2 / 131 / 560 (2300/s) | 12.2 / 16 / 2178 (2300/s) |

jemalloc (primary, measured window, MiB mean/max): allocated, active, resident, retained, dirty pages; mallocs/s

| rep | label | allocated | active | resident | retained | pdirty (pages) | mallocs/s | frees/s |
|---|---|---|---|---|---|---|---|---|
| 1 | h | 50.1 / 50.1 | 50.5 / 50.6 | 55.5 / 56.4 | 12.3 / 12.5 | 33 / 275 | 36 | 36 |
| 1 | i | 58.1 / 58.1 | 58.6 / 58.6 | 61.5 / 61.6 | 18.4 / 18.4 | 11 / 20 | 230061 | 230060 |
| 1 | p | 50.2 / 50.2 | 50.6 / 50.7 | 53.7 / 60.8 | 42.3 / 42.4 | 25 / 1835 | 230061 | 230061 |
| 2 | h | 50.1 / 50.1 | 50.5 / 50.5 | 53.4 / 53.8 | 12.5 / 12.5 | 1 / 107 | 27 | 27 |
| 2 | i | 58.1 / 58.1 | 58.6 / 58.6 | 61.5 / 61.6 | 18.4 / 18.4 | 11 / 18 | 230063 | 230063 |
| 2 | p | 50.2 / 50.2 | 50.6 / 50.7 | 53.5 / 53.7 | 12.3 / 12.4 | 10 / 58 | 230059 | 230059 |

Diag (replica, measured window; mean / p99 bucket / max, µs or a value; n per s):

| rep | label | r_group_apply | r_frame_apply16 | r_read_gap | r_read_bytes | r_apply_run | r_apply_run_frames |
|---|---|---|---|---|---|---|---|
| 1 | h | – | 3.0 / 8 / 8651 (14375/s) | 464.5 / 1049 / 1202113 (2153/s) | 9295.6 / 65536 / 65536 (2153/s) | 276.5 / 524 / 1202428 (3594/s) | 64.0 / 128 / 64 (3594/s) |
| 1 | i | 107.5 / 131 / 485043 (2300/s) | – | 453.1 / 1049 / 485103 (2207/s) | 9104.4 / 16384 / 65536 (2207/s) | 27824.5 / 67109 / 536826 (36/s) | 6400.0 / 8192 / 6400 (36/s) |
| 1 | p | 105.5 / 131 / 347169 (2300/s) | – | 453.4 / 1049 / 347220 (2205/s) | 9112.3 / 32768 / 65536 (2205/s) | 27824.5 / 67109 / 376414 (36/s) | 6400.0 / 8192 / 6400 (36/s) |
| 2 | h | – | 3.0 / 8 / 4925 (14375/s) | 465.4 / 1049 / 1460437 (2148/s) | 9313.8 / 65536 / 65536 (2148/s) | 276.4 / 524 / 1460697 (3594/s) | 64.0 / 128 / 64 (3594/s) |
| 2 | i | 125.1 / 131 / 1468149 (2299/s) | – | 470.3 / 1049 / 1468205 (2125/s) | 9451.5 / 131072 / 65536 (2125/s) | 27826.3 / 67109 / 1529913 (36/s) | 6400.0 / 8192 / 6400 (36/s) |
| 2 | p | 115.6 / 131 / 546473 (2300/s) | – | 463.7 / 1049 / 546529 (2156/s) | 9319.3 / 65536 / 65536 (2156/s) | 27824.4 / 67109 / 640024 (36/s) | 6400.0 / 8192 / 6400 (36/s) |

