#!/usr/bin/env bash
# 6L-C A/B: the production store with one key index (BASE) vs 16 shards (NEW),
# through the same benchmark (core-engine/src/store/index_bench.rs) compiled
# into each. Runs alternate base/new, swapping who goes first each
# repetition, so host drift lands on both. Usage: run.sh <base-bin> <new-bin> [reps]
set -euo pipefail
base=$1; new=$2; reps=${3:-3}
run() { # label bin env...
  local label=$1 bin=$2; shift 2
  env "$@" "$bin" --ignored --exact store::index_bench::bench_index --nocapture 2>/dev/null \
    | grep '^{' | sed "s/^{/{\"build\":\"$label\",/"
}
for rep in $(seq 1 "$reps"); do
  pairs=("base $base" "new $new"); (( rep % 2 == 0 )) && pairs=("new $new" "base $base")
  for cfg in \
    "BENCH_MODE=scan BENCH_KEYS=1000000 BENCH_COUNT=10" \
    "BENCH_MODE=scan BENCH_KEYS=1000000 BENCH_COUNT=100" \
    "BENCH_MODE=scan BENCH_KEYS=1000000 BENCH_COUNT=1000" \
    "BENCH_MODE=create BENCH_KEYS=400000 BENCH_THREADS=1" \
    "BENCH_MODE=create BENCH_KEYS=400000 BENCH_THREADS=2" \
    "BENCH_MODE=create BENCH_KEYS=400000 BENCH_THREADS=4" \
    "BENCH_MODE=create BENCH_KEYS=400000 BENCH_THREADS=8" \
    "BENCH_MODE=create BENCH_KEYS=400000 BENCH_THREADS=1 BENCH_CAPPED=1" \
    "BENCH_MODE=create BENCH_KEYS=400000 BENCH_THREADS=4 BENCH_CAPPED=1" \
    "BENCH_MODE=create BENCH_KEYS=400000 BENCH_THREADS=8 BENCH_CAPPED=1" \
    "BENCH_MODE=qsub BENCH_KEYS=1000000 BENCH_PATTERN=t:42:*" \
    "BENCH_MODE=qsub BENCH_KEYS=1000000 BENCH_PATTERN=t:4* BENCH_REPS=20" ; do
    for p in "${pairs[@]}"; do
      # shellcheck disable=SC2086
      run ${p%% *} ${p#* } $cfg
    done
  done
done
