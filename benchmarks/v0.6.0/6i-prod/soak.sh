#!/usr/bin/env bash
# Phase 6I-G: sustained production topology with failure injection. Runs
# INSIDE recached-6i-bench:local (SCRIPT=soak.sh run.sh <out> SERVER=<label>).
#
# Topology: one primary (AOF everysec, replication listener, strict sync
# scoping with tokens, 4 workers), one replica (2 workers), WS_CLIENTS
# token-scoped WebSocket clients (one tenant each, every 4th WATCHing, 5%
# reconnecting every 5 s), a probe writer (in-scope SETs + DEDUP retries),
# background memtier load (plain and 2 s-TTL keys, fixed rate), the
# acknowledged-write ledger (ledger.py write: pipelined INCR + DEDUP retries)
# and a WATCH/MULTI/EXEC compare-and-set loop racing plain INCRs.
#
# Injected during the run: BGSAVE every 30 s (snapshot + AOF checkpoint under
# load), the replica SIGKILLed and restarted every 45 s (replica attach under
# load), active expiry throughout (the TTL keys).
# At the end: load stops; replica convergence (DBSIZE + the ledger on the
# replica); then the primary is SIGKILLed and restarted from its snapshot +
# AOF, and the ledger is verified against it: no acknowledged write lost.
set -uo pipefail
BINS=${BINS:-/bins}; OUT=${OUT:-/out}; HERE=$(cd "$(dirname "$0")" && pwd)
SERVER=${SERVER:-6i-rc3}; SOAK_SECS=${SOAK_SECS:-600}; WS_CLIENTS=${WS_CLIENTS:-100}
SECRET=6i-soak-secret-0123456789abcdef0123456789abcdef
P=7700; PWS=7701; PREPL=7710; R=7720; RWS=7721
mkdir -p "$OUT/logs"
pdir=/tmp/soak-primary; rdir=/tmp/soak-replica; rm -rf $pdir $rdir; mkdir -p $pdir $rdir
log() { echo "$(date +%T) $*" | tee -a "$OUT/soak.log"; }
cpu_s() { [[ -r /proc/$1/stat ]] && awk -v hz="$(getconf CLK_TCK)" '{printf "%.2f", ($14 + $15) / hz}' "/proc/$1/stat" || echo 0; }
rss() { awk '/^VmRSS:/ {print $2}' "/proc/$1/status" 2>/dev/null || echo 0; }
pong() { for _ in $(seq 160); do redis-cli -p "$1" ping 2>/dev/null | grep -q PONG && return 0; sleep 0.25; done; return 1; }

start_primary() {
  env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$PWS RECACHED_METRICS_PORT=0 \
    RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH=$pdir/dump.rdb RECACHED_AOF_PATH=$pdir/a.aof \
    RECACHED_AOF_SYNC=everysec RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=$PREPL \
    RECACHED_SYNC_SECRET=$SECRET RECACHED_WS_AUTH=token RECACHED_MAX_CONNECTIONS=20000 \
    RECACHED_WORKER_THREADS=4 taskset -c 0-3 "$BINS/recached-server-$SERVER" >>"$OUT/logs/primary.log" 2>&1 &
  PPID_=$!; pong $P || { log "primary did not start"; tail -20 "$OUT/logs/primary.log"; exit 1; }
}
start_replica() {
  env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$R RECACHED_WS_PORT=$RWS RECACHED_METRICS_PORT=0 \
    RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH=$rdir/dump.rdb RECACHED_REPLICAOF=127.0.0.1:$PREPL \
    RECACHED_WORKER_THREADS=2 taskset -c 4-5 "$BINS/recached-server-$SERVER" >>"$OUT/logs/replica.log" 2>&1 &
  RPID=$!; pong $R || log "replica did not answer PING"
}

start_primary; start_replica
log "start: server $SERVER, ${SOAK_SECS}s, $WS_CLIENTS WebSocket clients"
echo "t_s,primary_cpu_s,primary_rss_kb,replica_rss_kb,primary_dbsize,replica_dbsize,aof_bytes,event" >"$OUT/samples.csv"

# Load.
taskset -c 6-7 node "$HERE/ws-sync.mjs" clients "ws://127.0.0.1:$PWS" "$SECRET" 0 "$WS_CLIENTS" $((SOAK_SECS + 10)) \
  "$OUT/ws-clients.json" >"$OUT/logs/ws-clients.log" 2>&1 & WSC=$!
for _ in $(seq 240); do grep -q READY "$OUT/logs/ws-clients.log" 2>/dev/null && break; sleep 0.25; done
taskset -c 6-7 node "$HERE/ws-sync.mjs" writer $P 0 "$WS_CLIENTS" 100 "$SOAK_SECS" "$OUT/ws-writer.json" \
  >"$OUT/logs/ws-writer.log" 2>&1 & WSW=$!
taskset -c 6-7 memtier_benchmark -s 127.0.0.1 -p $P --protocol=redis -t 2 -c 6 --hide-histogram \
  --key-minimum=1 --key-maximum=200000 -d 64 --key-prefix=bg: --ratio=1:4 --pipeline=1 \
  --rate-limiting=300 --test-time="$SOAK_SECS" --json-out-file="$OUT/memtier-bg.json" >"$OUT/logs/memtier-bg.log" 2>&1 & MT1=$!
taskset -c 6-7 memtier_benchmark -s 127.0.0.1 -p $P --protocol=redis -t 1 -c 4 --hide-histogram \
  --key-minimum=1 --key-maximum=5000000 -d 32 --key-prefix=ttl: --command="set __key__ __data__ EX 2" \
  --command-key-pattern=R --pipeline=4 --rate-limiting=250 --test-time="$SOAK_SECS" \
  --json-out-file="$OUT/memtier-ttl.json" >"$OUT/logs/memtier-ttl.log" 2>&1 & MT2=$!
taskset -c 6-7 python3 -I "$HERE/ledger.py" write $P 8 "$SOAK_SECS" "$OUT/ledger.json" >"$OUT/logs/ledger.log" 2>&1 & LED=$!
taskset -c 6-7 python3 -I "$HERE/ledger.py" cas $P "$SOAK_SECS" "$OUT/cas.json" >"$OUT/logs/cas.log" 2>&1 & CAS=$!
taskset -c 6-7 python3 -I "$HERE/repl_check.py" probe $P $R $((SOAK_SECS * 5)) >"$OUT/repl-probe.txt" 2>&1 & PRB=$!

t0=$(date +%s)
while (( $(date +%s) - t0 < SOAK_SECS )); do
  sleep 5
  t=$(( $(date +%s) - t0 )); ev=""
  if (( t % 30 < 5 )); then
    redis-cli -p $P bgsave >/dev/null 2>&1; ev="bgsave"
  fi
  if (( t % 45 < 5 && t > 10 )); then
    kill -9 "$RPID" 2>/dev/null; wait "$RPID" 2>/dev/null; start_replica; ev="$ev replica-restart"
  fi
  echo "$t,$(cpu_s $PPID_),$(rss $PPID_),$(rss $RPID),$(redis-cli -p $P dbsize | tr -d '\r'),$(redis-cli -p $R dbsize 2>/dev/null | tr -d '\r'),$(stat -c %s $pdir/a.aof 2>/dev/null),$ev" >>"$OUT/samples.csv"
done
log "load phase over"
for p in $MT1 $MT2 $LED $CAS $WSW $PRB; do wait "$p" 2>/dev/null; done
wait "$WSC" 2>/dev/null
log "load stopped; ledger: $(cat "$OUT/logs/ledger.log"); cas: $(cat "$OUT/logs/cas.log")"

# Replica convergence: the same DBSIZE (TTL keys excepted, both expire them),
# and the ledger holds on the replica too.
c=$(python3 -I "$HERE/repl_check.py" converge $P $R 1 120); log "replica converge: $c"
python3 -I "$HERE/ledger.py" verify $R "$OUT/ledger.json" >"$OUT/ledger-replica.json" 2>&1; rv=$?
log "ledger on replica: $(cat "$OUT/ledger-replica.json") (exit $rv)"

# Crash recovery: SIGKILL the primary, restart it from snapshot + AOF.
kill -9 "$PPID_"; wait "$PPID_" 2>/dev/null
rs=$(date +%s%N); start_primary; re=$(date +%s%N)
log "primary restarted from AOF in $(( (re - rs) / 1000000 )) ms; aof $(stat -c %s $pdir/a.aof) bytes"
python3 -I "$HERE/ledger.py" verify $P "$OUT/ledger.json" >"$OUT/ledger-recovered.json" 2>&1; st=$?
log "ledger after crash recovery: $(cat "$OUT/ledger-recovered.json") (exit $st)"
kill -9 "$PPID_" "$RPID" 2>/dev/null
grep -E "WARN|ERROR" "$OUT/logs/primary.log" | sed 's/\x1b\[[0-9;]*m//g' | cut -c28- | sort | uniq -c | sort -rn | head -20 >"$OUT/primary-warnings.txt"
log "done"
