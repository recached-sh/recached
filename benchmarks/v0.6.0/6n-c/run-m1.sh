#!/usr/bin/env bash
# Recached v0.6.0 Phase 6N-C3, Gate M1: the targeted measurement, and nothing
# else. It reuses Phase 6N-1 Test B's workload (benchmarks/v0.6.0/6n-1,
# b_run): AOF everysec, one primary worker, 50 clients, SET, 32-byte values,
# keys drawn from 100,000, REQUESTS per run, REPS interleaved repetitions.
#
# Cells (one row per server kind, all interleaved within a repetition):
#   rc-standalone   Recached, no replication listener            (reference)
#   rc-inline       Recached + one connected replica, RECACHED_AOF_FANOUT=inline
#   rc-coalesced    Recached + one connected replica, RECACHED_AOF_FANOUT=coalesced
#   redis           Redis 7.2.5 + one connected replica, appendonly everysec
#   valkey          Valkey 9.1.0 + one connected replica, appendonly everysec
# at pipeline depth 1 and 16, plus a 4-worker diagnostic at P=1 for the three
# Recached kinds. The same RC_BIN serves every Recached cell.
#
# Usage (native Linux; not a Flatpak/toolbox; an idle host):
#   RC_SHA=<40-hex> RC_BIN=<recached-server built from RC_SHA> RC_SHA256=<hex> \
#     bash benchmarks/v0.6.0/6n-c/run-m1.sh
# Knobs: REPS [3]  REQUESTS [1000000]  BIN [~/recached-6l-bench/bin]
#        OUT [~/recached-6nc-bench/results-<utc>]  MAX_LOAD [1.0]
#        SERVER_CPUS [0-2]  BENCH_CPUS [3-5]  CELLS [all]  ALLOW_SANDBOX [0]
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
KIT=$(cd "$HERE/.." && pwd)                       # benchmarks/v0.6.0
RC_SHA=${RC_SHA:?set RC_SHA to the tested 40-hex commit}
RC_BIN=${RC_BIN:?set RC_BIN to a recached-server built from RC_SHA}
RC_SHA256=${RC_SHA256:?set RC_SHA256 to the expected sha256 of RC_BIN}
BIN=${BIN:-$HOME/recached-6l-bench/bin}
OUT=${OUT:-$HOME/recached-6nc-bench/results-$(date -u +%Y%m%dT%H%M%SZ)}
REPS=${REPS:-3}
REQUESTS=${REQUESTS:-1000000}
MAX_LOAD=${MAX_LOAD:-1.0}
SERVER_CPUS=${SERVER_CPUS:-0-2}
BENCH_CPUS=${BENCH_CPUS:-3-5}
ALLOW_SANDBOX=${ALLOW_SANDBOX:-0}
CELLS=${CELLS:-all}

die() { echo "run-m1: $*" >&2; exit 2; }
[[ $RC_SHA =~ ^[0-9a-f]{40}$ ]] || die "RC_SHA must be 40 hex digits"
[[ -x $RC_BIN ]] || die "RC_BIN $RC_BIN is not executable"
got=$(sha256sum "$RC_BIN" | cut -d' ' -f1)
[[ $got == "$RC_SHA256" ]] || die "RC_BIN sha256 $got != RC_SHA256 $RC_SHA256"
for b in redis-server valkey-server redis-benchmark redis-cli; do [[ -x $BIN/$b ]] || die "missing $BIN/$b"; done
if [[ -e /.flatpak-info && $ALLOW_SANDBOX != 1 ]]; then die "inside a Flatpak sandbox; run natively"; fi
load=$(cut -d' ' -f1 /proc/loadavg)
awk -v l="$load" -v m="$MAX_LOAD" 'BEGIN { exit !(l <= m) }' || die "load average $load > MAX_LOAD $MAX_LOAD: the host is not idle"

mkdir -p "$OUT"
CLI=$BIN/redis-cli BENCHB=$BIN/redis-benchmark
{
  echo "date_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "rc_sha: $RC_SHA"
  echo "rc_bin: $RC_BIN sha256=$got version=$("$RC_BIN" --version 2>/dev/null | head -1)"
  echo "kit_sha: $(git -C "$KIT" rev-parse HEAD 2>/dev/null || echo unknown) run-m1.sh sha256=$(sha256sum "$0" | cut -d' ' -f1)"
  echo "sandboxed: $([[ -e /.flatpak-info ]] && echo yes || echo no)"
  echo "kernel: $(uname -srvm)"
  echo "cpu: $(lscpu | sed -n 's/^Model name:[[:space:]]*//p') nproc=$(nproc)"
  echo "governors: $(cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor 2>/dev/null | sort | uniq -c | tr '\n' ' ')"
  echo "loadavg_at_start: $(cat /proc/loadavg)"
  echo "perf: $(command -v perf || echo absent)"
  echo "cpus: server=$SERVER_CPUS bench=$BENCH_CPUS reps=$REPS requests=$REQUESTS cells=$CELLS"
  for b in redis-server valkey-server redis-benchmark; do echo "$b: $(sha256sum "$BIN/$b" | cut -d' ' -f1) $("$BIN/$b" --version | head -1)"; done
  echo "rustc_of_rc_bin: ${RC_RUSTC:-unknown}"
} | tee "$OUT/environment.txt"

P=6800 PREPL=6810 R=6820
# Whatever happens, leave no server behind to hold the ports of the next run.
trap 'kill -9 $(jobs -p) 2>/dev/null || true' EXIT
tck=$(getconf CLK_TCK)
cpu() { awk -v t="$tck" '{ sub(/^.*\) /, ""); printf "%.3f", ($12 + $13) / t }' "/proc/$1/stat"; }
rss_kb() { awk '/^VmRSS:/ { print $2 }' "/proc/$1/status"; }

# run <kind> <pipeline> <workers> <rep>
run() {
  local kind=$1 pipe=$2 workers=$3 rep=$4
  local d="$OUT/$kind-P$pipe-w$workers-r$rep"
  rm -rf "$d"; mkdir -p "$d/p" "$d/r"
  local ppid rpid="" replicated=1 first=${SERVER_CPUS%%-*} last=${SERVER_CPUS##*-}
  local pcpus=$first
  (( workers > 1 )) && pcpus="$first-$((last - 1))"
  case $kind in
    rc-*)
      local -a penv=(RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0
        RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$d/p/dump.rdb" RECACHED_AOF_PATH="$d/p/appendonly.aof"
        RECACHED_AOF_SYNC=everysec RECACHED_WORKER_THREADS=$workers)
      case $kind in
        rc-standalone) replicated=0 ;;
        rc-inline) penv+=(RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=$PREPL RECACHED_AOF_FANOUT=inline) ;;
        rc-coalesced) penv+=(RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=$PREPL RECACHED_AOF_FANOUT=coalesced) ;;
      esac
      env "${penv[@]}" taskset -c "$pcpus" "$RC_BIN" >"$d/primary.log" 2>&1 & ppid=$! ;;
    redis|valkey)
      taskset -c "$pcpus" "$BIN/$kind-server" --port $P --bind 127.0.0.1 --save '' --appendonly yes \
        --appendfsync everysec --dir "$d/p" --logfile "$d/primary.log" --protected-mode no & ppid=$! ;;
  esac
  for _ in $(seq 80); do "$CLI" -p $P ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
  "$BENCHB" -p $P -n 100000 -c 20 -P 64 -r 100000 -d 32 -t set -q >/dev/null 2>&1   # populate
  local prb=""
  if (( replicated )); then
    case $kind in
      rc-*) env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$R RECACHED_WS_PORT=$((R + 1)) RECACHED_METRICS_PORT=0 \
              RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$d/r/dump.rdb" RECACHED_REPLICAOF=127.0.0.1:$PREPL \
              RECACHED_WORKER_THREADS=1 taskset -c "$last" "$RC_BIN" >"$d/replica.log" 2>&1 & rpid=$! ;;
      *) taskset -c "$last" "$BIN/$kind-server" --port $R --bind 127.0.0.1 --save '' --appendonly no \
           --replicaof 127.0.0.1 $P --dir "$d/r" --logfile "$d/replica.log" --protected-mode no & rpid=$! ;;
    esac
    # Digits only: a replica still loading answers "LOADING …", not a count.
    local n=0 want; want=$("$CLI" -p $P dbsize | tr -cd 0-9)
    for _ in $(seq 240); do n=$("$CLI" -p $R dbsize 2>/dev/null | tr -cd 0-9 || true); (( ${n:-0} >= want )) && break; sleep 0.25; done
    (( ${n:-0} >= want )) || { echo "$kind: replica has $n of $want keys after its first sync" >&2; kill -9 $ppid $rpid; exit 1; }
    taskset -c "$BENCH_CPUS" python3 -I "$KIT/6l/soak/repl_check.py" probe $P $R 100000 >"$d/lag-probe.txt" 2>&1 & prb=$!
  fi
  local c0 c1 a0="" a1=""
  c0=$(cpu $ppid)
  [[ $kind == rc-* ]] && a0=$("$CLI" -p $P info persistence | tr -d '\r' | grep '^recached_aof_' | tr '\n' ' ')
  taskset -c "$BENCH_CPUS" "$BENCHB" -p $P -n "$REQUESTS" -c 50 -P "$pipe" -d 32 -r 100000 --threads 2 --csv -t set >"$d/bench.csv"
  c1=$(cpu $ppid)
  [[ $kind == rc-* ]] && a1=$("$CLI" -p $P info persistence | tr -d '\r' | grep '^recached_aof_' | tr '\n' ' ')
  local rss rrss="n/a" conv="n/a" lag="n/a" sample="n/a"
  rss=$(rss_kb $ppid)
  if (( replicated )); then
    kill $prb 2>/dev/null || true; wait $prb 2>/dev/null || true
    lag=$(python3 -I "$KIT/6l/soak/repl_check.py" summarize "$d/lag-probe.txt")
    conv=$(python3 -I "$KIT/6l/soak/repl_check.py" converge $P $R 1 120)
    rrss=$(rss_kb $rpid)
    sample=ok   # 300 benchmark keys must read the same on both nodes
    for i in $(shuf -i 0-99999 -n 300 --random-source=<(yes)); do
      k=$(printf 'key:%012d' "$i")
      [[ $("$CLI" -p $P get "$k") == "$("$CLI" -p $R get "$k")" ]] || { sample="DIVERGED:$k"; break; }
    done
  fi
  {
    echo "kind=$kind P=$pipe workers=$workers rep=$rep cpu_s=$(awk -v a="$c0" -v b="$c1" 'BEGIN{printf "%.3f", b - a}') rss_kb=$rss replica_rss_kb=$rrss lag_p50_p99_max_ms=\"$lag\" converge=\"$conv\" sample=$sample"
    echo "  aof_before: $a0"; echo "  aof_after:  $a1"
    echo "  bench: $(tail -1 "$d/bench.csv")"
  } >>"$OUT/results.txt"
  kill -9 $ppid ${rpid:+$rpid} 2>/dev/null || true; wait 2>/dev/null || true; sleep 0.5
  grep -lE 'panicked|ERROR' "$d"/*.log >>"$OUT/log-errors.txt" 2>/dev/null || true
}

want() { [[ $CELLS == all || " $CELLS " == *" $1 "* ]]; }
for rep in $(seq 1 "$REPS"); do
  for pipe in 1 16; do
    for kind in rc-standalone rc-inline rc-coalesced redis valkey; do
      want "$kind-P$pipe" && run "$kind" "$pipe" 1 "$rep"
    done
  done
  for kind in rc-standalone rc-inline rc-coalesced; do
    want "$kind-w4" && run "$kind" 1 4 "$rep"
  done
done
echo "loadavg_at_end: $(cat /proc/loadavg)" >>"$OUT/environment.txt"
python3 -I "$HERE/summarize-m1.py" "$OUT/results.txt" | tee "$OUT/summary.md"
