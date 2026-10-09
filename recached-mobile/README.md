# recached-mobile

**The Recached client core for Kotlin and Swift, exported through
[UniFFI](https://mozilla.github.io/uniffi-rs/).**

Apps don't use this crate directly. They use the `recached-kotlin` and
`recached-swift` packages, which wrap the bindings generated from it in an
idiomatic API (`Flow`, `AsyncStream`, observable models). This README is for
people working on the core.

## What it does

One `RecachedClient` does four things:

- keeps a local copy of the cache in memory, so **reads never wait on the
  network**;
- persists that copy, server-pushed state included, to an on-device SQLite
  file, so **a cold start with no network shows the last-synced data**;
- commits every write to disk **in the same transaction as its outbox row**,
  before the call returns, so queued writes survive the app being killed and
  replay exactly once (`DEDUP`, with the original wire ids);
- reports the keys each server frame changed, so observers re-read only what
  changed.

The sync logic itself is `sync-client`, shared with the browser and server
clients. This crate adds persistence and the FFI surface.

## The app owns the socket

There is no async runtime and no networking in here. The platform layer owns a
WebSocket (OkHttp / `URLSessionWebSocketTask`) and drives the client:

```text
socket opened   → client.connectionOpened(sink)    // sends session + queued writes via sink
frame arrived   → client.frameReceived(bytes)      // → changedKeys, reconnect?
socket closed   → client.connectionClosed()        // → delay before reconnecting (ms)
```

`FrameSink.send` runs while the client holds its lock. That is what keeps
frames in the order reply matching depends on. So it must queue and return:
no blocking, and no calls back into the client.

## Building

```sh
# Kotlin + Android .so for arm64-v8a, armeabi-v7a, x86_64 (needs cargo-ndk + NDK)
recached-mobile/scripts/build-android.sh

# XCFramework + Swift bindings (macOS), then consume them as an app would
recached-mobile/scripts/build-apple.sh
recached-mobile/scripts/check-apple.sh
```

Both write to `target/mobile/`. The release workflow attaches the same
archives to each GitHub release.

Bindings are generated with the crate's own `uniffi-bindgen`
(`--features cli`), so the generator and the library always share one UniFFI
version:

```sh
cargo build -p recached-mobile
cargo run -p recached-mobile --features cli --bin uniffi-bindgen -- \
    generate --library target/debug/librecached_mobile.so --language swift --out-dir out/
```

Module names are set in `uniffi.toml`: Kotlin `dev.recached.ffi`; Swift
`RecachedCore`, over the C module `RecachedFFI` inside
`RecachedFFI.xcframework`.

## Testing

```sh
cargo test -p recached-mobile                       # unit tests, no server

cargo run -p recached --bin recached-server &
RECACHED_MOBILE_TEST_URL=ws://127.0.0.1:6380 cargo test -p recached-mobile --test live
```

The live suite covers each milestone guarantee against a real server:
exactly-once replay after a kill, deletions made while offline, cold start
from disk, and changed-key reporting.
