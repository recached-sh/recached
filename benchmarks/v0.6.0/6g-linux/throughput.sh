#!/usr/bin/env bash
# Phase 6G-A throughput / latency / CPU matrix over identical RESP workloads.
#
# Runs INSIDE recached-6g-bench:local (see run.sh). memtier_benchmark drives;
# server pinned to SERVER_CPUS, memtier to LOAD_CPUS.
#
# Modes:
#   single  one execution thread everywhere: RECACHED_WORKER_THREADS=1,
#           Valkey io-threads 1, Redis (single-threaded by design)
#   multi   RECACHED_WORKER_THREADS=$WORKERS, Valkey io-threads $WORKERS.
#           Redis has no equivalent and is not re-run; compare to its single row.
#
# Cells: max-rate runs report what each engine sustains; the `lat-fixed` cell
# offers the SAME request rate to every engine (memtier --rate-limiting), which
# is the only fair way to compare tail latency between engines of different
# peak throughput. Every cell also reports server CPU-seconds per million ops.
#
# Output: results.csv (one row per server × mode × cell × rep) and the raw
# memtier JSON per run under json/.
set -euo pipefail

BINS=${BINS:-/bins}
OUT=${OUT:-/out}
SERVERS=${SERVERS:-"v051 dev opt redis valkey"}
MODES=${MODES:-"single multi"}
REPS=${REPS:-3}
TEST_TIME=${TEST_TIME:-10}
SERVER_CPUS=${SERVER_CPUS:-0-3}
LOAD_CPUS=${LOAD_CPUS:-4-7}
WORKERS=${WORKERS:-4}
THREADS=${THREADS:-4}
CLIENTS=${CLIENTS:-12}         # per memtier thread: 48 connections
KEYS=${KEYS:-100000}
SIZE=${SIZE:-32}
FIXED_RATE=${FIXED_RATE:-500}  # requests/s per connection in lat-fixed (×48 = 24k/s)
CELLS=${CELLS:-"set-p1 get-p1 mix-p1 lat-fixed set-p16 get-p16 mix-gauss-p16 hot-incr-p16 hot-hset-p16"}
CMD_CELLS=${CMD_CELLS:-"incr-p16 hset-p16 hget-p16 lpush-p16 rpush-p16 sadd-p16 zadd-p16 mset-p16 mget-p16"}
PORT=${PORT:-7700}

mkdir -p "$OUT/json"
csv=$OUT/results.csv
[[ -s $csv ]] || echo "server,mode,cell,rep,ops_per_sec,avg_ms,p50_ms,p99_ms,p999_ms,server_cpu_s,cpu_s_per_mops,errors" >"$csv"

cli() { redis-cli -p "$PORT" "$@"; }
cpu_s() { awk -v hz="$(getconf CLK_TCK)" '{printf "%.3f", ($14 + $15) / hz}' "/proc/$1/stat"; }

start() {
  local name=$1 mode=$2 dir=/tmp/data-$1
  local workers; workers=$([[ $mode == single ]] && echo 1 || echo "$WORKERS")
  rm -rf "$dir"; mkdir -p "$dir"
  case $name in
    redis) taskset -c "$SERVER_CPUS" redis-server --port "$PORT" --bind 127.0.0.1 --save "" \
             --appendonly no --dir "$dir" --protected-mode no >"$OUT/server-$name.log" 2>&1 & ;;
    valkey) taskset -c "$SERVER_CPUS" valkey-server --port "$PORT" --bind 127.0.0.1 --save "" \
             --appendonly no --dir "$dir" --protected-mode no --io-threads "$workers" \
             >"$OUT/server-$name.log" 2>&1 & ;;
    # The final build with jemalloc's background thread switched off at run
    # time: isolates that thread's CPU cost (6G-D.2).
    final-nobg) env _RJEM_MALLOC_CONF=background_thread:false RECACHED_BIND=127.0.0.1 \
         RECACHED_PORT="$PORT" RECACHED_WS_PORT=$((PORT + 1)) RECACHED_METRICS_PORT=0 \
         RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$dir/dump.rdb" RECACHED_WORKER_THREADS="$workers" \
         taskset -c "$SERVER_CPUS" "$BINS/recached-server-final" >"$OUT/server-$name.log" 2>&1 & ;;
    *) env RECACHED_BIND=127.0.0.1 RECACHED_PORT="$PORT" RECACHED_WS_PORT=$((PORT + 1)) \
         RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$dir/dump.rdb" \
         RECACHED_WORKER_THREADS="$workers" \
         taskset -c "$SERVER_CPUS" "$BINS/recached-server-$name" >"$OUT/server-$name.log" 2>&1 & ;;
  esac
  PID=$!
  for _ in $(seq 80); do cli ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
  cli ping 2>/dev/null | grep -q PONG || { echo "$name did not start" >&2; cat "$OUT/server-$name.log" >&2; exit 1; }
  if [[ $name != redis && $name != valkey ]]; then
    local got; got=$(sed -n 's/.*Command execution: \([0-9]*\) worker threads.*/\1/p' "$OUT/server-$name.log" | head -1)
    [[ $got == "$workers" ]] || { echo "$name: asked for $workers workers, got '${got:-?}'" >&2; exit 1; }
  fi
}

stop() { kill -9 "$PID" 2>/dev/null || true; wait "$PID" 2>/dev/null || true; sleep 0.5; }

mt() {
  taskset -c "$LOAD_CPUS" memtier_benchmark -s 127.0.0.1 -p "$PORT" --protocol=redis \
    -t "$THREADS" -c "$CLIENTS" --hide-histogram --distinct-client-seed "$@"
}

# memtier arguments for one cell (besides the common ones).
cell_args() {
  local k="--key-minimum=1 --key-maximum=$KEYS -d $SIZE --key-prefix=k:"
  case $1 in
    set-p1)        echo "$k --ratio=1:0 --pipeline=1" ;;
    get-p1)        echo "$k --ratio=0:1 --pipeline=1" ;;
    mix-p1)        echo "$k --ratio=1:10 --pipeline=1" ;;
    lat-fixed)     echo "$k --ratio=1:10 --pipeline=1 --rate-limiting=$FIXED_RATE" ;;
    set-p16)       echo "$k --ratio=1:0 --pipeline=16" ;;
    get-p16)       echo "$k --ratio=0:1 --pipeline=16" ;;
    # Skewed reads: Gaussian around the middle, 99.7% of requests on 3% of keys.
    mix-gauss-p16) echo "$k --ratio=1:10 --pipeline=16 --key-pattern=G:G --key-stddev=$((KEYS / 200))" ;;
    hot-incr-p16)  echo "--pipeline=16 --command=incr_hot" ;;
    hot-hset-p16)  echo "--pipeline=16 -d $SIZE --key-minimum=1 --key-maximum=$KEYS --key-prefix=f: --command=hset_hot" ;;
    incr-p16)      echo "--pipeline=16 --key-minimum=1 --key-maximum=$KEYS --key-prefix=c: --command=incr" ;;
    hset-p16)      echo "--pipeline=16 -d $SIZE --key-minimum=1 --key-maximum=$KEYS --key-prefix=h: --command=hset" ;;
    hget-p16)      echo "--pipeline=16 --key-minimum=1 --key-maximum=$KEYS --key-prefix=h: --command=hget" ;;
    lpush-p16)     echo "--pipeline=16 -d $SIZE --key-minimum=1 --key-maximum=1000 --key-prefix=l: --command=lpush" ;;
    rpush-p16)     echo "--pipeline=16 -d $SIZE --key-minimum=1 --key-maximum=1000 --key-prefix=r: --command=rpush" ;;
    # Members are a second generated key: text (Recached refuses non-UTF-8
    # members by design, which memtier's -R random bytes would trip) and
    # distinct, so sets really grow.
    sadd-p16)      echo "--pipeline=16 --key-minimum=1 --key-maximum=1000 --key-prefix=s: --command=sadd" ;;
    zadd-p16)      echo "--pipeline=16 --key-minimum=1 --key-maximum=1000 --key-prefix=z: --command=zadd" ;;
    mset-p16)      echo "--pipeline=16 -d $SIZE --key-minimum=1 --key-maximum=$KEYS --key-prefix=m: --command=mset" ;;
    mget-p16)      echo "--pipeline=16 --key-minimum=1 --key-maximum=$KEYS --key-prefix=m: --command=mget" ;;
  esac
}

# --command strings contain spaces, so they are expanded here, not via echo.
command_for() {
  # memtier: `__key__` must stand alone; prefixes come from --key-prefix.
  case $1 in
    incr_hot) echo "incr hot:counter" ;;
    hset_hot) echo "hset hot:hash __key__ __data__" ;;
    incr)     echo "incr __key__" ;;
    hset)     echo "hset __key__ f1 __data__" ;;
    hget)     echo "hget __key__ f1" ;;
    lpush)    echo "lpush __key__ __data__" ;;
    rpush)    echo "rpush __key__ __data__" ;;
    sadd)     echo "sadd __key__ __key__" ;;
    zadd)     echo "zadd __key__ 1 __key__" ;;
    mset)     echo "mset __key__ __data__ __key__ __data__ __key__ __data__ __key__ __data__" ;;
    mget)     echo "mget __key__ __key__ __key__ __key__" ;;
  esac
}

run_cell() {
  local name=$1 mode=$2 cell=$3 rep=$4
  local json="$OUT/json/$name-$mode-$cell-$rep.json"
  local -a args; read -r -a args <<<"$(cell_args "$cell")"
  local -a final=()
  for a in "${args[@]}"; do
    if [[ $a == --command=* ]]; then
      final+=("--command=$(command_for "${a#--command=}")" "--command-key-pattern=R")
    else
      final+=("$a")
    fi
  done
  local c0 c1
  c0=$(cpu_s "$PID")
  mt "${final[@]}" --test-time="$TEST_TIME" --json-out-file="$json" >"$OUT/json/$name-$mode-$cell-$rep.txt" 2>&1 \
    || { echo "memtier failed: $name $mode $cell"; tail -5 "$OUT/json/$name-$mode-$cell-$rep.txt"; }
  c1=$(cpu_s "$PID")
  local errors; errors=$(grep -c 'error response' "$OUT/json/$name-$mode-$cell-$rep.txt" || true)
  python3 -I - "$json" "$name" "$mode" "$cell" "$rep" "$c0" "$c1" "$errors" >>"$csv" <<'PY'
import json, sys
path, name, mode, cell, rep, c0, c1, errors = sys.argv[1:9]
try:
    t = json.load(open(path))["ALL STATS"]["Totals"]
except Exception as e:
    print(f"{name},{mode},{cell},{rep},,,,,,,,parse-error")
    sys.exit()
ops = t.get("Ops/sec", 0.0)
lat = t.get("Percentile Latencies", {})
p = lambda k: lat.get(k, "")
cpu = float(c1) - float(c0)
# Total ops = ops/s × measured seconds; memtier reports both in Totals.
count = t.get("Count", 0) or 0
mops = count / 1e6 if count else 0
per = f"{cpu / mops:.3f}" if mops else ""
# memtier rounds "Average Latency" to whole ms in JSON; the accumulated total is exact.
avg = (t.get("Accumulated Latency", 0) / count) if count else 0
print(f"{name},{mode},{cell},{rep},{ops:.0f},{avg:.4f},{p('p50.00')},{p('p99.00')},{p('p99.90')},{cpu:.3f},{per},{errors}")
PY
  tail -1 "$csv"
}

prefill() {
  local per=$(( (KEYS + THREADS * CLIENTS - 1) / (THREADS * CLIENTS) ))
  mt --key-minimum=1 --key-maximum="$KEYS" -d "$SIZE" --key-prefix=k: --ratio=1:0 \
     --key-pattern=P:P --requests="$per" --pipeline=64 >"$OUT/prefill.log" 2>&1
  mt --key-minimum=1 --key-maximum="$KEYS" -d "$SIZE" --key-prefix=h: --requests="$per" --pipeline=64 \
     --command="hset __key__ f1 __data__" --command-key-pattern=P >>"$OUT/prefill.log" 2>&1
  mt --key-minimum=1 --key-maximum="$KEYS" -d "$SIZE" --key-prefix=m: --requests="$per" --pipeline=64 \
     --command="set __key__ __data__" --command-key-pattern=P >>"$OUT/prefill.log" 2>&1
  local n; n=$(cli dbsize | tr -d '\r')
  (( n >= 3 * KEYS * 9 / 10 )) || { echo "prefill loaded only $n keys" >&2; exit 1; }
}

for rep in $(seq "$REPS"); do
  for mode in $MODES; do
    for name in $SERVERS; do
      [[ $mode == multi && $name == redis ]] && continue
      start "$name" "$mode"
      prefill
      cells=$CELLS
      [[ $mode == single ]] && cells="$CELLS $CMD_CELLS"
      for cell in $cells; do run_cell "$name" "$mode" "$cell" "$rep"; done
      stop
    done
  done
done
