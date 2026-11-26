#!/usr/bin/env bash
# 6O-I transaction cells (copy of 6O-G g-tx.sh; adds aof_bytes per cell), native host.
# Per rep, per cell, per label (label order rotated every rep): a fresh server populated with
# 300,000 SETs over 100,000 keys (32 B), then txbench for SECS. Records txbench's line (tx,
# outside readers, outside writers), and per cell (review D, M-2/M-3/M-4):
#   - the server's on-CPU ns (schedstat) and the generator's (schedstat, sampled before exit);
#   - foreign CPU: all CPUs' busy time (/proc/stat) minus server, generator and replica;
#   - peak and current RSS; INFO evicted_keys (capped cells); the replica's state at the end, with the
#     primary's replication offset, lag, frames sent and link disconnects (review D2-11).
# Every cell first waits until no compiler runs and the 1-minute load is under MAX_LOAD.
#   LABELS="base g"  CELLS="<workers>:<mode>:<keyset>:<clients>:<writers>[:<txbench env,..>[:<server env,..>]] ..."
# Server env: RECACHED_AOF_SYNC adds an AOF path; REPLICA=1 enables replication and starts a
# replica (same binary, CPU 1) that follows the primary for the whole cell.
set -euo pipefail
B=$HOME/recached-6oi-bench; BIN=$HOME/recached-6l-bench/bin
LABELS=${LABELS:-"base g"}; REPS=${REPS:-5}; SECS=${SECS:-20}
OUT=${OUT:-$B/results-tx-$(date -u +%Y%m%dT%H%M%SZ)}; mkdir -p "$OUT"
MAX_LOAD=${MAX_LOAD:-3.0}
sched_ns() { cat /proc/"$1"/task/*/schedstat 2>/dev/null | awk '{ r += $1 } END { printf "%d", r }'; }
busy_ns() { awk '/^cpu /{ printf "%d", ($2+$3+$4+$7+$8) * 1e9 / 100 }' /proc/stat; }
pressure() { printf 'load=%s io_psi10=%s' "$(cut -d' ' -f1 /proc/loadavg)" "$(awk '/^some/{split($2,a,"=");print a[2]}' /proc/pressure/io 2>/dev/null)"; }
compilers() { ps -eo comm | grep -cE '^(cc1|rustc|cargo|ld|cc1plus)$' || true; }
[[ -e /.flatpak-info ]] && { echo "refusing to run in the sandbox"; exit 2; }
{ echo "host $(uname -r) $(nproc) cpus; load $(cut -d' ' -f1-3 /proc/loadavg); governors $(cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor 2>/dev/null | sort | uniq -c | tr '\n' ' ')"
  for l in $LABELS; do echo "$l $(sha256sum $B/bin/rc-$l | cut -d' ' -f1) size=$(stat -L -c %s $B/bin/rc-$l) -> $(readlink -f $B/bin/rc-$l)"; done
  echo "txbench $(sha256sum $B/bin/txbench | cut -d' ' -f1)"
  echo "reps=$REPS secs=$SECS max_load=$MAX_LOAD cells=$CELLS"; } > "$OUT/meta.txt"
# Review E reruns: a run that stops must not leave servers holding the next campaign's ports.
trap 'kill $(jobs -p) 2>/dev/null || true' EXIT
read -r -a LARR <<<"$LABELS"
P=${PORT_BASE:-23600}
for rep in $(seq 1 "$REPS"); do
  n=${#LARR[@]}; s=$(( (rep - 1) % n )); order=("${LARR[@]:$s}" "${LARR[@]:0:$s}")
  for cell in $CELLS; do
    IFS=: read -r workers mode keyset clients writers txenv srvenv <<<"$cell"
    txenv=${txenv:-}; srvenv=${srvenv:-}
    # A misplaced separator would shift the server env into a field nobody reads.
    [[ $srvenv == :* || $txenv == *RECACHED_* ]] && { echo "bad cell: $cell" >&2; exit 3; }
    for label in "${order[@]}"; do
      waited=0
      while [[ $(compilers) -gt 0 ]] || awk -v m="$MAX_LOAD" '{exit !($1 > m)}' /proc/loadavg; do sleep 5; waited=$((waited + 5)); done
      pre=$(pressure)
      d=$(mktemp -d -p "$OUT"); P=$((P + 4))
      pin=0; [[ $workers -gt 1 ]] && pin=0-2
      senv=${srvenv//,/ }
      raof=""; [[ $senv == *REPLICA_AOF=1* ]] && raof="RECACHED_AOF_PATH=$d/replica.aof RECACHED_AOF_SYNC=everysec"; senv=${senv//REPLICA_AOF=1/}
      replica=0; [[ $senv == *REPLICA=1* ]] && replica=1; senv=${senv//REPLICA=1/}
      aof=""; [[ $senv == *RECACHED_AOF_SYNC* ]] && aof="RECACHED_AOF_PATH=$d/append.aof"
      repl=""; [[ $replica == 1 ]] && repl="RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=$((P + 2))"
      env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0 \
        RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$d/dump.rdb" RECACHED_WORKER_THREADS=$workers \
        $aof $repl $senv \
        taskset -c $pin "$B/bin/rc-$label" >"$d/server.log" 2>&1 & pid=$!
      for _ in $(seq 80); do "$BIN/redis-cli" -p $P ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
      rpid=""
      if [[ $replica == 1 ]]; then
        env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$((P + 3)) RECACHED_WS_PORT=$((P + 3 + 1000)) RECACHED_METRICS_PORT=0 \
          RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$d/replica.rdb" RECACHED_WORKER_THREADS=1 \
          RECACHED_REPLICAOF=127.0.0.1:$((P + 2)) $raof taskset -c 1 "$B/bin/rc-$label" >"$d/replica.log" 2>&1 & rpid=$!
        for _ in $(seq 80); do "$BIN/redis-cli" -p $((P + 3)) ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
      fi
      taskset -c 3-5 "$BIN/redis-benchmark" -p $P -n 300000 -c 20 -P 64 -r 100000 -d 32 -t set -q >/dev/null 2>&1
      sleep 1
      k0=$(sched_ns $pid); b0=$(busy_ns); r0=0; [[ -n $rpid ]] && r0=$(sched_ns $rpid)
      ev0=$("$BIN/redis-cli" -p $P INFO stats 2>/dev/null | tr -d '\r' | awk -F: '/^evicted_keys/{print $2}')
      env ${txenv//,/ } taskset -c 3-5 "$B/bin/txbench" $P "$mode" "$keyset" "$clients" "$SECS" "$writers" >"$d/tx.out" 2>"$d/tx.err" & gen=$!
      sleep "$(awk -v s="$SECS" 'BEGIN{print s-0.3}')"
      g1=$(sched_ns $gen)
      wait $gen || true
      k1=$(sched_ns $pid); b1=$(busy_ns); r1=0; [[ -n $rpid ]] && r1=$(sched_ns $rpid)
      ev1=$("$BIN/redis-cli" -p $P INFO stats 2>/dev/null | tr -d '\r' | awk -F: '/^evicted_keys/{print $2}')
      line=$(cat "$d/tx.out")
      rinfo=""
      if [[ -n $rpid ]]; then
        rinfo="replica_dbsize=$("$BIN/redis-cli" -p $((P + 3)) DBSIZE 2>/dev/null) primary_dbsize=$("$BIN/redis-cli" -p $P DBSIZE 2>/dev/null) replica_oncpu_s=$(awk -v a=$r0 -v b=$r1 'BEGIN{printf "%.3f",(b-a)/1e9}')"
        # Review D2-11: that the replica streamed for the whole cell, from the primary's side.
        rinfo+=" $("$BIN/redis-cli" -p $P INFO replication 2>/dev/null | tr -d '\r' | grep -E '^(connected_slaves|recached_replication_(lag_frames|offset|send_frames|queue_disconnects|syncs_full|syncs_partial)):' | sed 's/^recached_replication_/repl_/' | tr ':' '=' | tr '\n' ' ')"
        rinfo+=" replica_hwm_kb=$(awk '/VmHWM/{print $2}' /proc/$rpid/status)"
        kill $rpid 2>/dev/null; wait $rpid 2>/dev/null || true
      fi
      hwm=$(awk '/VmHWM/{print $2}' /proc/$pid/status 2>/dev/null || true); rss=$(awk '/VmRSS/{print $2}' /proc/$pid/status 2>/dev/null || true)
      # AOF counters from the running server (a SIGTERM's shutdown save would truncate the file).
      aofstats=$("$BIN/redis-cli" -p $P INFO persistence 2>/dev/null | tr -d '\r' | grep -E '^recached_aof_(batches|frames|bytes|write_calls|write_usec|fsyncs|fsync_usec):' | sed 's/^recached_aof_/aof_/' | tr ':' '=' | tr '\n' ' ' || true)
      ts=$(date -u +%FT%TZ)
      alive=1; kill -0 $pid 2>/dev/null || alive=0
      kill $pid 2>/dev/null; wait $pid 2>/dev/null || true
      srv_s=$(awk -v a=$k0 -v b=$k1 'BEGIN{printf "%.3f",(b-a)/1e9}')
      gen_s=$(awk -v g=$g1 'BEGIN{printf "%.3f",g/1e9}')
      foreign=$(awk -v b0=$b0 -v b1=$b1 -v s=$srv_s -v g=$gen_s -v r0=$r0 -v r1=$r1 -v t=$SECS 'BEGIN{printf "%.3f", ((b1-b0)/1e9 - s - g - (r1-r0)/1e9) / t}')
      echo "label=$label rep=$rep workers=$workers txenv=$txenv srvenv=$srvenv $line oncpu_s=$srv_s gen_oncpu_s=$gen_s gen_cpus=$(awk -v g=$gen_s -v t=$SECS 'BEGIN{printf "%.2f",g/t}') foreign_cpus=$foreign $aofstats ts=$ts evicted=$(( ${ev1:-0} - ${ev0:-0} )) hwm_kb=$hwm rss_kb=$rss alive=$alive waited_s=$waited $rinfo $pre" | tee -a "$OUT/tx.log"
      rm -rf "$d"
    done
  done
done
echo done >> "$OUT/tx.log"
