# Small-hash memory investigation, 2026-11-14

Findings: `docs/reports/v0.6.0/V0.6-HASH-MEMORY-INVESTIGATION.md` (local report). Short version:
the first `HSET` overwrite of a 3-field hash doubled its field table (+228 live bytes per hash),
because `HashMap::insert` reserves before it finds an existing key. Fixed in
`CompactHash::insert`.

Host: macOS 15.8.1, Intel i5-8259U (4C/8T), 8 GB, load average 5–20. Memory results only;
nothing here is a throughput figure.

## `live-bytes/` — exact live heap, no allocator noise

A counting global allocator around the system allocator, linked to `core-engine`. Commands go
through RESP bytes → `Value::parse` → `Command::from_value` → `execute`, as in the server.

```bash
CARGO_TARGET_DIR=target/hash-live-bytes cargo run --release \
  --manifest-path benchmarks/v0.6.0/hash-memory-20261114/live-bytes/Cargo.toml -- 1000000 32 3
KIND=set  ...   # SADD of 3 members instead of HSET
KIND=zset ...   # ZADD of 3 members
```

Unfixed tree (`63f4e8e`), 1M keys:

```
loaded         live    732181024 B  (   732.2 B/key)
overwrite 1    live    960181044 B  (   960.2 B/key)  Δ   +228000020 B
overwrite 2    live    960181044 B  (   960.2 B/key)  Δ           +0 B
overwrite 3    live    960181044 B  (   960.2 B/key)  Δ           +0 B
deleted        live    169896320 B  (   169.9 B/key)
reloaded       live    732181088 B  (   732.2 B/key)
flushed        live        26304 B  (     0.0 B/key)
```

Fixed tree, 1M keys: `overwrite 1` = 732181044 B (Δ +20 B), flat after. 10k and 100k keys
showed the same +228 B/key once (925.7 and 896.7 B/key after the overwrite). Sets and sorted
sets: no growth on re-adding members.

## `run.sh` + `load.py` — server RSS timeline

`run.sh <server-bin> <label> <keys> [MALLOC_CONF]`: a fresh server, then load, three overwrites
and `FLUSHDB`, sampling RSS and `phys_footprint` 1/5/15/30 s after each phase. `MALLOC_CONF` is
passed as `_RJEM_MALLOC_CONF` (tikv-jemallocator's prefixed variable). `load.py` is the loader
from `scripts/bench-redis-valkey.sh`.

`timeline.csv`: `81f83e9` (unfixed) default, the fixed tree default, and `81f83e9` with
`dirty_decay_ms:0,muzzy_decay_ms:0`, all at 300k keys. The `vm_dirty_kb`/`vm_reclaimable_kb`
columns matched the wrong `footprint` row on this macOS version and should be ignored.
