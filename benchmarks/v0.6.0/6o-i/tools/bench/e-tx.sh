#!/usr/bin/env bash
# 6O-I PR 6, review E round 1: the transaction measurements it required.
# base = 6O-H 6068337, i = 6O-I dev 0a1f95e, p3 = #200 at 976574a (side table),
# p3nc = p3 with the per-member record check compiled out (control, review E M1).
B=$HOME/recached-6oi-bench
E=RECACHED_AOF_SYNC
run() { # name labels reps outroot cells...
  local name=$1 labels=$2 reps=$3 root=$4; shift 4
  LABELS="$labels" REPS=$reps SECS=20 MAX_LOAD=3.0 OUT=$root/results-e-$name CELLS="$*" bash $B/i-tx.sh > $B/logs/e-$name.console 2>&1
  [[ $root == /tmp ]] && cp -r $root/results-e-$name $B/ && rm -rf $root/results-e-$name
  echo "$(date -u +%FT%TZ) $name done" >> $B/logs/e-tx.console
}
echo "$(date -u +%FT%TZ) start" > $B/logs/e-tx.console
# A (H1, M5): set-set everysec on the HDD, AOF counters captured.
run A-hdd "base i p3" 5 $B "1:set-set:unrelated:50:0::$E=everysec"
# A on tmpfs (H1 cause; M1 attribution with the control build).
run A-tmpfs "base i p3 p3nc" 5 /tmp "1:set-set:unrelated:50:0::$E=everysec 1:setn:unrelated:20:0:TX_N=100:$E=everysec"
# B (M6, M3, L4): AOF and a replica together; replica cells with replica RSS.
run B "base i p3" 5 $B "1:set-set:unrelated:50:0::$E=everysec,REPLICA=1 1:setn:unrelated:20:0:TX_N=100:$E=everysec,REPLICA=1 1:set-set:unrelated:50:0::REPLICA=1 1:setn:unrelated:20:0:TX_N=100:REPLICA=1"
# C (M6, M2, M4, M3), 3 reps: coalesced fan-out; a 256 MiB backlog; a 3-worker primary with outside
# writers under 10,000-write transactions; a replica with its own AOF.
run C "base p3" 3 $B "1:set-set:unrelated:50:0::$E=everysec,RECACHED_AOF_FANOUT=coalesced 1:set-set:unrelated:50:0::REPLICA=1,RECACHED_REPL_BACKLOG_BYTES=268435456 3:setn:unrelated:2:4:TX_N=10000:REPLICA=1 1:setn:unrelated:20:0:TX_N=100:REPLICA=1,REPLICA_AOF=1"
echo "$(date -u +%FT%TZ) e-tx done" >> $B/logs/e-tx.console
