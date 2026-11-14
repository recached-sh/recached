
#### get-p16 — single (median of n; ops/s [min–max])

| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |
|---|---:|---:|---:|---:|---:|---:|
| v0.6 final | 234,762 [226,372–236,002] n=3 | 3.343 | 4.415 | 6.399 | 4.26 | 0 |
| v0.6 final, no bg thread | 220,916 [210,396–239,088] n=3 | 3.391 | 4.575 | 6.751 | 4.32 | 0 |

#### hget-p16 — single (median of n; ops/s [min–max])

| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |
|---|---:|---:|---:|---:|---:|---:|
| v0.6 final | 223,191 [212,783–223,954] n=3 | 3.647 | 5.439 | 14.015 | 4.70 | 0 |
| v0.6 final, no bg thread | 243,687 [204,372–245,662] n=3 | 3.519 | 4.831 | 8.575 | 4.52 | 0 |

#### hset-p16 — single (median of n; ops/s [min–max])

| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |
|---|---:|---:|---:|---:|---:|---:|
| v0.6 final | 167,007 [141,316–187,272] n=3 | 4.671 | 6.559 | 10.751 | 5.99 | 0 |
| v0.6 final, no bg thread | 151,126 [148,434–167,663] n=3 | 4.671 | 7.391 | 12.351 | 6.02 | 0 |

#### incr-p16 — single (median of n; ops/s [min–max])

| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |
|---|---:|---:|---:|---:|---:|---:|
| v0.6 final | 165,481 [144,927–185,705] n=3 | 4.159 | 7.487 | 23.039 | 5.49 | 0 |
| v0.6 final, no bg thread | 184,563 [177,888–191,484] n=3 | 4.287 | 6.559 | 9.855 | 5.62 | 0 |

#### lat-fixed — single (median of n; ops/s [min–max])

| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |
|---|---:|---:|---:|---:|---:|---:|
| v0.6 final | 24,770 [23,334–25,737] n=3 | 1.631 | 3.551 | 10.175 | 35.09 | 0 |
| v0.6 final, no bg thread | 20,649 [18,854–23,191] n=3 | 1.647 | 3.519 | 6.047 | 38.38 | 0 |

#### lpush-p16 — single (median of n; ops/s [min–max])

| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |
|---|---:|---:|---:|---:|---:|---:|
| v0.6 final | 172,600 [152,607–187,667] n=3 | 4.159 | 5.887 | 7.583 | 5.33 | 0 |
| v0.6 final, no bg thread | 185,177 [179,182–201,322] n=3 | 4.287 | 7.551 | 12.351 | 5.52 | 0 |

#### mget-p16 — single (median of n; ops/s [min–max])

| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |
|---|---:|---:|---:|---:|---:|---:|
| v0.6 final | 137,574 [132,630–152,716] n=3 | 5.631 | 9.023 | 22.271 | 7.28 | 0 |
| v0.6 final, no bg thread | 132,779 [128,686–142,739] n=3 | 5.535 | 7.423 | 9.855 | 7.07 | 0 |

#### mix-p1 — single (median of n; ops/s [min–max])

| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |
|---|---:|---:|---:|---:|---:|---:|
| v0.6 final | 23,427 [23,411–26,584] n=3 | 1.863 | 2.639 | 8.319 | 38.45 | 0 |
| v0.6 final, no bg thread | 26,518 [25,481–27,655] n=3 | 1.815 | 4.191 | 12.543 | 39.00 | 0 |

#### mset-p16 — single (median of n; ops/s [min–max])

| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |
|---|---:|---:|---:|---:|---:|---:|
| v0.6 final | 98,297 [96,219–100,253] n=3 | 7.967 | 10.495 | 14.911 | 10.18 | 0 |
| v0.6 final, no bg thread | 98,404 [84,030–108,282] n=3 | 7.967 | 11.711 | 17.535 | 10.27 | 0 |

#### rpush-p16 — single (median of n; ops/s [min–max])

| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |
|---|---:|---:|---:|---:|---:|---:|
| v0.6 final | 191,111 [158,402–210,204] n=3 | 4.127 | 5.759 | 13.375 | 5.28 | 0 |
| v0.6 final, no bg thread | 183,697 [181,295–202,472] n=3 | 4.223 | 6.527 | 9.727 | 5.49 | 0 |

#### sadd-p16 — single (median of n; ops/s [min–max])

| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |
|---|---:|---:|---:|---:|---:|---:|
| v0.6 final | 168,640 [166,099–169,855] n=3 | 4.607 | 6.943 | 10.559 | 5.94 | 0 |
| v0.6 final, no bg thread | 170,508 [166,801–187,570] n=3 | 4.671 | 6.175 | 8.511 | 5.93 | 0 |

#### set-p16 — single (median of n; ops/s [min–max])

| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |
|---|---:|---:|---:|---:|---:|---:|
| v0.6 final | 167,104 [151,561–179,334] n=3 | 4.255 | 7.167 | 11.583 | 5.55 | 0 |
| v0.6 final, no bg thread | 185,773 [182,773–207,054] n=3 | 4.191 | 5.983 | 8.575 | 5.38 | 0 |

#### zadd-p16 — single (median of n; ops/s [min–max])

| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |
|---|---:|---:|---:|---:|---:|---:|
| v0.6 final | 156,940 [155,197–170,732] n=3 | 4.959 | 7.263 | 12.735 | 6.42 | 0 |
| v0.6 final, no bg thread | 153,622 [150,109–154,196] n=3 | 4.991 | 7.999 | 17.919 | 6.51 | 0 |
