#!/bin/bash
# 6O-F3 stripe study driver. Launch natively: flatpak-spawn --host bash ~/recached-6of-bench/f-driver.sh
# STAGE=screen|tx|barrier|cmds|all  (LABELS_TX overrides the tx labels after the screen)
set -u
B=$HOME/recached-6of-bench; KIT=$B/kit
log() { echo "$(date -u +%FT%TZ) $*" >> $B/f-driver.log; }
quiet() { until awk '{exit !($1<1.0)}' /proc/loadavg && ! ps -eo comm | grep -qE '^(cc1|rustc|cargo|ld|recached-server)$'; do sleep 20; done; }
STAGE=${STAGE:-all}
if [[ $STAGE == all || $STAGE == screen ]]; then
  quiet; log "screen start"
  LABELS="base x256 x512 x1024 x2048 x1024s" REPS=3 SECS=10 OUT=$B/results-screen \
    CELLS="3:get-get:unrelated:50:4" bash $B/stripes.sh > $B/screen.console 2>&1; log "screen exit $?"
fi
if [[ $STAGE == all || $STAGE == tx ]]; then
  quiet; log "tx start"
  LABELS=${LABELS_TX:-"base x256 x512 x1024 x2048 x1024s"} REPS=5 SECS=30 OUT=$B/results-tx \
    CELLS="3:get-get:unrelated:50:0 3:mget-get:unrelated:50:0 3:get-set:unrelated:50:0 3:get-get:same:50:4 3:get-get:overlap:50:4 3:get-get:unrelated:50:4 3:get-set:unrelated:50:4 3:get-get:unrelated:0:4 3:dbsize-get:unrelated:50:4" \
    bash $B/stripes.sh > $B/tx.console 2>&1; log "tx exit $?"
fi
if [[ $STAGE == all || $STAGE == barrier ]]; then
  quiet; log "barrier start"
  LABELS="x256 x512 x1024 x2048 x1024s" REPS=3 OUT=$B/results-barrier bash $B/barrier.sh > $B/barrier.console 2>&1; log "barrier exit $?"
fi
if [[ $STAGE == all || $STAGE == cmds ]]; then
  BINS="base=$B/bin/rc-base x256=$B/bin/rc-x256 x1024=$B/bin/rc-x1024 x1024s=$B/bin/rc-x1024s"
  SHAS="base=410235b11634ff9ef1fe48736332d31c71caf74c x256=410235b+exp6of x1024=410235b+exp6of x1024s=410235b+exp6of"
  c=""; for t in get set lpush mset; do for l in base x256 x1024 x1024s; do c+="$l/rc-noaof-$t-c50-P1 "; done; done
  for l in base x256 x1024 x1024s; do c+="$l/rc-noaof-get-c50-P16 $l/rc-everysec-set-c50-P1 "; done
  quiet; log "cmds start"
  env RC_BINS="$BINS" RC_SHAS="$SHAS" UPROF=$B/bin/uprof MAX_LOAD=1.0 OUT=$B/results-cmds REQ_C50_P1=6000000 REQ_C50_P16=25000000 \
    REQ_SCALE_mset=0.5 REPS=5 ALTERNATE=1 CELLS="$c" bash $KIT/run-6od.sh > $B/cmds.console 2>&1; log "cmds exit $?"
  # 3 workers: GET P=16 (all-stripe-free path) and SET with AOF everysec (ordered writes take server stripes)
  c=""; for l in base x256 x1024 x1024s; do c+="$l/rc-noaof-get-c50-P16 $l/rc-everysec-set-c50-P1 "; done
  quiet; log "cmds3w start"
  env RC_BINS="$BINS" RC_SHAS="$SHAS" UPROF=$B/bin/uprof MAX_LOAD=1.0 OUT=$B/results-cmds3w REQ_C50_P1=8000000 REQ_C50_P16=60000000 \
    SERVER_PIN=0-2 BENCH_THREADS=3 LABEL_ENV_base="RECACHED_WORKER_THREADS=3" LABEL_ENV_x256="RECACHED_WORKER_THREADS=3" \
    LABEL_ENV_x1024="RECACHED_WORKER_THREADS=3" LABEL_ENV_x1024s="RECACHED_WORKER_THREADS=3" REPS=3 ALTERNATE=1 CELLS="$c" \
    bash $KIT/run-6od.sh > $B/cmds3w.console 2>&1; log "cmds3w exit $?"
fi
log "f driver ($STAGE) done"
