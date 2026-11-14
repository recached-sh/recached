#!/usr/bin/env bash
# Server-level small-hash RSS timeline under different jemalloc settings.
#
#   run.sh <server-bin> <label> <keys> [MALLOC_CONF]
#
# Fresh server per call, no persistence. Prints one CSV line per sample:
#   label,keys,conf,phase,seconds_after_phase,rss_kb,footprint_kb,vm_dirty_kb,vm_reclaimable_kb
set -euo pipefail
BIN=$1 LABEL=$2 KEYS=$3 CONF=${4:-}
PORT=${PORT:-7950}
DIR=$(mktemp -d)
HERE=$(cd "$(dirname "$0")" && pwd)

env -i HOME="$HOME" PATH=/usr/bin:/bin RECACHED_BIND=127.0.0.1 RECACHED_PORT=$PORT \
  RECACHED_WS_PORT=$((PORT + 1)) RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
  RECACHED_SAVE_PATH="$DIR/dump.rdb" ${CONF:+_RJEM_MALLOC_CONF=$CONF} \
  "$BIN" >"$DIR/server.log" 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null || true; rm -rf "$DIR"' EXIT
for _ in $(seq 1 100); do nc -z 127.0.0.1 $PORT 2>/dev/null && break; sleep 0.1; done

rss() { ps -o rss= -p "$PID" | tr -d ' '; }
tokb() { awk -v v="$1" -v u="$2" 'BEGIN { if (u ~ /^G/) v*=1048576; else if (u ~ /^M/) v*=1024; else if (u == "B") v/=1024; printf "%d", v }'; }
fp() {
  # Total phys_footprint, plus the VM_ALLOCATE region (jemalloc's mappings):
  # dirty and reclaimable (MADV_FREE'd, returned on pressure), all in KB.
  local out; out=$(footprint "$PID" 2>/dev/null)
  local total dirty recl
  total=$(awk '{for(i=1;i<=NF;i++) if ($i=="Footprint:") print $(i+1), $(i+2)}' <<<"$out" | head -1)
  read -r dirty recl < <(awk '/untagged \(VM_ALLOCATE\)/ {print $1" "$2" "$5" "$6; exit}' <<<"$out" |
    { read -r a b c d; echo "$(tokb "$a" "$b") $(tokb "$c" "$d")"; })
  # shellcheck disable=SC2086
  echo "$(tokb $total),${dirty:-0},${recl:-0}"
}
sample() { # <phase> <seconds already waited>
  echo "$LABEL,$KEYS,${CONF:-default},$1,$2,$(rss),$(fp)"
}
timeline() { # <phase>: samples at +1, +5, +15, +30 s
  sleep 1; sample "$1" 1
  sleep 4; sample "$1" 5
  sleep 10; sample "$1" 15
  sleep 15; sample "$1" 30
}

sleep 1; sample idle 1
python3 -I "$HERE/load.py" $PORT "$KEYS" 32 hash; timeline loaded
for n in 1 2 3; do python3 -I "$HERE/load.py" $PORT "$KEYS" 32 hash; timeline "overwrite$n"; done
redis-cli -p $PORT FLUSHDB >/dev/null; timeline flushed
