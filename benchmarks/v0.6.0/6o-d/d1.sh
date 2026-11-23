#!/bin/bash
# 6O-D1: the MSET reader-atomicity fix (src-fix, 51aa372) against the 6O-C final code on dev
# (src-base, 69f0d1c) and Redis 7.2.5. Same kit, host and workload as 6O-C (~30 s cells, 5 interleaved
# reps for the primary gate). Read-side controls GET/MGET at P=1 and P=16, 1 and 3 workers.
set -u
B=$HOME/recached-6od-bench; KIT=$B/kit
BASE=69f0d1c814c24c0f3f75a789ec93752b060187e3; FIX=$(git -C $B/src-fix rev-parse HEAD)
RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)"
log() { echo "$(date -u +%FT%TZ) $*" >> $B/driver.log; }
quiet() { until awk '{exit !($1<1.0)}' /proc/loadavg && ! pgrep -x 'cc1|rustc|updatedb|cargo' >/dev/null; do sleep 20; done; }
run() { local name=$1; shift; quiet; log "$name start: load $(cut -d' ' -f1 /proc/loadavg)"; env "$@" RC_RUSTC="$RUSTC" UPROF=$B/bin/uprof MAX_LOAD=1.0 OUT=$B/results-$name bash $KIT/run-6od.sh > $B/$name.console 2>&1; log "$name exit $?"; }
BINS="base=$B/bin/rc-base fix=$B/bin/rc-fix"; SHAS="base=$BASE fix=$FIX"
STAGE=${STAGE:-all}

if [[ $STAGE == all || $STAGE == primary ]]; then
# Primary: 1 worker, 50 clients, P=1. The gate commands plus GET/MGET for the fix's read-side cost.
c=""; for t in get mget set mset lpush; do c+="base/rc-noaof-$t-c50-P1 fix/rc-noaof-$t-c50-P1 redis-noaof-$t-c50-P1 "; done
run d1-primary RC_BINS="$BINS" RC_SHAS="$SHAS" REQ_C50_P1=6000000 REQ_SCALE_mset=0.5 REQ_SCALE_mget=0.5 REPS=5 ALTERNATE=1 CELLS="$c"
fi
if [[ $STAGE == all || $STAGE == p16 ]]; then
# Pipelined, 1 worker
c=""; for t in get mget set mset; do c+="base/rc-noaof-$t-c50-P16 fix/rc-noaof-$t-c50-P16 redis-noaof-$t-c50-P16 "; done
run d1-p16 RC_BINS="$BINS" RC_SHAS="$SHAS" REQ_C50_P16=25000000 REQ_SCALE_mset=0.2 REQ_SCALE_mget=0.2 REPS=3 ALTERNATE=1 CELLS="$c"
fi
if [[ $STAGE == all || $STAGE == 3w ]]; then
# Three workers: GET/MGET/MSET at P=1 and GET/MGET at P=16
c=""; for t in get-c50-P1 mget-c50-P1 mset-c50-P1 get-c50-P16 mget-c50-P16; do c+="base/rc-noaof-$t fix/rc-noaof-$t "; done
run d1-3w RC_BINS="$BINS" RC_SHAS="$SHAS" REQ_C50_P1=9000000 REQ_C50_P16=70000000 REQ_SCALE_mset=0.4 REQ_SCALE_mget=0.4 SERVER_PIN=0-2 BENCH_THREADS=3 \
  LABEL_ENV_base="RECACHED_WORKER_THREADS=3" LABEL_ENV_fix="RECACHED_WORKER_THREADS=3" REPS=3 ALTERNATE=1 CELLS="$c"
fi
if [[ $STAGE == all || $STAGE == aof ]]; then
# AOF everysec control: logged writes through the same store change
c=""; for t in set-c50-P1 mset-c50-P1; do c+="base/rc-everysec-$t fix/rc-everysec-$t "; done
run d1-aof RC_BINS="$BINS" RC_SHAS="$SHAS" REQ_C50_P1=4000000 REQ_SCALE_mset=0.5 REPS=3 ALTERNATE=1 CELLS="$c"
fi
log "d1 driver ($STAGE) done"
