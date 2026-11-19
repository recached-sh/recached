| Cell | n | rps median | rps range | CPU µs/op | AOF write(2)/cmd | cmds/batch | p50 ms | p95 ms | p99 ms | lag p50 / p99 ms | converge | RSS MiB | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|---:|---|
| rc-coalesced P=1 w=1 | 2 | 99,980 | 99,980–99,980 | 7.18 | 0.069 | 14.4 | 0.479 | 0.663 | 0.775 | 0.10 / 0.48 | ok | 62 | none |
| rc-coalesced P=16 w=1 | 2 | 216,165 | 210,305–222,025 | 3.65 | 0.037 | 27.0 | 3.507 | 4.507 | 5.083 | 0.10 / 0.51 | ok | 67 | none |
| rc-inline P=16 w=1 | 2 | 177,740 | 173,762–181,719 | 4.81 | 1.000 | 1.0 | 4.183 | 4.527 | 4.987 | 1.52 / 3.69 | ok | 64 | none |

| Ratio (medians) | P=1 | P=16 |
|---|---:|---:|
| rc-coalesced / rc-standalone | n/a | n/a |
| rc-inline / rc-standalone | n/a | n/a |
| rc-coalesced / rc-inline | n/a | 1.216 |
| rc-coalesced / redis | n/a | n/a |
| rc-coalesced / valkey | n/a | n/a |

**M1: NOT VERIFIED** (a required cell is missing)
