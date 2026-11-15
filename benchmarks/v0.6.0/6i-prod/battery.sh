#!/usr/bin/env bash
# Phase 6I measurement battery, in order, on the host. Every step writes its
# own directory under benchmarks/v0.6.0/6i-final/. Re-running skips steps
# whose output exists. Binaries: target/6i/linux-bins (see SHA256SUMS).
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=$(mkdir -p "$here/../6i-final" && cd "$here/../6i-final" && pwd)
root=$(cd "$here/../../.." && pwd)
log() { echo "$(date +%T) $*" | tee -a "$out/battery.log"; }
waitq() { until awk -v l="$(sysctl -n vm.loadavg | awk '{print $2}')" 'BEGIN{exit !(l < 6)}'; do sleep 30; done; }

[[ -s $out/ttl/ttl-churn.csv ]] || { waitq; log "ttl churn"; SCRIPT=ttl-churn.sh "$here/run.sh" "$out/ttl" SERVERS="6h 6i redis" >>"$out/battery.log" 2>&1; }
[[ -s $out/ws/ws-results.csv && $(wc -l <"$out/ws/ws-results.csv") -gt 60 ]] || { waitq; log "websocket matrix"; SCRIPT=ws.sh "$here/run.sh" "$out/ws" SERVERS="6h 6i" CLIENT_SET="1 10 100 500" WORKER_SET="1 4" TEST_TIME=10 REPS=2 >>"$out/battery.log" 2>&1; }
if [[ ! -s $out/inproc/request-path.csv ]]; then
  waitq; log "in-process"; mkdir -p "$out/inproc"
  docker run --rm -v "$root/target/6i/linux-bins":/bins:ro -v "$here/../6h-request-path":/scripts:ro -v "$out/inproc":/out \
    -e BENCH_REPS=5 -e TMPDIR=/tmp recached-6i-bench:local \
    bash -c "/scripts/abba-n.sh /out/request-path.csv 3 request_path_bench 6h=/bins/t-6h 6i=/bins/t-6i && BENCH_METRICS=1 /scripts/abba-n.sh /out/request-path-metrics.csv 2 request_path_bench 6i=/bins/t-6i && /scripts/abba-n.sh /out/hot-key.csv 2 hot_key_scaling_bench 6h=/bins/t-6h 6i=/bins/t-6i" >>"$out/battery.log" 2>&1
fi
log "production matrix"
MAX_LOAD=6 MIN_ROWS=100 "$here/gated.sh" "$out/prod" 3 SERVERS="6h 6i redis valkey" \
  CONFIGS="standalone standalone-metrics aof-everysec repl capped" PREFILL_TTL=1 \
  CELLS_STANDALONE="set-p16 get-p16 incr-p16 incr-new-p16 incr-hot-p16 incr-ttl-p16 hset-p16 hget-p16 lat-fixed wlat-fixed" >>"$out/battery.log" 2>&1
[[ -s $out/aof-always/results.csv ]] || { waitq; log "aof always"; "$here/run.sh" "$out/aof-always" SERVERS="6i redis valkey" CONFIGS="aof-always" REPS=2 >>"$out/battery.log" 2>&1; }
log "done"
