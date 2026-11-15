#!/usr/bin/env bash
# Build the shipped release profile of recached-server (thin LTO, codegen-units
# 1, overflow checks, jemalloc) for Linux in recached-6g-rust:local, from a
# source tree, into <out-dir>/recached-server-<label>. SYM=1 keeps line tables
# and symbols for perf. A dedicated target volume per label avoids reusing
# crates built from another tree with older mtimes.
# Usage: build.sh <label> <tree-dir> <out-dir>
set -euo pipefail
label=$1; tree=$(cd "$2" && pwd); out=$(mkdir -p "$3" && cd "$3" && pwd)
extra=(); suffix=
if [[ ${SYM:-0} == 1 ]]; then
  extra=(-e CARGO_PROFILE_RELEASE_DEBUG=line-tables-only -e CARGO_PROFILE_RELEASE_STRIP=false); suffix=-sym
fi
docker run --rm -v "$tree":/src:ro -v recached-6g-cargo-registry:/usr/local/cargo/registry \
  -v "recached-6j-target-$label":/target -v "$out":/out -e CARGO_TARGET_DIR=/target ${extra[@]+"${extra[@]}"} -w /src \
  recached-6g-rust:local \
  bash -euc "cargo build --release --locked -p recached --bin recached-server && \
             cp /target/release/recached-server /out/recached-server-$label$suffix"
(cd "$out" && shasum -a 256 "recached-server-$label$suffix")
