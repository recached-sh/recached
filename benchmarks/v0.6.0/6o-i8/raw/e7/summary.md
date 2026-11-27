# e7: 3 runs, 3 valid

Runs excluded from paired ratios (invalid, a failed check, or a replica resync during a contention load):

- rep 1 c10k-repl-w4-diag2 h: valid=True ['replica disconnected 16x / resynchronized 16x during the load']

## c10k-repl-w4-diag2

| rep | label | tx/s | tx p99 ms | writer ops/s | writer p99 µs | writer max µs | primary µs/tx | primary Minstr/tx | replica µs/tx | lag max | full syncs (after prefill) | catch-up s | generator CPUs | foreign CPUs | checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 110.55 | 22 | 1284 | 12336 | 41003 | 11925 | – | 6846 | 152209707 | 16 | 0.50 | 0.42 | 0.42 | digest=eq |
| 1 | p | 95.82 | 26 | 466 | 20665 | 33908 | 12750 | – | 7078 | 20005 | 0 | 0.25 | 0.37 | 0.40 | digest=eq |
| 1 | px | 100.67 | 25 | 525 | 18720 | 29659 | 12476 | – | 7143 | 20078 | 0 | 0.25 | 0.38 | 0.39 | digest=eq |

| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | 87.3 | 87.5 | 94.1 | 108.2 | 1.22 | 94.5 92.1 94.9 94.7 | 100.4 | 63.5 | 118.1 | 85.0 | 110.4 |
| 1 | p | 59.7 | 126.0 | 121.0 | 140.4 | -5.18 | 123.9 119.7 125.0 115.2 | 108.3 | 60.7 | 141.9 | 113.1 | 30.4 |
| 1 | px | 60.1 | 82.6 | 83.2 | 86.2 | 0.30 | 83.0 83.1 83.2 83.5 | 84.2 | 61.9 | 85.9 | 75.0 | 30.5 |

Paired per rep against h (median [range]):

| label | tx_per_s | primary_cpu_us_per_tx | primary_instr_per_tx | writer_ops_per_s | w_p99_us | replica_cpu_us_per_tx | RSS window mean | RSS window max | RSS post end |
|---|---|---|---|---|---|---|---|---|---|
| p | – | – | – | – | – | – | – | – | – |
| px | – | – | – | – | – | – | – | – | – |

Diag (primary, measured window; times in µs (waiters_at_release: a count): mean / upper bound of the log2 bucket holding the 99th percentile / max; n per s):

| rep | label | stream_hold_g | stream_hold_s16 | stream_wait_g | stream_wait_s | stream_free_s | stripe_wait_big | stripe_hold_big | stripe_wait_small | stripe_hold_small | stripe_wait_all | stripe_hold_all | stripe_gap | waiters_at_release | gate_close | tx_apply | tx_log |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | h | – | – | – | 0.4 / 4 / 16 (14/s) | – | – | – | 2758.8 / 16777 / 40883 (1286/s) | 5.0 / 16 / 2157 (1286/s) | 3283.4 / 8389 / 45840 (111/s) | 8745.6 / 16777 / 31644 (111/s) | 289.9 / 8389 / 9045 (111/s) | 3.91 / 8 / 4 (111/s) | 0.3 / 1 / 6 (111/s) | 6073.8 / 8389 / 18591 (111/s) | 1296.1 / 2097 / 3316 (111/s) |
| 1 | p | 1479.2 / 2097 / 3841 (96/s) | – | – | 0.9 / 8 / 16 (23/s) | – | – | – | 7987.4 / 16777 / 22108 (466/s) | 6.6 / 33 / 701 (466/s) | 4390.9 / 16777 / 13941 (96/s) | 10312.6 / 16777 / 22003 (96/s) | 124.5 / 1049 / 8072 (96/s) | 3.87 / 8 / 4 (96/s) | 0.3 / 1 / 5 (96/s) | 7097.6 / 16777 / 14247 (96/s) | 1713.1 / 4194 / 4311 (96/s) |
| 1 | px | 1208.2 / 2097 / 3342 (101/s) | – | – | 0.7 / 8 / 19 (26/s) | – | – | – | 7100.8 / 16777 / 17540 (525/s) | 6.1 / 16 / 1575 (525/s) | 3845.5 / 16777 / 12161 (101/s) | 9802.4 / 16777 / 16449 (101/s) | 131.2 / 2097 / 8352 (101/s) | 3.88 / 8 / 4 (101/s) | 0.3 / 1 / 59 (101/s) | 7083.1 / 16777 / 13514 (101/s) | 1214.7 / 2097 / 3349 (101/s) |

jemalloc (primary, measured window, MiB mean/max): allocated, active, resident, retained, dirty pages; mallocs/s

| rep | label | allocated | active | resident | retained | pdirty (pages) | mallocs/s | frees/s |
|---|---|---|---|---|---|---|---|---|
| 1 | h | 54.4 / 73.7 | 58.1 / 77.8 | 91.4 / 110.3 | 58.6 / 73.4 | 6933 / 9638 | 4450559 | 4450458 |
| 1 | p | 54.5 / 59.5 | 56.6 / 62.2 | 118.1 / 135.2 | 79.4 / 108.2 | 14196 / 18859 | 5760564 | 5760622 |
| 1 | px | 54.5 / 58.5 | 57.9 / 61.7 | 82.5 / 85.3 | 20.6 / 31.7 | 4809 / 6077 | 4040993 | 4040692 |

Diag (replica, measured window; mean / p99 bucket / max, µs or a value; n per s):

| rep | label | r_group_apply | r_frame_apply16 | r_read_gap | r_read_bytes | r_apply_run | r_apply_run_frames |
|---|---|---|---|---|---|---|---|
| 1 | h | – | 1.0 / 2 / 702 (43856/s) | 535.2 / 8389 / 21833 (1226/s) | 49726.6 / 131072 / 65536 (1226/s) | 66.4 / 131 / 2254 (10964/s) | 64.0 / 128 / 64 (10964/s) |
| 1 | p | 6971.0 / 16777 / 18230 (96/s) | 4.2 / 16 / 26 (29/s) | 622.2 / 16777 / 18562 (1607/s) | 51912.0 / 131072 / 65536 (1607/s) | 113823.7 / 268435 / 179297 (9/s) | 109116.2 / 262144 / 160048 (9/s) |
| 1 | px | 7031.5 / 16777 / 12812 (101/s) | 4.1 / 16 / 25 (33/s) | 610.1 / 16777 / 16086 (1639/s) | 53481.1 / 131072 / 65536 (1639/s) | 102499.2 / 268435 / 167491 (10/s) | 103240.6 / 262144 / 160048 (10/s) |

