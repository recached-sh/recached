#!/usr/bin/env bash
# 6N-C6 targeted validation: base = dev 2807434 (byte-identical to 6N-C5's
# measured 4328153 binary), c6 = local integration of #144 + #145.
set -uo pipefail
L=$HOME/recached-6nc6-bench
K=$HOME/Documents/OpenSource/recached-6nc6-bench/benchmarks/v0.6.0
export RC_BINS="base=$L/bin/rc-base c6=$L/bin/rc-c6"
export RC_SHAS="base=280743439950811c57764771b171234a78f2f5e6 c6=655868fbaf0627d9dacef2bba784b27c150de1bd"
export RC_RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)"
hostenv() { echo "== $1 $(date -u +%FT%TZ) loadavg: $(cat /proc/loadavg)"; for r in cpu io memory; do echo "psi_$r: $(tr '\n' ' ' < /proc/pressure/$r)"; done; grep -E "^(MemAvailable|SwapTotal|SwapFree|Dirty):" /proc/meminfo | tr -s ' ' | tr '\n' ' '; echo; }
until awk '{ exit !($1 <= 4.0) }' /proc/loadavg; do sleep 15; done
hostenv start | tee $L/host-pressure.txt
echo "start $(date -u +%FT%TZ) $(cat /proc/loadavg)"
CELLS="base/rc-inline-P1 c6/rc-inline-P1 base/rc-coalesced-P1 c6/rc-coalesced-P1 \
base/rc-inline-P16 c6/rc-inline-P16 base/rc-coalesced-P16 c6/rc-coalesced-P16 \
base/rc-repl-noaof-P64 c6/rc-repl-noaof-P64 base/rc-coalesced-P64 c6/rc-coalesced-P64 \
base/rc-repl-noaof-P128 c6/rc-repl-noaof-P128 \
base/rc-coalesced-w2 c6/rc-coalesced-w2 base/rc-inline-w2 c6/rc-inline-w2" \
  MAX_LOAD=4.0 OUT=$L/results-main REPS=3 bash "$K/6n-c5/run-c5.sh" > $L/main.console 2>&1
rc=$?; hostenv after-main >> $L/host-pressure.txt; echo "main done $(date -u +%FT%TZ) rc=$rc"
sleep 20
MAX_LOAD=4.0 MODE=slow OUT=$L/results-slow REPS=3 bash "$K/6n-c6/run-c6-extra.sh" > $L/slow.console 2>&1
rc=$?; echo "slow done $(date -u +%FT%TZ) rc=$rc"
sleep 20
MAX_LOAD=4.0 MODE=two OUT=$L/results-two REPS=3 bash "$K/6n-c6/run-c6-extra.sh" > $L/two.console 2>&1
rc=$?; hostenv end >> $L/host-pressure.txt; echo "two done $(date -u +%FT%TZ) rc=$rc"
