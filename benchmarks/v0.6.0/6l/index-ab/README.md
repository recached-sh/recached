# 6L-C A/B: one key index vs 16 shards, through the production store

`core-engine/src/store/index_bench.rs` is compiled into two trees:
- **base**: `dev` 43d5e0f (6K plus 6L-A/B), one index
- **new**: this branch, 16 shards

`run.sh <base-bin> <new-bin> [reps]` runs them alternately, swapping which goes first on each repetition. `summarize.py <jsonl>` prints the medians with the min–max spread.

Host: macOS laptop, i5-8259U (4 cores / 8 threads), load 3–17 from other work on the same host. These numbers are indicative only.

## `macos-i5-8259u.jsonl` (3 reps)

| Measure | Base | New | New / base |
|---|---:|---:|---:|
| New keys, uncapped, 1 / 2 / 4 / 8 threads | 281k / 136k / 129k / 151k per s | 289k / 331k / 618k / 715k per s | 1.03 / 2.44 / 4.81 / 4.73 |
| New keys, capped + reservations, 1 / 4 / 8 threads | 216k / 112k / 109k per s | 228k / 471k / 588k per s | 1.06 / 4.21 / 5.38 |
| SCAN full traversal, 1M keys, COUNT 10 / 100 / 1000 | 1.23 / 1.03 / 0.76 s | 1.03 / 0.80 / 0.91 s | 0.84 / 0.78 / 1.20 |

## QSUB snapshots

Files: `macos-qsub-scatter.jsonl` (5 reps) and `macos-qsub-recheck.jsonl` (7 reps). The keyspace is 1,000 tenants × 1,000 keys.

| Pattern | Keys matched | Base | New | New / base |
|---|---:|---:|---:|---:|
| `t:42:*` | 1,000 | 151 µs | 199 µs | 1.32 |
| `t:4*` | 111,000 | 35.6 ms | 47.8 ms | 1.34 |
| `t:42:*`, keys created in scattered order | 1,000 | 156 µs | 202 µs | 1.30 |
| `t:4*`, keys created in scattered order | 111,000 | 35.4 ms | 46.6 ms | 1.32 |

**What was ruled out:**
- *Creation order.* Scattered creation changes nothing.
- *Extra work.* Both builds examine exactly the matched count.
- *My code rather than sharding.* The new code built with `INDEX_SHARDS = 1` matches base: 34.8 vs 34.3 ms, and 151 vs 160 µs.

So the ~1.3× is the cost of walking 16 trees instead of one. Its micro-architectural cause was not isolated.

> An earlier 3-rep run reported 0.91× for the narrow prefix. That was noise: its base spread was 147–339 µs.
