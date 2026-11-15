#!/usr/bin/env bash
# A release server with line tables and symbols (shipped profile otherwise:
# thin LTO, codegen-units 1, overflow checks, jemalloc), for perf.
# Usage: build-sym.sh <label> <tree-dir> <out-dir>  ->  <out-dir>/recached-server-<label>-sym
set -euo pipefail
label=$1; tree=$(cd "$2" && pwd); out=$(mkdir -p "$3" && cd "$3" && pwd)
docker run --rm -v "$tree":/src:ro -v recached-6g-cargo-registry:/usr/local/cargo/registry \
  -v recached-6g-target-prof:/target -v "$out":/out -e CARGO_TARGET_DIR=/target \
  -e CARGO_PROFILE_RELEASE_DEBUG=line-tables-only -e CARGO_PROFILE_RELEASE_STRIP=false -w /src \
  recached-6g-rust:local \
  bash -euc "cargo build --release --locked -p recached --bin recached-server && \
             cp /target/release/recached-server /out/recached-server-$label-sym"
