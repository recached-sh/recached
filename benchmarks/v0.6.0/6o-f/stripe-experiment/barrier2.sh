#!/usr/bin/env bash
# 6O-F3 capped-store all-stripe paths (barrier.sh's capped-set used a cap above the
# dataset, so no write could evict and each took only its own stripes).
#   capped-evict  SET over 100,000 keys with RECACHED_MAX_KEYS=50000 (allkeys-lru), c50 P1,
#                 1 worker: writes that may evict take the all-write barrier
#   capped-exec   txbench get-set unrelated c50 on 3 workers, RECACHED_MAX_KEYS=1000000:
#                 every EXEC on a capped store takes the all-write barrier
set -euo pipefail
B=$HOME/recached-6of-bench; BIN=$HOME/recached-6l-bench/bin
LABELS=${LABELS:-"base x256 x512 x1024 x2048 x1024s"}; REPS=${REPS:-3}
OUT=${OUT:-$B/results-barrier2}; mkdir -p "$OUT"
[[ -e /.flatpak-info ]] && { echo "refusing to run in the sandbox"; exit 2; }
cli() { "$BIN/redis-cli" -p "$@"; }
expi() { cli "$1" INFO exp6of 2>/dev/null | tr -d '\r' | grep -E '^(all_calls|all_ns|all_max_ns|stripe_calls):' | tr '\n' ' ' || true; }
quiet() { while awk '{exit !($1 > 1.0)}' /proc/loadavg; do sleep 5; done; }
ready() { for _ in $(seq 120); do cli "$1" ping 2>/dev/null | grep -q PONG && return; sleep 0.25; done; }
{ for l in $LABELS; do echo "$l $(sha256sum $B/bin/rc-$l | cut -d' ' -f1)"; done; } > "$OUT/meta.txt"
read -r -a LARR <<<"$LABELS"; P=31600
for rep in $(seq 1 "$REPS"); do
  n=${#LARR[@]}; s=$(( (rep - 1) % n )); order=("${LARR[@]:$s}" "${LARR[@]:0:$s}")
  for label in "${order[@]}"; do
    quiet; d=$(mktemp -d -p "$OUT"); P=$((P + 4))
    RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
      RECACHED_SAVE_PATH="$d/dump.rdb" RECACHED_WORKER_THREADS=1 RECACHED_MAX_KEYS=50000 RECACHED_EVICTION=allkeys-lru \
      taskset -c 0 "$B/bin/rc-$label" >"$d/s.log" 2>&1 & pid=$!
    ready $P; sleep 1; a=$(expi $P)
    r=$(taskset -c 3-5 "$BIN/redis-benchmark" -p $P -n 2000000 -c 50 -r 100000 -d 32 --threads 2 -t set -q 2>/dev/null | tr '\r' '\n' | grep 'requests per second' | tail -1)
    b=$(expi $P); k=$(cli $P DBSIZE)
    echo "label=$label rep=$rep test=capped-evict result=[$r] dbsize=$k before=[$a] after=[$b]" | tee -a "$OUT/barrier2.log"
    kill $pid 2>/dev/null || { echo "server $pid for $label exited early" | tee -a "$OUT/barrier2.log"; }; wait $pid 2>/dev/null || true; rm -rf "$d"
    quiet; d=$(mktemp -d -p "$OUT"); P=$((P + 4))
    RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
      RECACHED_SAVE_PATH="$d/dump.rdb" RECACHED_WORKER_THREADS=3 RECACHED_MAX_KEYS=1000000 \
      taskset -c 0-2 "$B/bin/rc-$label" >"$d/s.log" 2>&1 & pid=$!
    ready $P; taskset -c 3-5 "$BIN/redis-benchmark" -p $P -n 300000 -c 20 -P 64 -r 100000 -d 32 -t set -q >/dev/null 2>&1; sleep 1
    a=$(expi $P); line=$(taskset -c 3-5 "$B/bin/txbench" $P get-set unrelated 50 20 4); b=$(expi $P)
    echo "label=$label rep=$rep test=capped-exec $line before=[$a] after=[$b]" | tee -a "$OUT/barrier2.log"
    kill $pid 2>/dev/null || { echo "server $pid for $label exited early" | tee -a "$OUT/barrier2.log"; }; wait $pid 2>/dev/null || true; rm -rf "$d"
  done
done
echo done >> "$OUT/barrier2.log"
