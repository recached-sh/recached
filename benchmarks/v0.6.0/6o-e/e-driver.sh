#!/bin/bash
# 6O-E performance: rc-base (1f9232f), rc-e1 (dev 0f2f79c: EXEC isolation only), rc-fix (final PR 4 head:
# E1+E2+E3), Redis 7.2.5. Same kit, host and methodology as 6O-D (interleaved reps, ~30 s P=1 cells).
# Launch natively: flatpak-spawn --host bash ~/recached-6oe-bench/e-driver.sh
set -u
B=$HOME/recached-6oe-bench; KIT=$B/kit
BASE=1f9232f44806adace36a6a348e33b3208650f411; E1=$(git -C $B/src-e1 rev-parse HEAD); FIX=$(git -C $B/src-fix rev-parse HEAD)
RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)"
log() { echo "$(date -u +%FT%TZ) $*" >> $B/driver.log; }
quiet() { until awk '{exit !($1<1.0)}' /proc/loadavg && ! pgrep -x 'cc1|rustc|updatedb|cargo' >/dev/null; do sleep 20; done; }
run() { local name=$1; shift; quiet; log "$name start: load $(cut -d' ' -f1 /proc/loadavg)"; env "$@" RC_RUSTC="$RUSTC" UPROF=$B/bin/uprof MAX_LOAD=1.0 OUT=$B/results-$name bash $KIT/run-6od.sh > $B/$name.console 2>&1; log "$name exit $?"; }
BINS="base=$B/bin/rc-base e1=$B/bin/rc-e1 fix=$B/bin/rc-fix"; SHAS="base=$BASE e1=$E1 fix=$FIX"
STAGE=${STAGE:-all}
if [[ $STAGE == all || $STAGE == tx ]]; then
  quiet; log "tx start: load $(cut -d' ' -f1 /proc/loadavg)"
  LABELS="base e1 fix" REPS=3 SECS=10 OUT=$B/results-tx CELLS="1:get-get:unrelated:1:0 1:mget-get:unrelated:1:0 1:get-get:unrelated:50:0 3:get-get:unrelated:50:0 1:mget-get:unrelated:50:0 3:mget-get:unrelated:50:0 1:exists-get:unrelated:50:0 3:exists-get:unrelated:50:0 1:get-set:unrelated:50:0 1:mget-mset:unrelated:50:0 1:watch:unrelated:50:0 3:get-get:same:50:4 3:get-get:overlap:50:4 3:get-get:unrelated:50:4" \
    bash $B/tx.sh > $B/tx.console 2>&1; log "tx exit $?"
fi
if [[ $STAGE == all || $STAGE == primary ]]; then
  c=""; for t in get mget; do c+="base/rc-noaof-$t-c50-P1 e1/rc-noaof-$t-c50-P1 fix/rc-noaof-$t-c50-P1 redis-noaof-$t-c50-P1 "; done
  for t in set mset lpush; do c+="base/rc-noaof-$t-c50-P1 fix/rc-noaof-$t-c50-P1 redis-noaof-$t-c50-P1 "; done
  run e-primary RC_BINS="$BINS" RC_SHAS="$SHAS" REQ_C50_P1=6000000 REQ_SCALE_mset=0.5 REQ_SCALE_mget=0.5 REPS=5 ALTERNATE=1 CELLS="$c"
fi
if [[ $STAGE == all || $STAGE == p16 ]]; then
  c=""; for t in get mget; do c+="base/rc-noaof-$t-c50-P16 e1/rc-noaof-$t-c50-P16 fix/rc-noaof-$t-c50-P16 redis-noaof-$t-c50-P16 "; done
  run e-p16 RC_BINS="$BINS" RC_SHAS="$SHAS" REQ_C50_P16=25000000 REQ_SCALE_mget=0.2 REPS=3 ALTERNATE=1 CELLS="$c"
fi
if [[ $STAGE == all || $STAGE == 3w ]]; then
  c=""; for t in get-c50-P1 mget-c50-P1 get-c50-P16 mget-c50-P16; do c+="base/rc-noaof-$t fix/rc-noaof-$t "; done
  run e-3w RC_BINS="$BINS" RC_SHAS="$SHAS" REQ_C50_P1=9000000 REQ_C50_P16=70000000 REQ_SCALE_mget=0.4 SERVER_PIN=0-2 BENCH_THREADS=3 \
    LABEL_ENV_base="RECACHED_WORKER_THREADS=3" LABEL_ENV_fix="RECACHED_WORKER_THREADS=3" REPS=3 ALTERNATE=1 CELLS="$c"
fi
if [[ $STAGE == all || $STAGE == aof ]]; then
  # AOF everysec: ordered writes, which now fence before checking for watchers (6O-E2 review round 2)
  c=""; for t in set-c50-P1 mset-c50-P1; do c+="base/rc-everysec-$t fix/rc-everysec-$t "; done
  run e-aof RC_BINS="$BINS" RC_SHAS="$SHAS" REQ_C50_P1=4000000 REQ_SCALE_mset=0.5 REPS=3 ALTERNATE=1 CELLS="$c"
fi
log "e driver ($STAGE) done"
