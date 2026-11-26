#!/usr/bin/env bash
# 6O-I PR 6: the replica cells again with #200's compact group mark:
# base (6O-H 6068337), i (6O-I dev 0a1f95e), p2 (6O-I + #200 at 4d46dbc).
B=$HOME/recached-6oi-bench
export LABELS="base i p2" REPS=${REPS:-5} SECS=${SECS:-20} MAX_LOAD=${MAX_LOAD:-3.0}
export OUT=$B/results-r-$(date -u +%Y%m%dT%H%M%SZ)
export CELLS="1:set-set:unrelated:50:0::REPLICA=1 1:setn:unrelated:20:0:TX_N=100:REPLICA=1"
bash $B/i-tx.sh > $B/logs/r-campaign.console 2>&1
echo "campaign done $(date -u +%FT%TZ) out=$OUT" >> $B/logs/r-campaign.console
