#!/usr/bin/env bash
# build-jcc.sh <label>...: release build of src-<label> with LLVM's 32-byte branch-boundary alignment
# (Intel JCC erratum mitigation) into bin/rc-<label>-jcc: a layout control for review D2-1.
set -euo pipefail
export PATH=$HOME/.cargo/bin:$PATH
B=$HOME/recached-6og-bench
for label in "$@"; do
  cd "$B/src-$label"
  echo "== $label source: $(git rev-parse HEAD)"
  RUSTFLAGS="-C llvm-args=-x86-branches-within-32B-boundaries" CARGO_TARGET_DIR=$B/target-$label-jcc \
    cargo build --release --locked -p recached --bin recached-server
  cp "$B/target-$label-jcc/release/recached-server" "$B/bin/rc-$label-jcc"
  sha256sum "$B/bin/rc-$label-jcc"
done
