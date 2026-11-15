#!/usr/bin/env bash
# 6I-G / D6: does expired data leave memory? SET <unique key> <32 B> EX 1 as
# fast as memtier goes for LOAD_SECS, then idle for IDLE_SECS, sampling the
# key count and RSS every 2 s. A key that expired but was never removed still
# counts in DBSIZE and RSS. Runs inside recached-6i-bench:local (SCRIPT=ttl-churn.sh run.sh <out>).
set -euo pipefail
BINS=${BINS:-/bins}; OUT=${OUT:-/out}; SERVERS=${SERVERS:-"6h redis"}
LOAD_SECS=${LOAD_SECS:-20}; IDLE_SECS=${IDLE_SECS:-30}; PORT=7700
echo "server,phase,t_s,dbsize,rss_kb" >"$OUT/ttl-churn.csv"
for name in $SERVERS; do
  dir=/tmp/data-ttl; rm -rf $dir; mkdir -p $dir
  case $name in
    redis) taskset -c 0-3 redis-server --port $PORT --bind 127.0.0.1 --save "" --appendonly no \
             --dir $dir --protected-mode no >"$OUT/server-$name.log" 2>&1 & ;;
    *) env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$PORT RECACHED_WS_PORT=$((PORT+1)) RECACHED_METRICS_PORT=0 \
         RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH=$dir/dump.rdb RECACHED_WORKER_THREADS=1 \
         taskset -c 0-3 "$BINS/recached-server-$name" >"$OUT/server-$name.log" 2>&1 & ;;
  esac
  pid=$!
  for _ in $(seq 80); do redis-cli -p $PORT ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
  sample() { echo "$name,$1,$2,$(redis-cli -p $PORT dbsize | tr -d '\r'),$(awk '/^VmRSS:/ {print $2}' /proc/$pid/status)" >>"$OUT/ttl-churn.csv"; }
  sample start 0
  taskset -c 4-7 memtier_benchmark -s 127.0.0.1 -p $PORT --protocol=redis -t 4 -c 12 --hide-histogram \
    --pipeline=16 -d 32 --key-minimum=1 --key-maximum=100000000 --key-prefix=t: \
    --command="set __key__ __data__ EX 1" --command-key-pattern=R --test-time=$LOAD_SECS \
    >"$OUT/memtier-$name.txt" 2>&1 &
  mt=$!
  for t in $(seq 2 2 $LOAD_SECS); do sleep 2; sample load $t; done
  wait $mt || true
  for t in $(seq 2 2 $IDLE_SECS); do sleep 2; sample idle $t; done
  kill -9 $pid 2>/dev/null || true; wait $pid 2>/dev/null || true
  grep Totals "$OUT/memtier-$name.txt" | sed "s/^/$name /"
done
cat "$OUT/ttl-churn.csv"
