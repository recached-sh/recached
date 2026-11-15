#!/usr/bin/env bash
# Build and run the 6J-B prototype inside recached-6g-rust:local (Linux VM), with
# the prototype pinned to vCPUs 0-7 as the thread count requires. Writes
# results.jsonl next to this script. Usage: run.sh [keys] [reps]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
keys=${1:-1000000}; reps=${2:-3}
docker run --rm -v "$here":/src -v recached-6g-cargo-registry:/usr/local/cargo/registry \
  -v recached-6j-target-scanproto:/target -e CARGO_TARGET_DIR=/target -w /src recached-6g-rust:local \
  bash -euc "
    cargo build --release 2>&1 | tail -1
    bin=/target/release/scan-index-prototype
    out=/src/results.jsonl; : > \$out
    for v in global sharded16 sharded64; do \$bin check \$v >> \$out; done
    for rep in \$(seq $reps); do
      for t in 1 2 4 8; do
        for v in global sharded16 sharded64 none; do \$bin create \$v $keys \$t >> \$out; done
      done
    done
    for n in 100000 1000000; do
      for v in global sharded16 sharded64; do
        for c in 10 100 1000; do \$bin scan \$v \$n \$c >> \$out; done
      done
    done
    { nproc; grep -m1 'model name' /proc/cpuinfo; uname -r; } > /src/conditions.txt
  "
echo "results: $here/results.jsonl"
