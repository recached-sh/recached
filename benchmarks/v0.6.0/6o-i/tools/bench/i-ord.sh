#!/bin/bash
# 6O-I PR 6, review E H2: ordinary commands on the paths 6O-I changed.
# Labels base (6O-H 6068337) and p3 (#200 at 976574a), rotated every rep; kinds noaof (control),
# everysec (AOF), replica (one replica, no AOF), everysec+replica; tests set incr lpush mset.
# A fresh primary (CPU 0; replica CPU 1) per run; redis-benchmark c50 P1 on CPUs 3-5, --csv.
# Records ops/s, the primary's on-CPU seconds (schedstat), its and the replica's VmHWM.
set -u
B=$HOME/recached-6oi-bench; BIN=$HOME/recached-6l-bench/bin
OUT=$B/results-ord-$(date -u +%Y%m%dT%H%M%SZ); mkdir -p $OUT
REPS=${REPS:-5}; LABELS=(base p3); P=25600
sched_ns() { cat /proc/"$1"/task/*/schedstat 2>/dev/null | awk '{ r += $1 } END { printf "%d", r }'; }
{ for l in "${LABELS[@]}"; do echo "$l $(sha256sum $B/bin/rc-$l | cut -d' ' -f1) -> $(readlink -f $B/bin/rc-$l)"; done; echo "reps=$REPS host $(uname -r) load $(cut -d' ' -f1-3 /proc/loadavg)"; } > $OUT/meta.txt
for rep in $(seq 1 $REPS); do
  s=$(( (rep - 1) % 2 )); order=("${LABELS[@]:$s}" "${LABELS[@]:0:$s}")
  for kind in noaof everysec replica everysec+replica; do for t in set incr lpush mset; do for label in "${order[@]}"; do
    until awk '{exit !($1<3.0)}' /proc/loadavg && ! ps -eo comm | grep -qE '^(cc1|rustc|cargo|ld)$'; do sleep 5; done
    d=$(mktemp -d -p $OUT); P=$((P+5))
    aof=""; [[ $kind == everysec* ]] && aof="RECACHED_AOF_PATH=$d/a.aof RECACHED_AOF_SYNC=everysec"
    repl=""; [[ $kind == *replica ]] && repl="RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=$((P+2))"
    env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P+1)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
      RECACHED_SAVE_PATH=$d/p.rdb RECACHED_WORKER_THREADS=1 $aof $repl taskset -c 0 $B/bin/rc-$label >$d/p.log 2>&1 & pp=$!
    for _ in $(seq 80); do $BIN/redis-cli -p $P ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
    rp=""
    if [[ $kind == *replica ]]; then
      env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$((P+3)) RECACHED_WS_PORT=$((P+4)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
        RECACHED_SAVE_PATH=$d/r.rdb RECACHED_WORKER_THREADS=1 RECACHED_REPLICAOF=127.0.0.1:$((P+2)) taskset -c 1 $B/bin/rc-$label >$d/r.log 2>&1 & rp=$!
      for _ in $(seq 80); do $BIN/redis-cli -p $((P+3)) ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
      sleep 1
    fi
    n=1000000; [[ $t == mset ]] && n=300000
    k0=$(sched_ns $pp)
    csv=$(taskset -c 3-5 $BIN/redis-benchmark -p $P -n $n -c 50 -P 1 -r 100000 -d 32 -t $t --csv 2>/dev/null | tail -1)
    k1=$(sched_ns $pp)
    rps=$(echo "$csv" | awk -F'","' '{gsub(/"/,"",$2); print $2}')
    p99=$(echo "$csv" | awk -F'","' '{gsub(/"/,"",$7); print $7}')
    mx=$(echo "$csv" | awk -F'","' '{gsub(/"/,"",$8); print $8}')
    phwm=$(awk '/VmHWM/{print $2}' /proc/$pp/status); rhwm=0; [[ -n $rp ]] && rhwm=$(awk '/VmHWM/{print $2}' /proc/$rp/status)
    echo "label=$label rep=$rep kind=$kind test=$t requests=$n rps=$rps p99_ms=$p99 max_ms=$mx oncpu_s=$(awk -v a=$k0 -v b=$k1 'BEGIN{printf "%.3f",(b-a)/1e9}') primary_hwm_kb=$phwm replica_hwm_kb=$rhwm load=$(cut -d' ' -f1 /proc/loadavg) ts=$(date -u +%FT%TZ)" | tee -a $OUT/ord.log
    [[ -n $rp ]] && { kill $rp; wait $rp 2>/dev/null; }
    kill $pp; wait $pp 2>/dev/null; rm -rf $d
  done; done; done
done
echo "ord done $(date -u +%FT%TZ) out=$OUT" >> $B/logs/i-ord.console
