# Plan: Kotlin and Swift SDKs over UniFFI

Status: **M0 done** on `feat/mobile-sdk-prep`; **M1 done** on
`feat/recached-mobile` (stacked on it). Started 2026-10-09.

## Goal

A first release of native Kotlin (Android) and Swift (iOS) SDKs built on one
Rust core, with the app owning networking. Each platform ships one small demo
app that shows all four of these working together:

1. **Local reads** — a cold start in airplane mode shows the last-synced data
   at once.
2. **Durable queued writes** — writes made offline survive the app being
   killed and reach the server exactly once after reconnect.
3. **Reconnect recovery** — changes made on the server while the phone was
   offline, deletions included, show up after reconnect without a restart.
4. **Reactive updates** — an edit from another client (a browser tab) shows up
   live, and only the views reading the changed keys re-render.

Out of scope for this milestone: Flutter and React Native (later roadmap
steps), pub/sub in the mobile API, and background sync while the app is
suspended.

## Repository layout

| Repo | Contents |
|---|---|
| `recached` (this repo) | The `recached-mobile` UniFFI crate, its SQLite persistence and its Rust tests. The release builds Android `.so` files (cargo-ndk) and an XCFramework and attaches them. |
| `recached-sh/recached-kotlin` | Generated bindings, OkHttp transport, the `Flow` API, the demo app and Maven publishing. |
| `recached-sh/recached-swift` | Generated bindings, `URLSessionWebSocketTask` transport, the Swift API, the demo app and SwiftPM (`Package.swift` at the root, binary target from the release). |

Why the FFI crate lives here: `sync-client` and `core-engine` are
`publish = false`, the core changes the SDKs need land here anyway, and
workspace CI then catches a `sync-client` change that breaks the FFI on the PR
that makes it, not at the next bump.

Why the wrappers do not: SwiftPM requires `Package.swift` at the repository
root and clones the whole repository, and Gradle/Xcode jobs would slow every
server PR. This is the split taladb uses, so its `engine-bump` workflow,
`check-package.sh` and `check-publication.sh` carry over almost unchanged.

Each release attaches the generated bindings next to the binaries, so the
wrappers never run a generator themselves, and the code and the library always
come from the same build. `uniffi` is pinned exactly in
`recached-mobile/Cargo.toml`, and the crate carries its own `uniffi-bindgen`
binary.

| Release asset | Contents |
|---|---|
| `recached-mobile-android-<v>.zip` | `jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/librecached_mobile.so` (16 KB-aligned, API 24) and `kotlin/dev/recached/ffi/recached_mobile.kt` |
| `RecachedFFI-<v>.xcframework.zip` | Static `librecached_mobile.a` for iOS, the iOS simulator (arm64 + x86_64) and macOS (arm64 + x86_64), with the `RecachedFFI` C module. Its `.sha256` is the SwiftPM checksum. |
| `recached-mobile-swift-<v>.zip` | `RecachedCore.swift`, the generated Swift module |

Module names: Kotlin `dev.recached.ffi`, wrapped by `dev.recached`. On Swift,
the C module `RecachedFFI` is the binary target, the generated module is
`RecachedCore`, and the wrapper's public module is `Recached`.

## Who owns what

| Rust (`recached-mobile`) | Kotlin / Swift |
|---|---|
| Local store (`core-engine`) | WebSocket I/O |
| Sync state machine (`sync-client`): outbox, `DEDUP`, reply correlation, session replay, backoff delay | Reconnect timer (sleep for the delay Rust returns) |
| SQLite persistence: data, outbox, metadata | Network-reachability and app-lifecycle hooks that reconnect immediately |
| Change sets: which keys a frame or write changed | Fan-out to `Flow` / `AsyncStream` / observable models |

The Rust side runs no async runtime and opens no sockets. Persistence stays in
Rust, so both platforms share one implementation, and it can be tested on
Linux.

### FFI surface (as built in M1)

```text
RecachedClient.open(path, config)            // restores data + outbox from SQLite
  get / getString / getJson / exists / ttl / getMatching    // local memory, no lock
  set / setEx / del / incrBy / jset / jmerge // apply + commit with outbox row, then send
  watch(pattern) / unwatch(pattern) / setSyncToken(token)
  pendingWrites()

  // transport entry points, called by the platform socket
  connectionOpened(sink: FrameSink)          // sends session + queued writes via sink
  frameReceived(bytes) -> { changedKeys, reconnect }
  connectionClosed()   -> delayMs

FrameSink.send(frame)                        // implemented by the platform
```

Rules:

- `RecachedClient` holds `SyncClient` behind a `Mutex`. Reads go straight to
  the store without taking it.
- Change notifications come back as **return values**. Writes don't return
  keys: each changes exactly the key it names, so the wrapper notifies that
  key. Frames return `changedKeys`.
- **Frames go out through a `FrameSink` the platform implements, called while
  the lock is held.** This changes the M0 sketch, where frames were return
  values. Reply matching is FIFO, so frames must reach the socket in the order
  the client recorded them. Returning frames would leave two threads that
  write at once free to send in either order. A crash after a misattributed
  reply would then lose a write. So the one callback made under the lock is
  `send`, and its contract is: queue and return, never block, never call back
  into the client. OkHttp's `WebSocket.send` and
  `URLSessionWebSocketTask.send(_:completionHandler:)` both qualify.
- A write is committed **before** it is sent, so its reply can never retire a
  row that is not on disk yet. If the commit fails, the frame is still sent
  (it is already recorded as in flight) and the call returns `Storage`.

### Persistence

SQLite in WAL mode, through `rusqlite` with the bundled build. Tables:

- `kv(key PRIMARY KEY, entry BLOB)`: one `SnapshotEntry` per key, encoded
  with rmp-serde. It's a materialised copy of the store rather than a command
  log, so it never needs compacting. On launch:
  `store.restore(all rows)`.
- `outbox(id PRIMARY KEY, frame BLOB)`.
- `meta(key PRIMARY KEY, value)`: client id and session epoch.

Each local write runs in one transaction: apply it to the store, upsert the
`kv` rows for its changed keys, and insert the outbox row. It commits
**before** the call returns. The browser adapter fires its outbox write and
does not wait for it, so there a queued write is not durable when `set()`
returns.

Each incoming frame writes `snapshot_key()` for every changed key, deleting
the row when the key is gone, and deletes retired outbox rows, all in one
transaction. The browser persists only its own writes, never server state,
which is why it cannot do goal 1. Mobile has to.

## Milestones

### M0 — Core prep (this repo) · in progress

- [x] `Incoming::Applied` / `AppliedReply` report the changed keys. That
  covers server pushes, `keychange` including the FLUSHDB sentinel,
  `keydelta`, and `qstate` including keys it reconciles away.
- [x] `sync_client::mutation_keys(&Command, &store)` so adapters can report
  local writes the same way.
- [x] `KeyValueStore::snapshot_key(key)` for per-key persistence.
- [x] Test the reconnect path: a key deleted while the client was offline is
  removed **and reported**.
- [x] The browser SDKs use the reported keys too: `onKeyChange` and
  `onPatternChange` in `recached-edge`. React and Vue hooks now re-read only
  when their own key or pattern changes, and fall back to `onMutation` on an
  older `recached-edge`.

### M1 — `recached-mobile` crate · done

- [x] Crate: `lib` + `cdylib` + `staticlib`, UniFFI 0.32.2 proc-macros (pinned
  exactly), its own `uniffi-bindgen` behind `--features cli`.
- [x] SQLite schema (`user_version` 1), open/restore, and write and frame
  transactions as above. WAL with `synchronous = NORMAL`: commits survive an
  app kill or crash; only OS crash or power loss can drop the last few.
- [x] Unit tests (12) against hand-written frames.
- [x] Live tests (4) against a real `recached-server`. They run in CI next to
  the `recached-embed` suite:
  - a write replayed after a kill applies exactly once; with `DEDUP` disabled
    the same test fails with 10 instead of 5;
  - a key deleted while offline is gone after reconnect, and from disk;
  - a cold start with no network reads the last server-pushed state;
  - another client's write arrives, and its key is reported.
- [x] Packaging: `scripts/build-android.sh` (verified here, NDK 27, all ABIs
  16 KB-aligned) and `scripts/build-apple.sh` + `scripts/check-apple.sh`. The
  check builds a SwiftPM consumer, runs it on macOS, and builds it for the iOS
  simulator. The Apple scripts run only on CI (no Mac here).
- [x] CI jobs `mobile-android` and `mobile-apple`; release job `mobile`
  attaches the archives.
- [x] Generated Swift checked on Linux with Swift 6.4: it compiles in Swift 6
  mode, and a smoke program covers open/write/sink/frames/errors/reopen.
- [ ] Dispatch to the wrapper repos on release. This waits for those repos
  (M2/M3), as taladb's `NATIVE_PACKAGES_DISPATCH_TOKEN` setup did.

### M2 — Kotlin

- [ ] OkHttp WebSocket transport. Reconnect loop driven by `connectionClosed()`.
- [ ] `ConnectivityManager.NetworkCallback` and `ProcessLifecycleOwner` reset
  the wait and reconnect immediately.
- [ ] `observe(key): Flow<T?>` (`StateFlow`, `distinctUntilChanged`) and
  `observeMatching(pattern)`.
- [ ] Host JVM tests plus device tests on the Samsung A16 (the emulator does
  not boot in this environment).

### M3 — Swift

- [ ] `URLSessionWebSocketTask` transport, `NWPathMonitor`, scene-phase
  handling. iOS suspends sockets in the background, so reconnect on
  foreground.
- [ ] `AsyncStream` as the core API. An `ObservableObject` model for iOS 15+,
  and `@Observable` on iOS 17+.
- [ ] Linux test suite here (Swift 6.4, or the `swift:6.4-noble` container). The
  iOS build and tests run in CI only, since there is no Mac.

### M4 — Demo apps and release

- [ ] A shared checklist on each platform, against `recached-server` with sync
  scopes. It shows connection state and a pending-writes badge.
- [ ] A scripted acceptance run for the four goals, also usable as a release
  check.
- [ ] Docs: a getting-started page for each platform. Update the
  [roadmap](../roadmap.md).

## Open questions

- **Auth refresh:** how the app supplies a fresh `SYNC TOKEN` on reconnect. It
  could be a pull at `connectionOpened()`, or a setter the app calls before
  reconnect.
- **Outbox cap on mobile:** the default is 10,000 rows. Decide what happens
  when it overflows: an event on the observable state, or refusing the write.
- **Minimum OS versions:** proposed Android API 24 and iOS 15.
- **Local-only keys:** whether keys outside every `watch` pattern are
  persisted too. They are not kept fresh, and `recached-embed` refuses to read
  them for that reason (`NotHydrated`).

## Risks

- **Apple builds:** the Apple path is exercised only in CI. Get the
  XCFramework building early in M1, not at release.
- **UniFFI upgrades:** a version bump changes the generated code. Bindings and
  library must always be regenerated together.
- **Large `qstate` snapshots:** a full snapshot rewrites many `kv` rows in one
  transaction. Measure on the device before the demo uses large patterns.
