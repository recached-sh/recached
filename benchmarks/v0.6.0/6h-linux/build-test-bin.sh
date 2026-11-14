#!/usr/bin/env bash
# Build one tree's server *test* binary (which carries the in-process
# request-path benchmark, server-native/src/tests/request_path.rs) for Linux,
# with the shipped release profile, in recached-6g-rust:local.
# Usage: build-test-bin.sh <label> <tree-dir> <out-dir>  ->  <out-dir>/t-<label>
# One cargo target volume (TARGET_VOLUME, default recached-6h-target) is shared
# by every tree, so a second tree rebuilds only what differs.
set -euo pipefail
label=$1; tree=$(cd "$2" && pwd); out=$(mkdir -p "$3" && cd "$3" && pwd)
docker run --rm \
  -v "$tree":/src:ro \
  -v recached-6g-cargo-registry:/usr/local/cargo/registry \
  -v "${TARGET_VOLUME:-recached-6h-target}":/target \
  -v "$out":/out \
  -e CARGO_TARGET_DIR=/target -w /src \
  recached-6g-rust:local \
  bash -euc "cargo build --release --locked -p recached --bin recached-server && \
             cp /target/release/recached-server /out/recached-server-$label && \
             bin=\$(cargo test --release --locked -p recached --no-run 2>&1 | sed -n 's/.*Executable unittests src\/main.rs (\(.*\))/\1/p') && \
             cp \"\$bin\" /out/t-$label && ls -la /out"
