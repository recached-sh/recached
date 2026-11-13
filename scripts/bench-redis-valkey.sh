#!/usr/bin/env bash
#
# Recached vs Redis vs Valkey: throughput, tail latency, CPU and memory over
# identical RESP workloads, with provenance for every number.
#
# What it measures, per server:
#   - throughput and latency (p50/p95/p99/max) from redis-benchmark --csv, for
#     each command × pipeline depth × client count × value size, repeated REPS
#     times — the summary reports the median and the spread, never the best run
#   - CPU-seconds consumed by the *server process* during each run, read from
#     the kernel before and after, so throughput per CPU-second is comparable
#     across servers with different threading models
#   - process RSS: idle, after loading N keys, after FLUSHDB, after churn — and
#     from those, incremental bytes per key. RSS is what the OS charges; it is
#     NOT Recached's logical `used_memory` counter, which is not comparable to
#     Redis's allocator-inclusive figures
#
# What it deliberately does not do:
#   - compare an embedded/in-process read against a TCP round trip
#   - rank workloads that are not the same RESP commands with the same replies
#   - read/write *ratios* or Zipfian keys: redis-benchmark cannot generate them
#     (memtier_benchmark can; it is not a dependency of this script). "hot"
#     key distribution is supported: no -r means every request hits one key
#
# Fairness knobs:
#   SINGLE=1  one execution worker everywhere: RECACHED_WORKER_THREADS=1,
#             Valkey io-threads 1 (Redis is single-threaded for execution)
#   SINGLE=0  whole-machine budget: Recached default workers, Valkey io-threads
#             = WORKERS. Redis has no equivalent; that is the comparison.
#   PIN=1     (Linux) taskset the servers onto SERVER_CPUS and the load
#             generator onto BENCH_CPUS so the client does not steal server CPU
#
# Usage:
#   scripts/bench-redis-valkey.sh                         # quick matrix
#   PROFILE=full REPS=5 scripts/bench-redis-valkey.sh     # the published matrix
#   SERVERS="recached redis" scripts/bench-redis-valkey.sh
#   BASELINE_BIN=target/v0.6-baseline/recached-server-7cdbd0d \
#     SERVERS="baseline recached" scripts/bench-redis-valkey.sh   # before/after
#
# Output: $OUT/results.csv, $OUT/memory.csv, $OUT/conditions.txt; summarise
# with scripts/bench-summarize.py $OUT.

set -euo pipefail

RECACHED_BIN=${RECACHED_BIN:-./target/release/recached-server}
BASELINE_BIN=${BASELINE_BIN:-}
REDIS_BIN=${REDIS_BIN:-$(command -v redis-server || true)}
VALKEY_BIN=${VALKEY_BIN:-$(command -v valkey-server || echo /usr/local/opt/valkey/bin/valkey-server)}
BENCH=${BENCH:-$(command -v redis-benchmark || true)}
CLI=${CLI:-$(command -v redis-cli || true)}

SERVERS=${SERVERS:-"recached redis valkey"}
PROFILE=${PROFILE:-quick}
REPS=${REPS:-3}
SINGLE=${SINGLE:-1}
WORKERS=${WORKERS:-4}
PERSIST=${PERSIST:-none}          # none | aof-everysec
PORT_BASE=${PORT_BASE:-7400}
OUT=${OUT:-bench-results/$(date +%Y%m%d-%H%M%S)}
PIN=${PIN:-0}
SERVER_CPUS=${SERVER_CPUS:-0-3}
BENCH_CPUS=${BENCH_CPUS:-4-7}
BENCH_THREADS=${BENCH_THREADS:-2}

case $PROFILE in
  quick)
    TESTS=${TESTS:-"set get incr mset mget hset lpush sadd zadd"}
    PIPELINES=${PIPELINES:-"1 16"}
    CLIENT_COUNTS=${CLIENT_COUNTS:-"1 50"}
    SIZES=${SIZES:-"32"}
    KEYSPACES=${KEYSPACES:-"100000"}
    REQUESTS=${REQUESTS:-100000}
    MEM_KEYS=${MEM_KEYS:-"100000"}
    MEM_SIZES=${MEM_SIZES:-"32 1024"}
    ;;
  full)
    TESTS=${TESTS:-"set get incr mset mget hset hget lpush rpush sadd zadd"}
    PIPELINES=${PIPELINES:-"1 8 32"}
    CLIENT_COUNTS=${CLIENT_COUNTS:-"1 16 100"}
    SIZES=${SIZES:-"32 256 1024 4096"}
    KEYSPACES=${KEYSPACES:-"10000 100000 hot"}
    REQUESTS=${REQUESTS:-300000}
    MEM_KEYS=${MEM_KEYS:-"10000 100000 1000000"}
    MEM_SIZES=${MEM_SIZES:-"32 256 1024 4096"}
    ;;
  *) echo "PROFILE must be quick or full" >&2; exit 1 ;;
esac

for tool in "$BENCH" "$CLI"; do
  [[ -n $tool && -x $tool ]] || { echo "redis-benchmark and redis-cli are required" >&2; exit 1; }
done
command -v python3 >/dev/null || { echo "python3 is required (key generation, summaries)" >&2; exit 1; }
if [[ $PIN == 1 ]] && ! command -v taskset >/dev/null; then
  echo "PIN=1 needs taskset (Linux); continuing unpinned" >&2
  PIN=0
fi

mkdir -p "$OUT"
SERVER_PID=""
cleanup() {
  if [[ -n $SERVER_PID ]]; then
    disown "$SERVER_PID" 2>/dev/null || true
    kill -9 "$SERVER_PID" 2>/dev/null || true
  fi
  return 0
}
trap cleanup EXIT

# ── provenance ───────────────────────────────────────────────────────────────
conditions() {
  echo "date_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "host_os: $(uname -srm)"
  if [[ -r /proc/cpuinfo ]]; then
    echo "cpu: $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //')"
    echo "logical_cpus: $(nproc)"
    echo "governor: $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null || echo n/a)"
    echo "memory: $(grep MemTotal /proc/meminfo | awk '{print $2" kB"}')"
    echo "numa: $(lscpu 2>/dev/null | grep -i 'NUMA node(s)' | awk '{print $NF}' || echo n/a)"
  else
    echo "cpu: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown)"
    echo "logical_cpus: $(sysctl -n hw.ncpu 2>/dev/null || echo ?)"
    echo "physical_cpus: $(sysctl -n hw.physicalcpu 2>/dev/null || echo ?)"
    echo "memory_bytes: $(sysctl -n hw.memsize 2>/dev/null || echo ?)"
    echo "governor: n/a (macOS)"
    echo "numa: n/a (macOS)"
  fi
  echo "load_average_at_start: $(uptime | sed 's/.*load average[s]*: //')"
  echo "pinning: $([[ $PIN == 1 ]] && echo "servers=$SERVER_CPUS bench=$BENCH_CPUS" || echo none)"
  echo "network: loopback (127.0.0.1)"
  echo "bench_tool: $("$BENCH" --version 2>/dev/null | head -1 || echo redis-benchmark)"
  echo "bench_threads: $BENCH_THREADS"
  echo "profile: $PROFILE reps=$REPS single_worker=$SINGLE workers=$WORKERS persistence=$PERSIST"
  echo "requests_per_run: $REQUESTS"
  for s in $SERVERS; do
    local bin; bin=$(server_bin "$s")
    echo "server.$s.binary: $bin"
    echo "server.$s.sha256: $(shasum -a 256 "$bin" 2>/dev/null | cut -d' ' -f1 || sha256sum "$bin" | cut -d' ' -f1)"
    echo "server.$s.version: $(server_version "$s")"
  done
  if [[ -d .git ]]; then
    echo "recached_commit: $(git rev-parse HEAD)$(git diff --quiet || echo ' (dirty)')"
  fi
  echo "rustc: $(rustc --version 2>/dev/null || echo n/a)"
  echo "recached_build: release profile from Cargo.toml (opt-level=3, lto=thin, codegen-units=1, overflow-checks=true); allocator jemalloc"
}

server_bin() {
  case $1 in
    recached) echo "$RECACHED_BIN" ;;
    baseline) echo "$BASELINE_BIN" ;;
    redis) echo "$REDIS_BIN" ;;
    valkey) echo "$VALKEY_BIN" ;;
    *) echo "unknown server $1" >&2; exit 1 ;;
  esac
}

server_version() {
  local bin; bin=$(server_bin "$1")
  case $1 in
    recached|baseline) "$bin" --version 2>/dev/null || echo unknown ;;
    *) "$bin" --version 2>/dev/null | head -1 ;;
  esac
}

# ── process sampling ─────────────────────────────────────────────────────────
rss_kb() { ps -o rss= -p "$1" | tr -d ' '; }

# CPU seconds (user + system) consumed by a process so far.
cpu_secs() {
  local pid=$1
  if [[ -r /proc/$pid/stat ]]; then
    awk -v hz="$(getconf CLK_TCK)" '{printf "%.3f", ($14 + $15) / hz}' "/proc/$pid/stat"
  else
    # macOS ps prints [[dd-]hh:]mm:ss.cc
    ps -o time= -p "$pid" | awk '{
      n = split($1, p, ":"); s = 0
      for (i = 1; i <= n; i++) s = s * 60 + p[i]
      printf "%.3f", s
    }'
  fi
}

# ── servers ──────────────────────────────────────────────────────────────────
start_server() {
  local name=$1 port=$2
  local bin; bin=$(server_bin "$name")
  [[ -x $bin ]] || { echo "no executable for $name at '$bin'" >&2; exit 1; }
  local -a pre=()
  [[ $PIN == 1 ]] && pre=(taskset -c "$SERVER_CPUS")
  local dir="$OUT/data-$name"
  rm -rf "$dir"; mkdir -p "$dir"
  case $name in
    recached|baseline)
      local -a env=(RECACHED_BIND=127.0.0.1 RECACHED_PORT="$port" RECACHED_WS_PORT=$((port + 1))
        RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$dir/dump.rdb")
      [[ $SINGLE == 1 ]] && env+=(RECACHED_WORKER_THREADS=1) || env+=(RECACHED_WORKER_THREADS="$WORKERS")
      [[ $PERSIST == aof-everysec ]] && env+=(RECACHED_AOF_PATH="$dir/appendonly.aof" RECACHED_AOF_SYNC=everysec)
      env "${env[@]}" ${pre[@]+"${pre[@]}"} "$bin" >"$OUT/server-$name.log" 2>&1 &
      ;;
    redis|valkey)
      local -a args=(--port "$port" --bind 127.0.0.1 --save "" --dir "$dir" --protected-mode no)
      if [[ $PERSIST == aof-everysec ]]; then args+=(--appendonly yes --appendfsync everysec)
      else args+=(--appendonly no); fi
      if [[ $name == valkey ]]; then
        [[ $SINGLE == 1 ]] && args+=(--io-threads 1) || args+=(--io-threads "$WORKERS")
      fi
      ${pre[@]+"${pre[@]}"} "$bin" "${args[@]}" >"$OUT/server-$name.log" 2>&1 &
      ;;
  esac
  SERVER_PID=$!
  local tries=80
  while (( tries-- > 0 )); do
    "$CLI" -p "$port" ping 2>/dev/null | grep -q PONG && break
    sleep 0.25
  done
  "$CLI" -p "$port" ping 2>/dev/null | grep -q PONG || { echo "$name did not start; see $OUT/server-$name.log" >&2; exit 1; }
  if [[ $name == recached || $name == baseline ]]; then
    local want; want=$([[ $SINGLE == 1 ]] && echo 1 || echo "$WORKERS")
    local got; got=$(sed -n 's/.*Command execution: \([0-9]*\) worker threads.*/\1/p' "$OUT/server-$name.log" | head -1)
    [[ "$got" == "$want" ]] || { echo "$name: asked for $want workers, server reports '${got:-unknown}'" >&2; exit 1; }
  fi
}

stop_server() {
  if [[ -n $SERVER_PID ]]; then
    disown "$SERVER_PID" 2>/dev/null || true
    kill -9 "$SERVER_PID" 2>/dev/null || true
  fi
  SERVER_PID=""
  sleep 0.5
}

# ── workload ─────────────────────────────────────────────────────────────────
# Recached has no FLUSHALL, and redis-cli exits 0 on -ERR — FLUSHDB everywhere.
flush() { "$CLI" -p "$1" flushdb >/dev/null; }

# One redis-benchmark invocation for one command. Echoes its single CSV row.
run_one() {
  local port=$1 test=$2 pipeline=$3 clients=$4 size=$5 keyspace=$6
  local -a pre=() ks=()
  [[ $PIN == 1 ]] && pre=(taskset -c "$BENCH_CPUS")
  [[ $keyspace != hot ]] && ks=(-r "$keyspace")
  local -a work
  case $test in
    # redis-benchmark has no MGET test; 10 random keys per call, like its MSET.
    mget) work=(MGET key:__rand_int__ key:__rand_int__ key:__rand_int__ key:__rand_int__ key:__rand_int__
                    key:__rand_int__ key:__rand_int__ key:__rand_int__ key:__rand_int__ key:__rand_int__) ;;
    hget) work=(HGET myhash:__rand_int__ field) ;;
    *) work=(-t "$test") ;;
  esac
  ${pre[@]+"${pre[@]}"} "$BENCH" -p "$port" -n "$REQUESTS" -c "$clients" -P "$pipeline" -d "$size" \
    ${ks[@]+"${ks[@]}"} --threads "$BENCH_THREADS" --csv "${work[@]}" 2>/dev/null | tail -n 1
}

prefill() {
  # GET/MGET/HGET on an empty keyspace answer nil and measure nothing useful.
  local port=$1 size=$2 keyspace=$3
  local -a ks=()
  [[ $keyspace != hot ]] && ks=(-r "$keyspace")
  local n=$([[ $keyspace == hot ]] && echo 1000 || echo $((keyspace * 3)))
  "$BENCH" -p "$port" -n "$n" -c 50 -P 32 -d "$size" ${ks[@]+"${ks[@]}"} -t set -q >/dev/null 2>&1
  "$BENCH" -p "$port" -n "$n" -c 50 -P 32 -d "$size" ${ks[@]+"${ks[@]}"} -q HSET myhash:__rand_int__ field "$(head -c "$size" /dev/zero | tr '\0' x)" >/dev/null 2>&1
}

throughput_matrix() {
  local name=$1 port=$2
  local csv="$OUT/results.csv"
  for keyspace in $KEYSPACES; do
    for size in $SIZES; do
      flush "$port"
      prefill "$port" "$size" "$keyspace"
      # Warm-up, discarded.
      run_one "$port" set 16 50 "$size" "$keyspace" >/dev/null
      for rep in $(seq 1 "$REPS"); do
        for pipeline in $PIPELINES; do
          for clients in $CLIENT_COUNTS; do
            for test in $TESTS; do
              local c0 c1 row
              c0=$(cpu_secs "$SERVER_PID")
              row=$(run_one "$port" "$test" "$pipeline" "$clients" "$size" "$keyspace")
              c1=$(cpu_secs "$SERVER_PID")
              # row: "test","rps","avg","min","p50","p95","p99","max"
              echo "$name,$SINGLE,$PERSIST,$test,$pipeline,$clients,$size,$keyspace,$rep,$REQUESTS,$(awk -v a="$c0" -v b="$c1" 'BEGIN{printf "%.3f", b-a}'),$(echo "$row" | cut -d, -f2- | tr -d '"')" >>"$csv"
            done
          done
        done
      done
    done
  done
}

# Mass-insert N keys of SIZE bytes, pipelined in batches. Not `redis-cli
# --pipe`: it ends with ECHO plus an empty line, which servers before Recached
# 0.6 could not answer, so the baseline could never be loaded with it. Every
# reply to SET/HSET is one line, so counting lines counts replies; an error
# reply aborts the run rather than understating the dataset.
load_keys() {
  local port=$1 n=$2 size=$3 kind=$4
  python3 -I - "$port" "$n" "$size" "$kind" <<'PY'
import socket, sys
port, n, size, kind = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
val = b"x" * size
s = socket.create_connection(("127.0.0.1", port))
def enc(*parts):
    return b"*%d\r\n" % len(parts) + b"".join(b"$%d\r\n%s\r\n" % (len(p), p) for p in parts)
BATCH = 5000
for start in range(0, n, BATCH):
    out = []
    for i in range(start, min(n, start + BATCH)):
        k = b"mem:%010d" % i
        if kind == "string":
            out.append(enc(b"SET", k, val))
        elif kind == "hash":
            out.append(enc(b"HSET", k, b"f1", val, b"f2", val, b"f3", val))
        else:
            out.append(enc(b"SET", k, val, b"EX", b"3600"))
    s.sendall(b"".join(out))
    want, got, buf = len(out), 0, b""
    while got < want:
        chunk = s.recv(1 << 16)
        if not chunk:
            sys.exit("server closed the connection")
        buf += chunk
        got = buf.count(b"\r\n")
    if b"\r\n-" in b"\r\n" + buf:
        sys.exit("error reply while loading: " + buf.split(b"-", 1)[1][:200].decode(errors="replace"))
PY
}

dbsize() { "$CLI" -p "$1" dbsize | tr -d '\r'; }

memory_matrix() {
  local name=$1 port=$2
  local csv="$OUT/memory.csv"
  for kind in string hash ttl; do
    for n in $MEM_KEYS; do
      for size in $MEM_SIZES; do
        # A fresh process per scenario: RSS one scenario leaves behind (and
        # allocators keep plenty) must not become the next one's "idle".
        stop_server
        start_server "$name" "$port"
        sleep 1
        local idle loaded flushed churned got
        idle=$(rss_kb "$SERVER_PID")
        load_keys "$port" "$n" "$size" "$kind"
        got=$(dbsize "$port")
        sleep 1
        loaded=$(rss_kb "$SERVER_PID")
        # Churn: overwrite every key once more, then measure steady state.
        load_keys "$port" "$n" "$size" "$kind"
        sleep 1
        churned=$(rss_kb "$SERVER_PID")
        flush "$port"; sleep 2
        flushed=$(rss_kb "$SERVER_PID")
        echo "$name,$SINGLE,$PERSIST,$kind,$n,$size,$got,$idle,$loaded,$churned,$flushed" >>"$csv"
      done
    done
  done
}

# ── main ─────────────────────────────────────────────────────────────────────
conditions | tee "$OUT/conditions.txt"
echo "server,single_worker,persistence,test,pipeline,clients,size,keyspace,rep,requests,server_cpu_s,rps,avg_ms,min_ms,p50_ms,p95_ms,p99_ms,max_ms" >"$OUT/results.csv"
echo "server,single_worker,persistence,kind,keys,value_size,dbsize,rss_idle_kb,rss_loaded_kb,rss_after_overwrite_kb,rss_after_flush_kb" >"$OUT/memory.csv"
case $(uname -s) in
  Linux) ;;
  *) echo "# WARNING: not Linux — no CPU pinning, no governor control; treat results as indicative." | tee -a "$OUT/conditions.txt" ;;
esac

port=$PORT_BASE
for s in $SERVERS; do
  echo "### $s"
  start_server "$s" "$port"
  echo "rss_idle_kb_at_start.$s: $(rss_kb "$SERVER_PID")" >>"$OUT/conditions.txt"
  [[ ${SKIP_THROUGHPUT:-0} == 1 ]] || throughput_matrix "$s" "$port"
  [[ ${SKIP_MEMORY:-0} == 1 ]] || memory_matrix "$s" "$port"
  stop_server
  port=$((port + 10))
done

echo "# results: $OUT/results.csv  $OUT/memory.csv  $OUT/conditions.txt"
echo "# summary: python3 scripts/bench-summarize.py $OUT"
