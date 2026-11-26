#!/bin/bash
# 6O-G native validation, second campaign (after review D, Part 1). Launch:
#   flatpak-spawn --host bash ~/recached-6og-bench/g-driver2.sh
# Labels: base = 25948a1; g1 = ce770d6 (perf/v0.6o-g-exec-overhead); gcap = 8a75116 (+ capped fit).
# g1 and gcap run the same code on uncapped stores, so uncapped stages compare base with gcap;
# the capped stage compares all three. STAGE=tx|capped|stall|mem|cmds|all
set -u
B=$HOME/recached-6og-bench; KIT=$B/kit
log() { echo "$(date -u +%FT%TZ) $*" >> $B/g-driver2.log; }
quiet() { until awk '{exit !($1<2.0)}' /proc/loadavg && ! ps -eo comm | grep -qE '^(cc1|rustc|cargo|ld|recached-server)$'; do sleep 20; done; }
STAGE=${STAGE:-all}
CAP="RECACHED_MAX_KEYS=200000,RECACHED_EVICTION=allkeys-lru"
CAPF="RECACHED_MAX_KEYS=90000,RECACHED_EVICTION=allkeys-lru"
if [[ $STAGE == all || $STAGE == tx ]]; then
  quiet; log "tx start"
  LABELS="base gcap" REPS=5 SECS=20 OUT=$B/results2-tx PORT_BASE=23600 CELLS="\
1:get-get:unrelated:50:0 1:mget-get:unrelated:50:0 1:exists-get:unrelated:50:0 1:get-set:unrelated:50:0 \
1:mget-mset:unrelated:50:0 1:watch:unrelated:50:0 1:set-set:unrelated:50:0 1:mset10:unrelated:50:0 1:wide20:unrelated:50:0 \
1:set-set:same:50:4 1:set-set:overlap:50:4 1:set-set:unrelated:50:4 \
3:get-get:unrelated:50:4 3:set-set:unrelated:50:4 3:wide20:unrelated:50:4 3:mget-mset:unrelated:50:4 \
3:set-set:unrelated:24:4:READERS=8,READ_MODE=mget10 3:wide20:unrelated:16:4:READERS=8 3:wide20:overlap:16:4:READERS=8 \
1:set-set:unrelated:50:0::RECACHED_AOF_SYNC=everysec 1:set-set:unrelated:50:0::RECACHED_AOF_SYNC=everysec,RECACHED_AOF_FANOUT=coalesced \
1:set-set:unrelated:8:0::RECACHED_AOF_SYNC=always 1:wide20:unrelated:50:0::RECACHED_AOF_SYNC=everysec \
1:set-set:unrelated:50:0::REPLICA=1 1:wide20:unrelated:50:0::REPLICA=1" \
    bash $B/g-tx.sh > $B/tx2.console 2>&1; log "tx exit $?"
fi
if [[ $STAGE == all || $STAGE == capped ]]; then
  quiet; log "capped start"
  c=""; for cap in $CAP $CAPF; do c+="1:set-set:unrelated:50:4::$cap 3:set-set:unrelated:24:4:READERS=8:$cap 3:wide20:unrelated:16:4:READERS=8:$cap "; done
  LABELS="base g1 gcap" REPS=5 SECS=20 OUT=$B/results2-capped PORT_BASE=24600 CELLS="$c" bash $B/g-tx.sh > $B/capped2.console 2>&1; log "capped exit $?"
fi
if [[ $STAGE == all || $STAGE == stall ]]; then
  quiet; log "stall start"
  c=""
  for n in 2 10 100 1000 10000; do c+="3:setn:unrelated:1:4:TX_N=$n,READERS=8 "; done
  c+="3:setn:unrelated:1:4:TX_N=10000,READERS=8,READ_MODE=mget10 1:setn:unrelated:1:4:TX_N=10000,READERS=8 "
  for n in 10 1000 10000; do c+="3:setn:unrelated:1:4:TX_N=$n,READERS=8:$CAP 3:setn:unrelated:1:4:TX_N=$n,READERS=8:$CAPF "; done
  c+="1:setn:unrelated:1:4:TX_N=10000,READERS=8:$CAPF 3:flushdb:unrelated:1:4:READERS=8 3:setn:unrelated:1:4:TX_N=1000,READERS=8:RECACHED_AOF_SYNC=always "
  LABELS="base gcap" REPS=3 SECS=20 OUT=$B/results2-stall PORT_BASE=25600 CELLS="$c" bash $B/g-tx.sh > $B/stall2.console 2>&1; log "stall exit $?"
fi
if [[ $STAGE == all || $STAGE == mem ]]; then
  quiet; log "mem start"
  for r in 1 2 3; do for l in base gcap; do python3 $B/g-mem.py $B/bin/rc-$l $l $r >> $B/results2-mem.log 2>&1; done; done; log "mem exit $?"
fi
if [[ $STAGE == all || $STAGE == cmds ]]; then
  quiet; log "cmds start"
  c=""; for t in lpush mset; do for l in base gcap; do c+="$l/rc-everysec-$t-c50-P1 "; done; done
  env RC_BINS="base=$B/bin/rc-base gcap=$B/bin/rc-gcap" RC_SHAS="base=25948a1b90e2ca1bc83f837b203bb2d416f8377f gcap=8a75116" \
    MAX_LOAD=3.0 OUT=$B/results2-cmds REQ_C50_P1=4000000 REQ_SCALE_mset=0.5 REPS=5 ALTERNATE=1 CELLS="$c" \
    bash $KIT/run-6od.sh > $B/cmds2.console 2>&1; log "cmds exit $?"
fi
log "g driver2 ($STAGE) done"
