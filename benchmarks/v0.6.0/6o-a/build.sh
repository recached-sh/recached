#!/usr/bin/env bash
# build.sh <src-dir> <label>: release + profiling builds of recached-server.
set -euo pipefail
export PATH=$HOME/.cargo/bin:$PATH
src=$1 label=$2 out=$HOME/recached-6oa-bench
cd "$src"
echo "source: $(git rev-parse HEAD) dirty=$(git status --porcelain | wc -l)"
rustc --version
CARGO_TARGET_DIR=$out/target-$label cargo build --release --locked -p recached --bin recached-server
cp $out/target-$label/release/recached-server $out/bin/rc-$label
CARGO_PROFILE_RELEASE_STRIP=none CARGO_PROFILE_RELEASE_DEBUG=line-tables-only RUSTFLAGS="-C force-frame-pointers=yes" \
  CARGO_TARGET_DIR=$out/target-$label-prof cargo build --release --locked -p recached --bin recached-server
cp $out/target-$label-prof/release/recached-server $out/bin/rc-$label-prof
sha256sum $out/bin/rc-$label $out/bin/rc-$label-prof
