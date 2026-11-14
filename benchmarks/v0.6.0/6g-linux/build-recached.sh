#!/usr/bin/env bash
# Build one Recached tree's release server for Linux, exactly as release.yml's
# profile does (Cargo.toml [profile.release]: thin LTO, codegen-units 1,
# overflow checks, jemalloc). Usage: build-recached.sh <label> <tree-dir> <out-dir>
set -euo pipefail
label=$1; tree=$(cd "$2" && pwd); out=$(mkdir -p "$3" && cd "$3" && pwd)
docker run --rm \
  -v "$tree":/src:ro \
  -v recached-6g-cargo-registry:/usr/local/cargo/registry \
  -v "recached-6g-target-$label":/target \
  -v "$out":/out \
  -e CARGO_TARGET_DIR=/target -w /src \
  recached-6g-rust:local \
  bash -euc "cargo build --release --locked -p recached --bin recached-server && \
             cp /target/release/recached-server /out/recached-server-$label && \
             /out/recached-server-$label --version"
