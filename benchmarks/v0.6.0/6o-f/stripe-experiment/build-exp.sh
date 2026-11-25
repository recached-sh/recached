#!/usr/bin/env bash
# build-exp.sh LABEL:SERVER:STORE ...  release builds of src-exp (410235b + exp6of.patch)
set -euo pipefail
export PATH=$HOME/.cargo/bin:$PATH
B=$HOME/recached-6of-bench
cd "$B/src-exp"
echo "== source $(git rev-parse HEAD) + patch sha $(git diff | sha256sum | cut -c1-16)"
for spec in "$@"; do
  IFS=: read -r label server store <<<"$spec"
  echo "== $label server=$server store=$store"
  RC6OF_SERVER_STRIPES=$server RC6OF_STORE_STRIPES=$store CARGO_TARGET_DIR=$B/target-$label \
    nice -n 19 cargo build --release --locked -p recached --bin recached-server 2>&1 | tail -1
  cp "$B/target-$label/release/recached-server" "$B/bin/rc-$label"
  sha256sum "$B/bin/rc-$label"
done
