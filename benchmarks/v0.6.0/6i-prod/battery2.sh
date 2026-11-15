#!/usr/bin/env bash
# Phase 6I final battery (after PR #97): every step writes its own directory
# under benchmarks/v0.6.0/6i-final2/ and is skipped if its output exists.
# Binaries: target/6i/linux-bins (6i = PR #97 head; SHA256SUMS).
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=$(mkdir -p "$here/../6i-final2" && cd "$here/../6i-final2" && pwd)
root=$(cd "$here/../../.." && pwd)
bins=$root/target/6i/linux-bins
log() { echo "$(date +%T) $*" | tee -a "$out/battery.log"; }
waitq() { until awk -v l="$(sysctl -n vm.loadavg | awk '{print $2}')" 'BEGIN{exit !(l < 6)}'; do sleep 30; done; }

log "A: standalone comparison, v0.5.1 / 6G / 6H / 6I / Redis / Valkey"
MAX_LOAD=6 MIN_ROWS=60 "$here/gated.sh" "$out/standalone" 3 SERVERS="v051 6g 6h 6i redis valkey" \
  CONFIGS="standalone" PREFILL_TTL=1 \
  CELLS_STANDALONE="set-p16 get-p16 incr-p16 incr-new-p16 incr-hot-p16 incr-ttl-p16 hset-p16 hget-p16 lat-fixed wlat-fixed" \
  >>"$out/battery.log" 2>&1
log "B: production configurations, 6H / 6I / Redis / Valkey"
MAX_LOAD=6 MIN_ROWS=60 "$here/gated.sh" "$out/prod" 3 SERVERS="6h 6i redis valkey" \
  CONFIGS="standalone-metrics aof-everysec repl capped" >>"$out/battery.log" 2>&1
[[ -s $out/aof-always/results.csv ]] || { waitq; log "C: aof always"; "$here/run.sh" "$out/aof-always" SERVERS="6i redis valkey" CONFIGS="aof-always" REPS=2 >>"$out/battery.log" 2>&1; }
if [[ ! -s $out/inproc/hot-key.csv ]]; then
  waitq; log "D: in-process hot-key scaling"; mkdir -p "$out/inproc"
  docker run --rm --name recached-6i-inproc -v "$bins":/bins:ro -v "$here/../6h-request-path":/scripts:ro -v "$out/inproc":/out \
    -e BENCH_REPS=5 -e TMPDIR=/tmp recached-6i-bench:local \
    bash -c "/scripts/abba-n.sh /out/hot-key.csv 2 hot_key_scaling_bench 6h=/bins/t-6h 6i=/bins/t-6i" >>"$out/battery.log" 2>&1
fi
if [[ ! -s $out/perf-incr-new/top.txt ]]; then
  waitq; log "E: perf INCR new key / existing"
  for c in incr-new-p16 incr-existing-p16; do
    mkdir -p "$out/perf-${c%-p16}"
    docker run --rm --privileged --name recached-6i-perf -v "$bins":/bins:ro -v "$here":/scripts:ro -v "$out/perf-${c%-p16}":/out \
      -e BIN=6i-sym -e CELL=$c recached-6g-perf:local /scripts/perf.sh >>"$out/battery.log" 2>&1
  done
fi
[[ -s $out/soak/soak.log ]] || { waitq; log "F: soak"; SCRIPT=soak.sh "$here/run.sh" "$out/soak" SERVER=6i SOAK_SECS=600 WS_CLIENTS=100 >>"$out/battery.log" 2>&1; }
log "done"
