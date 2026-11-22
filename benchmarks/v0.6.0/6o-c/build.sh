#!/usr/bin/env bash
# build.sh <label>...: release build of ~/recached-6oc-bench/src-<label> into bin/rc-<label>.
# One target dir PER label: a shared one did not rebuild the workspace crates for a
# source at a different path (6O-C: rc-c1diag and rc-basediag came out identical to rc-c1).
set -euo pipefail
export PATH=$HOME/.cargo/bin:$PATH
B=$HOME/recached-6oc-bench
for label in "$@"; do
  cd "$B/src-$label"
  echo "== $label source: $(git rev-parse HEAD) dirty=$(git status --porcelain | wc -l)"
  rustc --version
  CARGO_TARGET_DIR=$B/target-$label cargo build --release --locked -p recached --bin recached-server
  cp "$B/target-$label/release/recached-server" "$B/bin/rc-$label"
  sha256sum "$B/bin/rc-$label"
done
