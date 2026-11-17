# 6L dedicated-Linux run, final, 2026-11-17

This is the second run on the owner's Fedora machine, made with `benchmarks/v0.6.0/6l/linux/run.sh`. It supersedes the first run in `../fedora-i5-9400f-20261117/`:
- The `6l` build is the final 6L `dev`: 6L-A..E, #110–#116.
- Each battery run makes 2,000,000 requests instead of 100,000.

Server data directories are omitted. The username is redacted (`/home/<user>`). `E-memory/summary.md` was generated afterwards with `scripts/bench-summarize.py`, because the kit does not write it for phase E.

## Conditions

| | |
|---|---|
| CPU | Intel i5-9400F, 6 cores, no SMT, governor `performance`, turbo on |
| OS | Fedora kernel 7.3.3, 16 GB. Run again inside a **Flatpak runtime**. An editor and agent CLIs were open (top CPU 4.5%) |
| CPU split | server on CPUs 0–2; `redis-benchmark` (2 threads) on 3–5. 4 and 8 workers are oversubscribed |
| Load | C 2.1 at start, D 0.9, E 1.8; 0.7 at the end |
| Recached `6k` | `10c6854` |
| Recached `6l` | `159d7e9` (`dev` after #116). Does not include #117 (soak harness) or #118 (SCAN cursor budget, promotion); neither touches a measured path |
| Redis / Valkey | 7.2.5 and 9.1.0, built from source, jemalloc 5.3.0 |
| Requests | battery 2,000,000 × 5 reps (E: 3); B 1,000,000 × 5 reps |

## Headline numbers

### A. In-process capped writes, 4 threads (gated → reserved)

| | ops/s |
|---|---:|
| overwrite | 1.48M → 4.83M |
| INCR | 1.51M → 5.03M |
| new keys | 439k → 1.24M |
| new keys, uncapped | 1.31M (first run, before the sharded index: 685k) |

### B. Server capped SET, median of 5

| SET ops/s | 6k | 6l, reservations off | 6l, reservations on | on / 6k |
|---:|---:|---:|---:|---:|
| 1 worker | 443k | 445k | 493k | 1.11× |
| 2 workers | 603k | 605k | 866k | 1.44× |
| 4 workers | 433k | 432k | 1,089k | **2.51×** |
| 8 workers | 458k | 457k | 1,091k | 2.38× |

INCR has the same shape (2.52× at 4 workers). p99 at 4 workers falls from 7.0 to 3.8 ms.

### C. Battery, 1 worker, no persistence (Recached 6l / Redis)

| | GET | SET | INCR | LPUSH | SADD | MSET | ZADD |
|---|---:|---:|---:|---:|---:|---:|---:|
| throughput, P=1, 50 clients | 0.86 | 0.83 | 0.84 | 0.71 | 0.70 | 0.76 | 0.86 |
| CPU per op, P=1, 50 clients | 1.14 | 1.20 | 1.20 | 1.46 | 1.51 | 1.32 | 1.10 |

With 1 client, Recached is 1.03–1.10× Redis's throughput. 6l matches 6k on every row. Near 1M ops/s, `redis-benchmark` rates still quantize in ~11% steps.

### D. AOF `everysec`

Extra CPU-seconds per million SETs, compared with no persistence:

| | Recached | Redis |
|---|---:|---:|
| 1 client | +10.3 | +6.9 |
| 50 clients | **+8.4** | **+0.8** |

- With one client, both servers pay about the same per write.
- With 50 clients, Redis covers every ready client with one `write(2)` per event-loop pass. Recached still writes once per connection flush.

### E. Settled RSS per key, 1M × 32 B

| B/key | Recached 6l | Redis | Valkey | 6l / Redis |
|---|---:|---:|---:|---:|
| string | 208 | 110 | 84 | 1.89× |
| hash, 3 fields | 385 | 205 | 193 | 1.88× |
| string + TTL | 291 | 147 | 123 | 1.98× |

No server log contains `ERROR` or `panicked`.
