#!/usr/bin/env bash
# 6J-E F1 check against a real binary: with RECACHED_PASSWORD set and no sync
# secret, does a WebSocket that never sends AUTH receive pushes? Prints
# LEAK <frames> or NOLEAK per server. Runs inside recached-6i-bench:local.
set -euo pipefail
for name in ${SERVERS:-6i}; do
  env RECACHED_BIND=127.0.0.1 RECACHED_PORT=7710 RECACHED_WS_PORT=7711 RECACHED_METRICS_PORT=0 \
    RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH=/tmp/noauth.rdb RECACHED_PASSWORD=pw \
    "/bins/recached-server-$name" >/tmp/noauth-$name.log 2>&1 &
  pid=$!
  for _ in $(seq 60); do redis-cli -p 7710 -a pw --no-auth-warning ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
  node -e '
    const ws = new WebSocket("ws://127.0.0.1:7711"); const got = [];
    ws.onmessage = (e) => got.push(String(e.data).replace(/\r\n/g, " "));
    ws.onopen = () => setTimeout(() => {
      require("child_process").execSync("redis-cli -p 7710 -a pw --no-auth-warning SET secret s3cr3t");
      setTimeout(() => { console.log(process.argv[1] + ": " + (got.length ? "LEAK " + JSON.stringify(got) : "NOLEAK")); process.exit(0); }, 500);
    }, 300);' "$name"
  kill -9 $pid 2>/dev/null || true; wait $pid 2>/dev/null || true
done
