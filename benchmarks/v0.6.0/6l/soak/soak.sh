#!/usr/bin/env bash
# Phase 6L-G: production soak, native Linux (no Docker). Builds on the 6J soak
# (../../6j/soak/soak.sh); see README.md for what is checked and how to read it.
#
# Usage:
#   bash soak.sh                         # 40 min, ~950 WebSocket clients
#   SOAK_SECS=3600 bash soak.sh          # 60 min
#   SOAK_SECS=120 WS_CLIENTS=40 ROUTE_CLIENTS=15 NARROW_CLIENTS=10 REPLICA_CLIENTS=10 bash soak.sh   # smoke
#
# Topology, all on 127.0.0.1, RECACHED_SECURITY=multitenant on both nodes
# (password on RESP, token-only WebSocket, origins, caps):
#   primary  AOF everysec, replication listener, key cap with volatile-lru,
#            capped-write reservations on (RESERVATIONS=off to compare)
#   replica  same caps (accounted, not enforced while following)
# Load: rate-limited bg:* reads/overwrites and ttl:* churn (EX 10) that keeps
# the key cap binding; the acknowledged-write ledger (pipelined INCR + DEDUP);
# a WATCH/MULTI/EXEC compare-and-set loop; WebSocket tenants on the primary
# (probe propagation, 5% reconnecting); 6J routing groups G (token renewal),
# H (QSUB churn), J (slow readers); scope-narrowing clients; clients on the
# replica's WebSocket port; full SCAN traversals on both nodes.
# Injected: BGSAVE every 60 s; replica SIGKILL + restart every 240 s; every
# 300 s the replica is SIGSTOPped while a burst overflows its queue and the
# backlog, so on SIGCONT the LIVE replica must full-resync (4001 to its
# WebSocket clients). At the end: quiesce, replica == primary, ledger on the
# replica, primary SIGKILL + restart from snapshot + AOF, ledger again,
# replica == primary again, verdict.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
J6=$(cd "$HERE/../../6j" && pwd)

SERVER_BIN=${SERVER_BIN:-$HOME/recached-6l-bench/bin/recached-server-6l}
OUT=${OUT:-$HOME/recached-6l-soak/results-$(date -u +%Y%m%dT%H%M%SZ)}
SOAK_SECS=${SOAK_SECS:-2400}
WS_CLIENTS=${WS_CLIENTS:-600}          # ws-sync tenants on the primary
ROUTE_CLIENTS=${ROUTE_CLIENTS:-150}    # split over G / H / J
NARROW_CLIENTS=${NARROW_CLIENTS:-100}
REPLICA_CLIENTS=${REPLICA_CLIENTS:-100}
BG_KEYS=${BG_KEYS:-200000}
STABLE_KEYS=${STABLE_KEYS:-20000}
TTL_ROOM=${TTL_ROOM:-3000}             # key-cap headroom for TTL keys; the TTL churn (EX 10) keeps far more live
RESERVATIONS=${RESERVATIONS:-on}
MAX_MEMORY=${MAX_MEMORY:-2gb}
# MEMORY_BINDING=1 (6N-0 run 5): the memory limit binds instead of the key
# cap — a large-value writer (BIG_RATE x BIG_SIZE per s) on top of the usual
# load, the key cap set out of reach, and INFO memory sampled at 20 Hz.
MEMORY_BINDING=${MEMORY_BINDING:-0}
if [[ $MEMORY_BINDING == 1 ]]; then MAX_MEMORY=${MAX_MEMORY_BINDING:-192mb}; fi
BIG_RATE=${BIG_RATE:-40}
export BIG_SIZE=${BIG_SIZE:-131072}
PRIMARY_WORKERS=${PRIMARY_WORKERS:-3}
REPLICA_WORKERS=${REPLICA_WORKERS:-1}
PRIMARY_CPUS=${PRIMARY_CPUS:-0-2}
REPLICA_CPUS=${REPLICA_CPUS:-3}
LOAD_CPUS=${LOAD_CPUS:-4-5}
STALL_EVERY=${STALL_EVERY:-300}
STALL_SECS=${STALL_SECS:-15}
RESTART_EVERY=${RESTART_EVERY:-240}
BURST_N=${BURST_N:-600}                # x 64 KiB during each stall: overflows a 1 MiB queue + 2 MiB backlog
export SOAK_PASSWORD=${SOAK_PASSWORD:-soak-6l-password-$(od -An -N8 -tx1 /dev/urandom | tr -d ' \n')}
SECRET=6l-soak-secret-0123456789abcdef0123456789abcdef
P=7700; PWS=7701; PMET=7709; PREPL=7710; R=7720; RWS=7721; RMET=7729
export BG_KEYS

# Every key without a TTL the run can create; the cap leaves TTL_ROOM above it.
NONTTL=$(( BG_KEYS + STABLE_KEYS + 400 + 2 * WS_CLIENTS + 3 * NARROW_CLIENTS + SOAK_SECS * 20 + 1000 ))
MAX_KEYS=$(( NONTTL + TTL_ROOM ))
[[ $MEMORY_BINDING == 1 ]] && MAX_KEYS=$(( NONTTL + 10000000 ))

command -v node >/dev/null || { echo "missing: node (22+)"; exit 1; }
[[ $(node -p 'process.versions.node.split(".")[0]') -ge 22 ]] || { echo "node 22+ required (global WebSocket)"; exit 1; }
command -v python3 >/dev/null || { echo "missing: python3"; exit 1; }
[[ -x $SERVER_BIN ]] || { echo "no server binary at $SERVER_BIN (run the 6L kit first, or set SERVER_BIN)"; exit 1; }
ulimit -n 65536 2>/dev/null || ulimit -n 10240 2>/dev/null || true

mkdir -p "$OUT/logs"
pdir=$OUT/data-primary; rdir=$OUT/data-replica; mkdir -p "$pdir" "$rdir"
PY() { python3 -I "$HERE/soak6l.py" "$@"; }
log() { echo "$(date +%T) $*" | tee -a "$OUT/soak.log"; }
fact() { echo "$1=$2" >>"$OUT/facts.txt"; }
# `$(TS cpus) cmd …` pins with taskset where it exists. A prefix, not a function,
# so that `cmd &` leaves the command itself in $! (signals reach it).
TS() { [[ -n $1 ]] && command -v taskset >/dev/null && echo "taskset -c $1"; }
cpu_s() {
  if [[ -r /proc/$1/stat ]]; then awk -v hz="$(getconf CLK_TCK)" '{printf "%.2f", ($14 + $15) / hz}' "/proc/$1/stat"
  else ps -o time= -p "$1" 2>/dev/null | awk -F: '{ s = 0; for (i = 1; i <= NF; i++) s = s * 60 + $i; printf "%.2f", s }'; fi
}
rss() { ps -o rss= -p "$1" 2>/dev/null | tr -d ' ' || echo 0; }
fsize() { stat -c %s "$1" 2>/dev/null || stat -f %z "$1" 2>/dev/null || echo 0; }
metric() { PY metric "http://127.0.0.1:$1/metrics" "$2"; }

common_env=(RECACHED_BIND=127.0.0.1 RECACHED_SECURITY=multitenant RECACHED_SYNC_SECRET=$SECRET
  RECACHED_ALLOWED_ORIGINS=https://soak.invalid RECACHED_PASSWORD=$SOAK_PASSWORD
  RECACHED_MAX_KEYS=$MAX_KEYS RECACHED_MAX_MEMORY=$MAX_MEMORY RECACHED_EVICTION=volatile-lru
  RECACHED_MAX_CONNECTIONS=20000 RECACHED_CAPPED_RESERVATIONS=$RESERVATIONS RECACHED_SAVE_INTERVAL=0)
start_primary() {
  env "${common_env[@]}" RECACHED_PORT=$P RECACHED_WS_PORT=$PWS RECACHED_METRICS_PORT=$PMET \
    RECACHED_SAVE_PATH=$pdir/dump.rdb RECACHED_AOF_PATH=$pdir/a.aof RECACHED_AOF_SYNC=everysec \
    RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=$PREPL RECACHED_REPL_BUFFER_BYTES=1mb RECACHED_REPL_BACKLOG_BYTES=2mb \
    RECACHED_WORKER_THREADS=$PRIMARY_WORKERS $(TS "$PRIMARY_CPUS") "$SERVER_BIN" >>"$OUT/logs/primary.log" 2>&1 &
  PPID_=$!
  PY ping $P 40 || { log "primary did not start"; tail -30 "$OUT/logs/primary.log"; exit 1; }
}
start_replica() {
  env "${common_env[@]}" RECACHED_PORT=$R RECACHED_WS_PORT=$RWS RECACHED_METRICS_PORT=$RMET \
    RECACHED_SAVE_PATH=$rdir/dump.rdb RECACHED_REPLICAOF=127.0.0.1:$PREPL \
    RECACHED_WORKER_THREADS=$REPLICA_WORKERS $(TS "$REPLICA_CPUS") "$SERVER_BIN" >>"$OUT/logs/replica.log" 2>&1 &
  RPID=$!
  PY ping $R 40 || log "replica did not answer PING"
}
# The replica reports DBSIZE >= what was preloaded once its first full sync has landed.
wait_replica_synced() {
  for _ in $(seq 240); do
    local n; n=$(PY cmd $R DBSIZE 2>/dev/null || echo 0)
    (( ${n:-0} >= BG_KEYS + STABLE_KEYS )) && return 0; sleep 0.5
  done; return 1
}

# ── conditions ────────────────────────────────────────────────────────────────
{
  echo "date: $(date -u +%FT%TZ)"; uname -a
  command -v lscpu >/dev/null && lscpu | grep -E 'Model name|^CPU\(s\)|Thread|MHz' || sysctl -n machdep.cpu.brand_string 2>/dev/null
  echo "governor: $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null || echo n/a)"
  echo "load: $(cat /proc/loadavg 2>/dev/null || uptime)"
  echo "flatpak: $([[ -e /.flatpak-info ]] && echo yes || echo no)"
  echo "node: $(node --version)  python: $(python3 --version 2>&1)"
  echo "server: $SERVER_BIN $("$SERVER_BIN" --version 2>&1 | head -1)"
  echo "server sha256: $( (sha256sum "$SERVER_BIN" 2>/dev/null || shasum -a 256 "$SERVER_BIN") | cut -d' ' -f1)"
  echo "settings: SOAK_SECS=$SOAK_SECS WS_CLIENTS=$WS_CLIENTS ROUTE_CLIENTS=$ROUTE_CLIENTS NARROW_CLIENTS=$NARROW_CLIENTS"
  echo "          REPLICA_CLIENTS=$REPLICA_CLIENTS BG_KEYS=$BG_KEYS STABLE_KEYS=$STABLE_KEYS MAX_KEYS=$MAX_KEYS"
  echo "          MAX_MEMORY=$MAX_MEMORY RESERVATIONS=$RESERVATIONS workers $PRIMARY_WORKERS/$REPLICA_WORKERS"
  echo "          MEMORY_BINDING=$MEMORY_BINDING BIG_RATE=$BIG_RATE BIG_SIZE=$BIG_SIZE"
  echo "          cpus primary=$PRIMARY_CPUS replica=$REPLICA_CPUS load=$LOAD_CPUS"
  echo "          stall every ${STALL_EVERY}s for ${STALL_SECS}s (burst $BURST_N x 64 KiB), replica restart every ${RESTART_EVERY}s"
} >"$OUT/conditions.txt" 2>&1
: >"$OUT/facts.txt"
fact primary_workers "$PRIMARY_WORKERS"
fact max_keys "$MAX_KEYS"
fact memory_binding "$MEMORY_BINDING"

start_primary
PY preload $P "$BG_KEYS" "$STABLE_KEYS" | tee -a "$OUT/soak.log"
start_replica
wait_replica_synced || log "replica did not finish its first sync in 120 s"
log "start: $SOAK_SECS s; WebSocket clients $WS_CLIENTS + $ROUTE_CLIENTS + $NARROW_CLIENTS (primary) + $REPLICA_CLIENTS (replica); key cap $MAX_KEYS"
echo "t_s,primary_cpu_s,primary_rss_kb,replica_rss_kb,primary_dbsize,replica_dbsize,aof_bytes,primary_evicted,event" >"$OUT/samples.csv"

# ── load ──────────────────────────────────────────────────────────────────────
D=$SOAK_SECS
$(TS "$LOAD_CPUS") node "$HERE/ws-sync.mjs" clients "ws://127.0.0.1:$PWS" "$SECRET" 0 "$WS_CLIENTS" $((D + 10)) \
  "$OUT/ws-clients.json" >"$OUT/logs/ws-clients.log" 2>&1 & WSC=$!
for _ in $(seq 480); do grep -q READY "$OUT/logs/ws-clients.log" 2>/dev/null && break; sleep 0.25; done
RG=(); for g in G H J; do
  SCENARIO=$g CHURN_MS=${CHURN_MS:-5000} $(TS "$LOAD_CPUS") node "$J6/ws-route.mjs" clients "ws://127.0.0.1:$PWS" "$SECRET" \
    $(( 100000 + $(printf '%d' "'$g") * 1000 )) $(( ROUTE_CLIENTS / 3 )) "$ROUTE_CLIENTS" $((D + 10)) "$OUT/route-$g.json" \
    >"$OUT/logs/route-$g.log" 2>&1 & RG+=($!)
done
$(TS "$LOAD_CPUS") node "$HERE/ws-6l.mjs" narrow "ws://127.0.0.1:$PWS" $P "$SECRET" 0 "$NARROW_CLIENTS" "$D" "$OUT/narrow.json" \
  >"$OUT/logs/narrow.log" 2>&1 & NAR=$!
$(TS "$LOAD_CPUS") node "$HERE/ws-6l.mjs" replica "ws://127.0.0.1:$RWS" "$SECRET" 0 "$REPLICA_CLIENTS" "$OUT/replica-ws.json" \
  >"$OUT/logs/replica-ws.log" 2>&1 & RWC=$!
$(TS "$LOAD_CPUS") node "$HERE/ws-sync.mjs" writer $P 0 "$WS_CLIENTS" 100 "$D" "$OUT/ws-writer.json" \
  >"$OUT/logs/ws-writer.log" 2>&1 & WSW=$!
$(TS "$LOAD_CPUS") python3 -I "$HERE/soak6l.py" load bg $P 12 300 "$D" "$OUT/load-bg.json" >"$OUT/logs/load-bg.log" 2>&1 & LBG=$!
$(TS "$LOAD_CPUS") python3 -I "$HERE/soak6l.py" load ttl $P 4 1000 "$D" "$OUT/load-ttl.json" >"$OUT/logs/load-ttl.log" 2>&1 & LTT=$!
$(TS "$LOAD_CPUS") python3 -I "$HERE/ledger.py" write $P 8 "$D" "$OUT/ledger.json" >"$OUT/logs/ledger.log" 2>&1 & LED=$!
$(TS "$LOAD_CPUS") python3 -I "$HERE/ledger.py" cas $P "$D" "$OUT/cas.json" >"$OUT/logs/cas.log" 2>&1 & CAS=$!
$(TS "$LOAD_CPUS") python3 -I "$HERE/repl_check.py" probe $P $R $((D * 20)) >"$OUT/repl-probe.txt" 2>&1 & PRB=$!
$(TS "$LOAD_CPUS") python3 -I "$HERE/soak6l.py" scan $P "$D" "$BG_KEYS" "$STABLE_KEYS" "$OUT/scan-primary.json" \
  >"$OUT/logs/scan-primary.log" 2>&1 & SCP=$!
$(TS "$LOAD_CPUS") python3 -I "$HERE/soak6l.py" scan $R "$D" "$BG_KEYS" "$STABLE_KEYS" "$OUT/scan-replica.json" \
  "http://127.0.0.1:$RMET/metrics" >"$OUT/logs/scan-replica.log" 2>&1 & SCR=$!
LOADERS=($WSW $LBG $LTT $LED $CAS $PRB $SCP $SCR $NAR $WSC "${RG[@]}")
if [[ $MEMORY_BINDING == 1 ]]; then
  $(TS "$LOAD_CPUS") python3 -I "$HERE/soak6l.py" load big $P 4 $(( BIG_RATE / 4 )) "$D" "$OUT/load-big.json" \
    >"$OUT/logs/load-big.log" 2>&1 & LOADERS+=($!)
  $(TS "$LOAD_CPUS") python3 -I "$HERE/soak6l.py" memwatch $P "$D" "$OUT/memwatch.json" \
    >"$OUT/logs/memwatch.log" 2>&1 & LOADERS+=($!)
fi

# ── run, with failure injection ───────────────────────────────────────────────
stalls=0; stall_resyncs=0; last_stall=-1000; restarts=0
t0=$(date +%s)
while (( $(date +%s) - t0 < SOAK_SECS )); do
  sleep 5
  t=$(( $(date +%s) - t0 )); ev=""
  # No snapshot in the last 20 s: the crash at the end must leave writes only the AOF holds.
  if (( t % 60 < 5 && t < SOAK_SECS - 20 )); then PY cmd $P BGSAVE >/dev/null 2>&1; ev="bgsave"; fi
  if (( t >= STALL_EVERY / 2 && (t - STALL_EVERY / 2) % STALL_EVERY < 5 && t < SOAK_SECS - 60 )); then
    before=$(metric $RMET recached_replica_full_resync_total)
    kill -STOP "$RPID"
    PY burst $P "$BURST_N" 65536 20 >>"$OUT/logs/burst.log" 2>&1
    sleep "$STALL_SECS"
    kill -CONT "$RPID"
    stalls=$((stalls + 1)); last_stall=$t; got=no
    for _ in $(seq 60); do
      now=$(metric $RMET recached_replica_full_resync_total)
      if [[ -n $now && -n $before ]] && awk -v a="$now" -v b="$before" 'BEGIN { exit !(a > b) }'; then got=yes; break; fi
      sleep 1
    done
    [[ $got == yes ]] && stall_resyncs=$((stall_resyncs + 1))
    log "stall $stalls at t=$t: replica stopped ${STALL_SECS}s under a ${BURST_N} x 64 KiB burst; full resync of the live replica: $got"
    ev="$ev stall:$got"
  elif (( t % RESTART_EVERY < 5 && t > 10 && t - last_stall > 60 )); then
    kill -9 "$RPID" 2>/dev/null; wait "$RPID" 2>/dev/null; start_replica; restarts=$((restarts + 1)); ev="$ev replica-restart"
  fi
  t=$(( $(date +%s) - t0 ))
  echo "$t,$(cpu_s $PPID_),$(rss $PPID_),$(rss $RPID),$(PY cmd $P DBSIZE 2>/dev/null),$(PY cmd $R DBSIZE 2>/dev/null),$(fsize "$pdir/a.aof"),$(PY info $P evicted_keys),$ev" >>"$OUT/samples.csv"
done
log "load phase over: $stalls stalls ($stall_resyncs forced a full resync), $restarts replica restarts"
fact stalls "$stalls"; fact replica_full_resyncs_during_stalls "$stall_resyncs"; fact replica_restarts "$restarts"
for p in "${LOADERS[@]}"; do wait "$p" 2>/dev/null; done
log "load stopped; ledger: $(cat "$OUT/logs/ledger.log"); cas: $(cat "$OUT/logs/cas.log")"

# ── quiesced checks ───────────────────────────────────────────────────────────
sleep 25   # ttl:* (EX 10) and stall:* (EX 20) keys expire
c=$(python3 -I "$HERE/repl_check.py" converge $P $R 1 120); log "replica converge: $c"
PY compare $P $R "$OUT/compare-quiesced.json" | tee -a "$OUT/soak.log"
python3 -I "$HERE/ledger.py" verify $R "$OUT/ledger.json" >"$OUT/ledger-replica.json" 2>&1
log "ledger on replica: $(cat "$OUT/ledger-replica.json")"
fact primary_evicted_keys "$(PY info $P evicted_keys)"
fact oom_refusals "$(python3 -c 'import json,sys
t=0
for f in sys.argv[1:]:
    try: t+=json.load(open(f))["errors"].get("OOM",0)
    except (OSError,ValueError): pass
print(t)' "$OUT/load-bg.json" "$OUT/load-ttl.json" "$OUT/load-big.json")"
fact replica_evicted_keys "$(PY info $R evicted_keys)"
fact max_primary_dbsize "$(awk -F, 'NR > 1 && $5 ~ /^[0-9]+$/ && $5 > m { m = $5 } END { print m + 0 }' "$OUT/samples.csv")"
fact replica_full_resyncs_total_before_crash "$(metric $RMET recached_replica_full_resync_total)"
metric $PMET recached_replication_syncs_total >/dev/null 2>&1 && curl -s "http://127.0.0.1:$PMET/metrics" 2>/dev/null \
  | grep -E '^recached_(replication_(syncs|disconnects)_total|replicas_connected|replica_full_resync)' >"$OUT/primary-repl-metrics.txt"

# ── crash recovery ────────────────────────────────────────────────────────────
log "crashing the primary: aof $(fsize "$pdir/a.aof") bytes"
rf_before=$(metric $RMET recached_replica_full_resync_total)
kill -9 "$PPID_"; wait "$PPID_" 2>/dev/null
rs=$(python3 -c 'import time; print(time.monotonic_ns())'); start_primary; re=$(python3 -c 'import time; print(time.monotonic_ns())')
fact recovery_ms $(( (re - rs) / 1000000 ))
log "primary restarted from snapshot + AOF in $(( (re - rs) / 1000000 )) ms"
python3 -I "$HERE/ledger.py" verify $P "$OUT/ledger.json" >"$OUT/ledger-recovered.json" 2>&1
log "ledger after crash recovery: $(cat "$OUT/ledger-recovered.json")"
fact used_memory_after_recovery "$(PY info $P used_memory)"
fact maxmemory_after_recovery "$(PY info $P maxmemory)"
sleep 10
c=$(python3 -I "$HERE/repl_check.py" converge $P $R 1 120); log "replica converge after the primary's restart: $c"
PY compare $P $R "$OUT/compare-recovered.json" | tee -a "$OUT/soak.log"
fact replica_full_resyncs_after_crash "$(awk -v a="$(metric $RMET recached_replica_full_resync_total)" -v b="$rf_before" 'BEGIN { print a - b }')"

# ── wrap up ───────────────────────────────────────────────────────────────────
kill -TERM "$RWC" 2>/dev/null; wait "$RWC" 2>/dev/null
kill -9 "$PPID_" "$RPID" 2>/dev/null; wait 2>/dev/null
fact panics "$(cat "$OUT/logs/primary.log" "$OUT/logs/replica.log" | grep -c 'panicked at')"
for n in primary replica; do
  grep -E "WARN|ERROR" "$OUT/logs/$n.log" | sed 's/\x1b\[[0-9;]*m//g' | cut -c28- | sort | uniq -c | sort -rn | head -25 >"$OUT/$n-warnings.txt"
done
lag=$(python3 -I "$HERE/repl_check.py" summarize "$OUT/repl-probe.txt"); fact replica_lag "$lag"
fact probe_timeouts "$(grep -cE ' (timeout|down)$' "$OUT/repl-probe.txt")"
awk -F, -v half=$((SOAK_SECS / 2)) 'NR > 1 && $3 ~ /^[0-9]+$/ { if ($1 <= half) mid = $3; end = $3 }
  END { printf "rss_mid_kb=%d\nrss_end_kb=%d\nrss_second_half_growth_pct=%.1f\n", mid, end, mid ? (end - mid) * 100 / mid : 0 }' \
  "$OUT/samples.csv" >>"$OUT/facts.txt"
log "verdict:"
PY verdict "$OUT" "$MAX_KEYS" | tee "$OUT/verdict.txt"; vst=${PIPESTATUS[0]}
rm -rf "$pdir" "$rdir"
tar -C "$(dirname "$OUT")" -czf "$OUT.tar.gz" "$(basename "$OUT")"
log "results: $OUT.tar.gz (verdict exit $vst)"
exit "$vst"
