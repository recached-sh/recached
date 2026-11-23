#!/usr/bin/env bash
# build.sh <label>...: release build of ~/recached-6od-bench/src-<label> into bin/rc-<label>.
# One target dir PER label (6O-C lesson: a shared one did not rebuild for a source at another path).
set -euo pipefail
export PATH=$HOME/.cargo/bin:$PATH
B=$HOME/recached-6od-bench
for label in "$@"; do
  cd "$B/src-$label"
  echo "== $label source: $(git rev-parse HEAD) dirty=$(git status --porcelain | wc -l)"
  rustc --version
  CARGO_TARGET_DIR=$B/target-$label cargo build --release --locked -p recached --bin recached-server
  cp "$B/target-$label/release/recached-server" "$B/bin/rc-$label"
  sha256sum "$B/bin/rc-$label"
done
