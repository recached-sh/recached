#!/usr/bin/env bash
# 6O-F3 whole-keyspace barriers, native host, instrumented labels only (x*).
# Launch: flatpak-spawn --host bash barrier.sh
# Per rep, per label (order rotated), fresh populated server (100,000 keys, 32 B):
#   save-idle   20 x SAVE on an idle 3-worker server: wall ms per SAVE, all-stripe ns
#   save-load   20 x SAVE while txbench runs get-get/unrelated/c50/w4
#   sweep       SET key:__rand_int__ <32 B> PX 200 at c50 P1, 1 worker: rps, and the
#               sweeper's all-stripe barriers over the run
#   capped-set  SET at c50 P1, 1 worker, RECACHED_MAX_KEYS above the dataset: every
#               capped write takes the all-write barrier
#   fullsync    a replication-enabled primary under txbench load; a replica attaches
#               (full sync): the primary's all-stripe time
set -euo pipefail
B=$HOME/recached-6of-bench; BIN=$HOME/recached-6l-bench/bin
LABELS=${LABELS:-"x256 x512 x1024 x2048"}; REPS=${REPS:-3}
OUT=${OUT:-$B/results-barrier-$(date -u +%Y%m%dT%H%M%SZ)}; mkdir -p "$OUT"
[[ -e /.flatpak-info ]] && { echo "refusing to run in the sandbox"; exit 2; }
cli() { "$BIN/redis-cli" -p "$@"; }
expi() { cli "$1" INFO exp6of | tr -d '\r' | grep -E '^(all_calls|all_ns|all_max_ns|gate_drain_ns|gate_closes):' | tr '\n' ' '; }
quiet() { while awk '{exit !($1 > 1.0)}' /proc/loadavg; do sleep 5; done; }
ready() { for _ in $(seq 120); do cli "$1" ping 2>/dev/null | grep -q PONG && return; sleep 0.25; done; }
populate() { taskset -c 3-5 "$BIN/redis-benchmark" -p "$1" -n 300000 -c 20 -P 64 -r 100000 -d 32 -t set -q >/dev/null 2>&1; }
saves() { local port=$1 t0 t1 tot=0 i; for i in $(seq 20); do t0=$(date +%s%N); cli $port SAVE >/dev/null; t1=$(date +%s%N); tot=$((tot + t1 - t0)); done; awk -v t=$tot 'BEGIN{printf "save_ms_mean=%.2f", t/20/1e6}'; }
{ echo "host $(uname -r); load $(cut -d' ' -f1-3 /proc/loadavg)"; for l in $LABELS; do echo "$l $(sha256sum $B/bin/rc-$l | cut -d' ' -f1)"; done; echo "txbench $(sha256sum $B/bin/txbench | cut -d' ' -f1)"; } > "$OUT/meta.txt"
read -r -a LARR <<<"$LABELS"
P=25600
for rep in $(seq 1 "$REPS"); do
  n=${#LARR[@]}; s=$(( (rep - 1) % n )); order=("${LARR[@]:$s}" "${LARR[@]:0:$s}")
  for label in "${order[@]}"; do
    # save-idle, save-load
    quiet; d=$(mktemp -d -p "$OUT"); P=$((P + 4))
    RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
      RECACHED_SAVE_PATH="$d/dump.rdb" RECACHED_WORKER_THREADS=3 taskset -c 0-2 "$B/bin/rc-$label" >"$d/s.log" 2>&1 & pid=$!
    ready $P; populate $P; sleep 1
    a=$(expi $P); r=$(saves $P); b=$(expi $P)
    echo "label=$label rep=$rep test=save-idle $r before=[$a] after=[$b]" | tee -a "$OUT/barrier.log"
    taskset -c 3-5 "$B/bin/txbench" $P get-get unrelated 50 14 4 >"$d/tx.out" & gen=$!
    sleep 1; a=$(expi $P); r=$(saves $P); b=$(expi $P); wait $gen
    echo "label=$label rep=$rep test=save-load $r tx=[$(cat "$d/tx.out")] before=[$a] after=[$b]" | tee -a "$OUT/barrier.log"
    kill $pid; wait $pid 2>/dev/null || true; rm -rf "$d"
    # sweep
    quiet; d=$(mktemp -d -p "$OUT"); P=$((P + 4))
    RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
      RECACHED_SAVE_PATH="$d/dump.rdb" RECACHED_WORKER_THREADS=1 taskset -c 0 "$B/bin/rc-$label" >"$d/s.log" 2>&1 & pid=$!
    ready $P; sleep 1; a=$(expi $P)
    r=$(taskset -c 3-5 "$BIN/redis-benchmark" -p $P -n 3000000 -c 50 -r 100000 --threads 2 -q SET 'key:__rand_int__' vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv PX 200 2>/dev/null | tr '\r' '\n' | grep 'requests per second' | tail -1)
    b=$(expi $P); dbs=$(cli $P DBSIZE)
    echo "label=$label rep=$rep test=sweep result=[$r] dbsize_after=$dbs before=[$a] after=[$b]" | tee -a "$OUT/barrier.log"
    kill $pid; wait $pid 2>/dev/null || true; rm -rf "$d"
    # capped: every write on a capped store takes the all-write barrier
    quiet; d=$(mktemp -d -p "$OUT"); P=$((P + 4))
    RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
      RECACHED_SAVE_PATH="$d/dump.rdb" RECACHED_WORKER_THREADS=1 RECACHED_MAX_KEYS=1000000 taskset -c 0 "$B/bin/rc-$label" >"$d/s.log" 2>&1 & pid=$!
    ready $P; sleep 1; a=$(expi $P)
    r=$(taskset -c 3-5 "$BIN/redis-benchmark" -p $P -n 2000000 -c 50 -r 100000 -d 32 --threads 2 -t set -q 2>/dev/null | tr '\r' '\n' | grep 'requests per second' | tail -1)
    b=$(expi $P)
    echo "label=$label rep=$rep test=capped-set result=[$r] before=[$a] after=[$b]" | tee -a "$OUT/barrier.log"
    kill $pid; wait $pid 2>/dev/null || true; rm -rf "$d"
    # fullsync
    quiet; d=$(mktemp -d -p "$OUT"); P=$((P + 4)); RP=$((P + 2)); RPORT=$((P + 3))
    RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
      RECACHED_SAVE_PATH="$d/dump.rdb" RECACHED_WORKER_THREADS=3 RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=$RPORT \
      taskset -c 0-2 "$B/bin/rc-$label" >"$d/s.log" 2>&1 & pid=$!
    ready $P; populate $P; sleep 1
    taskset -c 3-4 "$B/bin/txbench" $P get-get unrelated 50 10 4 >"$d/tx.out" & gen=$!
    sleep 3; a=$(expi $P)
    mkdir -p "$d/r"
    RECACHED_BIND=127.0.0.1 RECACHED_PORT=$RP RECACHED_WS_PORT=$((RP + 100)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
      RECACHED_SAVE_PATH="$d/r/dump.rdb" RECACHED_WORKER_THREADS=1 RECACHED_REPLICAOF=127.0.0.1:$RPORT \
      taskset -c 5 "$B/bin/rc-$label" >"$d/r.log" 2>&1 & rpid=$!
    wait $gen; b=$(expi $P); rk=$(cli $RP DBSIZE 2>/dev/null || echo n/a)
    echo "label=$label rep=$rep test=fullsync tx=[$(cat "$d/tx.out")] replica_dbsize=$rk before=[$a] after=[$b]" | tee -a "$OUT/barrier.log"
    kill $rpid $pid; wait $rpid $pid 2>/dev/null || true; rm -rf "$d"
  done
done
echo done >> "$OUT/barrier.log"
