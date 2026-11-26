#!/usr/bin/env bash
# 6O-I PR 6: the transaction campaign, base (6O-H 6068337) against i (6O-I dev 0a1f95e).
B=$HOME/recached-6oi-bench
E=RECACHED_AOF_SYNC
export LABELS="base i" REPS=${REPS:-5} SECS=${SECS:-20} MAX_LOAD=${MAX_LOAD:-3.0}
export OUT=$B/results-tx-$(date -u +%Y%m%dT%H%M%SZ)
export CELLS="1:get-get:unrelated:50:0 \
1:set-set:unrelated:50:0 \
1:set-set:unrelated:50:0::$E=everysec \
1:set-set:unrelated:50:0::$E=always \
1:get-set:unrelated:50:0::$E=everysec \
1:mget-mset:unrelated:50:0::$E=everysec \
1:mset10:unrelated:50:0::$E=everysec \
1:wide20:unrelated:50:0::$E=everysec \
1:setn:unrelated:20:0:TX_N=100:$E=everysec \
1:setn:unrelated:8:0:TX_N=1000:$E=everysec \
1:setn:unrelated:2:0:TX_N=10000:$E=everysec \
1:set-set:unrelated:50:0::REPLICA=1 \
1:setn:unrelated:20:0:TX_N=100:REPLICA=1"
bash $B/i-tx.sh > $B/logs/i-campaign.console 2>&1
echo "campaign done $(date -u +%FT%TZ) out=$OUT" >> $B/logs/i-campaign.console
