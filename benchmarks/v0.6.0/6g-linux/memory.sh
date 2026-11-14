#!/usr/bin/env bash
# Phase 6G-A memory matrix: Linux process RSS per key, fresh server process
# per scenario, for Recached builds, Redis and Valkey.
#
# Runs INSIDE recached-6g-bench:local (see run-memory.sh). Server pinned to
# SERVER_CPUS, loader to LOAD_CPUS. Every number is /proc/<pid>/status VmRSS or
# VmHWM of the server process — what the OS charges — never a server's own
# used-memory counter.
#
# Phases per scenario (RSS sampled at each):
#   idle         after start + 1 s
#   loaded       load N keys, + 2 s          (comparable to the 6E numbers)
#   settled      + SETTLE s more              (jemalloc's dirty-page decay is
#                                              10 s by default; this is the
#                                              steady state an operator sees)
#   peak         VmHWM after load             (high-water mark, incl. rehash)
#   overwritten  load the same N keys again, + SETTLE s
#   deleted      DEL every key, + 2 s
#   reclaimed    + RECLAIM s more             (only when RECLAIM > 0; Recached's
#                                              reclaim step visits one shard/s)
#   flushed      FLUSHDB, + 2 s
#
# Output CSV: one row per (server, scenario, rep).
set -euo pipefail

BINS=${BINS:-/bins}
OUT=${OUT:-/out}
SERVERS=${SERVERS:-"v051 dev opt redis valkey"}
REPS=${REPS:-3}
SETTLE=${SETTLE:-12}
SERVER_CPUS=${SERVER_CPUS:-0-3}
LOAD_CPUS=${LOAD_CPUS:-4-7}
WORKERS=${WORKERS:-4}
PORT=${PORT:-7600}
# kind:keys:value_bytes:reclaim_seconds
SCENARIOS=${SCENARIOS:-"string:1000000:32:40 ttl:1000000:32:0 hash:1000000:32:0 string:1000000:16:0 string:1000000:64:0 string:1000000:256:0 string:100000:1024:0 string:100000:4096:0 string:100000:32:0 string:10000:32:0"}

mkdir -p "$OUT"
csv=$OUT/memory.csv
[[ -s $csv ]] || echo "server,kind,keys,value_bytes,rep,dbsize,idle_kb,loaded_kb,settled_kb,peak_kb,overwritten_kb,deleted_kb,reclaimed_kb,flushed_kb" >"$csv"

status_kb() { awk -v f="$2:" '$1 == f {print $2}' "/proc/$1/status"; }
rss() { status_kb "$1" VmRSS; }
hwm() { status_kb "$1" VmHWM; }
cli() { redis-cli -p "$PORT" "$@"; }

start() {
  local name=$1 dir=/tmp/data-$1
  rm -rf "$dir"; mkdir -p "$dir"
  case $name in
    redis) taskset -c "$SERVER_CPUS" redis-server --port "$PORT" --bind 127.0.0.1 --save "" \
             --appendonly no --dir "$dir" --protected-mode no >"$OUT/server-$name.log" 2>&1 & ;;
    valkey) taskset -c "$SERVER_CPUS" valkey-server --port "$PORT" --bind 127.0.0.1 --save "" \
             --appendonly no --dir "$dir" --protected-mode no --io-threads 1 >"$OUT/server-$name.log" 2>&1 & ;;
    # Allocator variants of the optimized build (6G-D.2): `opt-bg` is the
    # same binary with jemalloc's background purging thread; `opt-sys` is
    # built without the jemalloc global allocator (glibc malloc).
    opt-bg) env _RJEM_MALLOC_CONF=background_thread:true RECACHED_BIND=127.0.0.1 RECACHED_PORT="$PORT" \
         RECACHED_WS_PORT=$((PORT + 1)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
         RECACHED_SAVE_PATH="$dir/dump.rdb" RECACHED_WORKER_THREADS="$WORKERS" \
         taskset -c "$SERVER_CPUS" "$BINS/recached-server-opt" >"$OUT/server-$name.log" 2>&1 & ;;
    *) env RECACHED_BIND=127.0.0.1 RECACHED_PORT="$PORT" RECACHED_WS_PORT=$((PORT + 1)) \
         RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$dir/dump.rdb" \
         RECACHED_WORKER_THREADS="$WORKERS" \
         taskset -c "$SERVER_CPUS" "$BINS/recached-server-$name" >"$OUT/server-$name.log" 2>&1 & ;;
  esac
  PID=$!
  for _ in $(seq 80); do cli ping 2>/dev/null | grep -q PONG && return 0; sleep 0.25; done
  echo "$name did not start" >&2; cat "$OUT/server-$name.log" >&2; exit 1
}

stop() { kill -9 "$PID" 2>/dev/null || true; wait "$PID" 2>/dev/null || true; sleep 0.5; }

# Pipelined loader, as scripts/bench-redis-valkey.sh: counts one reply line
# per command and aborts on any error reply rather than understate a dataset.
drive() {
  taskset -c "$LOAD_CPUS" python3 -I - "$PORT" "$@" <<'PY'
import socket, sys
port, op, n, size = int(sys.argv[1]), sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
val = b"x" * size
s = socket.create_connection(("127.0.0.1", port))
def enc(*parts):
    return b"*%d\r\n" % len(parts) + b"".join(b"$%d\r\n%s\r\n" % (len(p), p) for p in parts)
BATCH = 5000
for start in range(0, n, BATCH):
    out = []
    for i in range(start, min(n, start + BATCH)):
        k = b"mem:%010d" % i
        if op == "string":
            out.append(enc(b"SET", k, val))
        elif op == "ttl":
            out.append(enc(b"SET", k, val, b"EX", b"3600"))
        elif op == "hash":
            out.append(enc(b"HSET", k, b"f1", val, b"f2", val, b"f3", val))
        elif op == "del":
            out.append(enc(b"DEL", k))
        else:
            sys.exit("unknown op " + op)
    s.sendall(b"".join(out))
    want, got, buf = len(out), 0, b""
    while got < want:
        chunk = s.recv(1 << 16)
        if not chunk:
            sys.exit("server closed the connection")
        buf += chunk
        got = buf.count(b"\r\n")
    if b"\r\n-" in b"\r\n" + buf:
        sys.exit("error reply: " + buf.split(b"-", 1)[1][:200].decode(errors="replace"))
PY
}

for rep in $(seq "$REPS"); do
  for sc in $SCENARIOS; do
    IFS=: read -r kind keys size reclaim <<<"$sc"
    # Interleave servers within each rep so drift on the host spreads evenly.
    for name in $SERVERS; do
      start "$name"; sleep 1
      idle=$(rss "$PID")
      drive "$kind" "$keys" "$size"
      got=$(cli dbsize | tr -d '\r')
      [[ $got == "$keys" ]] || { echo "$name $sc: dbsize $got != $keys" >&2; exit 1; }
      sleep 2; loaded=$(rss "$PID")
      sleep "$SETTLE"; settled=$(rss "$PID"); peak=$(hwm "$PID")
      drive "$kind" "$keys" "$size"; sleep "$SETTLE"; overwritten=$(rss "$PID")
      drive del "$keys" 0
      got=$(cli dbsize | tr -d '\r')
      [[ $got == 0 ]] || { echo "$name $sc: dbsize $got after DEL" >&2; exit 1; }
      sleep 2; deleted=$(rss "$PID")
      reclaimed=
      if (( reclaim > 0 )); then sleep "$reclaim"; reclaimed=$(rss "$PID"); fi
      cli flushdb >/dev/null; sleep 2; flushed=$(rss "$PID")
      echo "$name,$kind,$keys,$size,$rep,$keys,$idle,$loaded,$settled,$peak,$overwritten,$deleted,$reclaimed,$flushed" | tee -a "$csv"
      stop
    done
  done
done
