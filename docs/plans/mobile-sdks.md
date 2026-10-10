# Plan: Kotlin and Swift SDKs over UniFFI

Status (2026-10-10): **M0–M3 done** (#47–#50; SDK repos green on CI, Android
on a phone too). **M4 demos done**, with an acceptance run on the A16. Left:
publishing. Started 2026-10-09.

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

### M2 — Kotlin · done

- [x] OkHttp WebSocket transport. Reconnect loop driven by `connectionClosed()`,
  and a generation counter makes callbacks from a replaced socket inert. Every
  core call that depends on the current socket runs under the transport's
  lock, so the lock order is always transport, then core.
- [x] `ConnectivityManager` default-network callback and `ProcessLifecycleOwner`
  `onStart` reconnect at once. The database lives in `noBackupFilesDir`, so
  Auto Backup never clones a client identity onto a second device.
- [x] `observe` / `observeString` / `observeMatching`: callback flows,
  conflated, re-reading on invalidation and deduplicated, so a slow collector
  re-reads once and never misses the last value. Pattern routing uses the
  core's `any_key_matches`, so it follows the server's glob rules (no
  character classes).
- [x] Two modules: `:ffi` (generated bindings and native libraries,
  `recached-android-ffi`) and `:recached` (API, `recached-android`, under
  explicit-API mode and warnings-as-errors). UniFFI cannot emit `internal`.
- [x] 27 JVM tests pass, stable over 5 reruns: local, change bus, flows,
  MockWebServer transport (session order, replay after a drop, malformed
  frame, close), and live tests that start a real `recached-server` on free
  loopback ports.
- [x] Lint (warnings as errors), AAR check (ABIs, 16 KB alignment) and a Maven
  Central dry run of both artifacts.
- [x] Instrumented tests on CI's API 35 emulator, including live sync against
  a server on the runner (`ws://10.0.2.2`). The test APK targets SDK 36, so
  it opts into cleartext traffic. Apps need `wss://`, or a network security
  config for a plain `ws://` development server (README).
- [x] The same suite on the Samsung A16 (Android 16, arm64): 26 of 26 pass,
  including live sync to a local server through `adb reverse`. The
  server-starting live tests are JVM-only (`src/testJvm`): on a device they
  could only skip, and Android's reports list a skipped assumption as a
  failure.

Core changes M2 needed (on `feat/recached-mobile`): `any_key_matches` exported,
and the error text field renamed `reason`. A field named `message` collides
with Kotlin's `Throwable.message` in the generated class.

OkHttp is pinned at 5.3.2: 5.4 and 5.5 raise every consuming app's
compileSdk to 36 and 37 through AAR metadata.

### M3 — Swift · done

- [x] `URLSessionWebSocketTask` transport, with the delegate's open, close and
  complete callbacks. Callbacks from a replaced socket are inert by task
  identity, and pings every 20 s notice dead sockets. `NWPathMonitor` and the
  foreground notification reconnect at once (Apple only).
- [x] `AsyncStream` observers (`observe`, `observeString`, `observeMatching`),
  conflated with `bufferingNewest(1)`. Each re-read and yield happens under one
  lock, so the last value yielded is never older than the last change.
  `connectionStates()` and `pendingWriteCounts()` streams.
- [x] SwiftUI models: `ObservableKey` (`ObservableObject`, iOS 15+) and
  `ObservedKey` (`@Observable`, iOS 17+).
- [x] `open(named:)` uses Application Support, excluded from backups, for the
  same client-identity reason as Android's no-backup directory.
- [x] Package layout: `RecachedFFI` (an XCFramework binary target on Apple, a
  system library on Linux), `RecachedCore` (generated, **committed**, because
  SwiftPM users compile it), and `Recached` (the API). CI fails if the
  committed bindings differ from what the pinned core generates.
- [x] 25 tests pass on Linux (Swift 6.4, warnings as errors), stable over 5
  runs. They include live tests against a real `recached-server`, among them
  the server dying (SIGKILL) and coming back while a write is queued. Lint is
  clean, and the consumer-package check runs.
- [x] CI is green. macOS runs all 26 tests, including the live tests and
  `ObservableKey`. The iOS simulator runs everything except the live tests,
  which skip there because iOS cannot start a server process. Linux skips the
  live tests too: Ubuntu 24.04's libcurl (8.5), which `URLSessionWebSocketTask`
  uses on Linux, has no WebSocket support (reproduced in `swift:6.4-noble`).
  The tests probe for that and skip with the reason.

Lessons: on Linux, Foundation's `Process` hands the child its parent's signal
mask, so the test fixture stops the server with SIGKILL. The fixture is not
compiled for iOS, which has no `Process`. A test method marked `@MainActor`
makes SwiftPM's generated Linux test runner warn; the model checks run in
`@MainActor` helpers instead.

### M4 — Demo apps and release · demos done

- [x] Android: a Compose checklist in `recached-kotlin/demo`, with a
  connection chip and an unsynced-writes badge. Cleartext is allowed only to a
  development server on this machine. The release build is minified, which
  checks the bundled R8 rules.
- [x] `recached-kotlin/scripts/acceptance.py` drives the demo through
  uiautomator against a real server, reached through `adb reverse` and a relay
  it cuts to take the app offline while the server stays up. It checks the four
  goals: a backend write appears live; an item added offline survives a
  force-stop and reaches the server exactly once; a cold start offline shows
  everything; a deletion and an addition made while offline arrive after
  reconnect. It passes on the Samsung A16 (twice in a row), and CI runs it on
  the emulator.
- [x] iOS: a SwiftUI checklist in `recached-swift/Demo` (an XcodeGen spec).
  UI tests on the simulator check that an item added offline survives a
  relaunch still queued, and sync against a real server on the CI Mac. Green on
  the first CI run.
- [x] Docs: [Android](/android/getting-started) and [iOS](/ios/getting-started)
  getting-started pages, marked preview, and the roadmap updated.
- [ ] Release: publish `recached-android` to Maven Central; pin a recached
  release (with the mobile archives) in both SDK repos.

Found while building M4:

- **Reconnect flicker** (fixed in #51). `on_open` re-subscribed live queries
  before replaying the outbox, so each reconnect snapshot lacked the client's
  own queued writes. An item created offline vanished, and an offline edit
  reverted, until the write's own keychange restored it.
- **Lifecycle 2.11 forced API 37 on apps.** `lifecycle-process` 2.11
  constrains the whole lifecycle family, and `lifecycle-runtime-compose` 2.11
  needs compileSdk 37. The Kotlin SDK pins 2.10.
- **Unscoped servers fan out everything.** Without sync scopes, every
  WebSocket client gets every mutation, so a mobile client stores, and
  persists, keys it never watched. Production deployments need
  `RECACHED_SYNC_SECRET` and scoped tokens (docs/server/sync-scopes).

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
