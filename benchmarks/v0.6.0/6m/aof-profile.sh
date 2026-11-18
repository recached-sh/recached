#!/usr/bin/env bash
#
# Recached 6M-A: AOF execution profile.
#
# For every cell of servers x policies x workers x tests x pipeline x clients,
# runs one redis-benchmark load against a fresh server and records:
#   - throughput and p50/p99 (redis-benchmark --csv)
#   - server CPU-seconds consumed during the load (utime+stime delta)
#   - for Recached, the INFO persistence AOF counters' delta: batches, frames,
#     write(2) calls and their time, fsyncs, queue wait, waiters per batch,
#     lock contention, gate parks, and the frames-per-batch histogram
# Optionally (STRACE=1, Linux, Recached and Redis) a separate run under
# `strace -c -f` for kernel-side syscall counts; slow, so separate from the
# timing runs.
#
# Linux and macOS. On Linux, PIN=1 pins server and load generator to disjoint
# CPUs (SERVER_CPUS / BENCH_CPUS). macOS has no pinning: its numbers are
# indicative only.
#
# Usage:
#   RECACHED_BIN=... [BASE_BIN=...] [REDIS_BIN=... VALKEY_BIN=...] \
#   BENCH=.../redis-benchmark CLI=.../redis-cli OUT=dir bash aof-profile.sh
# Knobs (defaults in brackets):
#   SERVERS ["recached"]  any of: base recached redis valkey
#   POLICIES ["everysec"] any of: none no everysec always
#   WORKERS ["1"]  CLIENTS ["1 2 4 8 16 32 50 100 200"]  PIPELINES ["1"]
#   TESTS ["set"]  any of: set set-new incr hset mset get
#   REQUESTS [200000]  ALWAYS_REQUESTS [20000] (`always` fsyncs per batch)
#   KEYSPACE [100000]
#   REPS [1]  BENCH_THREADS [2]
#   EXTRA_ENV [""]  extra VAR=value words for Recached servers, e.g.
#     "RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=6710" to measure the
#     per-command append path a replication listener puts every write on

set -euo pipefail

OUT=${OUT:?set OUT}
RECACHED_BIN=${RECACHED_BIN:-}
BASE_BIN=${BASE_BIN:-}
REDIS_BIN=${REDIS_BIN:-}
VALKEY_BIN=${VALKEY_BIN:-}
BENCH=${BENCH:-$(command -v redis-benchmark || true)}
CLI=${CLI:-$(command -v redis-cli || true)}
SERVERS=${SERVERS:-recached}
POLICIES=${POLICIES:-everysec}
WORKERS=${WORKERS:-1}
CLIENTS=${CLIENTS:-"1 2 4 8 16 32 50 100 200"}
PIPELINES=${PIPELINES:-1}
TESTS=${TESTS:-set}
REQUESTS=${REQUESTS:-200000}
ALWAYS_REQUESTS=${ALWAYS_REQUESTS:-20000}
KEYSPACE=${KEYSPACE:-100000}
REPS=${REPS:-1}
BENCH_THREADS=${BENCH_THREADS:-2}
PIN=${PIN:-0}
SERVER_CPUS=${SERVER_CPUS:-}
BENCH_CPUS=${BENCH_CPUS:-}
STRACE=${STRACE:-0}
EXTRA_ENV=${EXTRA_ENV:-}
PORT=${PORT:-6700}

[[ -x $BENCH && -x $CLI ]] || { echo "BENCH and CLI must point at redis-benchmark and redis-cli" >&2; exit 1; }
mkdir -p "$OUT"
OS=$(uname -s)

server_bin() {
  case $1 in
    recached) echo "$RECACHED_BIN" ;; base) echo "$BASE_BIN" ;;
    redis) echo "$REDIS_BIN" ;; valkey) echo "$VALKEY_BIN" ;;
  esac
}

# Cumulative user+system CPU-seconds of a process.
cpu_seconds() {
  local pid=$1
  if [[ $OS == Linux ]]; then
    local tck; tck=$(getconf CLK_TCK)
    # Fields 14/15 (utime/stime) after the parenthesised comm.
    awk -v t="$tck" '{ sub(/^.*\) /, ""); printf "%.3f\n", ($12 + $13) / t }' "/proc/$pid/stat"
  else
    # macOS: cumulative "[[dd-]hh:]mm:ss.cc".
    ps -o time= -p "$pid" | awk '{
      n = split($1, p, ":"); s = 0
      for (i = 1; i <= n; i++) { v = p[i]; if (i < n) { sub(/^.*-/, "", v) } s = s * 60 + v }
      printf "%.3f\n", s }'
  fi
}

pre_server=(); pre_bench=()
if [[ $PIN == 1 ]]; then pre_server=(taskset -c "$SERVER_CPUS"); pre_bench=(taskset -c "$BENCH_CPUS"); fi

SERVER_PID=""
start_server() {
  local name=$1 policy=$2 workers=$3 dir="$OUT/data"
  local bin; bin=$(server_bin "$name")
  [[ -x $bin ]] || { echo "no executable for $name" >&2; exit 1; }
  rm -rf "$dir"; mkdir -p "$dir"
  case $name in
    recached|base)
      local -a env=(RECACHED_BIND=127.0.0.1 RECACHED_PORT="$PORT" RECACHED_WS_PORT=$((PORT + 1))
        RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$dir/dump.rdb"
        RECACHED_WORKER_THREADS="$workers")
      [[ $policy != none ]] && env+=(RECACHED_AOF_PATH="$dir/appendonly.aof" RECACHED_AOF_SYNC="$policy")
      # shellcheck disable=SC2206  # word-split on purpose: VAR=value words
      [[ -n $EXTRA_ENV ]] && env+=($EXTRA_ENV)
      env "${env[@]}" ${pre_server[@]+"${pre_server[@]}"} "$bin" >"$OUT/server-$name.log" 2>&1 &
      ;;
    redis|valkey)
      local -a args=(--port "$PORT" --bind 127.0.0.1 --save "" --dir "$dir" --protected-mode no)
      if [[ $policy == none ]]; then args+=(--appendonly no)
      else args+=(--appendonly yes --appendfsync "$policy"); fi
      [[ $name == valkey ]] && args+=(--io-threads "$workers")
      ${pre_server[@]+"${pre_server[@]}"} "$bin" "${args[@]}" >"$OUT/server-$name.log" 2>&1 &
      ;;
  esac
  SERVER_PID=$!
  local tries=80
  while (( tries-- > 0 )); do
    "$CLI" -p "$PORT" ping 2>/dev/null | grep -q PONG && return 0
    sleep 0.25
  done
  echo "$name did not start; see $OUT/server-$name.log" >&2; exit 1
}
stop_server() {
  [[ -n $SERVER_PID ]] && { kill -9 "$SERVER_PID" 2>/dev/null || true; wait "$SERVER_PID" 2>/dev/null || true; }
  SERVER_PID=""; sleep 0.3
}
trap stop_server EXIT

# INFO persistence AOF counters as "k=v k=v" (Recached only).
aof_counters() {
  "$CLI" -p "$PORT" info persistence 2>/dev/null | tr -d '\r' | grep '^recached_aof_' | sed 's/^recached_aof_//' | tr '\n' ' '
}

bench_args() {
  local test=$1 pipeline=$2 clients=$3 requests=$4
  local -a a=(-p "$PORT" -n "$requests" -c "$clients" -P "$pipeline" -d 32 --threads "$BENCH_THREADS" --csv)
  case $test in
    set)     a+=(-r "$KEYSPACE" -t set) ;;
    set-new) a+=(-r 100000000000 -t set) ;;   # effectively always a new key
    incr)    a+=(-r "$KEYSPACE" -t incr) ;;
    hset)    a+=(-r "$KEYSPACE" -t hset) ;;
    mset)    a+=(-r "$KEYSPACE" -t mset) ;;
    get)     a+=(-r "$KEYSPACE" -t get) ;;
    *) echo "unknown test $test" >&2; exit 1 ;;
  esac
  printf '%s\n' "${a[@]}"
}

# Pre-populate existing keys so `set`/`incr`/`hset` overwrite.
populate() {
  local test=$1
  case $test in
    set|get|mset) "$BENCH" -p "$PORT" -n "$KEYSPACE" -c 20 -P 64 -r "$KEYSPACE" -d 32 -t set -q >/dev/null 2>&1 ;;
    incr) "$BENCH" -p "$PORT" -n "$KEYSPACE" -c 20 -P 64 -r "$KEYSPACE" -t incr -q >/dev/null 2>&1 ;;
    hset) "$BENCH" -p "$PORT" -n "$KEYSPACE" -c 20 -P 64 -r "$KEYSPACE" -t hset -q >/dev/null 2>&1 ;;
  esac
}

{
  echo "date_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "os: $(uname -srvm)"
  if [[ $OS == Linux ]]; then
    echo "cpu: $(lscpu | sed -n 's/^Model name:[[:space:]]*//p')"
    echo "virtualization: $(systemd-detect-virt 2>/dev/null || echo unknown)"
    echo "governors: $(cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor 2>/dev/null | sort | uniq -c | tr '\n' ' ' || echo n/a)"
    echo "aof_fs: $(df -T "$OUT" | tail -1 | awk '{print $2}')"
  else
    echo "cpu: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown)"
    echo "aof_fs: $(df "$OUT" | tail -1 | awk '{print $1}') (macOS; indicative only)"
  fi
  echo "pin: $PIN server=$SERVER_CPUS bench=$BENCH_CPUS"
  echo "extra_env: ${EXTRA_ENV:-none}"
  echo "bench: $("$BENCH" --version 2>/dev/null | head -1) threads=$BENCH_THREADS"
  for s in $SERVERS; do
    b=$(server_bin "$s")
    echo "server.$s: $b sha256=$( (sha256sum "$b" 2>/dev/null || shasum -a 256 "$b") | cut -d' ' -f1) version=$("$b" --version 2>/dev/null | head -1)"
  done
  echo "matrix: servers=[$SERVERS] policies=[$POLICIES] workers=[$WORKERS] tests=[$TESTS] pipelines=[$PIPELINES] clients=[$CLIENTS] requests=$REQUESTS always_requests=$ALWAYS_REQUESTS keyspace=$KEYSPACE reps=$REPS"
} > "$OUT/conditions.txt"
cat "$OUT/conditions.txt"

CSV="$OUT/results.csv"
[[ -f $CSV ]] || echo "server,policy,workers,test,pipeline,clients,rep,requests,rps,p50_ms,p99_ms,server_cpu_s,cpu_us_per_op,counters" > "$CSV"

for rep in $(seq 1 "$REPS"); do
for policy in $POLICIES; do
for workers in $WORKERS; do
for server in $SERVERS; do
  # Redis has one execution thread whatever `workers` says: run it once.
  if [[ $server == redis && $workers != "${WORKERS%% *}" ]]; then continue; fi
  for test in $TESTS; do
  for pipeline in $PIPELINES; do
  for clients in $CLIENTS; do
    requests=$REQUESTS
    [[ $policy == always ]] && requests=$ALWAYS_REQUESTS
    start_server "$server" "$policy" "$workers"
    populate "$test"
    before_cpu=$(cpu_seconds "$SERVER_PID")
    before_ctr=""; [[ $server == recached || $server == base ]] && before_ctr=$(aof_counters)
    args=(); while IFS= read -r a; do args+=("$a"); done < <(bench_args "$test" "$pipeline" "$clients" "$requests")
    row=$(${pre_bench[@]+"${pre_bench[@]}"} "$BENCH" "${args[@]}" 2>/dev/null | tail -n 1)
    after_cpu=$(cpu_seconds "$SERVER_PID")
    after_ctr=""; [[ $server == recached || $server == base ]] && after_ctr=$(aof_counters)
    # row: "test","rps","avg","min","p50","p95","p99","max"
    rps=$(echo "$row" | cut -d, -f2 | tr -d '"')
    p50=$(echo "$row" | cut -d, -f5 | tr -d '"')
    p99=$(echo "$row" | cut -d, -f7 | tr -d '"')
    cpu=$(awk -v a="$before_cpu" -v b="$after_cpu" 'BEGIN { printf "%.3f", b - a }')
    us_per_op=$(awk -v c="$cpu" -v n="$requests" 'BEGIN { printf "%.3f", c * 1e6 / n }')
    delta=$(python3 - "$before_ctr" "$after_ctr" <<'PY'
import sys
def parse(s):
    d = {}
    for kv in s.split():
        k, _, v = kv.partition("=") if "=" in kv else kv.partition(":")
        d[k] = v
    return d
a, b = parse(sys.argv[1]), parse(sys.argv[2])
out = []
for k, v in b.items():
    if "," in v:
        av = a.get(k, ",".join("0" * len(v.split(","))))
        out.append(k + "=" + "/".join(str(int(y) - int(x)) for x, y in zip(av.split(","), v.split(","))))
    else:
        out.append(k + "=" + str(int(v) - int(a.get(k, "0"))))
print(" ".join(out))
PY
)
    echo "$server,$policy,$workers,$test,$pipeline,$clients,$rep,$requests,$rps,$p50,$p99,$cpu,$us_per_op,$delta" | tee -a "$CSV"
    stop_server

    if [[ $STRACE == 1 && $OS == Linux && $server != valkey ]]; then
      start_server "$server" "$policy" "$workers"
      populate "$test"
      strace -c -f -q -p "$SERVER_PID" -o "$OUT/strace-$server-$policy-w$workers-$test-P$pipeline-c$clients.txt" &
      spid=$!; sleep 0.5
      ${pre_bench[@]+"${pre_bench[@]}"} "$BENCH" "${args[@]}" >/dev/null 2>&1 || true
      kill -INT "$spid" 2>/dev/null || true; wait "$spid" 2>/dev/null || true
      stop_server
    fi
  done; done; done
done; done; done; done
echo "results: $CSV"
