#!/usr/bin/env bash
# Run the 6H memtier matrix one repetition at a time, each started only once
# the macOS host is quiet (1-minute load average below MAX_LOAD, default 5),
# so a busy laptop pauses the benchmark instead of polluting it. Each
# repetition writes its own directory: <out>/rep-<n>/ (results.csv,
# conditions.txt, raw JSON). The load at the start of every repetition is
# recorded in its conditions.txt by run.sh.
# Usage: gated.sh <out-dir> <reps> [VAR=value ...]   (passed to run.sh)
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=$1; reps=$2; shift 2
max=${MAX_LOAD:-5}
load1() { sysctl -n vm.loadavg | awk '{print $2}'; }
for r in $(seq "$reps"); do
  [[ -s $out/rep-$r/results.csv && $(wc -l <"$out/rep-$r/results.csv") -gt 30 ]] && continue
  until awk -v l="$(load1)" -v m="$max" 'BEGIN{exit !(l < m)}'; do
    echo "$(date +%T) rep $r waiting: load $(load1) >= $max"; sleep 60
  done
  echo "$(date +%T) rep $r starting at load $(load1)"
  rm -rf "$out/rep-$r"
  "$here/run.sh" "$out/rep-$r" REPS=1 "$@"
  echo "$(date +%T) rep $r done"
done
