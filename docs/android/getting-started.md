# Use Recached in Android

::: info 0.1.0 preview
The Kotlin SDK lives in [recached-sh/recached-kotlin](https://github.com/recached-sh/recached-kotlin)
and is on Maven Central, built on the native core of recached v0.4.0. Its API
may still change before 1.0.
:::

`recached-android` keeps a local copy of the keys you watch on the device, saved
to disk and kept in sync with the server. It is built on the same `core-engine`
and `sync-client` as the browser SDK, through [UniFFI](https://mozilla.github.io/uniffi-rs/),
sharing the sync protocol with browser and Swift clients. Persistence differs by adapter.

- **Local reads:** `get` reads device memory, never the network. After a restart
  with no connection it returns saved data that has not expired.
- **Durable queued writes:** a write is saved with its place in the outbox
  before a successful `set` returns. Pending writes survive an app kill and replay with their original duplicate-suppression identities. See [replay limits](/guide/client-support#write-durability-and-replay).
- **Reconnect recovery:** after a drop the client reconnects with backoff, or
  at once when the network or the app comes back, replays its queued writes,
  and catches up on what changed, deletions included.
- **Reactive updates:** `observe(key)` is a `Flow` that re-emits when that key
  changes, here, on another device, or on the server.

## Install

The SDK is on Maven Central, so the `mavenCentral()` repository most Android projects already declare is enough:

```kotlin
dependencies {
    implementation("dev.recached:recached-android:0.1.0")
}
```

`dev.recached:recached-android-ffi`, the native core, comes with it. No Rust toolchain is needed. To build the SDK from source instead, see the Development section of its [README](https://github.com/recached-sh/recached-kotlin#development).

Use minSdk 24 or higher. The native core is built for `arm64-v8a`, `armeabi-v7a`, and `x86_64`. Restrict your app to these ABIs:

```kotlin
android {
    defaultConfig {
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }
    }
}
```

ProGuard/R8 rules are bundled. Use `wss://` for deployed servers. A plain `ws://` development endpoint needs a network security configuration allowing cleartext traffic. From an emulator, reach the host through `10.0.2.2`; for a USB-connected device, use `adb reverse tcp:6380 tcp:6380`.

## Open one cache per process

```kotlin
import android.app.Application
import dev.recached.Recached
import dev.recached.RecachedConfig
import dev.recached.open

class App : Application() {
    val cache by lazy {
        Recached.open(this, RecachedConfig(url = "wss://cache.example.com", syncToken = token))
    }
}
```

The database lives in the app's no-backup directory: Auto Backup restoring it
onto another device would give two devices one client identity, and the
server would drop one device's writes as duplicates of the other's.

## Watch, read, write, observe

```kotlin
cache.watch("todo:*")                                // keep these keys in sync

cache.set("todo:42:title", "Buy milk")      // local now, server when it can
val todo: String? = cache.getString("todo:42:title")       // local memory: fine on the main thread

class TodosViewModel(cache: Recached) : ViewModel() {
    val todos = cache.observeMatching("todo:*")
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())
}
```

`watch` is not remembered across opens; call it at startup. Other writes:
`delete`, `incrBy` (merges with other clients' increments instead of
overwriting them), `jsonSet` / `jsonMerge`, `set(key, value, ttl)`.

`watch` requests a server snapshot asynchronously; a read may return a miss or saved data before it arrives. Re-register watches after every open. `observe` listens to local changes and do not start server subscriptions. Observers begin with the current value, suppress equal values, and coalesce updates for slow consumers.

The examples assume your app supplies a backend-minted `token` and a `render` function. Tokens authorize access; watches alone do not. Read [sync scopes](/server/sync-scopes) before sharing per-user data.

## Show sync state

```kotlin
cache.syncStatus        // StateFlow<SyncStatus>: DISCONNECTED … READY, DEGRADED (next release)
cache.connectionState   // StateFlow: the socket alone — OFFLINE, CONNECTING, CONNECTED, WAITING
cache.pendingWrites     // StateFlow<Long>: writes the server has not acknowledged yet
cache.refusedWrites     // SharedFlow<RefusedWrite>: writes the server refused for good
```

A write the server refuses for good, such as one to a key the sync token makes
read-only, leaves the queue without reaching the server, and `refusedWrites`
says so. One refused for a reason that can clear, such as a sync token not yet
accepted, stays queued and is retried on every reconnect.

A connected state means the socket is open, not that authorization or hydration has finished. A permanent refusal leaves its local effect until a permitted watched snapshot restores the server value; an unwatched key can keep that value. Collect refusal events while the cache is active because they are not saved for later replay.

## Check native limits

The default outbox holds 10,000 pending writes; set `maxPendingWrites` to change the cap. In 0.1 overflow drops the oldest queued write. From the next release, built on recached 0.5, a write past the cap throws `RecachedException.OutboxFull` and changes nothing (`evictOldestWhenFull = true` keeps the old behaviour), a write that throws `Storage` is rolled back rather than left in memory, and `syncStatus` reports `READY` only once authentication, the replay of queued writes and the watched patterns' snapshots have completed. Successful writes commit SQLite data and their queue rows together. SQLite uses WAL with `synchronous = NORMAL`; OS crashes or power loss can lose recent commits.

See [native API support and replay limits](/guide/client-support) for binary values, TTLs, JSON operations, refusal handling, and duplicate-suppression boundaries. Omit the config URL for a local cache; writes still queue for a later open with a server URL.

## Try the demo

`recached-kotlin/demo` is a small checklist on the SDK, and
`scripts/acceptance.py` drives it on a device or emulator to check all four
promises above against a real server. See the repository README.
