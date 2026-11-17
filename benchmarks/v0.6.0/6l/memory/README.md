# 6L-E memory: live heap per key

Harness: `benchmarks/v0.6.0/hash-memory-20261114/live-bytes`. A counting allocator wraps the system allocator, and commands go through RESP → `Command::from_value` → `execute`, as in the server. This is the **exact live heap**, not RSS. RSS needs Linux; see the Fedora kit, phase E.

```sh
RESTORE=1 KIND=string CARGO_TARGET_DIR=target/live-bytes cargo run --release \
  --manifest-path benchmarks/v0.6.0/hash-memory-20261114/live-bytes/Cargo.toml -- 1000000 32 0
```

`RESTORE=1` (new in 6L) also measures the keyspace restored from a snapshot of itself into a fresh store. That is the path of a restart, a replica's full sync and a restore.

1M keys `mem:%010d`, 32-byte values, on macOS:

| Build | Keyspace | string | string + TTL | 3-field hash |
|---|---|---:|---:|---:|
| `dev` 28d8cbd (6L-A..D) | built by writes | 182.7 | 252.0 | 346.7 |
| `dev` 28d8cbd | restored | 175.6 | 244.8 | — |
| + bulk-built index (6L-E) | built by writes | 182.7 | 252.0 | — |
| + bulk-built index (6L-E) | **restored** | **166.7** | **235.9** | — |

Built by writes, the index is unchanged: these keys arrive in ascending order, which leaves the B-tree's nodes about half full. 166.7 is roughly what periodically re-packing the index would reach for such a keyspace (not implemented; see the report).
