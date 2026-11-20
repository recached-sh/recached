#!/usr/bin/env bash
# Recached v0.6.0 Phase 6N-C6: the two cells ../6n-c5/run-c5.sh has no shape
# for. The workload is the kit's: SET, 32-byte values, keys from 100,000,
# 50 clients, AOF everysec with RECACHED_AOF_FANOUT=coalesced, one primary
# worker, the keyspace pre-populated and the replicas synced first.
#
#   MODE=slow  One replica, stopped (SIGSTOP) for STOP_S seconds a few
#              seconds into a P=16 run, then resumed. The primary's RSS and
#              INFO replication are sampled every 200 ms; afterwards the
#              replica must converge. A bounded slow-replica diagnostic: the
#              queue must stay within its budget, the primary must keep
#              serving, and the replica must be dropped and resynchronize.
#   MODE=two   Two replicas, both reading, through a P=16 run: neither may be
#              disconnected, and both must converge.
#   MODE=burst One replica through a run whose every scheduling round can
#              hold more frames than the pre-6N-C6 4096-frame cap: CLIENTS ×
#              PIPE (default 100 × 64 = 6400), AOF off (AOF=off by default
#              here) so fsync stalls do not shape the rounds. The replica must
#              stay connected and converge.
#
# Usage (native Linux, idle host):
#   RC_BINS="base=/path c6=/path" RC_SHAS="base=<sha> c6=<sha>" MODE=slow \
#     bash benchmarks/v0.6.0/6n-c6/run-c6-extra.sh
# Knobs: CLIENTS [50; burst 100]  PIPE [16; burst 64]  AOF [on; burst off]
#        REPS [3]  REQUESTS [6000000]  STOP_AT_S [3]  STOP_S [2]  BIN [~/recached-6l-bench/bin]
#        OUT [~/recached-6nc6-bench/<mode>-<utc>]  MAX_LOAD [1.0]  ALLOW_SANDBOX [0]
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
KIT=$(cd "$HERE/.." && pwd)
MODE=${MODE:?set MODE to slow or two}
RC_BINS=${RC_BINS:?set RC_BINS to label=path pairs}
RC_SHAS=${RC_SHAS:-}
BIN=${BIN:-$HOME/recached-6l-bench/bin}
OUT=${OUT:-$HOME/recached-6nc6-bench/$MODE-$(date -u +%Y%m%dT%H%M%SZ)}
REPS=${REPS:-3}
REQUESTS=${REQUESTS:-6000000}
STOP_AT_S=${STOP_AT_S:-3}
STOP_S=${STOP_S:-2}
MAX_LOAD=${MAX_LOAD:-1.0}
ALLOW_SANDBOX=${ALLOW_SANDBOX:-0}

die() { echo "run-c6-extra: $*" >&2; exit 2; }
[[ $MODE == slow || $MODE == two || $MODE == burst ]] || die "MODE must be slow, two or burst"
if [[ $MODE == burst ]]; then CLIENTS=${CLIENTS:-100} PIPE=${PIPE:-64} AOF=${AOF:-off}; fi
CLIENTS=${CLIENTS:-50} PIPE=${PIPE:-16} AOF=${AOF:-on}
declare -A RCBIN RCSUM RCSHA
LABELS=()
for pair in $RC_BINS; do
  label=${pair%%=*} path=${pair#*=}
  [[ -x $path ]] || die "binary $path for $label is not executable"
  RCBIN[$label]=$path; RCSUM[$label]=$(sha256sum "$path" | cut -d' ' -f1); LABELS+=("$label")
done
for pair in $RC_SHAS; do RCSHA[${pair%%=*}]=${pair#*=}; done
for b in redis-benchmark redis-cli; do [[ -x $BIN/$b ]] || die "missing $BIN/$b"; done
if [[ -e /.flatpak-info && $ALLOW_SANDBOX != 1 ]]; then die "inside a Flatpak sandbox; run natively"; fi
load=$(cut -d' ' -f1 /proc/loadavg)
awk -v l="$load" -v m="$MAX_LOAD" 'BEGIN { exit !(l <= m) }' || die "load average $load > MAX_LOAD $MAX_LOAD"

mkdir -p "$OUT"
CLI=$BIN/redis-cli BENCHB=$BIN/redis-benchmark
{
  echo "date_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ) mode=$MODE reps=$REPS requests=$REQUESTS clients=$CLIENTS pipe=$PIPE aof=$AOF stop_at_s=$STOP_AT_S stop_s=$STOP_S"
  for label in "${LABELS[@]}"; do
    echo "rc_bin[$label]: ${RCBIN[$label]} sha256=${RCSUM[$label]} source=${RCSHA[$label]:-unknown}"
  done
  echo "kit_sha: $(git -C "$KIT" rev-parse HEAD 2>/dev/null || echo unknown) run-c6-extra.sh sha256=$(sha256sum "$0" | cut -d' ' -f1)"
  echo "kernel: $(uname -srvm)"
  echo "cpu: $(lscpu | sed -n 's/^Model name:[[:space:]]*//p') nproc=$(nproc)"
  echo "loadavg_at_start: $(cat /proc/loadavg)"
} | tee "$OUT/environment.txt"

P=6800 PREPL=6810 R1=6820 R2=6830
trap 'kill -9 $(jobs -p) 2>/dev/null || true' EXIT
tck=$(getconf CLK_TCK)
cpu() { awk -v t="$tck" '{ sub(/^.*\) /, ""); printf "%.3f", ($12 + $13) / t }' "/proc/$1/stat"; }
rss_kb() { awk '/^VmRSS:/ { print $2 }' "/proc/$1/status"; }
info_repl() { "$CLI" -p $P info replication 2>/dev/null | tr -d '\r' | grep -E '^(recached_|connected_)' | tr '\n' ' ' || true; }

replica() { # port cpu dir
  mkdir -p "$3"
  env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$1 RECACHED_WS_PORT=$(($1 + 1)) RECACHED_METRICS_PORT=0 \
    RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$3/dump.rdb" RECACHED_REPLICAOF=127.0.0.1:$PREPL \
    RECACHED_WORKER_THREADS=1 taskset -c "$2" "$rcbin" >"$3.log" 2>&1 &
}
synced() { # port
  local n=0 want; want=$("$CLI" -p $P dbsize | tr -cd 0-9)
  for _ in $(seq 240); do n=$("$CLI" -p "$1" dbsize 2>/dev/null | tr -cd 0-9 || true); (( ${n:-0} >= want )) && return 0; sleep 0.25; done
  echo "replica on $1 has $n of $want keys" >&2; return 1
}
sample_keys() { # replica port
  local i k
  for i in $(shuf -i 0-99999 -n 300 --random-source=<(yes)); do
    k=$(printf 'key:%012d' "$i")
    [[ $("$CLI" -p $P get "$k") == "$("$CLI" -p "$1" get "$k")" ]] || { echo "DIVERGED:$k"; return; }
  done
  echo ok
}

for rep in $(seq 1 "$REPS"); do
  for label in "${LABELS[@]}"; do
    rcbin=${RCBIN[$label]}
    d="$OUT/$label-$MODE-r$rep"; rm -rf "$d"; mkdir -p "$d/p"
    aofenv=()
    [[ $AOF == on ]] && aofenv=(RECACHED_AOF_PATH="$d/p/appendonly.aof" RECACHED_AOF_SYNC=everysec RECACHED_AOF_FANOUT=coalesced)
    env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0 \
      RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$d/p/dump.rdb" RECACHED_WORKER_THREADS=1 \
      "${aofenv[@]}" RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=$PREPL taskset -c 0 "$rcbin" >"$d/primary.log" 2>&1 &
    ppid=$!
    for _ in $(seq 80); do "$CLI" -p $P ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
    "$BENCHB" -p $P -n 100000 -c 20 -P 64 -r 100000 -d 32 -t set -q >/dev/null 2>&1
    replica $R1 2 "$d/r1"; r1=$!
    r2=""
    if [[ $MODE == two ]]; then replica $R2 1 "$d/r2"; r2=$!; fi
    synced $R1 || { kill -9 $ppid $r1 ${r2:+$r2}; exit 1; }
    [[ -n $r2 ]] && { synced $R2 || { kill -9 $ppid $r1 $r2; exit 1; }; }
    i0=$(info_repl); c0=$(cpu $ppid); rss0=$(rss_kb $ppid)
    # Timeline: ms since start, primary RSS (kB), INFO replication.
    ( t0=$(date +%s%N); while kill -0 $ppid 2>/dev/null; do
        echo "$(( ($(date +%s%N) - t0) / 1000000 )) $(rss_kb $ppid 2>/dev/null || echo 0) $(info_repl)"
        sleep 0.2; done ) >"$d/timeline.txt" 2>/dev/null &
    tl=$!
    if [[ $MODE == slow ]]; then
      ( sleep "$STOP_AT_S"; kill -STOP $r1; echo "stop $(date +%s%N)"; sleep "$STOP_S"; kill -CONT $r1; echo "cont $(date +%s%N)" ) >"$d/stop.txt" &
      stopper=$!
    fi
    t0=$(date +%s%N)
    taskset -c 3-5 "$BENCHB" -p $P -n "$REQUESTS" -c "$CLIENTS" -P "$PIPE" -d 32 -r 100000 --threads 2 --csv -t set \
      >"$d/bench.csv" 2>"$d/bench.err"
    t1=$(date +%s%N)
    [[ $MODE == slow ]] && wait $stopper
    c1=$(cpu $ppid)
    conv1=$(python3 -I "$KIT/6l/soak/repl_check.py" converge $P $R1 1 180)
    conv2=n/a; [[ -n $r2 ]] && conv2=$(python3 -I "$KIT/6l/soak/repl_check.py" converge $P $R2 1 180)
    s1=$(sample_keys $R1); s2=n/a; [[ -n $r2 ]] && s2=$(sample_keys $R2)
    i1=$(info_repl)
    kill $tl 2>/dev/null || true; wait $tl 2>/dev/null || true
    peak=$(awk '{ if ($2 > m) m = $2 } END { print m + 0 }' "$d/timeline.txt")
    {
      echo "cell=$label/$MODE rep=$rep bin_sha256=${RCSUM[$label]} wall_s=$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.3f", (b - a) / 1e9}') cpu_s=$(awk -v a="$c0" -v b="$c1" 'BEGIN{printf "%.3f", b - a}') rss_kb_before=$rss0 rss_kb_peak=$peak rss_kb_after=$(rss_kb $ppid) replica1_rss_kb=$(rss_kb $r1) replica2_rss_kb=$([[ -n $r2 ]] && rss_kb $r2 || echo n/a) converge1=\"$conv1\" converge2=\"$conv2\" sample1=$s1 sample2=$s2"
      echo "  primary_log: queue_full_bytes=$(grep -c 'byte queue full' "$d/primary.log" || true) queue_full_items=$(grep -c 'channel full' "$d/primary.log" || true) replica_connects=$(grep -c 'Replica connected from' "$d/primary.log" || true)"
      for r in r1 r2; do
        [[ -s $d/$r.log ]] || continue
        echo "  ${r}_log: sync_ended=$(grep -c 'sync ended' "$d/$r.log" || true) full_loaded=$(grep -c 'full snapshot loaded' "$d/$r.log" || true) partial=$(grep -c 'partial synchronization reached' "$d/$r.log" || true)"
      done
      echo "  repl_before: $i0"; echo "  repl_after:  $i1"
      echo "  bench: $(tail -1 "$d/bench.csv")"
    } >>"$OUT/results.txt"
    kill -9 $ppid $r1 ${r2:+$r2} 2>/dev/null || true; wait 2>/dev/null || true; sleep 0.5
    rm -f "$d/p/appendonly.aof" "$d/p"/*.rdb "$d"/r*/*.rdb "$d/p"/appendonlydir/* 2>/dev/null || true
    grep -lE 'panicked' "$d"/*.log >>"$OUT/log-errors.txt" 2>/dev/null || true
  done
done
echo "loadavg_at_end: $(cat /proc/loadavg)" >>"$OUT/environment.txt"
cat "$OUT/results.txt"
