#!/usr/bin/env bash
# Phase 6J-A: sync fan-out by routing scenario. Runs INSIDE recached-6i-bench:local
# via `SCRIPT=ws-route.sh ../6i-prod/run.sh <out>` (run.sh mounts this directory's
# parent scripts at /scripts; see run-6j.sh, which mounts 6j/ instead).
#
# Server: strict sync scoping (RECACHED_SYNC_SECRET, RECACHED_WS_AUTH=token),
# no persistence, metrics off, WORKERS execution threads, pinned to CPUs 0-3.
# Clients: ws-route.mjs on 4-5 (SCENARIO picks their grants, see the .mjs).
# Load: memtier on 6-7, rate-limited to FIXED_RATE per connection x 24
# connections (12,000 writes/s by default), on the scenario's key family, plus
# a probe writer (in-scope SETs + DEDUP retries) at PROBE_RATE/s.
#
# A 0-client cell measures what the same server sustains with nobody connected.
set -euo pipefail
BINS=${BINS:-/bins}
OUT=${OUT:-/out}
HERE=$(cd "$(dirname "$0")" && pwd)
SERVERS=${SERVERS:-"6i"}
SCENARIOS=${SCENARIOS:-"D"}
CLIENT_SET=${CLIENT_SET:-"0 100 500"}
WORKER_SET=${WORKER_SET:-"1 4"}
REPS=${REPS:-1}
TEST_TIME=${TEST_TIME:-10}
PROBE_RATE=${PROBE_RATE:-200}
FIXED_RATE=${FIXED_RATE:-500}
PORT=${PORT:-7700}
WSPORT=$((PORT + 1))
SECRET=${SECRET:-6j-benchmark-secret-0123456789abcdef0123456789}
mkdir -p "$OUT/ws" "$OUT/logs" "$OUT/info"
csv=$OUT/ws-route-results.csv
[[ -s $csv ]] || echo "server,scenario,workers,clients,rep,offered,ops_per_sec,p99_ms,server_cpu_s,cpu_s_per_mops,rss_idle_kb,rss_clients_kb,kb_per_client,rss_end_kb,connected,pushes,probe_n,probe_p50_ms,probe_p95_ms,probe_p99_ms,probe_max_ms,leaks,reconnects,reconnect_fail,reconnect_p99_ms,renewals,renew_fail,churns,churn_fail,slow_opened,slow_closed,closes_unexpected,gap_closes,dedup_first,dedup_dup,dedup_wrong,dedup_applied,errors,route_events,route_candidates,route_skipped,forwarded,filtered" >"$csv"

cpu_s() { awk -v hz="$(getconf CLK_TCK)" '{printf "%.3f", ($14 + $15) / hz}' "/proc/$1/stat"; }
rss_kb() { awk '/^VmRSS:/ {print $2}' "/proc/$1/status"; }

keyargs() {
  case $1 in
    C) echo "--key-prefix=g0: --key-minimum=1 --key-maximum=100000" ;;
    E) echo "--key-prefix=e: --key-minimum=1 --key-maximum=$(( $2 > 0 ? 10 * $2 : 1000 ))" ;;
    *) echo "--key-prefix=bg: --key-minimum=1 --key-maximum=${KEY_MAX:-100000}" ;;
  esac
}

for rep in $(seq "$REPS"); do
 for scenario in $SCENARIOS; do
  for workers in $WORKER_SET; do
   for clients in $CLIENT_SET; do
    for name in $SERVERS; do
      tag="$name-$scenario-w$workers-c$clients-$rep"
      dir=/tmp/data-ws; rm -rf "$dir"; mkdir -p "$dir"
      env RECACHED_BIND=127.0.0.1 RECACHED_PORT="$PORT" RECACHED_WS_PORT="$WSPORT" \
        RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$dir/dump.rdb" \
        RECACHED_WORKER_THREADS="$workers" RECACHED_SYNC_SECRET="$SECRET" RECACHED_WS_AUTH=token \
        RECACHED_MAX_CONNECTIONS=20000 \
        taskset -c 0-3 "$BINS/recached-server-$name" >"$OUT/logs/server-$tag.log" 2>&1 &
      PID=$!
      for _ in $(seq 120); do redis-cli -p "$PORT" ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
      sleep 1
      idle=$(rss_kb "$PID")
      dur=$(( TEST_TIME + 8 ))
      cpids=()
      procs=0
      if (( clients > 0 )); then
        procs=1; (( clients >= 200 )) && procs=2; (( clients >= 2000 )) && procs=4
        per=$(( (clients + procs - 1) / procs ))
        for p in $(seq 0 $((procs - 1))); do
          first=$(( p * per )); n=$(( clients - first )); (( n > per )) && n=$per
          SCENARIO=$scenario taskset -c 4-5 node --max-old-space-size=2048 "$HERE/ws-route.mjs" clients \
            "ws://127.0.0.1:$WSPORT" "$SECRET" "$first" "$n" "$clients" "$dur" \
            "$OUT/ws/$tag-clients-$p.json" >"$OUT/logs/clients-$tag-$p.log" 2>&1 &
          cpids+=($!)
        done
        for p in $(seq 0 $((procs - 1))); do
          for _ in $(seq 480); do grep -q READY "$OUT/logs/clients-$tag-$p.log" 2>/dev/null && break; sleep 0.25; done
        done
      fi
      withc=$(rss_kb "$PID")
      wpid=
      if (( clients > 0 )); then
        SCENARIO=$scenario taskset -c 6-7 node "$HERE/ws-route.mjs" writer "$PORT" 0 "$clients" "$PROBE_RATE" "$TEST_TIME" \
          "$OUT/ws/$tag-writer.json" >"$OUT/logs/writer-$tag.log" 2>&1 &
        wpid=$!
      fi
      redis-cli -p "$PORT" INFO >"$OUT/info/$tag-before.txt" 2>&1 || true
      c0=$(cpu_s "$PID")
      # shellcheck disable=SC2046
      taskset -c 6-7 memtier_benchmark -s 127.0.0.1 -p "$PORT" --protocol=redis -t 2 -c 12 \
        --hide-histogram --distinct-client-seed $(keyargs "$scenario" "$clients") -d 32 \
        --ratio=1:0 --pipeline=1 --rate-limiting="$FIXED_RATE" --test-time="$TEST_TIME" \
        --json-out-file="$OUT/ws/$tag-memtier.json" >"$OUT/logs/memtier-$tag.log" 2>&1 || true
      c1=$(cpu_s "$PID")
      endrss=$(rss_kb "$PID")
      redis-cli -p "$PORT" INFO >"$OUT/info/$tag-after.txt" 2>&1 || true
      [[ -n $wpid ]] && { wait "$wpid" || true; }
      for c in "${cpids[@]}"; do wait "$c" || true; done
      gaps=$(grep -c "lagged, missed" "$OUT/logs/server-$tag.log" || true)
      kill -9 "$PID" 2>/dev/null || true; wait "$PID" 2>/dev/null || true; sleep 0.5
      python3 -I "$HERE/ws_route_row.py" "$OUT" "$tag" "$name" "$scenario" "$workers" "$clients" "$rep" \
        "$((FIXED_RATE * 24))" "$c0" "$c1" "$idle" "$withc" "$endrss" "$gaps" >>"$csv"
      tail -1 "$csv"
    done
   done
  done
 done
done
