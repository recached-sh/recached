#!/bin/bash
# 6O-C final validation: dev + C1 + C2 + C3 + WATCH fix (src-final) against the pinned 6O-B
# binary (rc-base, d5563d7) and Redis 7.2.5. Primary gate: 5 interleaved reps, ~30 s cells.
set -u
B=$HOME/recached-6oc-bench; KIT=$B/kit
BASE=d5563d78034f3ccccc91975051de2cb6099b76ba; FINAL=$(git -C $B/src-final rev-parse HEAD)
RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)"
log() { echo "$(date -u +%FT%TZ) $*" >> $B/driver.log; }
quiet() { until awk '{exit !($1<1.0)}' /proc/loadavg && ! pgrep -x 'cc1|rustc|updatedb|cargo' >/dev/null; do sleep 20; done; }
run() { local name=$1; shift; quiet; log "$name start: load $(cut -d' ' -f1 /proc/loadavg)"; env "$@" RC_RUSTC="$RUSTC" UPROF=$B/bin/uprof MAX_LOAD=1.0 OUT=$B/results-$name bash $KIT/run-6oc.sh > $B/$name.console 2>&1; log "$name exit $?"; }
BINS="base=$B/bin/rc-base final=$B/bin/rc-final"; SHAS="base=$BASE final=$FINAL"

# Primary gate: 1 worker, 50 clients, P=1
c=""; for t in set lpush sadd mset; do c+="base/rc-noaof-$t-c50-P1 final/rc-noaof-$t-c50-P1 redis-noaof-$t-c50-P1 "; done
run final-primary RC_BINS="$BINS" RC_SHAS="$SHAS" REQ_C50_P1=6000000 REQ_SCALE_mset=0.5 REPS=5 ALTERNATE=1 CELLS="$c"
# Secondary controls, 1 worker
c=""; for t in get incr zadd; do c+="base/rc-noaof-$t-c50-P1 final/rc-noaof-$t-c50-P1 redis-noaof-$t-c50-P1 "; done
for t in set-c1-P1 set-c50-P16 mset-c50-P16 lpush-c50-P16 sadd-c50-P16; do c+="base/rc-noaof-$t final/rc-noaof-$t "; done
c+="redis-noaof-set-c50-P16 redis-noaof-mset-c50-P16"
run final-secondary RC_BINS="$BINS" RC_SHAS="$SHAS" REQ_C1_P1=900000 REQ_C50_P1=5000000 REQ_C50_P16=25000000 REQ_SCALE_mset=0.2 REPS=3 ALTERNATE=1 CELLS="$c"
# Three workers: the P=16 tail and one client
c="base/rc-noaof-set-c50-P16 final/rc-noaof-set-c50-P16 base/rc-noaof-set-c1-P1 final/rc-noaof-set-c1-P1"
run final-3w RC_BINS="$BINS" RC_SHAS="$SHAS" REQ_C1_P1=900000 REQ_C50_P16=70000000 SERVER_PIN=0-2 BENCH_THREADS=3 \
  LABEL_ENV_base="RECACHED_WORKER_THREADS=3" LABEL_ENV_final="RECACHED_WORKER_THREADS=3" REPS=3 ALTERNATE=1 CELLS="$c"
# AOF carryover: standalone everysec
c=""; for t in set-c50-P1 incr-c50-P1 hset-c50-P1 set-c50-P16; do c+="base/rc-everysec-$t final/rc-everysec-$t redis-$t "; done
run final-aof RC_BINS="$BINS" RC_SHAS="$SHAS" REQ_C50_P1=4000000 REQ_C50_P16=20000000 REPS=3 ALTERNATE=1 CELLS="$c"
log "v2 driver done"
