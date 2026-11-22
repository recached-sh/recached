#!/usr/bin/env bash
# 6O-B B0 native baseline. Waits for a quiet host, then runs the no-persistence
# matrix (SET LPUSH SADD MSET x c1/c50 x P1/P16) and the GET/INCR/ZADD c50 P=1
# controls against standalone Redis 7.2.5 and Valkey 9.1.0, 3 interleaved
# repetitions (ALTERNATE=1). KIT is a frozen copy of this directory.
set -uo pipefail
B=$HOME/recached-6ob-bench KIT=${KIT:-$B/kit-b0}
log() { echo "$(date -u +%FT%TZ) $*"; }
quiet() {
  local l io; l=$(cut -d' ' -f1 /proc/loadavg); io=$(awk '/^some/ { split($2, a, "="); print a[2] }' /proc/pressure/io)
  ! pgrep -x updatedb >/dev/null && ! pgrep -x cc1 >/dev/null && ! pgrep -x rustc >/dev/null &&
    awk -v l="$l" -v io="$io" 'BEGIN { exit !(l < 1.0 && io < 5) }'
}
n=0; until quiet; do (( n++ % 30 == 0 )) && log "waiting: load $(cut -d' ' -f1 /proc/loadavg) io $(head -1 /proc/pressure/io)"; sleep 10; done
log "quiet; settling 120 s"; sleep 120
quiet || { log "not quiet after settling; waiting again"; until quiet; do sleep 10; done; }
SHA=51ae15c97ff19d3e7390c05f9655afead770e24f
cells=""
for t in set lpush sadd mset; do for c in 50 1; do for p in 1 16; do
  cells+="base/rc-noaof-$t-c$c-P$p redis-noaof-$t-c$c-P$p valkey-noaof-$t-c$c-P$p "
done; done; done
for t in get incr zadd; do cells+="base/rc-noaof-$t-c50-P1 redis-noaof-$t-c50-P1 valkey-noaof-$t-c50-P1 "; done
log "B0 start"
env RC_BINS="base=$B/bin/rc-base" RC_SHAS="base=$SHA" UPROF=$B/bin/uprof RC_RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)" \
  OUT=$B/results-b0 REPS=3 ALTERNATE=1 MAX_LOAD=1.5 CELLS="$cells" bash $KIT/run-6ob.sh > $B/b0.console 2>&1
log "B0 exit $?"
