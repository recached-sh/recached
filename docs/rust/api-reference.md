# API Reference (Rust, embedded)

Everything lives on `recached_embed::Cache`. See
[Getting Started](/rust/getting-started) for the concepts.

```rust
use recached_embed::{Cache, Error, Value};
```

---

## Constructing

### `Cache::connect(url)`

```rust
let cache = Cache::connect("ws://127.0.0.1:6380").await?;
```

Connects with defaults and returns once the socket is open. Fails with
`Error::Connect` if it cannot be opened.

### `Cache::builder(url)`

```rust
let cache = Cache::builder("wss://cache.internal:6380")
    .password(std::env::var("RECACHED_PASSWORD")?)
    .sync_token(token)
    .max_pending(50_000)
    .connect()
    .await?;
```

| Method | Purpose |
| --- | --- |
| `.password(p)` | The server's `RECACHED_PASSWORD` |
| `.sync_token(t)` | A scoped sync token — required when the server sets `RECACHED_SYNC_SECRET`. See [Sync Scopes](/server/sync-scopes) |
| `.max_pending(n)` | Cap on writes queued and not yet acknowledged (default 10,000). Beyond it a write fails with `Error::OutboxFull` and nothing is queued |
| `.evict_oldest_when_full(true)` | Past the cap, drop the oldest queued write instead (the pre-0.5 behaviour), counted by `pending_dropped()` |
| `.connect()` | Opens the socket and returns `Result<Cache>` |

`Cache` is `Clone`. Every clone shares one local store and one connection —
clone it into handlers rather than wrapping it in another `Arc`.

### Synchronization status

```rust
let mut changes = cache.status_changes(); // tokio::sync::watch::Receiver<SyncStatus>
while changes.changed().await.is_ok() {
    let SyncStatus { phase, error } = changes.borrow().clone();
}
let fares = cache.subscription_state("fare:*"); // Option<SubscriptionState>
```

`status().phase` is `SyncPhase::Ready` once the session is authenticated, every
queued write has been replayed and answered, and every pattern watched when the
socket opened has its snapshot. `is_connected()` reports the socket alone.

---

## Working set

### `watch(pattern)`

```rust
cache.watch("fare:*").await?;
```

Hydrates every key matching the glob and keeps it current. Returns only after
the initial state has been applied, so it doubles as a start-up barrier.
Re-subscribed automatically after a reconnect. Capped by the server at 10,000
keys per pattern.

### `unwatch(pattern)`

```rust
cache.unwatch("fare:*").await?;
```

Stops tracking the pattern. Keys it hydrated remain in local memory but stop
receiving updates, so they are dropped from the hydrated set — reading them now
returns `NotHydrated` rather than a value nothing refreshes.

### `is_hydrated(key)`

```rust
if cache.is_hydrated("fare:MNL-CEB") { /* get() can answer */ }
```

Whether any watched pattern covers the key. Synchronous.

---

## Local reads

All synchronous: no network, no `await`, no lock. Every one returns
`Err(Error::NotHydrated)` for a key no watched pattern covers.

| Method | Returns |
| --- | --- |
| `get(key)` | `Result<Option<Vec<u8>>>` — string value as bytes |
| `get_str(key)` | `Result<Option<String>>` — as above, UTF-8 decoded |
| `get_value(key)` | `Result<Value>` — the raw value, including collections |
| `contains(key)` | `Result<bool>` |
| `matching(pattern)` | `Vec<(String, Value)>` — local scan, **not** hydration-checked |

`get_value` returns collections as the type-tagged arrays described in the
[wire protocol](/server/protocol) — `["hash", field, value, …]`. Reading a
collection with `get`/`get_str` is `Err(Error::WrongType)`, not a wrong answer.

`matching` scans the local keyspace, so it is O(n) in cached keys. Fine at
start-up or on a background tick; not for a hot path.

---

## Round-trips

### `get_or_fetch(key)`

```rust
let user = cache.get_or_fetch("user:42").await?;
```

Serves from local memory when a watched pattern covers the key, otherwise pays
for one round-trip. The fetched value is **not** cached — nothing would keep it
current. Prefer `watch()` for anything read repeatedly.

### `read_command(args)`

```rust
let info = cache.read_command(&["INFO", "memory"]).await?;
```

One-shot round-trip, never replayed, nothing applied to the local store.
Returns the reply as-is.

---

## Writes

All resolve when the server acknowledges the write. While disconnected the
write is queued in the durable outbox and replayed on reconnect, and the call
returns `Error::Disconnected` — unacknowledged, not lost.

| Method | Command |
| --- | --- |
| `set(key, value)` | `SET` |
| `set_ex(key, value, ttl)` | `SET … PX` |
| `del(key)` | `DEL` |
| `incr_by(key, delta)` | `INCRBY` — returns the new `i64` |
| `write_command(args)` | Any mutation, e.g. `&["HSET", key, field, value]` |

```rust
cache.set("fare:MNL-CEB", "1925").await?;
cache.set_ex("session:abc", token, Duration::from_secs(900)).await?;
let hits = cache.incr_by("hits:today", 1).await?;
cache.write_command(&["SADD", "tags", "rush"]).await?;
```

`write_command` goes through the outbox and is wrapped in a duplicate-suppression
envelope like `set`. Pass mutations only — a read sent here would be
replayed pointlessly after a reconnect. Use `read_command` for those.

::: warning `set_ex` expires locally within about a second, not exactly
The server enforces the TTL exactly; a local copy learns about it when the
server's bounded background sweep removes the key. See
[Known limitations](/rust/getting-started#known-limitations).
:::

---

## Pub/sub

### `subscribe(channel)`

```rust
let mut rx = cache.subscribe("invalidations").await?;
while let Ok(message) = rx.recv().await {
    // message: Vec<u8>
}
```

Returns a `tokio::sync::broadcast::Receiver<Vec<u8>>`. Re-subscribed
automatically after a reconnect; messages published during the gap are lost.
The channel buffers 256 messages, after which a slow subscriber sees
`RecvError::Lagged`.

### `publish(channel, message)`

```rust
cache.publish("invalidations", "fare:*").await?;
```

Sent one-shot rather than through the outbox: replaying a publish after a
reconnect would deliver it twice.

---

## Introspection

| Method | Returns |
| --- | --- |
| `is_connected()` | Whether the sync socket is up. Local reads work either way — they just stop receiving updates |
| `pending_writes()` | Writes queued but not yet acknowledged |
| `local_bytes()` | Approximate bytes the local copy occupies in this process |
| `store()` | `&Arc<KeyValueStore>` — the underlying store, **bypassing** the hydration check |

The first three are cheap enough to export as metrics on a tick.

---

## Errors

```rust
pub enum Error {
    NotHydrated { key: String },
    WrongType { key: String },
    Server(String),
    Disconnected,
    Closed,
    Timeout,
    Connect(String),
}
```

| Variant | Meaning |
| --- | --- |
| `NotHydrated` | No watched pattern covers the key. `watch()` it, or use `get_or_fetch()` |
| `WrongType` | Read as a string but the key holds a collection (or vice versa) |
| `Server(msg)` | The server replied with an error |
| `Disconnected` | Socket down. Writes are queued and replayed; reads needing the network cannot proceed |
| `Closed` | The connection task has shut down — the `Cache` handle is dead |
| `Timeout` | A round-trip did not complete in time |
| `Connect(msg)` | The initial connection could not be established |

---

## Testing against a real server

```bash
cargo run -p recached --bin recached-server &
RECACHED_EMBED_TEST_URL=ws://127.0.0.1:6380 cargo test -p recached-embed
```

Without the environment variable the live suite skips, so `cargo test` stays
green with no server running.
