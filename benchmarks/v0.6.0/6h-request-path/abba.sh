#!/usr/bin/env bash
# ABBA in-process request-path benchmark: alternates two test binaries built
# from server-native/src/tests/request_path.rs, appending every run's rows to
# one CSV. Each binary runs its own min/median of BENCH_REPS internally.
#
# usage: abba.sh <labelA> <binA> <labelB> <binB> <out.csv> [rounds] [test-name]
#   test-name: request_path_bench (default) or hot_key_scaling_bench
# Env passed through: BENCH_REPS, BENCH_ONLY, BENCH_SYNC.
set -euo pipefail
la=$1 ba=$2 lb=$3 bb=$4 out=$5 rounds=${6:-2} test=${7:-request_path_bench}
var=BENCH_OUT; [[ $test == hot_key_scaling_bench ]] && var=BENCH_OUT_HOT
{ echo "# $(date -u +%FT%TZ) $test A=$la($(shasum -a 256 "$ba" | cut -c1-12)) B=$lb($(shasum -a 256 "$bb" | cut -c1-12))"
  echo "# host: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m); load: $(uptime | sed 's/.*load averages*: //')"; } >>"$out"
one() { env BENCH_LABEL="$1" "$var=$out" "$2" "$test" --exact "tests::request_path::$test" --ignored --nocapture --test-threads=1 >/dev/null; }
for r in $(seq "$rounds"); do
  one "$la" "$ba"; one "$lb" "$bb"; one "$lb" "$bb"; one "$la" "$ba"
  echo "# round $r done, load: $(uptime | sed 's/.*load averages*: //')" >>"$out"
done
