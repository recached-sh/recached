#!/usr/bin/env bash
# Interleaved in-process benchmark of N test binaries: each round runs them
# forward then backward (A B C D D C B A), so drift in host load biases no
# build. Appends every run's rows to one CSV.
#
# usage: abba-n.sh <out.csv> <rounds> <test-name> label=bin [label=bin ...]
#   test-name: request_path_bench | hot_key_scaling_bench
# Env passed through: BENCH_REPS, BENCH_ONLY, BENCH_SYNC.
set -euo pipefail
out=$1 rounds=$2 test=$3; shift 3
var=BENCH_OUT; [[ $test == hot_key_scaling_bench ]] && var=BENCH_OUT_HOT
sum() { { shasum -a 256 "$1" 2>/dev/null || sha256sum "$1"; } | cut -c1-16; }
load_avg() { cat /proc/loadavg 2>/dev/null || uptime | sed 's/.*load averages*: //'; }
cpu_name() { if [ -r /proc/cpuinfo ]; then grep -m1 'model name' /proc/cpuinfo | sed 's/.*: //'; else sysctl -n machdep.cpu.brand_string; fi; }
labels=(); bins=()
for lb in "$@"; do labels+=("${lb%%=*}"); bins+=("${lb#*=}"); done
{ echo "# $(date -u +%FT%TZ) $test reps=${BENCH_REPS:-default} sync=${BENCH_SYNC:-0}"
  for i in "${!labels[@]}"; do echo "#   ${labels[$i]} = ${bins[$i]} sha256 $(sum "${bins[$i]}")"; done
  echo "# host: $(cpu_name) ($(uname -s)); load: $(load_avg)"; } >>"$out"
one() { env BENCH_LABEL="${labels[$1]}" "$var=$out" "${bins[$1]}" "$test" --exact "tests::request_path::$test" --ignored --nocapture --test-threads=1 >/dev/null; }
n=${#labels[@]}
for r in $(seq "$rounds"); do
  for ((i = 0; i < n; i++)); do one $i; done
  for ((i = n - 1; i >= 0; i--)); do one $i; done
  echo "# round $r done, load: $(load_avg)" >>"$out"
done
