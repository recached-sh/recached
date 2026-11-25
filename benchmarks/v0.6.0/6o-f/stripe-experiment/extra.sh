#!/bin/bash
# 6O-F3 review F5: workloads that could favour more stripes.
B=$HOME/recached-6of-bench
until grep -q 'ttl exit' $B/f-driver.log; do sleep 30; done
until awk '{exit !($1<1.0)}' /proc/loadavg; do sleep 20; done
echo "$(date -u +%FT%TZ) extra start" >> $B/f-driver.log
LABELS="base x256 x512 x1024 x1024s" REPS=5 SECS=30 OUT=$B/results-extra-mset CELLS="3:mget-mset:unrelated:50:4" bash $B/stripes.sh > $B/extra1.console 2>&1
LABELS="base x256 x512 x1024 x1024s" REPS=5 SECS=30 OUT=$B/results-extra-aof SERVER_ENV="RECACHED_AOF_SYNC=everysec" CELLS="3:get-get:unrelated:50:4" bash $B/stripes.sh > $B/extra2.console 2>&1
echo "$(date -u +%FT%TZ) extra exit" >> $B/f-driver.log
