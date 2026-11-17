#!/usr/bin/env bash
# Interleaved 6L-D comparison: base (harness only), R1, R2 release test binaries
# of bench_replicated_set_cpu. Each repetition runs every build once, in turn,
# so host drift lands on all three alike.
# Usage: [BUILDS="base r1 r2"] run.sh <bin-dir> <out-dir> [reps]
set -euo pipefail
bins=$1; out=$2; reps=${3:-5}
mkdir -p "$out"
{ echo "date_utc: $(date -u +%FT%TZ)"; uname -srm
  (sysctl -n machdep.cpu.brand_string 2>/dev/null || grep -m1 'model name' /proc/cpuinfo) | sed 's/^/cpu: /'
  echo "load_at_start: $(uptime | sed 's/.*load average[s]*: //')"
  (cd "$bins" && shasum -a 256 bench-* 2>/dev/null || sha256sum bench-*); } > "$out/conditions.txt"
echo "build,rep,ws,writers,ops_s,cpu_ns_per_op,ws_gaps" > "$out/raw.csv"
for rep in $(seq 1 "$reps"); do
  for b in ${BUILDS:-base r1 r2}; do
    REPS=1 OPS=${OPS:-400000} "$bins/bench-$b" bench_replicated_set_cpu --ignored --nocapture --test-threads=1 2>/dev/null \
      | grep -E '^(true|false),' | awk -F, -v b="$b" -v r="$rep" '{print b","r","$1","$2","$3","$4","$5}' >> "$out/raw.csv"
  done
done
echo "load_at_end: $(uptime | sed 's/.*load average[s]*: //')" >> "$out/conditions.txt"
python3 "$(dirname "$0")/summarize.py" "$out/raw.csv" > "$out/summary.md"
cat "$out/summary.md"
