#!/usr/bin/env bash
# perf profile of one server under one memtier cell, inside recached-6g-perf:local
# (privileged). Usage on the host:
#   docker run --rm --privileged -v <bins>:/bins:ro -v <here>:/scripts:ro -v <out>:/out \
#     -e BIN=<label>-sym -e CONFIG=aof-everysec -e CELL=set-p16 recached-6g-perf:local /scripts/perf.sh
set -euo pipefail
BIN=${BIN:?}; CONFIG=${CONFIG:-standalone}; WORKERS=${WORKERS:-1}; SECS=${SECS:-8}
PORT=7700; dir=/tmp/data; rm -rf $dir; mkdir -p $dir
env=(RECACHED_BIND=127.0.0.1 RECACHED_PORT=$PORT RECACHED_WS_PORT=$((PORT+1)) RECACHED_METRICS_PORT=0
     RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH=$dir/dump.rdb RECACHED_WORKER_THREADS=$WORKERS)
case $CONFIG in
  aof-everysec) env+=(RECACHED_AOF_PATH=$dir/a.aof RECACHED_AOF_SYNC=everysec) ;;
  aof-always)   env+=(RECACHED_AOF_PATH=$dir/a.aof RECACHED_AOF_SYNC=always) ;;
  metrics)      env=("${env[@]/RECACHED_METRICS_PORT=0/RECACHED_METRICS_PORT=$((PORT+2))}") ;;
  capped)       env+=(RECACHED_MAX_MEMORY=1gb RECACHED_EVICTION=allkeys-lru) ;;
esac
env "${env[@]}" taskset -c 0-3 /bins/recached-server-$BIN >/out/server.log 2>&1 &
pid=$!
for _ in $(seq 80); do redis-cli -p $PORT ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
args=(--key-minimum=1 --key-maximum=100000 -d 32 --key-prefix=k: --pipeline=16)
case ${CELL:-set-p16} in
  set-p16) args+=(--ratio=1:0) ;;
  get-p16) args+=(--ratio=0:1) ;;
esac
taskset -c 4-7 memtier_benchmark -s 127.0.0.1 -p $PORT --protocol=redis -t 4 -c 12 --hide-histogram \
  "${args[@]}" --test-time=$((SECS + 4)) >/out/memtier.txt 2>&1 &
mt=$!
sleep 2
perf record -F 999 -g -p $pid -o /out/perf.data -- sleep $SECS >/out/perf-record.log 2>&1
wait $mt || true
perf report -i /out/perf.data --no-children --sort symbol --stdio 2>/dev/null | grep -v "^$" | head -120 >/out/top.txt
perf report -i /out/perf.data --children --sort symbol --stdio 2>/dev/null | grep -E "^ +[0-9]" | head -80 >/out/children.txt
kill -9 $pid 2>/dev/null || true
tail -3 /out/memtier.txt
