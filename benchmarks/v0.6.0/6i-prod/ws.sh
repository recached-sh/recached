#!/usr/bin/env bash
# Phase 6I-B Mode 4: browser synchronization under load. Runs INSIDE
# recached-6i-bench:local via `SCRIPT=ws.sh run.sh <out>`.
#
# Server: strict sync scoping (RECACHED_SYNC_SECRET, RECACHED_WS_AUTH=token),
# no persistence, metrics off, WORKERS execution threads, pinned to 0-3.
# Clients: ws-sync.mjs on 4-5, one tenant per client (t<i>:*), every 4th
# WATCHing its probe key, RECONNECT_PCT % reconnecting every 5 s.
# Load: memtier on 6-7 writing keys OUTSIDE every scope (bg:*) — each write is
# still broadcast to every WebSocket connection task for scope filtering —
# plus a probe writer (in-scope SETs + DEDUP retries) at PROBE_RATE/s.
#
# Cells per client count:
#   ws-max    memtier pipeline 16, as fast as the server goes
#   ws-fixed  memtier rate-limited to FIXED_RATE per connection (×24 conns)
# This is Recached-specific functionality; nothing here is a Redis comparison.
set -euo pipefail
BINS=${BINS:-/bins}
OUT=${OUT:-/out}
HERE=$(cd "$(dirname "$0")" && pwd)
SERVERS=${SERVERS:-"6h"}
CLIENT_SET=${CLIENT_SET:-"1 10 100 1000"}
WORKER_SET=${WORKER_SET:-"1 4"}
REPS=${REPS:-1}
TEST_TIME=${TEST_TIME:-10}
CELLS=${CELLS:-"ws-max ws-fixed"}
PROBE_RATE=${PROBE_RATE:-200}
FIXED_RATE=${FIXED_RATE:-500}
PORT=${PORT:-7700}
WSPORT=$((PORT + 1))
SECRET=${SECRET:-6i-benchmark-secret-0123456789abcdef0123456789}
mkdir -p "$OUT/ws" "$OUT/logs"
csv=$OUT/ws-results.csv
[[ -s $csv ]] || echo "server,workers,clients,cell,rep,ops_per_sec,p99_ms,server_cpu_s,cpu_s_per_mops,rss_idle_kb,rss_clients_kb,kb_per_client,rss_end_kb,connected,pushes,probe_n,probe_p50_ms,probe_p95_ms,probe_p99_ms,probe_max_ms,leaks,reconnects,reconnect_fail,reconnect_p99_ms,resumed,closes_unexpected,dedup_first,dedup_dup,dedup_wrong,dedup_applied,errors" >"$csv"

cpu_s() { awk -v hz="$(getconf CLK_TCK)" '{printf "%.3f", ($14 + $15) / hz}' "/proc/$1/stat"; }
rss_kb() { awk '/^VmRSS:/ {print $2}' "/proc/$1/status"; }

for rep in $(seq "$REPS"); do
 for workers in $WORKER_SET; do
  for name in $SERVERS; do
   for clients in $CLIENT_SET; do
    for cell in $CELLS; do
      tag="$name-w$workers-c$clients-$cell-$rep"
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
      # Clients in up to two processes (ws-sync is single-threaded).
      procs=1; (( clients >= 200 )) && procs=2
      per=$(( (clients + procs - 1) / procs ))
      dur=$(( TEST_TIME + 8 ))
      cpids=()
      for p in $(seq 0 $((procs - 1))); do
        first=$(( p * per )); n=$(( clients - first )); (( n > per )) && n=$per
        taskset -c 4-5 node "$HERE/ws-sync.mjs" clients "ws://127.0.0.1:$WSPORT" "$SECRET" "$first" "$n" "$dur" \
          "$OUT/ws/$tag-clients-$p.json" >"$OUT/logs/clients-$tag-$p.log" 2>&1 &
        cpids+=($!)
      done
      for p in $(seq 0 $((procs - 1))); do
        for _ in $(seq 240); do grep -q READY "$OUT/logs/clients-$tag-$p.log" 2>/dev/null && break; sleep 0.25; done
      done
      withc=$(rss_kb "$PID")
      taskset -c 6-7 node "$HERE/ws-sync.mjs" writer "$PORT" 0 "$clients" "$PROBE_RATE" "$TEST_TIME" \
        "$OUT/ws/$tag-writer.json" >"$OUT/logs/writer-$tag.log" 2>&1 &
      wpid=$!
      case $cell in
        ws-max)   margs=(--pipeline=16) ;;
        ws-fixed) margs=(--pipeline=1 --rate-limiting="$FIXED_RATE") ;;
      esac
      c0=$(cpu_s "$PID")
      taskset -c 6-7 memtier_benchmark -s 127.0.0.1 -p "$PORT" --protocol=redis -t 2 -c 12 \
        --hide-histogram --distinct-client-seed --key-minimum=1 --key-maximum=100000 -d 32 \
        --key-prefix=bg: --ratio=1:0 "${margs[@]}" --test-time="$TEST_TIME" \
        --json-out-file="$OUT/ws/$tag-memtier.json" >"$OUT/logs/memtier-$tag.log" 2>&1 || true
      c1=$(cpu_s "$PID")
      endrss=$(rss_kb "$PID")
      wait "$wpid" || true
      for c in "${cpids[@]}"; do wait "$c" || true; done
      kill -9 "$PID" 2>/dev/null || true; wait "$PID" 2>/dev/null || true; sleep 0.5
      python3 -I "$HERE/ws_row.py" "$OUT/ws" "$tag" "$name" "$workers" "$clients" "$cell" "$rep" \
        "$c0" "$c1" "$idle" "$withc" "$endrss" >>"$csv"
      tail -1 "$csv"
    done
   done
  done
 done
done
