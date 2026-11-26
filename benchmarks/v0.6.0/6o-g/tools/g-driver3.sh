#!/bin/bash
# 6O-G native validation, third campaign: review D part 2's requested measurements. Launch:
#   flatpak-spawn --host bash ~/recached-6og-bench/g-driver3.sh
# Labels: base = 25948a1; gcap = 8a75116 (dev + #180; dev's code on uncapped stores);
# base-jcc / gcap-jcc = the same sources built with -x86-branches-within-32B-boundaries (layout control).
# Stages (STAGE=uprof|layout|stall|cmds|all):
#   uprof  - D2-1: user-space counters (instructions, cycles, L1D) on the RELEASE binaries, small EXECs;
#   layout - D2-1: small-EXEC cells with four labels interleaved: does the gap survive a layout change?
#   stall  - D2-4: large transactions with observers (AOF everysec, attached replica) and with 3
#            concurrent transaction clients;
#   cmds   - D2-2: the ordinary command controls on the final binary.
set -u
B=$HOME/recached-6og-bench; KIT=$B/kit
log() { echo "$(date -u +%FT%TZ) $*" >> $B/g-driver3.log; }
quiet() { until awk '{exit !($1<2.0)}' /proc/loadavg && ! ps -eo comm | grep -qE '^(cc1|rustc|cargo|ld|recached-server)$'; do sleep 20; done; }
STAGE=${STAGE:-all}
if [[ $STAGE == all || $STAGE == uprof ]]; then
  quiet; log "uprof start"
  for m in get-get exists-get set-set; do
    OUT=$B/results3-uprof.log bash $B/g-uprof.sh gcap $m 3 > /dev/null 2>&1
    BASE=base-jcc OUT=$B/results3-uprof.log bash $B/g-uprof.sh gcap-jcc $m 3 > /dev/null 2>&1
  done; log "uprof exit $?"
fi
if [[ $STAGE == all || $STAGE == layout ]]; then
  quiet; log "layout start"
  LABELS="base gcap base-jcc gcap-jcc" REPS=5 SECS=20 OUT=$B/results3-layout PORT_BASE=26600 \
    CELLS="1:get-get:unrelated:50:0 1:exists-get:unrelated:50:0 1:set-set:unrelated:50:0" \
    bash $B/g-tx.sh > $B/layout3.console 2>&1; log "layout exit $?"
fi
if [[ $STAGE == all || $STAGE == stall ]]; then
  quiet; log "stall start"
  LABELS="base gcap" REPS=3 SECS=20 OUT=$B/results3-stall PORT_BASE=27600 CELLS="\
3:setn:unrelated:1:4:TX_N=10000,READERS=8:RECACHED_AOF_SYNC=everysec \
3:setn:unrelated:1:4:TX_N=10000,READERS=8:REPLICA=1 \
3:setn:unrelated:3:4:TX_N=10000,READERS=8 \
3:setn:unrelated:3:4:TX_N=1000,READERS=8 \
3:setn:unrelated:1:4:TX_N=1000,READERS=8:RECACHED_AOF_SYNC=everysec" \
    bash $B/g-tx.sh > $B/stall3.console 2>&1; log "stall exit $?"
fi
if [[ $STAGE == all || $STAGE == cmds ]]; then
  quiet; log "cmds start"
  c=""; for t in get mget set incr; do for l in base gcap; do c+="$l/rc-noaof-$t-c50-P1 "; done; done
  for l in base gcap; do c+="$l/rc-noaof-get-c50-P16 $l/rc-noaof-mget-c50-P16 $l/rc-everysec-incr-c50-P1 $l/rc-everysec-set-c50-P1 "; done
  env RC_BINS="base=$B/bin/rc-base gcap=$B/bin/rc-gcap" RC_SHAS="base=25948a1b90e2ca1bc83f837b203bb2d416f8377f gcap=8a75116" \
    MAX_LOAD=3.0 OUT=$B/results3-cmds REQ_C50_P1=4000000 REQ_C50_P16=20000000 REQ_SCALE_mget=0.5 REPS=5 ALTERNATE=1 CELLS="$c" \
    bash $KIT/run-6od.sh > $B/cmds3.console 2>&1; log "cmds exit $?"
fi
log "g driver3 ($STAGE) done"
