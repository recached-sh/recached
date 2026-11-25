#!/usr/bin/env bash
# 6O-F3 stripe study, native host. Launch: flatpak-spawn --host bash stripes.sh
# Derived from 6O-E's tx.sh. Per rep, per cell, per label (label order rotated
# every rep): a fresh server populated with 300,000 SETs over 100,000 keys
# (32 B), then txbench for SECS. Records txbench's line, the server's on-CPU
# ns (schedstat), uprof user counters, RSS, the generator's on-CPU share, and
# for instrumented labels (x*) the delta of `INFO exp6of` over the cell.
# SERVER_ENV="K=V ..." adds server settings (6O-F review F5). With RECACHED_AOF_SYNC in
# it, the cell also gets an AOF path: setting the path is what turns the AOF on.
#   LABELS="base x256 x512 x1024"  CELLS="<workers>:<mode>:<keyset>:<clients>:<writers> ..."
set -euo pipefail
B=$HOME/recached-6of-bench; BIN=$HOME/recached-6l-bench/bin
LABELS=${LABELS:-"base x256 x512 x1024"}; REPS=${REPS:-5}; SECS=${SECS:-30}
OUT=${OUT:-$B/results-stripes-$(date -u +%Y%m%dT%H%M%SZ)}; mkdir -p "$OUT"
MAX_LOAD=${MAX_LOAD:-1.0}
sched_ns() { cat /proc/"$1"/task/*/schedstat 2>/dev/null | awk '{ r += $1 } END { printf "%d", r }'; }
pressure() { printf 'load=%s io_psi10=%s' "$(cut -d' ' -f1 /proc/loadavg)" "$(awk '/^some/{split($2,a,"=");print a[2]}' /proc/pressure/io 2>/dev/null)"; }
info_exp() { "$BIN/redis-cli" -p "$1" INFO exp6of 2>/dev/null | tr -d '\r' | grep ':' | tr '\n' ' '; }
[[ -e /.flatpak-info ]] && { echo "refusing to run in the sandbox"; exit 2; }
{ echo "host $(uname -r) $(nproc) cpus; load $(cut -d' ' -f1-3 /proc/loadavg); governors $(cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor 2>/dev/null | sort | uniq -c | tr '\n' ' ')"
  for l in $LABELS; do echo "$l $(sha256sum $B/bin/rc-$l | cut -d' ' -f1) size=$(stat -c %s $B/bin/rc-$l)"; done
  echo "txbench $(sha256sum $B/bin/txbench | cut -d' ' -f1)"; echo "uprof $(sha256sum $B/bin/uprof | cut -d' ' -f1)"
  echo "reps=$REPS secs=$SECS cells=$CELLS server_env=${SERVER_ENV:-}"; } > "$OUT/meta.txt"
read -r -a LARR <<<"$LABELS"
P=21600
for rep in $(seq 1 "$REPS"); do
  # Rotate the label order each repetition.
  n=${#LARR[@]}; s=$(( (rep - 1) % n )); order=("${LARR[@]:$s}" "${LARR[@]:0:$s}")
  for cell in $CELLS; do
    IFS=: read -r workers mode keyset clients writers <<<"$cell"
    for label in "${order[@]}"; do
      while awk -v m="$MAX_LOAD" '{exit !($1 > m)}' /proc/loadavg; do sleep 5; done
      pre=$(pressure)
      d=$(mktemp -d -p "$OUT"); P=$((P + 2))
      pin=0; [[ $workers -gt 1 ]] && pin=0-2
      # A set RECACHED_AOF_PATH turns the AOF on: only when SERVER_ENV asks for one.
      aof=""; [[ ${SERVER_ENV:-} == *RECACHED_AOF_SYNC* ]] && aof="RECACHED_AOF_PATH=$d/append.aof"
      env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0 \
        RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$d/dump.rdb" RECACHED_WORKER_THREADS=$workers \
        $aof ${SERVER_ENV:-} \
        taskset -c $pin "$B/bin/rc-$label" >"$d/server.log" 2>&1 & pid=$!
      for _ in $(seq 80); do "$BIN/redis-cli" -p $P ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
      taskset -c 3-5 "$BIN/redis-benchmark" -p $P -n 300000 -c 20 -P 64 -r 100000 -d 32 -t set -q >/dev/null 2>&1
      sleep 1
      e0=""; [[ $label == x* ]] && e0=$(info_exp $P)
      k0=$(sched_ns $pid)
      "$B/bin/uprof" stat $pid >"$d/uprof.txt" 2>&1 & up=$!
      sleep 0.2
      taskset -c 3-5 "$B/bin/txbench" $P "$mode" "$keyset" "$clients" "$SECS" "$writers" >"$d/tx.out" & gen=$!
      sleep "$(awk -v s="$SECS" 'BEGIN{print s-1}')"
      g1=$(sched_ns $gen)
      wait $gen
      line=$(cat "$d/tx.out")
      kill -INT $up; wait $up || true
      k1=$(sched_ns $pid)
      e1=""; [[ $label == x* ]] && e1=$(info_exp $P)
      rss=$(awk '/VmRSS/{print $2}' /proc/$pid/status)
      kill $pid; wait $pid 2>/dev/null || true
      echo "label=$label rep=$rep workers=$workers $line oncpu_s=$(awk -v a=$k0 -v b=$k1 'BEGIN{printf "%.3f",(b-a)/1e9}') gen_cpu_share=$(awk -v g=$g1 -v s="$SECS" 'BEGIN{printf "%.3f", g/1e9/((s-1.2)*3)}') rss_kb=$rss $pre uprof=$(tr '\n' ' ' <"$d/uprof.txt" | tr -s ' ') exp_before=[$e0] exp_after=[$e1]" | tee -a "$OUT/tx.log"
      rm -rf "$d"
    done
  done
done
echo done >> "$OUT/tx.log"
