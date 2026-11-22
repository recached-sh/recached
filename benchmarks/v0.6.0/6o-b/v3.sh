#!/bin/bash
# V3: (a) precision re-run of the two cells whose V1 duration was short of the
# 20 s floor or whose ratio sat within one 250 ms step of 0.90: LPUSH and MSET,
# c50 P=1, ~30 s per cell, 3 interleaved reps; (b) user-space samples of the
# merged build (profiling binary of d5563d7), as the B1 samples were taken.
set -u
B=$HOME/recached-6ob-bench; KIT=$B/kit-v; source $KIT/v.env
log() { echo "$(date -u +%FT%TZ) $*" >> $B/v-driver.log; }
until awk '{exit !($1<1.0)}' /proc/loadavg; do sleep 20; done
log "V3a start: load $(cut -d' ' -f1 /proc/loadavg)"
cells=""; for t in lpush mset; do cells+="base/rc-noaof-$t-c50-P1 new/rc-noaof-$t-c50-P1 redis-noaof-$t-c50-P1 valkey-noaof-$t-c50-P1 "; done
env RC_BINS="base=$B/bin/rc-base new=$B/bin/rc-new" RC_SHAS="base=51ae15c97ff19d3e7390c05f9655afead770e24f new=$NEW_SHA" UPROF=$B/bin/uprof \
  RC_RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)" REQ_C50_P1=6000000 REQ_SCALE_mset=0.5 \
  OUT=$B/results-v3-precision REPS=3 ALTERNATE=1 MAX_LOAD=1.0 CELLS="$cells" bash $KIT/run-6ob.sh > $B/v3a.console 2>&1; log "V3a exit $?"
until awk '{exit !($1<1.0)}' /proc/loadavg; do sleep 20; done
log "V3b start: load $(cut -d' ' -f1 /proc/loadavg)"
pc="prof/rc-noaof-set-c50-P1 prof/rc-noaof-lpush-c50-P1 prof/rc-noaof-sadd-c50-P1 prof/rc-noaof-mset-c50-P1"
env RC_BINS="prof=$B/bin/rc-new-prof" RC_SHAS="prof=$NEW_SHA" UPROF=$B/bin/uprof RC_RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)" \
  REQ_C50_P1=1000000 OUT=$B/results-v3-samples REPS=1 MAX_LOAD=1.0 CELLS="$pc" RECORD_CELLS="$pc" bash $KIT/run-6ob.sh > $B/v3b.console 2>&1; log "V3b exit $?"
# (c) multi-worker control: 3 workers on CPUs 0-2, base vs new vs new with
# RECACHED_REPLY_COALESCE=off (same binary), longer cells and 3 reps: does the
# yield cost CPU for a lone connection when other workers are idle (V2 c1 P=1)?
until awk '{exit !($1<1.0)}' /proc/loadavg; do sleep 20; done
log "V3c start: load $(cut -d' ' -f1 /proc/loadavg)"
mc=""; for c in set-c1-P1 set-c50-P1 set-c50-P16; do mc+="base/rc-noaof-$c new/rc-noaof-$c off/rc-noaof-$c "; done
env RC_BINS="base=$B/bin/rc-base new=$B/bin/rc-new off=$B/bin/rc-new" RC_SHAS="base=51ae15c97ff19d3e7390c05f9655afead770e24f new=$NEW_SHA off=$NEW_SHA" UPROF=$B/bin/uprof \
  RC_RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)" REQ_C1_P1=1200000 REQ_C50_P1=9000000 REQ_C50_P16=60000000 \
  LABEL_ENV_base="RECACHED_WORKER_THREADS=3" LABEL_ENV_new="RECACHED_WORKER_THREADS=3" LABEL_ENV_off="RECACHED_WORKER_THREADS=3 RECACHED_REPLY_COALESCE=off" \
  SERVER_PIN=0-2 BENCH_THREADS=3 OUT=$B/results-v3-mw REPS=3 ALTERNATE=1 MAX_LOAD=1.0 CELLS="$mc" bash $KIT/run-6ob.sh > $B/v3c.console 2>&1; log "V3c exit $?"
