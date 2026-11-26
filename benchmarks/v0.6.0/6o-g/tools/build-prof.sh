#!/usr/bin/env bash
# build-prof.sh <label>...: profiling build (symbols, line tables, frame pointers) of src-<label> into bin/rc-<label>-prof.
set -euo pipefail
export PATH=$HOME/.cargo/bin:$PATH
B=$HOME/recached-6og-bench
for label in "$@"; do
  cd "$B/src-$label"
  echo "== $label source: $(git rev-parse HEAD) dirty=$(git status --porcelain | wc -l)"
  CARGO_PROFILE_RELEASE_STRIP=none CARGO_PROFILE_RELEASE_DEBUG=line-tables-only RUSTFLAGS="-C force-frame-pointers=yes" \
    CARGO_TARGET_DIR=$B/target-$label-prof cargo build --release --locked -p recached --bin recached-server
  cp "$B/target-$label-prof/release/recached-server" "$B/bin/rc-$label-prof"
  sha256sum "$B/bin/rc-$label-prof"
done
