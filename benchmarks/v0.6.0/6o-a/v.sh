#!/usr/bin/env bash
# 6O-A validation (PR 4): base (dev a697f54) vs opt (PR 2) vs standalone Redis/Valkey.
#   V1: P=1 SET/INCR/HSET, 5 reps, order reversed on even reps
#   V2: SET P=16, 3 reps, alternating
#   V3: no-AOF control, SET P=1, 3 reps, alternating
#   V4: opt + base counters with uprof on the profiling builds' samples (1 rep)
set -uo pipefail
B=$HOME/recached-6oa-bench KIT=$HOME/Documents/OpenSource/recached-6oa-profile/benchmarks/v0.6.0/6o-a
log() { echo "$(date -u +%FT%TZ) $*"; }
quiet() {
  local l io; l=$(cut -d' ' -f1 /proc/loadavg); io=$(awk '/^some/ { split($2, a, "="); print a[2] }' /proc/pressure/io)
  ! pgrep -x updatedb >/dev/null && ! pgrep -x cc1 >/dev/null && ! pgrep -x rustc >/dev/null &&
    awk -v l="$l" -v io="$io" 'BEGIN { exit !(l < 1.0 && io < 5) }'
}
waitq() { local n=0; until quiet; do (( n++ % 30 == 0 )) && log "waiting: load $(cut -d' ' -f1 /proc/loadavg) io $(head -1 /proc/pressure/io)"; sleep 10; done; }
waitq; log "quiet; settling 120 s"; sleep 120; waitq
BASE=a697f54f53ef061e2456972367433134fabde576 OPT=$(cat $B/opt.sha)
common=(RC_BINS="base=$B/bin/rc-base opt=$B/bin/rc-opt baseprof=$B/bin/rc-base-prof optprof=$B/bin/rc-opt-prof"
  RC_SHAS="base=$BASE opt=$OPT baseprof=$BASE optprof=$OPT" UPROF=$B/bin/uprof RC_RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)"
  ALTERNATE=1 MAX_LOAD=8)
log "V1 start"
env "${common[@]}" OUT=$B/results-v1 REPS=5 CELLS="base/rc-everysec-set-P1 opt/rc-everysec-set-P1 redis-set-P1 valkey-set-P1 base/rc-everysec-incr-P1 opt/rc-everysec-incr-P1 redis-incr-P1 valkey-incr-P1 base/rc-everysec-hset-P1 opt/rc-everysec-hset-P1 redis-hset-P1 valkey-hset-P1" \
  bash $KIT/run-6oa.sh > $B/v1.console 2>&1; log "V1 exit $?"
log "V2 start"
env "${common[@]}" OUT=$B/results-v2 REPS=3 CELLS="base/rc-everysec-set-P16 opt/rc-everysec-set-P16 redis-set-P16 valkey-set-P16" \
  bash $KIT/run-6oa.sh > $B/v2.console 2>&1; log "V2 exit $?"
log "V3 start"
env "${common[@]}" OUT=$B/results-v3 REPS=3 CELLS="base/rc-noaof-set-P1 opt/rc-noaof-set-P1 redis-noaof-set-P1" \
  bash $KIT/run-6oa.sh > $B/v3.console 2>&1; log "V3 exit $?"
log "V4 start"
env "${common[@]}" OUT=$B/results-v4-samples REPS=1 REQUESTS_P1=1000000 REQUESTS_P16=5000000 \
  CELLS="baseprof/rc-everysec-set-P1 optprof/rc-everysec-set-P1 optprof/rc-everysec-hset-P1 optprof/rc-everysec-set-P16" \
  RECORD_CELLS="optprof/rc-everysec-set-P1 optprof/rc-everysec-hset-P1 optprof/rc-everysec-set-P16" \
  bash $KIT/run-6oa.sh > $B/v4.console 2>&1; log "V4 exit $?"
log "all done"
