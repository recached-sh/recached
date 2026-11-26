#!/usr/bin/env bash
# 6O-I PR 6: re-measure the grouped and replica cells: base (6O-H 6068337),
# i (6O-I dev 0a1f95e), p (6O-I + perf fix 617aedb).
B=$HOME/recached-6oi-bench
E=RECACHED_AOF_SYNC
export LABELS="base i p" REPS=${REPS:-5} SECS=${SECS:-20} MAX_LOAD=${MAX_LOAD:-3.0}
export OUT=$B/results-p-$(date -u +%Y%m%dT%H%M%SZ)
export CELLS="1:set-set:unrelated:50:0 \
1:set-set:unrelated:50:0::$E=everysec \
1:wide20:unrelated:50:0::$E=everysec \
1:setn:unrelated:20:0:TX_N=100:$E=everysec \
1:set-set:unrelated:50:0::REPLICA=1 \
1:setn:unrelated:20:0:TX_N=100:REPLICA=1"
bash $B/i-tx.sh > $B/logs/p-campaign.console 2>&1
echo "campaign done $(date -u +%FT%TZ) out=$OUT" >> $B/logs/p-campaign.console
