#!/bin/bash
# 6O-C1 validation: base (6O-B, d5563d7) vs c1 (a63c024) vs c1 with RECACHED_REPLY_COALESCE=off,
# ~25-30 s cells, 3 interleaved reps (ALTERNATE), then diagnostic counters (1 rep, never throughput).
set -u
B=$HOME/recached-6oc-bench; KIT=$B/kit
BASE_SHA=d5563d78034f3ccccc91975051de2cb6099b76ba; C1_SHA=a63c024e94fb28edd86df950297a86cc1ba03d1a
RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)"
log() { echo "$(date -u +%FT%TZ) $*" >> $B/driver.log; }
quiet() { until awk '{exit !($1<1.0)}' /proc/loadavg && ! pgrep -x 'cc1|rustc|updatedb|cargo' >/dev/null; do sleep 20; done; }
quiet; log "C1a (1 worker) start: load $(cut -d' ' -f1 /proc/loadavg)"
cells=""; for t in set-c50-P1 set-c50-P16 mset-c50-P16; do cells+="base/rc-noaof-$t new/rc-noaof-$t off/rc-noaof-$t redis-noaof-$t "; done
env RC_BINS="base=$B/bin/rc-base new=$B/bin/rc-c1 off=$B/bin/rc-c1" RC_SHAS="base=$BASE_SHA new=$C1_SHA off=$C1_SHA" UPROF=$B/bin/uprof RC_RUSTC="$RUSTC"   REQ_C50_P1=5000000 REQ_C50_P16=30000000 REQ_SCALE_mset=0.2 LABEL_ENV_off="RECACHED_REPLY_COALESCE=off"   OUT=$B/results-c1-1w REPS=3 ALTERNATE=1 MAX_LOAD=1.0 CELLS="$cells" bash $KIT/run-6oc.sh > $B/c1a.console 2>&1; log "C1a exit $?"
quiet; log "C1b (3 workers) start: load $(cut -d' ' -f1 /proc/loadavg)"
mc=""; for c in set-c1-P1 set-c50-P1 set-c50-P16; do mc+="base/rc-noaof-$c new/rc-noaof-$c off/rc-noaof-$c "; done
env RC_BINS="base=$B/bin/rc-base new=$B/bin/rc-c1 off=$B/bin/rc-c1" RC_SHAS="base=$BASE_SHA new=$C1_SHA off=$C1_SHA" UPROF=$B/bin/uprof RC_RUSTC="$RUSTC"   REQ_C1_P1=900000 REQ_C50_P1=7500000 REQ_C50_P16=70000000   LABEL_ENV_base="RECACHED_WORKER_THREADS=3" LABEL_ENV_new="RECACHED_WORKER_THREADS=3" LABEL_ENV_off="RECACHED_WORKER_THREADS=3 RECACHED_REPLY_COALESCE=off"   SERVER_PIN=0-2 BENCH_THREADS=3 OUT=$B/results-c1-3w REPS=3 ALTERNATE=1 MAX_LOAD=1.0 CELLS="$mc" bash $KIT/run-6oc.sh > $B/c1b.console 2>&1; log "C1b exit $?"
quiet; log "C1c (diag counters, 1 worker) start"
dc=""; for t in set-c50-P1 set-c50-P16 mset-c50-P16; do dc+="bdiag/rc-noaof-$t cdiag/rc-noaof-$t "; done
env RC_BINS="bdiag=$B/bin/rc-basediag cdiag=$B/bin/rc-c1diag" RC_SHAS="bdiag=$BASE_SHA+diag cdiag=$C1_SHA+diag" RC_RUSTC="$RUSTC"   REQ_C50_P1=2000000 REQ_C50_P16=12000000 REQ_SCALE_mset=0.2 OUT=$B/results-c1-diag-1w REPS=1 MAX_LOAD=1.0 CELLS="$dc" bash $KIT/run-6oc.sh > $B/c1c.console 2>&1; log "C1c exit $?"
quiet; log "C1d (diag counters, 3 workers) start"
dc=""; for c in set-c1-P1 set-c50-P1 set-c50-P16; do dc+="bdiag/rc-noaof-$c cdiag/rc-noaof-$c "; done
env RC_BINS="bdiag=$B/bin/rc-basediag cdiag=$B/bin/rc-c1diag" RC_SHAS="bdiag=$BASE_SHA+diag cdiag=$C1_SHA+diag" RC_RUSTC="$RUSTC"   REQ_C1_P1=400000 REQ_C50_P1=3000000 REQ_C50_P16=30000000 LABEL_ENV_bdiag="RECACHED_WORKER_THREADS=3" LABEL_ENV_cdiag="RECACHED_WORKER_THREADS=3"   SERVER_PIN=0-2 BENCH_THREADS=3 OUT=$B/results-c1-diag-3w REPS=1 MAX_LOAD=1.0 CELLS="$dc" bash $KIT/run-6oc.sh > $B/c1d.console 2>&1; log "C1d exit $?"
log "C1 driver done"
