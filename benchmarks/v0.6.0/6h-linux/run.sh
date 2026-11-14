#!/usr/bin/env bash
# Phase 6H: run the 6G Linux throughput driver (../6g-linux/throughput.sh,
# unchanged, so the method matches 6G's) inside recached-6g-bench:local with
# the 6H binary set mounted. Binaries: target/6h/linux-bins/recached-server-<label>.
#   6g   = 6G final build (37710a0; same code tree as dev 42e5ca8), reused from 6G
#   6h   = this branch, built by ../6g-linux/build-recached.sh
#   v051 = v0.5.1
# Usage: [SCRIPT=memory.sh] run.sh <out-dir> [VAR=value ...]   (e.g. SERVERS="6g 6h redis valkey" REPS=5)
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=$(mkdir -p "$1" && cd "$1" && pwd); shift
bins=$(cd "$here/../../../target/6h/linux-bins" && pwd)
envs=(); for kv in "$@"; do envs+=(-e "$kv"); done
{
  echo "date_utc: $(date -u +%FT%TZ)"
  echo "host: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m), $(sysctl -n hw.ncpu 2>/dev/null) logical CPUs, macOS $(sw_vers -productVersion 2>/dev/null)"
  echo "host_load_at_start: $(uptime | sed 's/.*averages*: //')"
  echo "vm: $(docker info --format '{{.OperatingSystem}} kernel {{.KernelVersion}}, {{.NCPU}} vCPU, {{.MemTotal}} B')"
  echo "args: $*"
  echo "branch: $(git -C "$here" rev-parse --abbrev-ref HEAD) $(git -C "$here" rev-parse --short HEAD) (+ working tree)"
  (cd "$bins" && shasum -a 256 recached-server-* )
} >"$out/conditions.txt"
docker run --rm --name "recached-6h-throughput" \
  -v "$bins":/bins:ro -v "$here/../6g-linux":/scripts:ro -v "$out":/out \
  "${envs[@]}" recached-6g-bench:local \
  bash -c "{ uname -a; nproc; grep -m1 'model name' /proc/cpuinfo; free -m; redis-server --version; valkey-server --version; memtier_benchmark --version | head -1; for b in /bins/recached-server-*; do echo \$b: \$(\$b --version); done; } >> /out/conditions.txt; /scripts/${SCRIPT:-throughput.sh}"
