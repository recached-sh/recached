#!/usr/bin/env bash
# Build the XCFramework and Swift bindings, and package them. macOS only.
#
#   recached-mobile/scripts/build-apple.sh [out-dir]
#
# Produces in <out-dir>:
#
#   RecachedFFI-<version>.xcframework.zip  (+ .sha256)
#   recached-mobile-swift-<version>.zip    RecachedCore.swift (+ .sha256)
#
# The XCFramework's SHA-256 is exactly the checksum SwiftPM's
# binaryTarget(url:checksum:) asks for.
#
# Slices: iOS device (arm64), iOS simulator (arm64 + x86_64), macOS (arm64 +
# x86_64). Each is a static library named lib*.a, which is what SwiftPM
# expects of a library XCFramework. The C header and module map sit at the
# root of each slice's Headers, so Swift can `import RecachedFFI`.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$(mkdir -p "${1:-$ROOT/target/mobile}" && cd "${1:-$ROOT/target/mobile}" && pwd)"
VERSION="$(cargo metadata --manifest-path "$ROOT/Cargo.toml" --format-version 1 --no-deps |
    python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="recached-mobile"))')"
STAGE="$OUT/apple"
LIB=librecached_mobile.a
TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios aarch64-apple-darwin x86_64-apple-darwin)

rm -rf "$STAGE"
mkdir -p "$STAGE"/{headers,swift,ios,ios-simulator,macos}
cd "$ROOT"

rustup target add "${TARGETS[@]}"
export IPHONEOS_DEPLOYMENT_TARGET=15.0 MACOSX_DEPLOYMENT_TARGET=12.0
for target in "${TARGETS[@]}"; do
    cargo build --locked --release -p recached-mobile --lib --target "$target"
done

cp "target/aarch64-apple-ios/release/$LIB" "$STAGE/ios/$LIB"
lipo -create -output "$STAGE/ios-simulator/$LIB" \
    "target/aarch64-apple-ios-sim/release/$LIB" "target/x86_64-apple-ios/release/$LIB"
lipo -create -output "$STAGE/macos/$LIB" \
    "target/aarch64-apple-darwin/release/$LIB" "target/x86_64-apple-darwin/release/$LIB"

# Bindings from an unstripped host build: the release profile strips the
# metadata symbols library mode reads, and the interface is target-independent.
cargo build --locked -p recached-mobile --lib
cargo run --locked -q -p recached-mobile --features cli --bin uniffi-bindgen -- \
    generate --library "target/debug/librecached_mobile.dylib" \
    --language swift --no-format --out-dir "$STAGE/generated"
cp "$STAGE/generated/RecachedCore.swift" "$STAGE/swift/"
cp "$STAGE/generated/RecachedFFI.h" "$STAGE/headers/"
cp "$STAGE/generated/RecachedFFI.modulemap" "$STAGE/headers/module.modulemap"

XCF="$STAGE/RecachedFFI.xcframework"
xcodebuild -create-xcframework \
    -library "$STAGE/ios/$LIB" -headers "$STAGE/headers" \
    -library "$STAGE/ios-simulator/$LIB" -headers "$STAGE/headers" \
    -library "$STAGE/macos/$LIB" -headers "$STAGE/headers" \
    -output "$XCF"

XCF_ZIP="$OUT/RecachedFFI-$VERSION.xcframework.zip"
SWIFT_ZIP="$OUT/recached-mobile-swift-$VERSION.zip"
rm -f "$XCF_ZIP" "$SWIFT_ZIP"
(cd "$STAGE" && ditto -c -k --keepParent RecachedFFI.xcframework "$XCF_ZIP")
(cd "$STAGE/swift" && zip -q "$SWIFT_ZIP" RecachedCore.swift)
for zip in "$XCF_ZIP" "$SWIFT_ZIP"; do
    (cd "$OUT" && shasum -a 256 "$(basename "$zip")" > "$(basename "$zip").sha256")
done
echo "Built $XCF_ZIP ($(cut -d' ' -f1 "$XCF_ZIP.sha256")) and $SWIFT_ZIP"
