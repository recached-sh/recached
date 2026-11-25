#!/bin/bash
# 6O-F1 performance: dev before (410235b, rc-base) vs after PR #172 (6c394d4, rc-pr1).
B=$HOME/recached-6of-bench; KIT=$B/kit
BINS="base=$B/bin/rc-base pr1=$B/bin/rc-pr1"; SHAS="base=410235b11634ff9ef1fe48736332d31c71caf74c pr1=6c394d4070a1bc556170468b00bb638a86bb463c"
c=""; for t in incr-c50-P1 set-c50-P1 hset-c50-P1 incr-c50-P16; do c+="base/rc-noaof-$t pr1/rc-noaof-$t "; done
c+="base/rc-everysec-incr-c50-P1 pr1/rc-everysec-incr-c50-P1"
until awk '{exit !($1<1.0)}' /proc/loadavg; do sleep 20; done
env RC_BINS="$BINS" RC_SHAS="$SHAS" UPROF=$B/bin/uprof MAX_LOAD=1.0 OUT=$B/results-ttl REQ_C50_P1=6000000 REQ_C50_P16=25000000 \
  REPS=5 ALTERNATE=1 CELLS="$c" bash $KIT/run-6od.sh > $B/ttl.console 2>&1
echo "ttl exit $?" >> $B/f-driver.log
