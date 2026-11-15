#!/usr/bin/env bash
# Phase 6I-B: production-path RESP benchmarks. Runs INSIDE recached-6i-bench:local
# (recached-6g-bench:local + Node 22) via run.sh. memtier_benchmark drives.
#
# Configurations (CONFIGS):
#   standalone          no persistence, no replica, metrics exporter OFF
#                       (RECACHED_METRICS_PORT=0) — the 6G/6H method
#   standalone-metrics  as above with Recached's DEFAULT exporter ON and a
#                       scraper every 5 s. Redis/Valkey are not re-run (no
#                       exporter; their INFO commandstats are always on).
#   aof-everysec        AOF, fsync once per second, on every engine
#   aof-always          AOF, fsync per write (see "Durability" below)
#   repl                one primary + one connected replica, continuous writes;
#                       replica lag measured by probe keys, convergence checked
#   capped              a memory cap that is never reached (1 GB, allkeys-lru):
#                       the cost of *having* a cap, which nearly every cache and
#                       every RECACHED_SECURITY=multitenant deployment has
#
# Durability is NOT identical across engines and must not be claimed so:
#   - Redis/Valkey `appendfsync always` fsync once per event-loop iteration
#     before replying (group commit); Recached `always` fsyncs per write under
#     its AOF lock (docs/server/configuration.md). Both acknowledge only after
#     fsync, but the batching differs.
#   - Redis/Valkey auto AOF rewrite is disabled here (auto-aof-rewrite-percentage 0)
#     so no engine forks mid-cell; Recached has no AOF rewrite (it truncates on
#     snapshot, and autosave is off here).
#   - The disk is the Docker Desktop VM's virtual disk: fsync cost is NOT that of
#     a production SSD. AOF numbers are indicative only.
#
# CPU layout (8 vCPU VM): standalone/aof server on SERVER_CPUS (0-3), memtier on
# LOAD_CPUS (4-7). repl: primary on 0-1, replica on 2-3, memtier on 4-7 —
# single-worker only (Redis/Valkey replicas are single-threaded too).
#
# Output: results.csv (one row per server × config × workers × cell × rep),
# raw memtier JSON in json/, probe/convergence detail in repl/.
set -euo pipefail

BINS=${BINS:-/bins}
OUT=${OUT:-/out}
HERE=$(cd "$(dirname "$0")" && pwd)
SERVERS=${SERVERS:-"6h redis valkey"}
CONFIGS=${CONFIGS:-"standalone standalone-metrics aof-everysec aof-always repl capped"}
WORKER_SET=${WORKER_SET:-"1 4"}
REPS=${REPS:-1}
TEST_TIME=${TEST_TIME:-10}
SERVER_CPUS=${SERVER_CPUS:-0-3}
LOAD_CPUS=${LOAD_CPUS:-4-7}
THREADS=${THREADS:-4}
CLIENTS=${CLIENTS:-12}           # per memtier thread: 48 connections
KEYS=${KEYS:-100000}
SIZE=${SIZE:-32}
FIXED_RATE=${FIXED_RATE:-500}    # per connection in the *-fixed cells (×48 = 24k/s)
PORT=${PORT:-7700}
RPORT=$((PORT + 20))             # replica RESP port
REPL_PORT=$((PORT + 10))         # Recached replication listener

cells_for() {
  case $1 in
    standalone)         echo "${CELLS_STANDALONE:-set-p16 get-p16 incr-p16 hset-p16 hget-p16 lat-fixed wlat-fixed}" ;;
    standalone-metrics) echo "${CELLS_METRICS:-set-p16 get-p16 incr-p16 hset-p16 lat-fixed}" ;;
    aof-*)              echo "${CELLS_AOF:-set-p16 incr-p16 hset-p16 wlat-fixed}" ;;
    capped)             echo "${CELLS_CAPPED:-set-p16 get-p16 incr-p16 hset-p16 wlat-fixed}" ;;
    repl)               echo "${CELLS_REPL:-set-p16 incr-p16 hset-p16 wlat-fixed}" ;;
  esac
}

mkdir -p "$OUT/json" "$OUT/repl" "$OUT/logs"
csv=$OUT/results.csv
[[ -s $csv ]] || echo "server,config,workers,cell,rep,ops_per_sec,avg_ms,p50_ms,p99_ms,p999_ms,server_cpu_s,cpu_s_per_mops,replica_cpu_s,lag_p50_ms,lag_p99_ms,lag_max_ms,converge_ms,converged,errors" >"$csv"

cli()  { redis-cli -p "$PORT" "$@"; }
rcli() { redis-cli -p "$RPORT" "$@"; }
cpu_s() { [[ -n ${1:-} && -r /proc/$1/stat ]] || { echo 0; return; }
          awk -v hz="$(getconf CLK_TCK)" '{printf "%.3f", ($14 + $15) / hz}' "/proc/$1/stat"; }
wait_pong() { local p=$1 log=$2
  for _ in $(seq 120); do redis-cli -p "$p" ping 2>/dev/null | grep -q PONG && return 0; sleep 0.25; done
  echo "server on $p did not start" >&2; tail -20 "$log" >&2; exit 1; }

PID=""; RPID=""; SCRAPER=""

# start <name> <config> <workers> <role: primary|replica> <cpus>
launch() {
  local name=$1 config=$2 workers=$3 role=$4 cpus=$5
  local port=$PORT dir=/tmp/data-$name-$role log
  [[ $role == replica ]] && port=$RPORT
  log=$OUT/logs/$name-$config-$workers-$role.log
  rm -rf "$dir"; mkdir -p "$dir"
  local -a rargs=() renv=()
  case $config in
    aof-everysec) rargs=(--appendonly yes --appendfsync everysec --auto-aof-rewrite-percentage 0)
                  renv=(RECACHED_AOF_PATH="$dir/appendonly.aof" RECACHED_AOF_SYNC=everysec) ;;
    aof-always)   rargs=(--appendonly yes --appendfsync always --auto-aof-rewrite-percentage 0)
                  renv=(RECACHED_AOF_PATH="$dir/appendonly.aof" RECACHED_AOF_SYNC=always) ;;
    capped)       rargs=(--appendonly no --maxmemory 1gb --maxmemory-policy allkeys-lru)
                  renv=(RECACHED_MAX_MEMORY=1gb RECACHED_EVICTION=allkeys-lru) ;;
    *)            rargs=(--appendonly no) ;;
  esac
  if [[ $config == repl ]]; then
    if [[ $role == primary ]]; then renv+=(RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT="$REPL_PORT")
    else renv+=(RECACHED_REPLICAOF="127.0.0.1:$REPL_PORT"); rargs+=(--replicaof 127.0.0.1 "$PORT"); fi
  fi
  local metrics=0
  [[ $config == standalone-metrics && $role == primary ]] && metrics=$((PORT + 2))
  case $name in
    redis)  taskset -c "$cpus" redis-server --port "$port" --bind 127.0.0.1 --save "" --dir "$dir" \
              --protected-mode no "${rargs[@]}" >"$log" 2>&1 & ;;
    valkey) taskset -c "$cpus" valkey-server --port "$port" --bind 127.0.0.1 --save "" --dir "$dir" \
              --protected-mode no --io-threads "$workers" "${rargs[@]}" >"$log" 2>&1 & ;;
    *) env RECACHED_BIND=127.0.0.1 RECACHED_PORT="$port" RECACHED_WS_PORT=$((port + 1)) \
         RECACHED_METRICS_PORT="$metrics" RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$dir/dump.rdb" \
         RECACHED_WORKER_THREADS="$workers" "${renv[@]}" \
         taskset -c "$cpus" "$BINS/recached-server-$name" >"$log" 2>&1 & ;;
  esac
  local pid=$!
  wait_pong "$port" "$log"
  if [[ $name != redis && $name != valkey ]]; then
    local got; got=$(sed -n 's/.*Command execution: \([0-9]*\) worker threads.*/\1/p' "$log" | head -1)
    [[ $got == "$workers" ]] || { echo "$name: asked for $workers workers, got '${got:-?}'" >&2; exit 1; }
  fi
  if [[ $role == primary ]]; then PID=$pid; else RPID=$pid; fi
}

start() {
  local name=$1 config=$2 workers=$3
  if [[ $config == repl ]]; then
    launch "$name" "$config" "$workers" primary 0-1
    launch "$name" "$config" "$workers" replica 2-3
  else
    launch "$name" "$config" "$workers" primary "$SERVER_CPUS"
  fi
  if [[ $config == standalone-metrics && $name != redis && $name != valkey ]]; then
    # A Prometheus scrape every 5 s, as a typical deployment would have.
    ( while sleep 5; do
        s=$(date +%s%N)
        b=$(curl -s "http://127.0.0.1:$((PORT + 2))/metrics" | wc -c)
        echo "$(date +%s) bytes=$b scrape_ms=$(( ($(date +%s%N) - s) / 1000000 ))"
      done ) >>"$OUT/logs/scrape-$name-$workers.log" 2>&1 &
    SCRAPER=$!
  fi
}

stop() {
  [[ -n $SCRAPER ]] && { kill "$SCRAPER" 2>/dev/null || true; wait "$SCRAPER" 2>/dev/null || true; SCRAPER=""; }
  for p in $RPID $PID; do kill -9 "$p" 2>/dev/null || true; wait "$p" 2>/dev/null || true; done
  PID=""; RPID=""; sleep 0.5
}

mt() {
  taskset -c "$LOAD_CPUS" memtier_benchmark -s 127.0.0.1 -p "$PORT" --protocol=redis \
    -t "$THREADS" -c "$CLIENTS" --hide-histogram --distinct-client-seed "$@"
}

cell_args() {
  local k="--key-minimum=1 --key-maximum=$KEYS -d $SIZE --key-prefix=k:"
  case $1 in
    set-p16)    echo "$k --ratio=1:0 --pipeline=16" ;;
    get-p16)    echo "$k --ratio=0:1 --pipeline=16" ;;
    lat-fixed)  echo "$k --ratio=1:10 --pipeline=1 --rate-limiting=$FIXED_RATE" ;;
    wlat-fixed) echo "$k --ratio=1:0 --pipeline=1 --rate-limiting=$FIXED_RATE" ;;
    # Existing counters (prefilled): the numeric update without key creation.
    incr-p16)   echo "--pipeline=16 --key-minimum=1 --key-maximum=$KEYS --key-prefix=c: --command=incr" ;;
    hset-p16)   echo "--pipeline=16 -d $SIZE --key-minimum=1 --key-maximum=$KEYS --key-prefix=h: --command=hset" ;;
    hget-p16)   echo "--pipeline=16 --key-minimum=1 --key-maximum=$KEYS --key-prefix=h: --command=hget" ;;
    # 6I-F: INCR split by what it costs. New keys: a 50M-key range nobody
    # prefilled, so nearly every INCR creates its key. Hot: one counter.
    # TTL: prefilled counters that carry an expiry.
    incr-new-p16) echo "--pipeline=16 --key-minimum=1 --key-maximum=50000000 --key-prefix=n: --command=incr" ;;
    incr-hot-p16) echo "--pipeline=16 --command=incr_hot" ;;
    incr-ttl-p16) echo "--pipeline=16 --key-minimum=1 --key-maximum=$KEYS --key-prefix=v: --command=incr" ;;
  esac
}

command_for() {
  case $1 in
    incr) echo "incr __key__" ;;
    incr_hot) echo "incr hot:counter" ;;
    hset) echo "hset __key__ f1 __data__" ;;
    hget) echo "hget __key__ f1" ;;
  esac
}

prefill() {
  local per=$(( (KEYS + THREADS * CLIENTS - 1) / (THREADS * CLIENTS) ))
  mt --key-minimum=1 --key-maximum="$KEYS" -d "$SIZE" --key-prefix=k: --ratio=1:0 \
     --key-pattern=P:P --requests="$per" --pipeline=64 >"$OUT/logs/prefill.log" 2>&1
  mt --key-minimum=1 --key-maximum="$KEYS" -d "$SIZE" --key-prefix=h: --requests="$per" --pipeline=64 \
     --command="hset __key__ f1 __data__" --command-key-pattern=P >>"$OUT/logs/prefill.log" 2>&1
  mt --key-minimum=1 --key-maximum="$KEYS" --key-prefix=c: --requests="$per" --pipeline=64 \
     --command="set __key__ 0" --command-key-pattern=P >>"$OUT/logs/prefill.log" 2>&1
  if [[ ${PREFILL_TTL:-0} == 1 ]]; then
    mt --key-minimum=1 --key-maximum="$KEYS" --key-prefix=v: --requests="$per" --pipeline=64 \
       --command="set __key__ 0 EX 86400" --command-key-pattern=P >>"$OUT/logs/prefill.log" 2>&1
  fi
  local n; n=$(cli dbsize | tr -d '\r')
  (( n >= 3 * KEYS * 9 / 10 )) || { echo "prefill loaded only $n keys" >&2; exit 1; }
}

# Wait for the replica to hold what the primary holds: same DBSIZE and the
# same value for SAMPLE random keys of each prefix. Prints "<ms> <ok>".
converge() {
  python3 -I "$HERE/repl_check.py" converge "$PORT" "$RPORT" "$KEYS" "${1:-30}"
}

run_cell() {
  local name=$1 config=$2 workers=$3 cell=$4 rep=$5
  local tag="$name-$config-w$workers-$cell-$rep"
  local json="$OUT/json/$tag.json"
  local -a args; read -r -a args <<<"$(cell_args "$cell")"
  local -a final=()
  for a in "${args[@]}"; do
    if [[ $a == --command=* ]]; then
      final+=("--command=$(command_for "${a#--command=}")" "--command-key-pattern=R")
    else final+=("$a"); fi
  done
  local probe=""
  if [[ $config == repl ]]; then
    # Probe writes go to the primary at 20/s for the whole cell; each is timed
    # until it is readable on the replica.
    taskset -c "$LOAD_CPUS" python3 -I "$HERE/repl_check.py" probe "$PORT" "$RPORT" \
      "$((TEST_TIME * 20))" >"$OUT/repl/$tag.probe" 2>&1 &
    probe=$!
  fi
  local c0 c1 r0 r1
  c0=$(cpu_s "$PID"); r0=$(cpu_s "$RPID")
  mt "${final[@]}" --test-time="$TEST_TIME" --json-out-file="$json" >"$OUT/json/$tag.txt" 2>&1 \
    || { echo "memtier failed: $tag"; tail -5 "$OUT/json/$tag.txt"; }
  c1=$(cpu_s "$PID"); r1=$(cpu_s "$RPID")
  local lag="" conv=""
  if [[ -n $probe ]]; then
    wait "$probe" || true
    lag=$(python3 -I "$HERE/repl_check.py" summarize "$OUT/repl/$tag.probe")
    conv=$(converge 60)
    echo "$conv" >"$OUT/repl/$tag.converge"
    r1=$(cpu_s "$RPID")
  fi
  local errors; errors=$(grep -c 'error response' "$OUT/json/$tag.txt" || true)
  python3 -I - "$json" "$name" "$config" "$workers" "$cell" "$rep" "$c0" "$c1" "$r0" "$r1" \
      "$errors" "$lag" "$conv" >>"$csv" <<'PY'
import json, sys
(path, name, config, workers, cell, rep, c0, c1, r0, r1, errors, lag, conv) = sys.argv[1:14]
try:
    t = json.load(open(path))["ALL STATS"]["Totals"]
except Exception:
    print(f"{name},{config},{workers},{cell},{rep},,,,,,,,,,,,,,parse-error"); sys.exit()
ops = t.get("Ops/sec", 0.0)
lat = t.get("Percentile Latencies", {})
p = lambda k: lat.get(k, "")
cpu = float(c1) - float(c0)
rcpu = float(r1) - float(r0)
count = t.get("Count", 0) or 0
mops = count / 1e6 if count else 0
per = f"{cpu / mops:.3f}" if mops else ""
avg = (t.get("Accumulated Latency", 0) / count) if count else 0
l50, l99, lmax = (lag.split() + ["", "", ""])[:3]
cms, cok = (conv.split() + ["", ""])[:2]
rc = f"{rcpu:.3f}" if config == "repl" else ""
print(f"{name},{config},{workers},{cell},{rep},{ops:.0f},{avg:.4f},{p('p50.00')},{p('p99.00')},"
      f"{p('p99.90')},{cpu:.3f},{per},{rc},{l50},{l99},{lmax},{cms},{cok},{errors}")
PY
  tail -1 "$csv"
}

for rep in $(seq "$REPS"); do
  for config in $CONFIGS; do
    for workers in $WORKER_SET; do
      # Redis has one execution thread; repl is single-worker by design here.
      [[ $config == repl && $workers != 1 ]] && continue
      for name in $SERVERS; do
        [[ $workers != 1 && $name == redis ]] && continue
        [[ $config == standalone-metrics && ( $name == redis || $name == valkey ) ]] && continue
        start "$name" "$config" "$workers"
        prefill
        [[ $config == repl ]] && { c=$(converge 60); echo "initial sync: $c"; }
        for cell in $(cells_for "$config"); do run_cell "$name" "$config" "$workers" "$cell" "$rep"; done
        stop
      done
    done
  done
done
