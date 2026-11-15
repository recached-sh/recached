# 6J-B: ordered SCAN index alternatives

Standalone crate (`unsafe` counting allocator, so outside the workspace). It models the
keyspace as the server holds it, `DashMap<Arc<str>, Box<[u8; 64]>>` with the index sharing each
key's `Arc`, and measures:

- `global`: the production `Mutex<BTreeSet<Arc<str>>>`;
- `sharded16` / `sharded64`: N hash-sharded `Mutex<BTreeSet>`, where `SCAN` k-way merges the
  shards' ranges after the cursor. This keeps global byte order, exactly-once delivery and
  bounded work;
- `none`: no index, the lower bound.

`run.sh [keys] [reps]` builds and runs in `recached-6g-rust:local` and writes `results.jsonl`.
Results of record: 1M keys, 3 repetitions, Docker Desktop Linux VM (8 vCPU, i5-8259U).

They are in-process and engine-only (no RESP, no stripes), so the index's share of cost is larger
here than in the server, where 6I profiled it at ~30% of a creating write. See
`docs/reports/v0.6.0/V0.6J-SCAN-INDEX-DESIGN-REVIEW.md` §5.
