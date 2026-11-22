# Getting Started (Rust, embedded)

::: warning Not on crates.io — use a git dependency
Nothing in the Recached workspace is published to crates.io yet, and
`recached-embed` depends on `core-engine` and `sync-client` by path, so it
cannot be published before they are. Depend on it by git in the meantime:

```toml
[dependencies]
recached-embed = { git = "https://github.com/recached-sh/recached.git", branch = "dev" }
```

Publishing `core-engine` would make its `KeyValueStore` / `Command` / `Value`
API a public semver contract. It is deliberately still internal.
:::

`recached-edge` lets a **browser** keep a local copy of the cache and receive
pushes when it changes. `recached-embed` is the same thing for a **Rust
service**: reads come from local memory, and the WebSocket carries only writes
out and change notices in.

It is the same `core-engine` and the same `sync-client` state machine the
browser SDK uses — only the transport adapter differs (tokio and
`tokio-tungstenite` instead of `WebSocket` and IndexedDB), so merge semantics
cannot drift between the two.

---

## Why

Config-shaped data — fare tables, feature flags, tenant settings, entitlement
checks, rate-limit tiers — is read on nearly every request and changes a few
times a day. The usual options are all unsatisfying:

| Approach | Read cost | Problem |
| --- | --- | --- |
| Query the database | ~2 ms + a pool connection | Far too slow for per-request data |
| Cache in Redis | ~0.4 ms | Still a round-trip on every read |
| Local `HashMap` | ~50 ns | Your other pods serve stale data forever |

So most teams build the fourth option by hand: a local `HashMap` plus a pub/sub
listener that clears entries. The hard part was never the `HashMap` — it is
*"my listener was disconnected for eight seconds during a deploy; what did I
miss?"* Usually nothing detects it, and that instance stays stale until it
restarts.

`recached-embed` is that pattern with the reconnect path handled: on reconnect
the client re-subscribes and re-hydrates, so a missed window heals rather than
going quietly stale.

---

## Fit

**Good for** small, hot, shared, read-heavy, *slowly-changing* data.

**Wrong for** data that changes constantly (live counters, seat inventory),
working sets that do not fit in every process, keys read once (session tokens),
or anything where being stale by milliseconds is unacceptable — payments, seat
locks, inventory decrement. Use the server directly for those.

---

## Connect

Connect to the **WebSocket sync port (6380)**, not RESP on 6379. Live queries
and change pushes travel the sync path; a plain TCP client is never sent
keychange notifications and so cannot hold a coherent local copy.

```rust
use recached_embed::Cache;

let cache = Cache::connect("ws://127.0.0.1:6380").await?;
```

With authentication or a scoped sync token:

```rust
let cache = Cache::builder("wss://cache.internal:6380")
    .password(std::env::var("RECACHED_PASSWORD")?)
    .sync_token(token)          // when the server sets RECACHED_SYNC_SECRET
    .connect()
    .await?;
```

`connect()` returns once the socket is open, and fails loudly if it cannot be
opened — a typo'd URL should not look like a slow start-up. Every later drop
reconnects in the background with jittered exponential backoff.

`Cache` is cheap to clone and safe to share across tasks. Every clone reads the
same local store, so clone it into your handlers rather than wrapping it in
another `Arc`.

---

## Declare the working set

```rust
// Once, at start-up.
cache.watch("fare:*").await?;
```

`watch()` returns only after the server's initial state has been applied, so it
doubles as a start-up barrier — the first request never races hydration.
Re-subscription after a reconnect is automatic.

The server caps a complete initial state at **10,000 keys per pattern**. `watch()` returns an error for a broader pattern instead of applying a partial snapshot. Narrow the pattern or raise `RECACHED_MAX_QSUB_INITIAL_KEYS` deliberately.

---

## Read

```rust
// Hot path: no network, no await, no lock.
let fare = cache.get("fare:MNL-CEB")?;        // Option<Vec<u8>>
let fare = cache.get_str("fare:MNL-CEB")?;    // Option<String>
```

::: tip Hydration is part of the contract
The local store holds only what your `watch()` patterns hydrated. Reading
anything else returns `Error::NotHydrated` — **never `None`**.

```rust
cache.watch("fare:*").await?;

cache.get("fare:MNL-CEB")?;   // Ok(Some(..)) / Ok(None) — both authoritative
cache.get("user:42")?;        // Err(NotHydrated) — not a silent None
```

A silent `None` is indistinguishable from "the key does not exist", which is
exactly how an embedded cache serves confidently wrong answers.
:::

When you would rather pay for a round-trip than declare a pattern:

```rust
let user = cache.get_or_fetch("user:42").await?;
```

The fetched value is deliberately **not** cached — nothing would keep it
current, so storing it locally would make it stale on the first change.

---

## Write

```rust
cache.set("fare:MNL-CEB", "1925").await?;
cache.set_ex("session:abc", token, Duration::from_secs(900)).await?;
cache.del("fare:MNL-CEB").await?;
let hits = cache.incr_by("hits:today", 1).await?;
```

Writes resolve when the server acknowledges them. While disconnected they are
queued in the durable outbox and replayed on reconnect, and the call returns
`Error::Disconnected` — the write is **unacknowledged, not lost**.

For commands the typed helpers do not cover:

```rust
cache.write_command(&["HSET", "clinic:1", "tier", "gold"]).await?;
let info = cache.read_command(&["INFO", "memory"]).await?;
```

---

## Behaviour during an outage

Local reads keep working — they simply stop receiving updates. Check
`is_connected()` if staleness during an outage matters to your call site:

```rust
if !cache.is_connected() {
    // Values are as of the last update received.
}
```

`pending_writes()` reports how many writes are queued but unacknowledged, and
`local_bytes()` how much memory the local copy occupies in this process. Both
are cheap enough to export as metrics on a tick.

---

## Known limitations

These are behaviours of the sync protocol, not of this crate, and they affect
`recached-edge` in the browser identically.

::: tip A removal you miss while offline is reconciled on reconnect
This used to be a limitation and is not any more. `qstate` re-hydration is a
complete snapshot of its pattern, so a key held locally that the snapshot does
not carry has been deleted or has expired — and nothing else would ever say so,
because `keychange` only reports what happened while the socket was up. The
client now drops those keys when it re-hydrates.

The server refuses a pattern whose complete snapshot exceeds `RECACHED_MAX_QSUB_INITIAL_KEYS` (10,000 by default), so reconciliation never runs against a partial snapshot.
:::

::: tip TTL deletion is eventual, not instant
A local copy does not expire on its own clock. The server masks an expired key on read, then a background sweep announces its removal as an ordinary delete. In 0.5.x the sweep checks at most 256 TTL-bearing keys per one-second tick, so convergence time grows with the volatile keyspace. From 0.6 (unreleased) it removes expired keys in chunks of 256, continuing while a sample shows at least a quarter of it expired, within a 25 ms budget per tick; under heavy expiry churn a backlog can still build. Compare a stored deadline yourself when exact expiry matters.
:::

---

## Try it

```bash
cargo run -p recached --bin recached-server &
cargo run -p recached-embed --example two_pods
```

Two caches against one server: pod B sees pod A's write, update and delete
without ever asking for them.

Next: the [API reference](/rust/api-reference).
