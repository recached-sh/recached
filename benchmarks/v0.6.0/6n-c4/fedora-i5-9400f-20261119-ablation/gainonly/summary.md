| Cell | n | rps median | rps range | CPU µs/op | AOF write(2)/cmd | cmds/batch | p50 ms | p95 ms | p99 ms | lag p50 / p99 ms | converge | RSS MiB | errors |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|---:|---|
| rc-coalesced P=1 w=1 | 2 | 76,937 | 75,455–78,419 | 9.63 | 0.453 | 2.2 | 0.607 | 1.043 | 1.271 | 0.33 / 0.65 | ok | 62 | none |
| rc-coalesced P=16 w=1 | 2 | 186,027 | 181,686–190,367 | 4.83 | 0.907 | 1.1 | 3.743 | 8.143 | 10.951 | 0.41 / 1.08 | ok | 64 | none |

| Ratio (medians) | P=1 | P=16 |
|---|---:|---:|
| rc-coalesced / rc-standalone | n/a | n/a |
| rc-inline / rc-standalone | n/a | n/a |
| rc-coalesced / rc-inline | n/a | n/a |
| rc-coalesced / redis | n/a | n/a |
| rc-coalesced / valkey | n/a | n/a |

**M1: NOT VERIFIED** (a required cell is missing)
