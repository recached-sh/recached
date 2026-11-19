#!/bin/bash
# Waits for an idle host (1-min load < 1.0 for two checks in a row), then runs
# the 6N-C Gate M1 kit once, natively, against the pinned binary.
set -u
cd "$HOME/recached-6nc-bench/src-m1" || exit 1
ok=0
while (( ok < 2 )); do
  l=$(cut -d' ' -f1 /proc/loadavg)
  if awk -v l="$l" 'BEGIN { exit !(l < 1.0) }'; then ok=$((ok + 1)); else ok=0; fi
  echo "$(date -u +%FT%TZ) load=$l ok=$ok"
  sleep 30
done
export RC_SHA=0b48332d840811d35ee95b46b909384f211a122f
export RC_BIN=$HOME/recached-6nc-bench/target-m1/release/recached-server
export RC_SHA256=6d88663d9e7f789980ee6fd45e8d597ecc06fdc37a418ff10c142efcae1edfe4
export RC_RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)"
export OUT=$HOME/recached-6nc-bench/results-$(date -u +%Y%m%dT%H%M%SZ)
echo "starting M1 -> $OUT"
bash benchmarks/v0.6.0/6n-c/run-m1.sh
echo "M1 exit $?"
