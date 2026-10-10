# Getting Started (Android)

::: warning Preview: not on Maven Central yet
The Kotlin SDK lives in [recached-sh/recached-kotlin](https://github.com/recached-sh/recached-kotlin)
and is not published yet. Build it from source with its `scripts/build-engine.sh`,
or try the demo app there first.
:::

`recached-android` keeps a local copy of the keys you watch on the device, saved
to disk and kept in sync with the server. It is built on the same `core-engine`
and `sync-client` as the browser SDK, through [UniFFI](https://mozilla.github.io/uniffi-rs/),
so sync behaves the same on every platform.

- **Local reads:** `get` reads device memory, never the network. After a restart
  with no connection it returns the last-synced data.
- **Durable queued writes:** a write is saved with its place in the outbox
  before `set` returns, and reaches the server exactly once, even if the app is
  killed before the server replied.
- **Reconnect recovery:** after a drop the client reconnects with backoff, or
  at once when the network or the app comes back, replays its queued writes,
  and catches up on what changed, deletions included.
- **Reactive updates:** `observe(key)` is a `Flow` that re-emits when that key
  changes, here, on another device, or on the server.

## Install

```kotlin
dependencies {
    implementation("dev.recached:recached-android:0.1.0")
}
```

minSdk 24. The native core ships for `arm64-v8a`, `armeabi-v7a` and `x86_64`;
limit your app to those (`ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }`).
ProGuard/R8 rules are bundled.

Use a `wss://` URL. Android blocks cleartext by default, so a plain `ws://`
development server needs a network security config that allows it.

## Open one cache per process

```kotlin
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

cache.set("todo:42", """{"title":"Buy milk"}""")      // local now, server when it can
val todo: String? = cache.getString("todo:42")       // local memory: fine on the main thread

class TodosViewModel(cache: Recached) : ViewModel() {
    val todos = cache.observeMatching("todo:*")
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())
}
```

`watch` is not remembered across opens; call it at startup. Other writes:
`delete`, `incrBy` (merges with other clients' increments instead of
overwriting them), `jsonSet` / `jsonMerge`, `set(key, value, ttl)`.

## Show sync state

```kotlin
cache.connectionState   // StateFlow: OFFLINE, CONNECTING, CONNECTED, WAITING
cache.pendingWrites     // StateFlow<Long>: writes the server has not acknowledged yet
cache.refusedWrites     // SharedFlow<RefusedWrite>: writes the server refused for good
```

A write the server refuses for good, such as one to a key the sync token makes
read-only, leaves the queue without reaching the server, and `refusedWrites`
says so. One refused for a reason that can clear, such as a sync token not yet
accepted, stays queued and is retried on every reconnect.

## Try the demo

`recached-kotlin/demo` is a small checklist on the SDK, and
`scripts/acceptance.py` drives it on a device or emulator to check all four
promises above against a real server. See the repository README.
