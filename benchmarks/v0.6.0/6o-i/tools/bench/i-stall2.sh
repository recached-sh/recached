#!/bin/bash
# 6O-I PR 6, review E H3: readers of a replica while its primary commits transactions of
# TX_N = 1 (one record: no group), 100, 1000, 10000 writes. Replica workers 1 and 3; labels base and
# p3, rotated every rep; 5 reps. Primary CPU 0, replica CPU 1 (1 worker) or 1-2 (3 workers);
# txbench setn (2 clients) on CPU 3; the reader, redis-benchmark GET c10 P1 on CPUs 4-5 against the
# replica, a fixed request count (its latency summary is always printed), --csv.
set -u
B=$HOME/recached-6oi-bench; BIN=$HOME/recached-6l-bench/bin
OUT=$B/results-stall-$(date -u +%Y%m%dT%H%M%SZ); mkdir -p $OUT
REPS=${REPS:-5}; LABELS=(base p3); P=26600
{ for l in "${LABELS[@]}"; do echo "$l $(sha256sum $B/bin/rc-$l | cut -d' ' -f1) -> $(readlink -f $B/bin/rc-$l)"; done; echo "reps=$REPS"; } > $OUT/meta.txt
for rep in $(seq 1 $REPS); do
  s=$(( (rep - 1) % 2 )); order=("${LABELS[@]:$s}" "${LABELS[@]:0:$s}")
  for workers in 1 3; do for n in 1 100 1000 10000; do for label in "${order[@]}"; do
    until awk '{exit !($1<3.0)}' /proc/loadavg && ! ps -eo comm | grep -qE '^(cc1|rustc|cargo|ld)$'; do sleep 5; done
    d=$(mktemp -d -p $OUT); P=$((P+5))
    rcpu=1; [[ $workers == 3 ]] && rcpu=1-2
    env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P+1)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
      RECACHED_SAVE_PATH=$d/p.rdb RECACHED_WORKER_THREADS=1 RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=$((P+2)) \
      taskset -c 0 $B/bin/rc-$label >$d/p.log 2>&1 & pp=$!
    for _ in $(seq 80); do $BIN/redis-cli -p $P ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
    env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$((P+3)) RECACHED_WS_PORT=$((P+4)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
      RECACHED_SAVE_PATH=$d/r.rdb RECACHED_WORKER_THREADS=$workers RECACHED_REPLICAOF=127.0.0.1:$((P+2)) \
      taskset -c $rcpu $B/bin/rc-$label >$d/r.log 2>&1 & rp=$!
    for _ in $(seq 80); do $BIN/redis-cli -p $((P+3)) ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
    taskset -c 3-5 $BIN/redis-benchmark -p $P -n 300000 -c 20 -P 64 -r 100000 -d 32 -t set -q >/dev/null 2>&1
    sleep 2
    env TX_N=$n taskset -c 3 $B/bin/txbench $P setn unrelated 2 25 0 >$d/tx.out 2>&1 & tp=$!
    sleep 1
    csv=$(taskset -c 4-5 $BIN/redis-benchmark -p $((P+3)) -n 600000 -c 10 -P 1 -r 100000 -t get --csv 2>/dev/null | tail -1)
    wait $tp
    f() { echo "$csv" | awk -F'","' -v i=$1 '{gsub(/"/,"",$i); print $i}'; }
    echo "label=$label rep=$rep replica_workers=$workers tx_n=$n $(cat $d/tx.out) reader_rps=$(f 2) reader_p50_ms=$(f 5) reader_p99_ms=$(f 7) reader_max_ms=$(f 8) replica_hwm_kb=$(awk '/VmHWM/{print $2}' /proc/$rp/status) replica_dbsize=$($BIN/redis-cli -p $((P+3)) DBSIZE) primary_dbsize=$($BIN/redis-cli -p $P DBSIZE) load=$(cut -d' ' -f1 /proc/loadavg) ts=$(date -u +%FT%TZ)" | tee -a $OUT/stall.log
    kill $rp $pp 2>/dev/null; wait $rp $pp 2>/dev/null; rm -rf $d
  done; done; done
done
echo "stall done $(date -u +%FT%TZ) out=$OUT" >> $B/logs/i-stall2.console
