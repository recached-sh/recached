# Plan: Kotlin and Swift SDKs over UniFFI

Status: **M0 in progress** on `feat/mobile-sdk-prep` (started 2026-10-09).

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

The wrappers generate bindings with `uniffi-bindgen --library` from the
released binary, so the generated code and the library always come from the
same build. The `uniffi` version is pinned in one place.

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

### FFI surface (sketch)

```text
RecachedClient.open(path, config)          // restores data + outbox from SQLite
  get / getBytes / jget / getMatching      // local, synchronous
  set / setEx / del / incrBy / jset / ...  // local apply + durable outbox row, returns changed keys
  watch(pattern) / unwatch(pattern)        // live queries
  pendingWrites()

  // transport entry points, called by the platform socket
  connectionOpened()      -> [frame]
  frameReceived(bytes)    -> { changedKeys, reconnect }
  connectionClosed()      -> delayMs
```

Rules:

- `RecachedClient` holds `SyncClient` behind a `Mutex`.
- Effects come back as **return values**, not callbacks. Rust never calls into
  Kotlin or Swift while holding the lock. The browser adapter's
  `RefCell`-across-`await` panic is this same class of bug.
- Socket callbacks arrive on OkHttp's or URLSession's threads, and the mutex
  serialises them. Reads go straight to the store without taking the lock.

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

### M1 — `recached-mobile` crate

- [ ] Crate skeleton: `cdylib` + `staticlib`, UniFFI proc-macros, `uniffi`
  pinned.
- [ ] SQLite schema, open/restore, write and frame transactions as above.
- [ ] Linux tests against an in-process `recached-server`:
  - kill the process with writes queued, reopen, and confirm each write lands
    exactly once (`DEDUP` across epochs);
  - delete a key while offline, then reconnect;
  - restore a cold start with no network.
- [ ] Release workflow: Android ABIs (arm64-v8a, armeabi-v7a, x86_64) and an
  XCFramework (ios-arm64, ios-arm64-simulator, macos) attached with
  checksums, plus a dispatch to the wrapper repos.

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
