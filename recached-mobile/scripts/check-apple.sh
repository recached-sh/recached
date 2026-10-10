#!/usr/bin/env bash
# Consume the output of build-apple.sh the way an app would, before anyone
# depends on it: a throwaway SwiftPM package with the XCFramework as a binary
# target and the generated Swift as a source target. Runs it on macOS, then
# builds it for the iOS simulator. macOS only.
#
#   recached-mobile/scripts/check-apple.sh [out-dir]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$(cd "${1:-$ROOT/target/mobile}" && pwd)"
PKG="$(mktemp -d)/Consumer"
trap 'rm -rf "$(dirname "$PKG")"' EXIT

mkdir -p "$PKG/Sources/RecachedCore" "$PKG/Sources/Smoke"
cp -R "$OUT/apple/RecachedFFI.xcframework" "$PKG/"
cp "$OUT/apple/swift/RecachedCore.swift" "$PKG/Sources/RecachedCore/"

cat > "$PKG/Package.swift" <<'EOF'
// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "Consumer",
    platforms: [.iOS(.v15), .macOS(.v12)],
    targets: [
        .binaryTarget(name: "RecachedFFI", path: "RecachedFFI.xcframework"),
        .target(name: "RecachedCore", dependencies: ["RecachedFFI"]),
        .executableTarget(name: "Smoke", dependencies: ["RecachedCore"]),
    ]
)
EOF

cat > "$PKG/Sources/Smoke/main.swift" <<'EOF'
import Foundation
import RecachedCore

final class Sink: FrameSink, @unchecked Sendable {
    var frames: [Data] = []
    func send(frame: Data) { frames.append(frame) }
}

let path = NSTemporaryDirectory() + "recached-smoke-\(ProcessInfo.processInfo.processIdentifier).db"
let client = try RecachedClient.open(path: path, config: ClientConfig())
try client.set(key: "k", value: Data("v".utf8))
precondition(client.get(key: "k") == Data("v".utf8), "read back what was written")

let sink = Sink()
client.connectionOpened(sink: sink)
precondition(sink.frames.count == 2, "session frame + the queued write")

let change = Data("*3\r\n$9\r\nkeychange\r\n$1\r\nx\r\n$1\r\n1\r\n".utf8)
// Outside the precondition: its condition is a non-throwing autoclosure.
let changed = try client.frameReceived(frame: change).changedKeys
precondition(changed == ["x"], "the frame reports the key it changed")

let reopened = try RecachedClient.open(path: path, config: ClientConfig())
precondition(reopened.get(key: "x") == Data("1".utf8), "server state survives a restart")
print("RecachedCore smoke test passed")
EOF

cd "$PKG"
swift run Smoke
xcodebuild -scheme Consumer -destination 'generic/platform=iOS Simulator' -quiet build
echo "XCFramework consumed on macOS and built for the iOS simulator."
