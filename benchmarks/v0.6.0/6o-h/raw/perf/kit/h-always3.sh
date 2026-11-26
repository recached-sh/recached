#!/bin/bash
# 6O-H review E F1 re-measurement: appendfsync always, INCR on expiring counters.
set -u
B=${BENCH_DIR:-$HOME/recached-6oh-bench}; KIT=$B/kit
c=""; for l in base comb comb2; do c+="$l/rc-always-incrvol-c50-P1 "; done
env RC_BINS="base=$B/bin/rc-base comb=$B/bin/rc-comb comb2=$B/bin/rc-comb2" RC_SHAS="base=f9a364d comb=d98dc17 comb2=53a2f1e" MAX_LOAD=2.5 OUT=$B/results-always3 ALWAYS_REQUESTS=30000   REPS=3 ALTERNATE=1 CELLS="$c" bash $KIT/run-6od.sh > $B/always3.console 2>&1
echo "exit $?" > $B/results-always3/DONE
