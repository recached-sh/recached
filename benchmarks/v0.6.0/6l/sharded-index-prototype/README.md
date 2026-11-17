# 6L-C: sharded ordered index with a per-cursor merge frontier

This is 6J-B's standalone prototype (`benchmarks/v0.6.0/6j/scan-index-prototype` on branch `fix/v0.6j-scan-cursor-eviction`) extended for 6L.

**Variants**
- `global`: the production `Mutex<BTreeSet<Arc<str>>>`.
- `sharded16`: 6J's 16 hash shards. Every SCAN call seeks all 16 shards and k-way merges them.
- `sharded16c<B>` (new): the same shards, but each cursor keeps a **merge frontier** between calls:
  - per shard, a buffer of up to B keys plus the position to refill from;
  - the shards with keys, ordered by their next key.
  - A shard with an empty buffer that may still hold keys is refilled under its own lock before the merge continues, so output stays in byte order.
  - A call locks only the shards it refills, not all 16.
  - Merge comparisons go through each key's first 16 bytes as a big-endian `u128`, falling back to full key comparison on ties. That ordering is exact.
  - The proof that keys present for the whole iteration come back exactly once and in order is in the doc comment on `Frontier` in `src/main.rs`.

**Run**
```
cargo build --release
B=target/release/sharded-index-prototype
$B check sharded16c32
$B scan sharded16c32 1000000 10
$B create sharded16c32 1000000 8
```
`SCAN_REPEAT=n` repeats untimed traversals first, for profiling.

## Results: 1M keys, macOS i5-8259U (4C/8T), medians of 3 interleaved; indicative

**SCAN full traversal**

| COUNT | global | sharded16 | sharded16c32 | sharded16c64 |
|---:|---:|---:|---:|---:|
| 10 | 0.238 s | 1.832 s (7.7×) | 0.256 s (**1.07×**) | 0.258 s (1.08×) |
| 100 | 0.128 s | 0.487 s (3.8×) | 0.237 s (1.85×) | 0.238 s (1.86×) |
| 1000 | 0.131 s | 0.390 s (3.0×) | 0.268 s (2.05×) | 0.259 s (1.98×) |

**Key creation**

| Threads | global | sharded (×) | CPU-s per M creates | Live B/key |
|---:|---:|---:|---|---|
| 1 | 409k/s | 405k/s (0.99) | 2.39 → 2.44 | 182.6 → 183.3 |
| 4 | 182k/s | 605k/s (3.32) | 11.84 → 4.26 | |
| 8 | 177k/s | 824k/s (4.65) | 14.47 → 5.11 | 182.7 → 183.4 |

**Correctness**
- `check`: 20k stable keys while 3 threads churn keys before, between and after them, at COUNT 1/7/10/100/1000. 0 failures for every variant.
- Mutations (shard marked exhausted too early; refilled shard not re-placed; never refilled) each fail 5 of 5 COUNT settings.

**What it took to get from 6.8× to 1.07× at COUNT 10**

| Step | COUNT 10 |
|---|---:|
| frontier cache (no per-call seek of 16 shards) | 1.6× |
| move keys out of buffers instead of a heap of cloned keys | ~1.8× (no gain) |
| `u128` prefix comparisons (profile: ~70% of time was `memcmp` in the merge) | 1.07× |
| prefix stored in the buffer | no gain; dropped |

At COUNT 100 and above, the ~1.9× that remains is the merge's ~log₂16 comparisons per key, which one tree's in-order walk doesn't pay.

**Files**
- `scan-macos*.jsonl`, `create-macos.jsonl`: raw results.
- `conditions.txt`: host and load notes.
