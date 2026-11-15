#!/usr/bin/env bash
# Phase 6I follow-up to battery2.sh, run after it on a quieter host:
#   G  capped stores, 6H / 6I / Redis (battery2's capped cells were bimodal
#      for 6I under heavy host load — reps 1-2 at 6H's all-stripe-barrier CPU
#      cost, rep 3 at the per-key-stripe cost — so they are re-measured alone)
#   H  standalone 1 worker, 6H / 6I / Redis, the gate cells
#   I  RSS per key (../6g-linux/memory.sh, unchanged method), 6H / 6I / Redis / Valkey
# Each step writes under benchmarks/v0.6.0/6i-final2/ and is skipped if done.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=$(cd "$here/../6i-final2" && pwd)
root=$(cd "$here/../../.." && pwd)
bins=$root/target/6i/linux-bins
log() { echo "$(date +%T) $*" | tee -a "$out/battery.log"; }
waitq() { until awk -v l="$(sysctl -n vm.loadavg | awk '{print $2}')" 'BEGIN{exit !(l < 6)}'; do sleep 60; done; }

log "G: capped rerun, 6H / 6I / Redis"
MAX_LOAD=6 MIN_ROWS=20 "$here/gated.sh" "$out/capped2" 3 SERVERS="6h 6i redis" CONFIGS="capped" \
  >>"$out/battery.log" 2>&1

log "H: standalone 1 worker rerun, 6H / 6I / Redis"
MAX_LOAD=6 MIN_ROWS=20 "$here/gated.sh" "$out/standalone-1w" 3 SERVERS="6h 6i redis" CONFIGS="standalone" \
  WORKER_SET=1 CELLS_STANDALONE="set-p16 get-p16 incr-p16 incr-new-p16 hset-p16 hget-p16" \
  >>"$out/battery.log" 2>&1

if [[ ! -s $out/mem/memory.csv ]]; then
  waitq; log "I: memory"
  mkdir -p "$out/mem"
  {
    echo "date_utc: $(date -u +%FT%TZ)"
    echo "host_load_at_start: $(uptime | sed 's/.*averages*: //')"
    (cd "$bins" && shasum -a 256 recached-server-6h recached-server-6i)
  } >"$out/mem/conditions.txt"
  docker run --rm --name recached-6i-mem -v "$bins":/bins:ro -v "$here/../6g-linux":/scripts:ro -v "$out/mem":/out \
    -e SERVERS="6h 6i redis valkey" -e REPS=3 \
    -e SCENARIOS="string:1000000:32:0 ttl:1000000:32:0 hash:1000000:32:0" \
    recached-6i-bench:local /scripts/memory.sh >>"$out/battery.log" 2>&1
fi
log "battery3 done"
