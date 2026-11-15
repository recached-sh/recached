#!/usr/bin/env bash
# Phase 6I: run one in-container script (default prod.sh) inside
# recached-6i-bench:local (recached-6g-bench:local + Node 22.23.4, made with
# `docker commit`, no registry pull) with the immutable 6I binary set mounted:
# target/6i/linux-bins/recached-server-<label> (checksums in its SHA256SUMS).
# Usage: [SCRIPT=prod.sh] run.sh <out-dir> [VAR=value ...]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=$(mkdir -p "$1" && cd "$1" && pwd); shift
bins=$(cd "$here/../../../target/6i/linux-bins" && pwd)
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
docker run --rm --name "recached-6i-${SCRIPT%%.*}" \
  -v "$bins":/bins:ro -v "$here":/scripts:ro -v "$out":/out \
  "${envs[@]}" recached-6i-bench:local \
  bash -c "{ uname -a; nproc; grep -m1 'model name' /proc/cpuinfo; free -m; redis-server --version; valkey-server --version; memtier_benchmark --version | head -1; node --version; for b in /bins/recached-server-*; do echo \$b: \$(\$b --version); done; } >> /out/conditions.txt; /scripts/${SCRIPT:-prod.sh}"
