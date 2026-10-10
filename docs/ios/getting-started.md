# Getting Started (iOS and macOS)

::: warning Preview: not released yet
The Swift package lives in [recached-sh/recached-swift](https://github.com/recached-sh/recached-swift)
and does not pin a released core yet. Until it does, Apple platforms need the
XCFramework built locally with its `scripts/build-engine.sh` on a Mac.
:::

`Recached` keeps a local copy of the keys you watch on the device, saved to
disk and kept in sync with the server. It is built on the same `core-engine`
and `sync-client` as the browser and Android SDKs, through
[UniFFI](https://mozilla.github.io/uniffi-rs/).

- **Local reads:** `get` reads device memory, never the network. After a
  restart with no connection it returns the last-synced data.
- **Durable queued writes:** a write is saved with its place in the outbox
  before `set` returns, and reaches the server exactly once, even if the app is
  killed before the server replied.
- **Reconnect recovery:** after a drop the client reconnects with backoff, or
  at once when the network or the app comes back, replays its queued writes,
  and catches up on what changed, deletions included.
- **Reactive updates:** `observe(key)` is an `AsyncStream` that yields when
  that key changes, here, on another device, or on the server.

## Install

```swift
.package(url: "https://github.com/recached-sh/recached-swift", from: "0.1.0")
// target dependency: .product(name: "Recached", package: "recached-swift")
```

iOS 15+ and macOS 12+.

## Open one cache and keep it

```swift
let cache = try Recached.open(
    named: "app",
    config: RecachedConfig(url: URL(string: "wss://cache.example.com")!, syncToken: token)
)
```

`open(named:)` keeps the database in Application Support, excluded from
backups, for the same reason as on Android: a restore must not give two devices
one client identity.

## Watch, read, write, observe

```swift
cache.watch("todo:*")                             // keep these keys in sync

try cache.set("todo:42", #"{"title":"Buy milk"}"#) // local now, server when it can
let todo = try cache.getString("todo:42")         // local memory: fine on the main actor

for await todos in cache.observeMatching("todo:*") {
    render(todos)                                 // now, then after every change
}
```

In SwiftUI, `ObservedKey` (iOS 17+, Observation) or `ObservableKey` (iOS 15+,
`ObservableObject`) follows one key:

```swift
@State private var title = ObservedKey(cache, "todo:42:title")
// Text(title.string ?? "")
```

## Show sync state

```swift
cache.connectionState     // .offline, .connecting, .connected, .waiting
cache.pendingWrites       // writes the server has not acknowledged yet
cache.connectionStates()  // AsyncStream, starting with the current value
cache.pendingWriteCounts()
```

## Try the demo

`recached-swift/Demo` is a small SwiftUI checklist (an XcodeGen spec), with UI
tests that run on the simulator against a real server. See the repository
README.
