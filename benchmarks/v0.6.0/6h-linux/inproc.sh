#!/usr/bin/env bash
# The in-process request-path and hot-key benchmarks on Linux: runs
# ../6h-request-path/abba-n.sh inside recached-6g-bench:local over the Linux
# test binaries t-<label> in target/6h/linux-bins.
# Usage: inproc.sh <out-dir> <rounds> <test-name> <label>...
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=$(mkdir -p "$1" && cd "$1" && pwd); rounds=$2; test=$3; shift 3
bins=$(cd "$here/../../../target/6h/linux-bins" && pwd)
args=(); for l in "$@"; do args+=("$l=/bins/t-$l"); done
docker run --rm --name recached-6h-inproc \
  -v "$bins":/bins:ro -v "$here/../6h-request-path":/scripts:ro -v "$out":/out \
  -e BENCH_REPS="${BENCH_REPS:-5}" -e TMPDIR=/tmp recached-6g-bench:local \
  bash -c "nproc; /scripts/abba-n.sh /out/$test.csv $rounds $test ${args[*]}"
