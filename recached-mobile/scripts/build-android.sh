#!/usr/bin/env bash
# Build the Android libraries and Kotlin bindings, and package them.
#
#   recached-mobile/scripts/build-android.sh [out-dir]
#
# Produces <out-dir>/recached-mobile-android-<version>.zip (+ .sha256):
#
#   jniLibs/<abi>/librecached_mobile.so   arm64-v8a, armeabi-v7a, x86_64
#   kotlin/dev/recached/ffi/recached_mobile.kt
#
# Needs cargo-ndk and an NDK (ANDROID_NDK_HOME, or the newest under
# $ANDROID_HOME/ndk). The release profile strips symbols, so the bindings are
# generated from an unstripped host build of the same source: UniFFI's library
# mode reads the interface from metadata symbols, and the interface does not
# depend on the target.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$(mkdir -p "${1:-$ROOT/target/mobile}" && cd "${1:-$ROOT/target/mobile}" && pwd)"
VERSION="$(cargo metadata --manifest-path "$ROOT/Cargo.toml" --format-version 1 --no-deps |
    python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="recached-mobile"))')"
STAGE="$OUT/android"
API_LEVEL=24

rm -rf "$STAGE"
mkdir -p "$STAGE/jniLibs" "$STAGE/kotlin"
cd "$ROOT"

# Android 15+ devices may use 16 KB pages, and Play requires native libraries
# aligned for them. NDK r28+ does this by default; older NDKs need the flag.
RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-z,max-page-size=16384" \
    cargo ndk --platform "$API_LEVEL" \
    -t arm64-v8a -t armeabi-v7a -t x86_64 \
    -o "$STAGE/jniLibs" \
    build --locked --release -p recached-mobile --lib

cargo build --locked -p recached-mobile --lib
cargo run --locked -q -p recached-mobile --features cli --bin uniffi-bindgen -- \
    generate --library "target/debug/librecached_mobile.so" \
    --language kotlin --no-format --out-dir "$STAGE/kotlin"

ZIP="$OUT/recached-mobile-android-$VERSION.zip"
rm -f "$ZIP"
(cd "$STAGE" && zip -qr "$ZIP" jniLibs kotlin)
(cd "$OUT" && sha256sum "$(basename "$ZIP")" > "$(basename "$ZIP").sha256")
echo "Built $ZIP"
