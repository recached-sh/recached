# raw/f0-reproduction

| File | Build | Note |
|---|---|---|
| `counter-tests-on-410235b.log`, `repeat-5-on-410235b.log` | `410235b` + the reproduction tests (`0dbf5d4`) | 10/15 failing in 5 runs out of 5 (the delta probe enabled in the first) |
| `f1-probe-on-410235b.log` | `410235b` | 6O-E F1 probe: 10 runs, `[new, old]` each time |
| `redis-diff.md` | Redis 7.2.5, `rc-base` (`410235b`), **`rc-ttl` (`e55f7d5`, the rejected counter-frame fix)** | `base` brings the fixed-window counter back permanent. The `ttl` column is evidence for the rejected design only; it was not shipped |
