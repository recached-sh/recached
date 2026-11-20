#!/usr/bin/env bash
# 6N-C6: the stall-affected cells again with c6 measured FIRST in every pair,
# to separate an order effect (the second binary of a pair pays for the
# first's AOF writeback and deletion on the HDD) from the binary.
set -uo pipefail
L=$HOME/recached-6nc6-bench
K=$HOME/Documents/OpenSource/recached-6nc6-bench/benchmarks/v0.6.0
until grep -q "two done" $L/run-all.console; do sleep 15; done
sleep 30
export RC_BINS="base=$L/bin/rc-base c6=$L/bin/rc-c6"
export RC_SHAS="base=280743439950811c57764771b171234a78f2f5e6 c6=655868fbaf0627d9dacef2bba784b27c150de1bd"
export RC_RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)"
echo "start $(date -u +%FT%TZ) $(cat /proc/loadavg) io: $(head -1 /proc/pressure/io)"
CELLS="c6/rc-inline-P1 base/rc-inline-P1 c6/rc-coalesced-P1 base/rc-coalesced-P1 \
c6/rc-coalesced-P16 base/rc-coalesced-P16 c6/rc-coalesced-P64 base/rc-coalesced-P64" \
  MAX_LOAD=4.0 OUT=$L/results-reversed REPS=3 bash "$K/6n-c5/run-c5.sh" > $L/reversed.console 2>&1
echo "reversed done $(date -u +%FT%TZ) rc=$? io: $(head -1 /proc/pressure/io)"
