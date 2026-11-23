#!/usr/bin/env bash
# 6O-E transaction benchmarks, native host. Launch: flatpak-spawn --host bash tx.sh
# For each rep, for each cell, for each label (interleaved): a fresh server,
# populated with 300,000 SETs over 100,000 keys (32 B), then txbench for SECS.
# Records txbench's line, the server's on-CPU ns (schedstat), uprof user
# counters, RSS.
#   LABELS="base fix"  CELLS="<workers>:<mode>:<keyset>:<clients>:<writers> ..."
set -euo pipefail
B=$HOME/recached-6oe-bench; BIN=$HOME/recached-6l-bench/bin
LABELS=${LABELS:-"base fix"}; REPS=${REPS:-3}; SECS=${SECS:-10}
OUT=${OUT:-$B/results-tx-$(date -u +%Y%m%dT%H%M%SZ)}; mkdir -p "$OUT"
MAX_LOAD=${MAX_LOAD:-1.0}
sched_ns() { cat /proc/"$1"/task/*/schedstat 2>/dev/null | awk '{ r += $1 } END { printf "%d", r }'; }
[[ -e /.flatpak-info ]] && { echo "refusing to run in the sandbox"; exit 2; }
{ echo "host $(uname -r) $(nproc) cpus; load $(cut -d' ' -f1-3 /proc/loadavg)"; for l in $LABELS; do [[ $l == redis ]] && echo "redis $($BIN/redis-server --version)" || echo "$l $(sha256sum $B/bin/rc-$l | cut -d' ' -f1)"; done; echo "txbench $(sha256sum $B/bin/txbench | cut -d' ' -f1)"; echo "uprof $(sha256sum $B/bin/uprof | cut -d' ' -f1)"; } > "$OUT/meta.txt"
P=17600
for rep in $(seq 1 "$REPS"); do
  for cell in $CELLS; do
    IFS=: read -r workers mode keyset clients writers <<<"$cell"
    for label in $LABELS; do
      while awk -v m="$MAX_LOAD" '{exit !($1 > m)}' /proc/loadavg; do sleep 5; done
      d=$(mktemp -d -p "$OUT"); P=$((P + 2))
      pin=0; [[ $workers -gt 1 ]] && pin=0-2
      if [[ $label == redis ]]; then
        taskset -c 0 "$BIN/redis-server" --port $P --save '' --appendonly no --dir "$d" >"$d/server.log" 2>&1 & pid=$!
      else
      RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0 \
        RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$d/dump.rdb" RECACHED_WORKER_THREADS=$workers \
        taskset -c $pin "$B/bin/rc-$label" >"$d/server.log" 2>&1 & pid=$!
      fi
      for _ in $(seq 80); do "$BIN/redis-cli" -p $P ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
      taskset -c 3-5 "$BIN/redis-benchmark" -p $P -n 300000 -c 20 -P 64 -r 100000 -d 32 -t set -q >/dev/null 2>&1
      sleep 1
      k0=$(sched_ns $pid)
      "$B/bin/uprof" stat $pid >"$d/uprof.txt" 2>&1 & up=$!
      sleep 0.2
      line=$(taskset -c 3-5 "$B/bin/txbench" $P "$mode" "$keyset" "$clients" "$SECS" "$writers")
      kill -INT $up; wait $up || true
      k1=$(sched_ns $pid)
      rss=$(awk '/VmRSS/{print $2}' /proc/$pid/status)
      kill $pid; wait $pid 2>/dev/null || true
      tx=$(sed -E 's/.* tx=([0-9]+) .*/\1/' <<<"$line")
      wo=$(sed -E 's/.*writer_ops_per_s=([0-9]+).*/\1/' <<<"$line")
      echo "label=$label rep=$rep workers=$workers $line oncpu_s=$(awk -v a=$k0 -v b=$k1 'BEGIN{printf "%.3f",(b-a)/1e9}') rss_kb=$rss uprof=$(tr '\n' ' ' <"$d/uprof.txt" | tr -s ' ')" | tee -a "$OUT/tx.log"
      rm -rf "$d"
    done
  done
done
echo done >> "$OUT/tx.log"
