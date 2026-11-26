#!/bin/bash
# 6O-G native validation driver. Launch: flatpak-spawn --host bash ~/recached-6og-bench/g-driver.sh
# STAGE=cmds|cmds3w|tx|stall|mem|all
set -u
B=$HOME/recached-6og-bench; KIT=$B/kit
log() { echo "$(date -u +%FT%TZ) $*" >> $B/g-driver.log; }
quiet() { until awk '{exit !($1<1.5)}' /proc/loadavg && ! ps -eo comm | grep -qE '^(cc1|rustc|cargo|ld|recached-server)$'; do sleep 20; done; }
STAGE=${STAGE:-all}
BINS="base=$B/bin/rc-base g=$B/bin/rc-g"
SHAS="base=${SHA_BASE:-25948a1b90e2ca1bc83f837b203bb2d416f8377f} g=${SHA_G:-unknown}"
CAP="RECACHED_MAX_KEYS=200000,RECACHED_EVICTION=allkeys-lru"
if [[ $STAGE == all || $STAGE == cmds ]]; then
  c=""; for t in get mget set lpush mset incr; do for l in base g; do c+="$l/rc-noaof-$t-c50-P1 "; done; done
  for l in base g; do c+="$l/rc-noaof-get-c50-P16 $l/rc-noaof-mget-c50-P16 $l/rc-everysec-incr-c50-P1 $l/rc-everysec-set-c50-P1 "; done
  c+="redis-noaof-get-c50-P1 redis-noaof-set-c50-P1 "
  quiet; log "cmds start"
  env RC_BINS="$BINS" RC_SHAS="$SHAS" MAX_LOAD=1.5 OUT=$B/results-cmds REQ_C50_P1=4000000 REQ_C50_P16=20000000 \
    REQ_SCALE_mset=0.5 REQ_SCALE_mget=0.5 REPS=5 ALTERNATE=1 CELLS="$c" bash $KIT/run-6od.sh > $B/cmds.console 2>&1; log "cmds exit $?"
fi
if [[ $STAGE == all || $STAGE == cmds3w ]]; then
  c=""; for l in base g; do c+="$l/rc-noaof-get-c50-P16 $l/rc-noaof-mget-c50-P16 $l/rc-everysec-set-c50-P1 "; done
  quiet; log "cmds3w start"
  env RC_BINS="$BINS" RC_SHAS="$SHAS" MAX_LOAD=1.5 OUT=$B/results-cmds3w REQ_C50_P1=6000000 REQ_C50_P16=40000000 REQ_SCALE_mget=0.5 \
    SERVER_PIN=0-2 BENCH_THREADS=3 LABEL_ENV_base="RECACHED_WORKER_THREADS=3" LABEL_ENV_g="RECACHED_WORKER_THREADS=3" REPS=3 ALTERNATE=1 \
    CELLS="$c" bash $KIT/run-6od.sh > $B/cmds3w.console 2>&1; log "cmds3w exit $?"
fi
if [[ $STAGE == all || $STAGE == tx ]]; then
  quiet; log "tx start"
  LABELS="base g" REPS=5 SECS=20 MAX_LOAD=1.5 OUT=$B/results-tx PORT_BASE=23600 CELLS="\
1:get-get:unrelated:50:0 1:mget-get:unrelated:50:0 1:exists-get:unrelated:50:0 1:get-set:unrelated:50:0 \
1:mget-mset:unrelated:50:0 1:watch:unrelated:50:0 1:set-set:unrelated:50:0 1:mset10:unrelated:50:0 1:wide20:unrelated:50:0 \
1:set-set:same:50:4 1:set-set:overlap:50:4 1:set-set:unrelated:50:4 \
3:get-get:unrelated:50:4 3:set-set:unrelated:50:4 3:wide20:unrelated:50:4 3:mget-mset:unrelated:50:4 \
1:set-set:unrelated:50:4::$CAP 3:wide20:unrelated:50:4::$CAP \
1:set-set:unrelated:50:0::RECACHED_AOF_SYNC=everysec 1:set-set:unrelated:50:0::RECACHED_AOF_SYNC=everysec,RECACHED_AOF_FANOUT=coalesced \
1:set-set:unrelated:8:0::RECACHED_AOF_SYNC=always 1:wide20:unrelated:50:0::RECACHED_AOF_SYNC=everysec \
1:set-set:unrelated:50:0::RECACHED_REPL_PORT=29990" \
    bash $B/g-tx.sh > $B/tx.console 2>&1; log "tx exit $?"
fi
if [[ $STAGE == all || $STAGE == stall ]]; then
  quiet; log "stall start"
  c=""
  for n in 2 10 100 1000 10000; do c+="3:setn:unrelated:1:4:TX_N=$n,READERS=8 "; done
  for n in 1000 10000; do c+="3:setn:unrelated:1:4:TX_N=$n,READERS=8::$CAP 3:setn:unrelated:1:4:TX_N=$n,READERS=8,READ_MODE=mget10 "; done
  c+="1:setn:unrelated:1:4:TX_N=10000,READERS=8 1:setn:unrelated:1:4:TX_N=10000,READERS=8::$CAP 3:wide20:unrelated:8:4:READERS=8 "
  c+="3:flushdb:unrelated:1:4:READERS=8 3:setn:unrelated:1:4:TX_N=1000,READERS=8::RECACHED_AOF_SYNC=always "
  LABELS="base g" REPS=3 SECS=20 MAX_LOAD=1.5 OUT=$B/results-stall PORT_BASE=25600 CELLS="$c" bash $B/g-tx.sh > $B/stall.console 2>&1; log "stall exit $?"
fi
if [[ $STAGE == all || $STAGE == mem ]]; then
  quiet; log "mem start"
  for l in base g; do for r in 1 2 3; do python3 $B/g-mem.py $B/bin/rc-$l $l $r >> $B/results-mem.log 2>&1; done; done; log "mem exit $?"
fi
log "g driver ($STAGE) done"
