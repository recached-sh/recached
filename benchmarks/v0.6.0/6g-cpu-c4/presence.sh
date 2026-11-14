#!/usr/bin/env bash
# C4's algorithmic case: with N ephemeral (ESET) claims held open, every
# write used to scan all N under two global mutexes. Holds N claims on one
# connection, then measures pipelined SET on unrelated keys. A/B/B/A.
set -uo pipefail
A=../../../target/6g/bins/server-x5
B=../../../target/6g/bins/server-c4
CLAIMS=${CLAIMS:-20000}
echo "build,claims,set_rps"
for which in A B B A; do
  bin=${!which}
  RECACHED_PORT=7861 RECACHED_WS_PORT=7862 RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
    RECACHED_SAVE_PATH=/tmp/6g-presence.rdb RECACHED_WORKER_THREADS=4 "$bin" >/dev/null 2>&1 &
  pid=$!
  for _ in $(seq 120); do redis-cli -p 7861 ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
  python3 -I -c '
import socket, sys, time
n = int(sys.argv[1]); s = socket.create_connection(("127.0.0.1", 7861))
s.sendall(b"".join(b"*3\r\n$4\r\nESET\r\n$%d\r\npresence:%d\r\n$1\r\n1\r\n" % (len(b"presence:%d" % i), i) for i in range(n)))
buf = b""
while buf.count(b"\r\n") < n: buf += s.recv(1 << 16)
print("claims held", flush=True); time.sleep(600)' "$CLAIMS" > /tmp/6g-presence-holder.log 2>&1 &
  holder=$!
  until grep -q 'claims held' /tmp/6g-presence-holder.log 2>/dev/null; do sleep 0.2; done
  rps=$(redis-benchmark -p 7861 -t set -n 300000 -P 16 -c 50 -r 100000 -q 2>/dev/null | tr '\r' '\n' | awk '/requests per second/{print $2}' | tail -1)
  echo "$which,$CLAIMS,$rps"
  kill $holder $pid; wait $pid 2>/dev/null; sleep 0.5
done
