#!/bin/bash
# 6O-H native validation driver. Launch: flatpak-spawn --host bash ~/recached-6oh-bench/h-driver.sh
# STAGE=cmds|aof|always|all
set -u
B=$HOME/recached-6oh-bench; KIT=$B/kit
log() { echo "$(date -u +%FT%TZ) $*" >> $B/h-driver.log; }
quiet() { until awk '{exit !($1<1.5)}' /proc/loadavg && ! ps -eo comm | grep -qE '^(cc1|rustc|cargo|ld|recached-server)$'; do sleep 20; done; }
STAGE=${STAGE:-all}
BINS="base=$B/bin/rc-base h=$B/bin/rc-h"
SHAS="base=${SHA_BASE:-f9a364dd1634203e7c1de65550940fe4f9ca9246} h=${SHA_H:-unknown}"
if [[ $STAGE == all || $STAGE == cmds ]]; then
  c=""
  for t in get set incr hset lpush mset; do for l in base h; do c+="$l/rc-noaof-$t-c50-P1 "; done; done
  for l in base h; do c+="$l/rc-noaof-get-c50-P16 $l/rc-noaof-incr-c50-P16 $l/rc-noaof-mget-c50-P16 "; done
  quiet; log "cmds start"
  env RC_BINS="$BINS" RC_SHAS="$SHAS" MAX_LOAD=1.5 OUT=$B/results-cmds REQ_C50_P1=4000000 REQ_C50_P16=20000000 \
    REQ_SCALE_mset=0.5 REQ_SCALE_mget=0.5 REPS=5 ALTERNATE=1 CELLS="$c" bash $KIT/run-6od.sh > $B/cmds.console 2>&1; log "cmds exit $?"
fi
if [[ $STAGE == all || $STAGE == aof ]]; then
  c=""
  for t in incr set hset setpx100 setpx1000 incrvol; do for l in base h; do c+="$l/rc-everysec-$t-c50-P1 "; done; done
  for l in base h; do c+="$l/rc-noaof-setpx100-c50-P1 $l/rc-noaof-incrvol-c50-P1 "; done
  quiet; log "aof start"
  env RC_BINS="$BINS" RC_SHAS="$SHAS" MAX_LOAD=1.5 OUT=$B/results-aof REQ_C50_P1=3000000 \
    REPS=5 ALTERNATE=1 CELLS="$c" bash $KIT/run-6od.sh > $B/aof.console 2>&1; log "aof exit $?"
fi
if [[ $STAGE == all || $STAGE == always ]]; then
  c=""; for l in base h; do c+="$l/rc-always-set-c50-P1 $l/rc-always-incrvol-c50-P1 "; done
  quiet; log "always start"
  env RC_BINS="$BINS" RC_SHAS="$SHAS" MAX_LOAD=1.5 OUT=$B/results-always ALWAYS_REQUESTS=50000 \
    REPS=5 ALTERNATE=1 CELLS="$c" bash $KIT/run-6od.sh > $B/always.console 2>&1; log "always exit $?"
fi
log "h driver ($STAGE) done"
