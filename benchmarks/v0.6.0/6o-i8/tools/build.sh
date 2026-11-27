#!/usr/bin/env bash
# 6O-I8: build H, I and P (and optional variants) with one toolchain, profile and flag set.
#   build.sh <label> <srcdir> [extra RUSTFLAGS]   -> bin/rc-<label>, logs/build-<label>.txt
set -euo pipefail
export PATH=$HOME/.cargo/bin:$PATH
B=$HOME/recached-6oi8-bench; label=$1; src=$2; extra=${3:-}
unset CARGO_BUILD_RUSTFLAGS
cd "$src"
{ echo "== build $label $(git rev-parse HEAD) dirty=$(git status --porcelain | wc -l) $(date -u +%FT%TZ)"
  rustc -vV; echo "RUSTFLAGS='$extra' profile overrides: ${PROF_ENV:-none}"
  env ${PROF_ENV:-} RUSTFLAGS="$extra" cargo build --release --locked -q -p recached --bin recached-server --target-dir "$B/target-$label"
  cp "$B/target-$label/release/recached-server" "$B/bin/rc-$label"
  sha256sum "$B/bin/rc-$label"
  echo "== done $(date -u +%FT%TZ)"; } > "$B/logs/build-$label.txt" 2>&1
