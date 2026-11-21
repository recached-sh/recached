#!/usr/bin/env bash
# 6O-A A0 + A1 native runs. Waits for a quiet host, then:
#   A0: baseline vs standalone Redis/Valkey, SET P=1/P=16, INCR, HSET (3 reps)
#   A1a: AOF policy ablation, SET P=1 (3 reps)
#   A1b: LBR samples on the profiling build (1 rep)
set -uo pipefail
B=$HOME/recached-6oa-bench KIT=$HOME/Documents/OpenSource/recached-6oa-profile/benchmarks/v0.6.0/6o-a
log() { echo "$(date -u +%FT%TZ) $*"; }
quiet() {
  local l io; l=$(cut -d' ' -f1 /proc/loadavg); io=$(awk '/^some/ { split($2, a, "="); print a[2] }' /proc/pressure/io)
  ! pgrep -x updatedb >/dev/null && ! pgrep -x cc1 >/dev/null && ! pgrep -x rustc >/dev/null &&
    awk -v l="$l" -v io="$io" 'BEGIN { exit !(l < 1.0 && io < 5) }'
}
n=0; until quiet; do (( n++ % 30 == 0 )) && log "waiting: load $(cut -d' ' -f1 /proc/loadavg) io $(head -1 /proc/pressure/io)"; sleep 10; done
log "quiet; settling 120 s"; sleep 120
quiet || { log "not quiet after settling; waiting again"; until quiet; do sleep 10; done; }
SHA=a697f54f53ef061e2456972367433134fabde576
common=(RC_BINS="base=$B/bin/rc-base prof=$B/bin/rc-base-prof" RC_SHAS="base=$SHA prof=$SHA" UPROF=$B/bin/uprof RC_RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)")
log "A0 start"
env "${common[@]}" OUT=$B/results-a0 REPS=3 CELLS="base/rc-everysec-set-P1 redis-set-P1 valkey-set-P1 base/rc-everysec-incr-P1 redis-incr-P1 valkey-incr-P1 base/rc-everysec-hset-P1 redis-hset-P1 valkey-hset-P1 base/rc-everysec-set-P16 redis-set-P16 valkey-set-P16" \
  bash $KIT/run-6oa.sh > $B/a0.console 2>&1; log "A0 exit $?"
log "A1a start"
env "${common[@]}" OUT=$B/results-a1-ablation REPS=3 MAX_LOAD=1.5 CELLS="base/rc-noaof-set-P1 redis-noaof-set-P1 base/rc-aofno-set-P1 redis-aofno-set-P1 base/rc-everysec-set-P1 redis-set-P1 base/rc-always-set-P1 redis-always-set-P1" \
  bash $KIT/run-6oa.sh > $B/a1a.console 2>&1; log "A1a exit $?"
log "A1b start"
env "${common[@]}" OUT=$B/results-a1-samples REPS=1 MAX_LOAD=1.5 REQUESTS_P1=1000000 REQUESTS_P16=5000000 \
  CELLS="prof/rc-everysec-set-P1 prof/rc-noaof-set-P1 prof/rc-everysec-incr-P1 prof/rc-everysec-hset-P1 prof/rc-everysec-set-P16" \
  RECORD_CELLS="prof/rc-everysec-set-P1 prof/rc-noaof-set-P1 prof/rc-everysec-incr-P1 prof/rc-everysec-hset-P1 prof/rc-everysec-set-P16" \
  bash $KIT/run-6oa.sh > $B/a1b.console 2>&1; log "A1b exit $?"
log "all done"
