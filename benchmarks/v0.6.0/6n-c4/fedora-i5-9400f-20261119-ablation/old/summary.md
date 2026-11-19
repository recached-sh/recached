| Cell | n | rps median | rps range | CPU µs/op | AOF write(2)/cmd | cmds/batch | p50 ms | p95 ms | p99 ms | lag p50 / p99 ms | converge | RSS MiB | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|---:|---|
| rc-coalesced P=1 w=1 | 2 | 89,887 | 88,873–90,901 | 8.14 | 0.213 | 4.7 | 0.531 | 0.727 | 0.863 | 0.22 / 0.46 | ok | 62 | none |
| rc-coalesced P=16 w=1 | 2 | 114,556 | 108,003–121,109 | 6.90 | 0.823 | 1.2 | 6.323 | 8.023 | 9.043 | 0.10 / 0.53 | ok | 59 | none |

| Ratio (medians) | P=1 | P=16 |
|---|---:|---:|
| rc-coalesced / rc-standalone | n/a | n/a |
| rc-inline / rc-standalone | n/a | n/a |
| rc-coalesced / rc-inline | n/a | n/a |
| rc-coalesced / redis | n/a | n/a |
| rc-coalesced / valkey | n/a | n/a |

**M1: NOT VERIFIED** (a required cell is missing)
