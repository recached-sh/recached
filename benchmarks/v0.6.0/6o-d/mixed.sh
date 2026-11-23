#!/bin/bash
# 6O-D1 mixed contention: GET, MGET x10 and MSET x10 at once against one server (3 workers on CPUs 0-2),
# three redis-benchmark processes on CPUs 3-5, 100,000-key space populated by 300,000 SETs first.
# base (69f0d1c) and fix alternate, REPS repetitions. Output: $OUT/mixed.txt (one line per rep and label).
set -u
B=$HOME/recached-6od-bench; BIN=$HOME/recached-6l-bench/bin; OUT=${OUT:-$B/results-d1-mixed}; REPS=${REPS:-3}
N_GET=${N_GET:-6000000}; N_MGET=${N_MGET:-1200000}; N_MSET=${N_MSET:-1200000}
mkdir -p $OUT; P=17501
[[ -e /.flatpak-info ]] && { echo "run natively"; exit 2; }
keys() { for _ in $(seq 10); do printf 'key:__rand_int__ '; done; }
for rep in $(seq $REPS); do
  order="base fix"; (( rep % 2 == 0 )) && order="fix base"
  for label in $order; do
    until awk '{exit !($1<1.0)}' /proc/loadavg; do sleep 10; done
    d=$OUT/$label-r$rep; rm -rf $d; mkdir -p $d/p
    env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P+1)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
      RECACHED_SAVE_PATH=$d/p/dump.rdb RECACHED_WORKER_THREADS=3 taskset -c 0-2 $B/bin/rc-$label > $d/server.log 2>&1 & pid=$!
    for _ in $(seq 80); do $BIN/redis-cli -p $P ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
    $BIN/redis-benchmark -p $P -c 20 -P 64 -r 100000 -d 32 -q -n 300000 -t set >/dev/null 2>&1
    t0=$(date +%s%N)
    taskset -c 3 $BIN/redis-benchmark -p $P -n $N_GET -c 20 -P 1 -d 32 -r 100000 --csv -t get > $d/get.csv 2>&1 & g=$!
    taskset -c 4 $BIN/redis-benchmark -p $P -n $N_MGET -c 15 -P 1 -d 32 -r 100000 --csv MGET $(keys) > $d/mget.csv 2>&1 & m=$!
    taskset -c 5 $BIN/redis-benchmark -p $P -n $N_MSET -c 15 -P 1 -d 32 -r 100000 --csv -t mset > $d/mset.csv 2>&1 & s=$!
    wait $g $m $s; t1=$(date +%s%N)
    cpu=$(awk '{print ($14+$15)/100}' /proc/$pid/stat)
    kill -9 $pid; wait $pid 2>/dev/null
    rps() { tail -1 $1 | cut -d, -f2 | tr -d '"'; }
    p99() { tail -1 $1 | cut -d, -f7 | tr -d '"'; }
    echo "label=$label rep=$rep wall_s=$(( (t1-t0)/1000000 ))e-3 server_cpu_s=$cpu get_rps=$(rps $d/get.csv) get_p99=$(p99 $d/get.csv) mget_rps=$(rps $d/mget.csv) mget_p99=$(p99 $d/mget.csv) mset_rps=$(rps $d/mset.csv) mset_p99=$(p99 $d/mset.csv) sha256=$(sha256sum $B/bin/rc-$label | cut -c1-16)" | tee -a $OUT/mixed.txt
  done
done
