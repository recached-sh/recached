#!/bin/bash
# 6O-H review E F1 re-measurement: appendfsync always, INCR on expiring counters.
set -u
B=${BENCH_DIR:-$HOME/recached-6oh-bench}; KIT=$B/kit
c=""; for l in base final comb; do c+="$l/rc-always-incrvol-c50-P1 "; done
env RC_BINS="base=$B/bin/rc-base final=$B/bin/rc-final comb=$B/bin/rc-comb" RC_SHAS="base=f9a364d final=c093795 comb=d98dc17" MAX_LOAD=2.5 OUT=$B/results-always2 ALWAYS_REQUESTS=30000   REPS=3 ALTERNATE=1 CELLS="$c" bash $KIT/run-6od.sh > $B/always2.console 2>&1
echo "exit $?" > $B/results-always2/DONE
