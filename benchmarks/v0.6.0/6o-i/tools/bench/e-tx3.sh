#!/usr/bin/env bash
# 6O-I PR 6: B and C a third time (the second attempt met servers left by the first on its ports;
# i-tx.sh now kills its children on exit). After the full suite, on fresh ports.
B=$HOME/recached-6oi-bench
E=RECACHED_AOF_SYNC
until grep -q "suite done" $B/logs/suite-final.txt 2>/dev/null; do sleep 20; done
run() {
  local name=$1 labels=$2 reps=$3 port=$4; shift 4
  PORT_BASE=$port LABELS="$labels" REPS=$reps SECS=20 MAX_LOAD=3.0 OUT=$B/results-e3-$name CELLS="$*" bash $B/i-tx.sh > $B/logs/e3-$name.console 2>&1
  echo "$(date -u +%FT%TZ) $name done exit=$?" >> $B/logs/e-tx3.console
}
run B "base i p3" 5 33600 "1:set-set:unrelated:50:0::$E=everysec,REPLICA=1 1:setn:unrelated:20:0:TX_N=100:$E=everysec,REPLICA=1 1:set-set:unrelated:50:0::REPLICA=1 1:setn:unrelated:20:0:TX_N=100:REPLICA=1"
run C "base p3" 3 43600 "1:set-set:unrelated:50:0::$E=everysec,RECACHED_AOF_FANOUT=coalesced 1:set-set:unrelated:50:0::REPLICA=1,RECACHED_REPL_BACKLOG_BYTES=268435456 3:setn:unrelated:2:4:TX_N=10000:REPLICA=1 1:setn:unrelated:20:0:TX_N=100:REPLICA=1,REPLICA_AOF=1"
echo "$(date -u +%FT%TZ) e-tx3 done" >> $B/logs/e-tx3.console
