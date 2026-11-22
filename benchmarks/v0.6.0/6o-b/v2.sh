#!/bin/bash
# V2 only (retry): multi-worker sanity, 3 workers on CPUs 0-2, 2 reps.
set -u
B=$HOME/recached-6ob-bench; KIT=$B/kit-v; source $KIT/v.env
log() { echo "$(date -u +%FT%TZ) $*" >> $B/v-driver.log; }
until awk '{exit !($1<1.0)}' /proc/loadavg; do sleep 20; done
log "V2 retry (native): load $(cut -d' ' -f1 /proc/loadavg)"
env RC_BINS="base=$B/bin/rc-base new=$B/bin/rc-new" RC_SHAS="base=51ae15c97ff19d3e7390c05f9655afead770e24f new=$NEW_SHA" UPROF=$B/bin/uprof \
  RC_RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)" REQ_C1_P1=600000 REQ_C1_P16=8000000 REQ_C50_P1=3000000 REQ_C50_P16=20000000 \
  REQ_SCALE_mset=0.3 REQ_SCALE_zadd=0.6 LABEL_ENV_base="RECACHED_WORKER_THREADS=3" LABEL_ENV_new="RECACHED_WORKER_THREADS=3" SERVER_PIN=0-2 BENCH_THREADS=3 \
  OUT=$B/results-v2-mw REPS=2 ALTERNATE=1 MAX_LOAD=1.0 \
  CELLS="base/rc-noaof-set-c50-P1 new/rc-noaof-set-c50-P1 base/rc-noaof-set-c50-P16 new/rc-noaof-set-c50-P16 base/rc-noaof-lpush-c50-P1 new/rc-noaof-lpush-c50-P1 base/rc-noaof-set-c1-P1 new/rc-noaof-set-c1-P1" \
  bash $KIT/run-6ob.sh > $B/v2.console 2>&1; log "V2 exit $?"
