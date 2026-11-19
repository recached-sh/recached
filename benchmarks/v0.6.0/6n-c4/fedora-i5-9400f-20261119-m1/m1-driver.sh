#!/usr/bin/env bash
# 6N-C4 Gate M1 + profiling ablation, native Fedora, exact merged binary.
set -u
KIT=/home/dennispaler/Documents/OpenSource/recached-6nc4-bench-pr/benchmarks/v0.6.0/6n-c4/run-m1.sh
B=$HOME/recached-6nc4-bench/target-c4/release/recached-server
SHA=625f88a973e2affef8ab9bb7b1bfae04462ac407
SUM=2ac8601c7f4c5ba14eea7733e94b2a7d7e7d42b7c134ceb36e9f7557efa20710
for _ in $(seq 360); do l=$(cut -d' ' -f1 /proc/loadavg); awk -v l=$l 'BEGIN{exit !(l<0.8)}' && break; sleep 10; done
stamp=$(date -u +%Y%m%dT%H%M%SZ)
echo "start $stamp load $(cat /proc/loadavg)"
OUT=$HOME/recached-6nc4-bench/results-$stamp RC_BIN=$B RC_SHA=$SHA RC_SHA256=$SUM RC_RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)" \
  REPS=3 MAX_LOAD=1.0 bash $KIT >$HOME/recached-6nc4-bench/results-$stamp.log 2>&1
echo "m1 exit $?"
OUT=$HOME/recached-6nc4-bench/profile-$stamp RC_BIN=$B RC_SHA=$SHA RC_SHA256=$SUM RC_RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)" \
  REPS=2 MAX_LOAD=4.0 CELLS="rc-noaof-P1 rc-repl-noaof-P1 rc-coalesced-norep-P1 rc-standalone-P1 rc-coalesced-P1 rc-noaof-P16 rc-repl-noaof-P16 rc-coalesced-norep-P16 rc-standalone-P16 rc-coalesced-P16" \
  bash $KIT >$HOME/recached-6nc4-bench/profile-$stamp.log 2>&1
echo "profile exit $?"
echo "done $(date -u +%FT%TZ) $stamp"
