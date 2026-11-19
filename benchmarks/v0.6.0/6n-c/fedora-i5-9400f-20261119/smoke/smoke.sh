#!/bin/sh
# Native smoke test of the M1 binary: a bad mode is refused; coalesced is reported.
B=$HOME/recached-6nc-bench/target-m1/release/recached-server
S=$HOME/recached-6nc-bench/smoke-1
cd "$S" || exit 1
RECACHED_AOF_FANOUT=watermark RECACHED_BIND=127.0.0.1 RECACHED_PORT=6890 RECACHED_WS_PORT=6891 RECACHED_METRICS_PORT=0 \
  RECACHED_AOF_PATH="$S/a.aof" RECACHED_SAVE_PATH="$S/d.rdb" timeout 5 "$B" >"$S/bad.log" 2>&1
echo "bad_exit=$?"; tail -1 "$S/bad.log"
RECACHED_AOF_FANOUT=coalesced RECACHED_BIND=127.0.0.1 RECACHED_PORT=6890 RECACHED_WS_PORT=6891 RECACHED_METRICS_PORT=0 \
  RECACHED_AOF_PATH="$S/a.aof" RECACHED_SAVE_PATH="$S/d.rdb" RECACHED_SAVE_INTERVAL=0 "$B" >"$S/ok.log" 2>&1 &
p=$!
sleep 1.5
"$HOME/recached-6l-bench/bin/redis-cli" -p 6890 info persistence | tr -d '\r' | grep -E 'aof_enabled|recached_aof_fanout'
kill "$p"; wait "$p" 2>/dev/null
cat /proc/loadavg
