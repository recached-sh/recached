#!/bin/bash
# Native user-space counters for one txbench cell: base vs a candidate, alternating.
# Usage: [BASE=<label>] [OUT=<file>] g-uprof.sh <cand> <mode> <reps>
B=$HOME/recached-6og-bench; BIN=$HOME/recached-6l-bench/bin; C=$1; MODE=$2; REPS=${3:-3}; P=28600
BL=${BASE:-base}
for r in $(seq $REPS); do o="$BL $C"; (( r % 2 == 0 )) && o="$C $BL"; for l in $o; do
  d=$(mktemp -d -p $B); P=$((P+2))
  env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P+1)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH=$d/dump.rdb RECACHED_WORKER_THREADS=1 taskset -c 0 $B/bin/rc-$l > $d/server.log 2>&1 & pid=$!
  for _ in $(seq 80); do $BIN/redis-cli -p $P ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
  taskset -c 3-5 $BIN/redis-benchmark -p $P -n 300000 -c 20 -P 64 -r 100000 -d 32 -t set -q >/dev/null 2>&1; sleep 1
  $B/bin/uprof stat $pid > $d/uprof.txt 2>&1 & up=$!; sleep 0.2
  line=$(taskset -c 3-5 $B/bin/txbench $P $MODE unrelated 50 10 0)
  kill -INT $up; wait $up
  tx=$(echo "$line" | tr ' ' '\n' | grep '^tx=' | cut -d= -f2)
  echo "label=$l mode=$MODE rep=$r tx=$tx $(echo $line | tr ' ' '\n' | grep tx_per_s) uprof=[$(tr '\n' ' ' < $d/uprof.txt)]" | tee -a ${OUT:-/dev/null}
  kill $pid; wait $pid 2>/dev/null; rm -rf $d
done; done
