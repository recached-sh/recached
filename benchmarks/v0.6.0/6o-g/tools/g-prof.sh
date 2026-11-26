#!/bin/bash
# Native sampled profiles of small EXECs, base vs gcap, from profiling builds (symbols, frame
# pointers; same sources as the release binaries). Launch on the host after campaign 2:
#   flatpak-spawn --host bash ~/recached-6og-bench/g-prof.sh <mode...>
# Per rep and label (alternating): fresh server on CPU 0 (1 worker), populate, then txbench
# for SECS with `uprof stat` and `uprof record` attached. Writes $OUT/<label>-<mode>-r<rep>/.
B=$HOME/recached-6og-bench; BIN=$HOME/recached-6l-bench/bin
REPS=${REPS:-3}; SECS=${SECS:-15}; OUT=${OUT:-$B/results2-prof}; P=29600
mkdir -p "$OUT"
for mode in "$@"; do for r in $(seq "$REPS"); do
  order="base gcap"; (( r % 2 == 0 )) && order="gcap base"
  for l in $order; do
    d=$OUT/$l-$mode-r$r; rm -rf "$d"; mkdir -p "$d"; P=$((P + 2))
    while awk '{exit !($1 > 3.0)}' /proc/loadavg || ps -eo comm | grep -qE '^(cc1|rustc|cargo|ld)$'; do sleep 5; done
    env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0 \
      RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH=$d/dump.rdb RECACHED_WORKER_THREADS=1 \
      taskset -c 0 $B/bin/rc-$l-prof > $d/server.log 2>&1 & pid=$!
    for _ in $(seq 80); do $BIN/redis-cli -p $P ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
    taskset -c 3-5 $BIN/redis-benchmark -p $P -n 300000 -c 20 -P 64 -r 100000 -d 32 -t set -q >/dev/null 2>&1; sleep 1
    $B/bin/uprof stat $pid > $d/uprof-stat.txt 2>&1 & st=$!
    $B/bin/uprof record $pid 2999 $d/samples.txt lbr > $d/uprof-record.txt 2>&1 & rec=$!
    sleep 0.3
    taskset -c 3-5 $B/bin/txbench $P $mode unrelated 50 $SECS 0 > $d/tx.out 2>$d/tx.err
    kill -INT $st $rec; wait $st $rec
    echo "label=$l mode=$mode rep=$r $(cat $d/tx.out) uprof=[$(tr '\n' ' ' < $d/uprof-stat.txt)]" | tee -a $OUT/prof.log
    kill $pid; wait $pid 2>/dev/null; rm -f $d/dump.rdb
  done
done; done
echo done >> $OUT/prof.log
