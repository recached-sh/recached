# 6L dedicated-Linux run, 2026-11-17

Produced by `benchmarks/v0.6.0/6l/linux/run.sh` on the owner's Fedora machine. Build logs and server data directories are omitted. The username and hostname are redacted.

## Conditions

| | |
|---|---|
| CPU | Intel i5-9400F, 6 cores, no SMT, governor `performance`, turbo on |
| OS | Fedora kernel 7.3.3, 16 GB. The kit ran inside a **Flatpak runtime** (`os: Freedesktop SDK`), so virtualization reads "unknown" |
| CPU split | server on CPUs 0–2, `redis-benchmark` (2 threads) on 3–5. The server has **3 cores**, so 4 and 8 workers are oversubscribed |
| Load | 1.8 at start (5.5 over the prior 5 min), 0.7 at the end |
| Recached `6k` | `10c6854` |
| Recached `6l` | `5cc9367`: 6L-A + 6L-B. **No** 6L-C (sharded index), 6L-D (replication lock) or 6L-E |
| Redis / Valkey | 7.2.5 and 9.1.0, built from source, jemalloc 5.3.0 |

## What this run supports

### A. In-process capped writes (gated vs reservations), pinned to 3 cores

| | 1 thread | 4 threads | 8 threads |
|---|---:|---:|---:|
| overwrite, gated → reserved | 2.29M → 2.95M | 1.53M → **5.02M** (3.27×) | 1.66M → 4.97M |
| INCR, gated → reserved | 2.35M → 3.05M | 1.55M → **5.14M** (3.31×) | 1.62M → 4.99M |
| new keys, gated → reserved | 671k → 700k | 468k → 695k | 459k → 720k |
| new keys, uncapped (index ceiling) | 742k | 685k | 692k |

p99 at 4 threads: 20–22 µs gated, 0.75–0.77 µs reserved.

### B. Server capped SET/INCR at 1/2/4/8 workers (5 reps, tight spreads)

| SET ops/s | 6k | 6l, reservations off | 6l, reservations on | on / 6k |
|---:|---:|---:|---:|---:|
| 1 worker | 441k | 447k | 482k | 1.09× |
| 2 workers | 603k | 609k | 816k | 1.35× |
| 4 workers | 432k | 435k | 742k | 1.72× |
| 8 workers | 457k | 463k | 845k | 1.85× |

- INCR: the same shape, 1.95× at 8 workers.
- With reservations off, 6l matches 6k: no cost when off.
- p99 at 4–8 workers falls from 6.9–7.3 ms to 4.1–4.3 ms.

### C. Standard battery, 1 worker, no persistence

- **Throughput does not rank servers here.** At 100,000 requests per run, every server lands on the same quantized rates (≈44k, 133k, 400k), set by the load generator. Exceptions: MSET at 50 clients (Recached 80k / Redis 100k at P=1; 133k / 200k at P=16) and MGET.
- **CPU-seconds per million requests** is the comparable figure, with ±0.1 resolution (10 ms ticks over ~0.25 s runs). P=16, 50 clients, Recached 6l vs Redis:

| | GET | SET | HSET | INCR | ZADD | MGET | LPUSH | MSET | SADD |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Recached 6l | 0.6 | 0.8 | 0.8 | 0.8 | 1.3 | 3.1 | 0.9 | 5.0 | 1.1 |
| Redis | 0.8 | 0.8 | 0.9 | 0.7 | 2.1 | 3.4 | 0.7 | 3.9 | 0.7 |

### D. Same, AOF everysec. **New finding: writes cost ~1.6–2.2× Redis's CPU**

- HSET, LPUSH and ZADD reach only ~200k against Redis's ~400k, with p99 3.5 ms vs 1.6 ms (HSET).
- CPU per million: SET 2.1 vs 1.3, INCR 2.2 vs 1.1, HSET 2.4 vs 1.4, LPUSH 2.4 vs 1.2, SADD 1.3 vs 0.6, MSET 9.5 vs 5.8.
- GET is unaffected (0.7 vs 0.7).

### E. Settled RSS per key, 1M × 32-byte values

| | Recached 6l | Redis | Valkey | Recached / Redis |
|---|---:|---:|---:|---:|
| string | 203 | 110 | 84 | 1.85× |
| string + TTL | 282 | 147 | 123 | 1.92× |
| 3-field hash | 390 | 204 | 193 | 1.91× |

## Limits, and the next run

- Use `REQUESTS ≥ 2,000,000` (`run.sh` passes `PROFILE=quick` with 100k), so that throughput and CPU resolve.
- Use a stronger or second load generator. The current one saturates near 400k ops/s.
- Re-run against the final 6L `dev` (6L-C/D/E merged).
- Run outside Flatpak, or record why it was used.
