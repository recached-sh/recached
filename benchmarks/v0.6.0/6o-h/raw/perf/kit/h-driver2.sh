#!/bin/bash
# 6O-H native stage 2: Redis on the expiry workloads, then replication,
# replay and sweeper measurements. Waits for h-driver.sh to finish.
set -u
B=$HOME/recached-6oh-bench; KIT=$B/kit
log() { echo "$(date -u +%FT%TZ) $*" >> $B/h-driver.log; }
until grep -q "h driver (all) done" $B/h-driver.log; do sleep 20; done
quiet() { until awk '{exit !($1<1.5)}' /proc/loadavg && ! ps -eo comm | grep -qE '^(cc1|rustc|cargo|ld|recached-server|redis-server)$'; do sleep 20; done; }
quiet; log "redis start"
env RC_BINS="base=$B/bin/rc-base h=$B/bin/rc-h" RC_SHAS="base=x h=x" MAX_LOAD=1.5 OUT=$B/results-redis REQ_C50_P1=3000000 REPS=3 ALTERNATE=1 \
  CELLS="redis-set-c50-P1 redis-setpx100-c50-P1 redis-setpx1000-c50-P1 redis-incrvol-c50-P1" bash $KIT/run-6od.sh > $B/redis.console 2>&1; log "redis exit $?"
quiet; log "repl start"
TMPDIR_BENCH=$B/tmp REPS=3 python3 -I $B/h-repl.py $B/results-repl base=$B/bin/rc-base h=$B/bin/rc-h > $B/repl.console 2>&1; log "repl exit $?"
log "h driver2 done"
