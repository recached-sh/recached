# r1: 24 runs, 24 valid

## raof-10

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 920 | 920 | 1.0000 | 91994 | 177.7 | 402.6 | 0.37 | 1 | 1 | 71600 | 0.00 | 2.8 | 36779 | 0.244 | 246 | 6.6 | 0.30 | 1039 | digest=eq primary=true replica=true restart=true |
| 1 | i | 920 | 920 | 1.0000 | 91999 | 182.2 | 145.3 | 0.13 | 0 | 0 | 80500 | 0.00 | 56.2 | 7384 | 0.052 | 230 | 7.0 | 0.25 | 625 | digest=eq primary=true replica=true restart=true |
| 1 | p | 920 | 920 | 1.0000 | 91999 | 184.8 | 142.1 | 0.13 | 0 | 0 | 200 | 0.00 | 45.8 | 6015 | 0.042 | 218 | 7.0 | 0.25 | 761 | digest=eq primary=true replica=true restart=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 31.4 | 59.2 | 58.0 | 62.4 | -1.06 | 58.8 57.6 57.6 | 57.6 | 57.6 | 81.7 | 50.7 | 40.8 |
| 1 | i | 31.7 | 66.1 | 65.7 | 66.1 | -0.02 | 65.7 65.7 65.7 | 65.7 | 65.7 | 75.1 | 58.1 | 36.9 |
| 1 | p | 31.4 | 57.5 | 57.3 | 57.5 | -0.00 | 57.3 57.3 57.3 | 57.3 | 57.3 | 59.7 | 50.2 | 32.6 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 1.000 [1.000–1.000] | 1.025 [1.025–1.025] | 0.361 [0.361–0.361] | 20.382 [20.382–20.382] | 1.132 [1.132–1.132] | 1.059 [1.059–1.059] | 1.140 [1.140–1.140] |
| p | 1.000 [1.000–1.000] | 1.040 [1.040–1.040] | 0.353 [0.353–0.353] | 16.604 [16.604–16.604] | 0.988 [0.988–0.988] | 0.922 [0.922–0.922] | 0.995 [0.995–0.995] |

## raof-15

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 1380 | 1380 | 1.0000 | 137997 | 164.9 | 423.2 | 0.58 | 0 | 0 | 123200 | 0.00 | 2.5 | 33232 | 0.343 | 186 | 10.3 | 0.21 | 925 | digest=eq primary=true replica=true restart=true |
| 1 | i | 1380 | 1380 | 1.0000 | 137997 | 162.2 | 140.2 | 0.19 | 0 | 0 | 200 | 0.00 | 32.4 | 4255 | 0.045 | 180 | 10.5 | 0.21 | 804 | digest=eq primary=true replica=true restart=true |
| 1 | p | 1380 | 1380 | 1.0000 | 137994 | 162.0 | 135.4 | 0.19 | 0 | 0 | 300 | 0.00 | 43.8 | 5754 | 0.060 | 232 | 10.5 | 0.26 | 818 | digest=eq primary=true replica=true restart=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 47.8 | 57.9 | 57.4 | 57.9 | -0.03 | 57.4 57.4 57.4 | 57.4 | 57.2 | 59.7 | 50.0 | 32.3 |
| 1 | i | 56.0 | 66.2 | 65.8 | 66.2 | -0.03 | 65.8 65.8 65.8 | 65.8 | 65.8 | 76.4 | 58.1 | 28.7 |
| 1 | p | 47.8 | 57.5 | 57.4 | 57.5 | -0.01 | 57.4 57.3 57.4 | 57.4 | 57.4 | 59.6 | 50.1 | 32.0 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 1.000 [1.000–1.000] | 0.984 [0.984–0.984] | 0.331 [0.331–0.331] | 12.997 [12.997–12.997] | 1.146 [1.146–1.146] | 1.144 [1.144–1.144] | 1.151 [1.151–1.151] |
| p | 1.000 [1.000–1.000] | 0.982 [0.982–0.982] | 0.320 [0.320–0.320] | 17.577 [17.577–17.577] | 0.999 [0.999–0.999] | 0.994 [0.994–0.994] | 1.003 [1.003–1.003] |

## raof-25

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 2300 | 2300 | 1.0000 | 229992 | 157.3 | 356.3 | 0.82 | 2 | 2 | 18587100 | 0.00 | 2.5 | 33504 | 0.537 | 265 | 16.0 | 0.30 | 1938 | digest=eq primary=true replica=true restart=true |
| 1 | i | 2300 | 2300 | 1.0000 | 229991 | 155.8 | 123.2 | 0.28 | 4 | 4 | 3635400 | 0.00 | 56.7 | 7449 | 0.124 | 265 | 16.7 | 0.30 | 2049 | digest=eq primary=true replica=true restart=true |
| 1 | p | 2300 | 2300 | 1.0000 | 229997 | 150.6 | 112.0 | 0.26 | 2 | 2 | 9385900 | 0.00 | 70.2 | 9227 | 0.152 | 314 | 16.4 | 0.37 | 1895 | digest=eq primary=true replica=true restart=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 31.5 | 59.2 | 58.8 | 62.6 | 0.16 | 57.8 60.0 58.6 | 57.9 | 57.9 | 81.7 | 51.3 | 70.3 |
| 1 | i | 31.6 | 75.5 | 67.9 | 82.6 | -4.40 | 71.2 66.4 66.1 | 66.1 | 66.1 | 91.7 | 60.2 | 50.9 |
| 1 | p | 31.3 | 57.4 | 59.1 | 69.7 | -0.45 | 59.6 60.0 57.7 | 57.6 | 57.6 | 84.5 | 51.9 | 56.4 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 1.000 [1.000–1.000] | 0.990 [0.990–0.990] | 0.346 [0.346–0.346] | 22.572 [22.572–22.572] | 1.155 [1.155–1.155] | 1.319 [1.319–1.319] | 1.143 [1.143–1.143] |
| p | 1.000 [1.000–1.000] | 0.958 [0.958–0.958] | 0.314 [0.314–0.314] | 27.957 [27.957–27.957] | 1.005 [1.005–1.005] | 1.113 [1.113–1.113] | 0.995 [0.995–0.995] |

## raof-40

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 3680 | 3680 | 1.0000 | 367988 | 131.6 | 103.5 | 0.38 | 21 | 21 | 37099200 | 1.00 | 2.8 | 37134 | 0.285 | 202 | 7.7 | 0.43 | 5212 | digest=eq primary=true replica=true restart=true |
| 1 | i | 3680 | 3680 | 1.0000 | 367999 | 139.6 | 78.3 | 0.29 | 16 | 16 | 36731500 | 0.25 | 70.3 | 9229 | 0.168 | 221 | 18.2 | 0.37 | 5029 | digest=eq primary=true replica=true restart=true |
| 1 | p | 3680 | 3680 | 1.0000 | 367992 | 137.6 | 57.0 | 0.21 | 22 | 22 | 38641000 | 0.25 | 103.1 | 13549 | 0.180 | 211 | 13.3 | 0.41 | 8082 | digest=eq primary=true replica=true restart=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 31.4 | 65.3 | 67.1 | 75.8 | 0.25 | 66.0 68.9 66.2 | 67.9 | 58.7 | 84.8 | 59.7 | 108.5 |
| 1 | i | 31.4 | 74.5 | 74.3 | 87.5 | 4.49 | 71.6 74.1 77.3 | 71.7 | 65.7 | 96.3 | 67.1 | 97.5 |
| 1 | p | 31.0 | 72.5 | 68.2 | 75.9 | -0.70 | 69.2 66.5 68.8 | 75.6 | 57.2 | 87.9 | 61.1 | 82.5 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 1.000 [1.000–1.000] | 1.061 [1.061–1.061] | 0.756 [0.756–0.756] | 25.229 [25.229–25.229] | 1.108 [1.108–1.108] | 1.155 [1.155–1.155] | 1.119 [1.119–1.119] |
| p | 1.000 [1.000–1.000] | 1.046 [1.046–1.046] | 0.551 [0.551–0.551] | 37.033 [37.033–37.033] | 1.017 [1.017–1.017] | 1.002 [1.002–1.002] | 0.974 [0.974–0.974] |

## raof-55

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 5060 | 5060 | 1.0000 | 505978 | 121.1 | 87.6 | 0.44 | 19 | 19 | 52024500 | 5.51 | 2.8 | 37433 | 0.323 | 217 | 8.6 | 0.38 | 10452 | digest=eq primary=true replica=true restart=true |
| 1 | i | 5060 | 5060 | 1.0000 | 505985 | 123.4 | 40.1 | 0.20 | 22 | 22 | 50455100 | 1.25 | 117.8 | 15474 | 0.206 | 318 | 13.3 | 0.57 | 13793 | digest=eq primary=true replica=true restart=true |
| 1 | p | 5060 | 5060 | 1.0000 | 505992 | 122.8 | 38.3 | 0.19 | 21 | 21 | 51074800 | 1.26 | 160.1 | 21027 | 0.277 | 315 | 13.2 | 0.59 | 12307 | digest=eq primary=true replica=true restart=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 59.3 | 57.3 | 67.3 | 78.0 | 0.66 | 66.8 68.2 67.0 | 67.0 | 60.7 | 87.1 | 60.1 | 111.1 |
| 1 | i | 31.2 | 82.6 | 77.3 | 87.0 | -0.73 | 78.3 74.8 78.9 | 69.2 | 74.9 | 97.4 | 70.2 | 125.3 |
| 1 | p | 31.2 | 71.8 | 67.2 | 80.7 | -0.96 | 66.2 68.1 67.4 | 65.0 | 66.8 | 88.5 | 60.1 | 122.2 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 1.000 [1.000–1.000] | 1.019 [1.019–1.019] | 0.458 [0.458–0.458] | 41.956 [41.956–41.956] | 1.147 [1.147–1.147] | 1.115 [1.115–1.115] | 1.234 [1.234–1.234] |
| p | 1.000 [1.000–1.000] | 1.014 [1.014–1.014] | 0.438 [0.438–0.438] | 57.014 [57.014–57.014] | 0.998 [0.998–0.998] | 1.034 [1.034–1.034] | 1.102 [1.102–1.102] |

## raof-70

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 6440 | 6440 | 1.0000 | 643983 | 116.7 | 70.0 | 0.45 | 16 | 16 | 67502800 | 2.25 | 3.1 | 41150 | 0.374 | 270 | 9.1 | 0.49 | 19651 | digest=eq primary=true replica=true restart=true |
| 1 | i | 6440 | 6440 | 1.0000 | 643969 | 119.9 | 34.8 | 0.22 | 22 | 22 | 65569600 | 4.01 | 95.4 | 12537 | 0.184 | 294 | 14.7 | 0.55 | 22019 | digest=eq primary=true replica=true restart=true |
| 1 | p | 6440 | 6440 | 1.0000 | 643973 | 120.6 | 35.2 | 0.23 | 23 | 23 | 64525000 | 1.50 | 93.3 | 12255 | 0.189 | 290 | 15.4 | 0.56 | 22170 | digest=eq primary=true replica=true restart=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 60.0 | 62.1 | 66.6 | 77.4 | 4.20 | 63.4 68.9 67.6 | 77.4 | 57.5 | 86.4 | 59.1 | 116.0 |
| 1 | i | 31.1 | 79.1 | 77.0 | 85.9 | 0.70 | 76.9 76.0 78.0 | 73.6 | 71.4 | 94.1 | 69.9 | 109.5 |
| 1 | p | 31.5 | 64.1 | 67.6 | 77.8 | 2.31 | 66.4 67.1 69.5 | 65.7 | 61.7 | 90.6 | 60.2 | 111.8 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 1.000 [1.000–1.000] | 1.027 [1.027–1.027] | 0.498 [0.498–0.498] | 30.924 [30.924–30.924] | 1.155 [1.155–1.155] | 1.111 [1.111–1.111] | 1.240 [1.240–1.240] |
| p | 1.000 [1.000–1.000] | 1.033 [1.033–1.033] | 0.502 [0.502–0.502] | 30.229 [30.229–30.229] | 1.014 [1.014–1.014] | 1.006 [1.006–1.006] | 1.072 [1.072–1.072] |

## raof-85

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 7820 | 7818 | 0.9998 | 781804 | 113.2 | 64.2 | 0.50 | 13 | 13 | 81951100 | 2.76 | 3.1 | 41803 | 0.421 | 271 | 10.1 | 0.43 | 31952 | digest=eq primary=true replica=true restart=true |
| 1 | i | 7820 | 7823 | 1.0004 | 782321 | 115.0 | 28.2 | 0.22 | 20 | 20 | 80226800 | 3.26 | 120.8 | 15873 | 0.246 | 375 | 15.5 | 0.60 | 54994 | digest=eq primary=true replica=true restart=true |
| 1 | p | 7820 | 7820 | 1.0000 | 781973 | 115.8 | 29.4 | 0.23 | 22 | 22 | 77245700 | 2.51 | 107.5 | 14126 | 0.234 | 355 | 16.6 | 0.62 | 51127 | digest=eq primary=true replica=true restart=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 59.8 | 64.2 | 66.9 | 74.6 | -2.32 | 68.7 65.5 66.6 | 70.1 | 57.7 | 83.5 | 59.4 | 146.8 |
| 1 | i | 31.5 | 76.9 | 76.8 | 84.9 | 2.70 | 75.6 77.1 77.8 | 76.5 | 68.1 | 94.0 | 69.6 | 149.8 |
| 1 | p | 31.0 | 68.8 | 67.5 | 75.9 | -0.45 | 68.2 67.1 67.1 | 68.6 | 56.5 | 87.4 | 60.4 | 147.5 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 1.001 [1.001–1.001] | 1.016 [1.016–1.016] | 0.439 [0.439–0.439] | 38.542 [38.542–38.542] | 1.148 [1.148–1.148] | 1.139 [1.139–1.139] | 1.181 [1.181–1.181] |
| p | 1.000 [1.000–1.000] | 1.023 [1.023–1.023] | 0.458 [0.458–0.458] | 34.300 [34.300–34.300] | 1.008 [1.008–1.008] | 1.018 [1.018–1.018] | 0.980 [0.980–0.980] |

## raof-100

| rep | label | offered/s | accepted/s | acc ratio | frames/s | primary µs/tx | replica µs/tx | replica CPUs | full syncs (after prefill) | queue disconnects (after prefill) | lag max | catch-up s | r AOF µs/write | r write(2) µs/MB | r write(2) share | r ms/fsync | r AOF MB/s | disk util | due p99 µs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 9200 | 8936 | 0.9712 | 893555 | 107.7 | 55.5 | 0.50 | 15 | 15 | 90023500 | 1.27 | 2.9 | 39014 | 0.379 | 260 | 9.7 | 0.44 | 5055277 | digest=eq primary=true replica=true restart=true |
| 1 | i | 9200 | 8822 | 0.9589 | 882175 | 109.0 | 26.2 | 0.23 | 23 | 23 | 95214400 | 3.26 | 113.9 | 14959 | 0.238 | 342 | 15.9 | 0.60 | 6999675 | digest=eq primary=true replica=true restart=true |
| 1 | p | 9200 | 8811 | 0.9577 | 881106 | 109.1 | 27.0 | 0.24 | 21 | 21 | 94369800 | 6.01 | 108.5 | 14253 | 0.249 | 375 | 17.5 | 0.62 | 6214539 | digest=eq primary=true replica=true restart=true |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 59.6 | 67.3 | 63.2 | 72.2 | 1.08 | 63.0 62.0 64.6 | 64.2 | 62.5 | 83.6 | 55.9 | 136.0 |
| 1 | i | 76.3 | 74.8 | 75.7 | 84.0 | 0.52 | 74.9 77.0 75.1 | 80.9 | 64.6 | 93.1 | 68.5 | 124.3 |
| 1 | p | 59.3 | 63.8 | 67.2 | 80.2 | -1.54 | 67.9 67.1 66.6 | 65.3 | 58.5 | 89.6 | 60.1 | 140.7 |

Paired per rep against h (median [range]):

| label | accepted_per_s | primary_cpu_us_per_tx | replica_cpu_us_per_tx | r_aof_us_per_write | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|
| i | 0.987 [0.987–0.987] | 1.012 [1.012–1.012] | 0.472 [0.472–0.472] | 38.917 [38.917–38.917] | 1.197 [1.197–1.197] | 1.164 [1.164–1.164] | 1.033 [1.033–1.033] |
| p | 0.986 [0.986–0.986] | 1.013 [1.013–1.013] | 0.487 [0.487–0.487] | 37.081 [37.081–37.081] | 1.063 [1.063–1.063] | 1.111 [1.111–1.111] | 0.936 [0.936–0.936] |

