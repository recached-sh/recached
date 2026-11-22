#!/usr/bin/env bash
# 6O-B validation, native. NEW_SHA / binaries set by the caller's environment
# file. V1: primary matrix + controls + AOF carryover, 3 interleaved reps.
# V2: multi-worker sanity (3 workers on CPUs 0-2), 2 reps.
set -uo pipefail
B=$HOME/recached-6ob-bench KIT=$B/kit-v
source $B/kit-v/v.env
log() { echo "$(date -u +%FT%TZ) $*"; }
quiet() {
  local l io; l=$(cut -d' ' -f1 /proc/loadavg); io=$(awk '/^some/ { split($2, a, "="); print a[2] }' /proc/pressure/io)
  ! pgrep -x updatedb >/dev/null && ! pgrep -x cc1 >/dev/null && ! pgrep -x rustc >/dev/null &&
    awk -v l="$l" -v io="$io" 'BEGIN { exit !(l < 1.0 && io < 5) }'
}
n=0; until quiet; do (( n++ % 30 == 0 )) && log "waiting: load $(cut -d' ' -f1 /proc/loadavg)"; sleep 10; done
log "quiet; settling 120 s"; sleep 120
cells=""
for t in set lpush sadd mset; do
  for cp in c50-P1 c50-P16; do cells+="base/rc-noaof-$t-$cp new/rc-noaof-$t-$cp redis-noaof-$t-$cp valkey-noaof-$t-$cp "; done
  for cp in c1-P1 c1-P16; do cells+="base/rc-noaof-$t-$cp new/rc-noaof-$t-$cp redis-noaof-$t-$cp "; done
done
for t in get incr zadd; do cells+="base/rc-noaof-$t-c50-P1 new/rc-noaof-$t-c50-P1 redis-noaof-$t-c50-P1 "; done
for c in set-c50-P1 incr-c50-P1 hset-c50-P1 set-c50-P16; do cells+="base/rc-everysec-$c new/rc-everysec-$c redis-$c "; done
common=(RC_BINS="base=$B/bin/rc-base new=$B/bin/rc-new" RC_SHAS="base=51ae15c97ff19d3e7390c05f9655afead770e24f new=$NEW_SHA" UPROF=$B/bin/uprof
  RC_RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)" REQ_C1_P1=600000 REQ_C1_P16=8000000 REQ_C50_P1=3000000 REQ_C50_P16=20000000
  REQ_SCALE_mset=0.3 REQ_SCALE_zadd=0.6)
log "V1 start"
env "${common[@]}" OUT=$B/results-v1 REPS=3 ALTERNATE=1 MAX_LOAD=1.5 CELLS="$cells" bash $KIT/run-6ob.sh > $B/v1.console 2>&1; log "V1 exit $?"
mw="base/rc-noaof-set-c50-P1 new/rc-noaof-set-c50-P1 base/rc-noaof-set-c50-P16 new/rc-noaof-set-c50-P16 base/rc-noaof-lpush-c50-P1 new/rc-noaof-lpush-c50-P1 base/rc-noaof-set-c1-P1 new/rc-noaof-set-c1-P1"
log "V2 start"
env "${common[@]}" LABEL_ENV_base="RECACHED_WORKER_THREADS=3" LABEL_ENV_new="RECACHED_WORKER_THREADS=3" SERVER_PIN=0-2 BENCH_THREADS=3 \
  OUT=$B/results-v2-mw REPS=2 ALTERNATE=1 MAX_LOAD=2 CELLS="$mw" bash $KIT/run-6ob.sh > $B/v2.console 2>&1; log "V2 exit $?"
