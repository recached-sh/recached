# 6L-B capped-store reservations, in-process

`bench_reserved_writes` in `core-engine/src/store/reservation_tests.rs`:

```
cargo test --release -p core-engine --lib bench_reserved_writes -- --ignored --nocapture
BENCH_WORKLOADS=new_keys,new_keys_uncapped   # optional filter
```

**Setup**
- One store per run, with 200k resident keys and 1,024 hot keys per thread.
- 400k operations split across 1/2/4/8 threads.
- Gated (reservations off) and reserved runs are interleaved; the table is the median of 5.
- Latency is sampled every 32nd operation.
- Caps sit far above the workload (50M keys, 16 GiB), so this measures admission, not eviction. That is why the fallback rate is 0. Fallback near the cap is exercised by the race tests.

**Workloads**

| Workload | What it does |
|---|---|
| `new_keys` | `SET` of a new key on each operation |
| `overwrite` | `SET` on the thread's hot keys |
| `incr` | `INCR` on the thread's hot keys |
| `new_keys_uncapped` | `new_keys` on a store with no caps, as the ceiling reference |

`macos-i5-8259u.csv` is from a 4-core/8-thread laptop at load average ~5. These numbers are **indicative only**; release claims need the dedicated-Linux run.
