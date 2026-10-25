# Use Recached in iOS and macOS

::: info 0.1.0 preview
The Swift package lives in [recached-sh/recached-swift](https://github.com/recached-sh/recached-swift)
and links the prebuilt native core of recached v0.4.0. Its API may still change
before 1.0.
:::

`Recached` keeps a local copy of the keys you watch on the device, saved to
disk and kept in sync with the server. It is built on the same `core-engine`
and `sync-client` as the browser and Android SDKs, through
[UniFFI](https://mozilla.github.io/uniffi-rs/).

- **Local reads:** `get` reads device memory, never the network. After a
  restart with no connection it returns saved data that has not expired.
- **Durable queued writes:** a write is saved with its place in the outbox
  before a successful `set` returns. Pending writes survive an app kill and replay with their original duplicate-suppression identities. See [replay limits](/guide/client-support#write-durability-and-replay).
- **Reconnect recovery:** after a drop the client reconnects with backoff, or
  at once when the network becomes available or an iOS app returns to the foreground, replays its queued writes,
  and catches up on what changed, deletions included.
- **Reactive updates:** `observe(key)` is an `AsyncStream` that yields when
  that key changes, here, on another device, or on the server.

## Install

In Xcode, choose **File → Add Package Dependencies…** and enter `https://github.com/recached-sh/recached-swift`. In a `Package.swift`:

```swift
.package(url: "https://github.com/recached-sh/recached-swift", from: "0.1.0")
// target dependency: .product(name: "Recached", package: "recached-swift")
```

The native core comes as a prebuilt XCFramework attached to the recached v0.4.0 release, so no Rust toolchain is needed. To build it from source instead, see the Development section of the package's [README](https://github.com/recached-sh/recached-swift#development).

iOS 15+ and macOS 12+ are supported. `ObservedKey` requires iOS 17+ or macOS 14+; `ObservableKey` works on the minimum supported versions.

## Open one cache and keep it

```swift
import Foundation
import Recached

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

try cache.set("todo:42:title", "Buy milk") // local now, server when it can
let todo = try cache.getString("todo:42:title")         // local memory: fine on the main actor

for await todos in cache.observeMatching("todo:*") {
    render(todos)                                 // now, then after every change
}
```

In SwiftUI, `ObservedKey` (iOS 17+, Observation) or `ObservableKey` (iOS 15+,
`ObservableObject`) follows one key:

```swift
import SwiftUI

@available(iOS 17, macOS 14, *)
@MainActor
struct TitleView: View {
    @State private var title: ObservedKey

    init(cache: Recached) {
        _title = State(initialValue: ObservedKey(cache, "todo:42:title"))
    }

    var body: some View { Text(title.string ?? "") }
}
```

`watch` requests a server snapshot asynchronously; a read may return a miss or saved data before it arrives. Re-register watches after every open. `observe` and SwiftUI models listen to local changes and do not start server subscriptions. Observers begin with the current value, suppress equal values, and coalesce updates for slow consumers.

The examples assume your app supplies a backend-minted `token` and a `render` function. Tokens authorize access; watches alone do not. Read [sync scopes](/server/sync-scopes) before sharing per-user data.

## Show sync state

```swift
cache.syncStatus          // .disconnected … .ready, .degraded (next release)
cache.connectionState     // the socket alone: .offline, .connecting, .connected, .waiting
cache.pendingWrites       // writes the server has not acknowledged yet
cache.connectionStates()  // AsyncStream, starting with the current value
cache.pendingWriteCounts()
cache.refusedWrites()     // AsyncStream<RefusedWrite>: writes the server refused for good
```

A write the server refuses for good, such as one to a key the sync token makes
read-only, leaves the queue without reaching the server, and `refusedWrites()`
says so. One refused for a reason that can clear, such as a sync token not yet
accepted, stays queued and is retried on every reconnect.

A connected state means the socket is open, not that authorization or hydration has finished. A permanent refusal leaves its local effect until a permitted watched snapshot restores the server value; an unwatched key can keep that value. Collect refusal events while the cache is active because they are not saved for later replay.

## Check native limits

The default outbox holds 10,000 pending writes; set `maxPendingWrites` to change the cap. In 0.1 overflow drops the oldest queued write. From the next release, built on recached 0.5, a write past the cap throws `RecachedError.outboxFull` and changes nothing (`evictOldestWhenFull: true` keeps the old behaviour), a write that throws `.storage` is rolled back rather than left in memory, and `syncStatus` reports `.ready` only once authentication, the replay of queued writes and the watched patterns' snapshots have completed. Successful writes commit SQLite data and their queue rows together. SQLite uses WAL with `synchronous = NORMAL`; OS crashes or power loss can lose recent commits.

See [native API support and replay limits](/guide/client-support) for binary values, TTLs, JSON operations, refusal handling, and duplicate-suppression boundaries. Omit the config URL for a local cache; writes still queue for a later open with a server URL.

## Try the demo

`recached-swift/Demo` is a small SwiftUI checklist (an XcodeGen spec), with UI
tests that run on the simulator against a real server. See the repository
README.
