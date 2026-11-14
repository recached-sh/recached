#!/usr/bin/env bash
# Run a 6G Linux benchmark script inside recached-6g-bench:local with the
# built servers mounted, writing provenance next to the results.
# Usage: run.sh <memory.sh|throughput.sh> <out-dir> [VAR=value ...]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
script=$1; out=$(mkdir -p "$2" && cd "$2" && pwd); shift 2
bins=$(cd "$here/../../../target/6g/linux-bins" && pwd)
envs=(); for kv in "$@"; do envs+=(-e "$kv"); done
{
  echo "date_utc: $(date -u +%FT%TZ)"
  echo "host: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m), $(sysctl -n hw.ncpu 2>/dev/null) logical CPUs, macOS $(sw_vers -productVersion 2>/dev/null)"
  echo "host_load_at_start: $(uptime | sed 's/.*averages*: //')"
  echo "vm: $(docker info --format '{{.OperatingSystem}} kernel {{.KernelVersion}}, {{.NCPU}} vCPU, {{.MemTotal}} B')"
  echo "args: $*"
  echo "trees: v051=$(git -C "$here/../../../target/6g/wt-v051" rev-parse --short HEAD) dev=$(git -C "$here/../../../target/6g/wt-dev" rev-parse --short HEAD) opt=$(git -C "$here/../../../target/6g/wt-opt" rev-parse --short HEAD 2>/dev/null)"
  (cd "$bins" && shasum -a 256 recached-server-* )
} >"$out/conditions.txt"
docker run --rm --name "recached-6g-$(basename "$script" .sh)" \
  -v "$bins":/bins:ro -v "$here":/scripts:ro -v "$out":/out \
  "${envs[@]}" recached-6g-bench:local \
  bash -c "{ uname -a; nproc; grep -m1 'model name' /proc/cpuinfo; free -m; redis-server --version; valkey-server --version; memtier_benchmark --version | head -1; for b in /bins/recached-server-*; do echo \$b: \$(\$b --version); done; } >> /out/conditions.txt; /scripts/$script"
