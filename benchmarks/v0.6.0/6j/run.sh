#!/usr/bin/env bash
# Phase 6J: run one in-container script (default ws-route.sh) inside
# recached-6i-bench:local with the 6J binary set mounted read-only:
# target/6j/linux-bins/recached-server-<label> (checksums in conditions.txt).
# Usage: [SCRIPT=ws-route.sh] run.sh <out-dir> [VAR=value ...]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=$(mkdir -p "$1" && cd "$1" && pwd); shift
bins=$(cd "${BINS_DIR:-$here/../../../target/6j/linux-bins}" && pwd)
envs=(); for kv in "$@"; do envs+=(-e "$kv"); done
{
  echo "date_utc: $(date -u +%FT%TZ)"
  echo "host: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m), $(sysctl -n hw.ncpu 2>/dev/null) logical CPUs, macOS $(sw_vers -productVersion 2>/dev/null)"
  echo "host_load_at_start: $(uptime | sed 's/.*averages*: //')"
  echo "vm: $(docker info --format '{{.OperatingSystem}} kernel {{.KernelVersion}}, {{.NCPU}} vCPU, {{.MemTotal}} B')"
  echo "args: $*"
  echo "branch: $(git -C "$here" rev-parse --abbrev-ref HEAD) $(git -C "$here" rev-parse --short HEAD) (+ working tree)"
  (cd "$bins" && shasum -a 256 recached-server-*)
} >"$out/conditions.txt"
docker run --rm --name "recached-6j-$(echo "${SCRIPT:-ws-route}" | tr /. --)" --ulimit nofile=65536:65536 \
  -v "$bins":/bins:ro -v "$here":/scripts:ro -v "$out":/out \
  "${envs[@]}" recached-6i-bench:local \
  bash -c "{ uname -a; nproc; grep -m1 'model name' /proc/cpuinfo; free -m; redis-server --version; memtier_benchmark --version | head -1; node --version; for b in /bins/recached-server-*; do echo \$b: \$(\$b --version); done; } >> /out/conditions.txt; /scripts/${SCRIPT:-ws-route.sh}"
