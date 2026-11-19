#!/usr/bin/env bash
# Recached v0.6.0 Phase 6N-C4: Gate M1 revalidation after the coalescer fix,
# plus the ablation cells used to attribute CPU without `perf`.
#
# Derived from benchmarks/v0.6.0/6n-c/run-m1.sh. The workload is unchanged:
# AOF everysec, one primary worker (four for the w4 cells), 50 clients, SET,
# 32-byte values, keys drawn from 100,000, the keyspace pre-populated and the
# replica synced before the timed run, REPS interleaved repetitions.
#
# What changed against the 6N-C kit (benchmark side only):
#   * REQUESTS_P1 / REQUESTS_P16 replace the single REQUESTS. 6N-C ran P=16
#     cells for ~2 s (1,000,000 requests), and three servers reported exactly
#     499,500 rps: a generator/timing ceiling, not a server limit. P=16 now
#     runs 10,000,000 requests by default (~15-60 s per cell).
#   * Throughput is also computed from an independent wall clock around
#     redis-benchmark (`wall_s`), and the generator's own CPU is recorded
#     (`gen_user_s`, `gen_sys_s`), so a generator-bound cell can be spotted.
#   * BENCH_THREADS (redis-benchmark --threads) is a knob; default 2 as 6N-C.
#   * Per primary: context switches and read/write syscalls over the timed
#     run (summed over its threads; /proc/<pid>/task/*/status and /proc/<pid>/io),
#     and CPU seconds per thread name before and after.
#   * Ablation kinds for CPU attribution (only when CELLS names them):
#       rc-noaof            Recached, no AOF, no replication
#       rc-repl-noaof       Recached + one connected replica, no AOF
#       rc-coalesced-norep  replication enabled, no replica attached, AOF coalesced
#   * redis-cli INFO greps tolerate a server with no AOF lines.
#
# Usage (native Linux; not a Flatpak/toolbox; an idle host):
#   RC_SHA=<40-hex> RC_BIN=<recached-server built from RC_SHA> RC_SHA256=<hex> \
#     bash benchmarks/v0.6.0/6n-c4/run-m1.sh
# Knobs: REPS [3]  REQUESTS_P1 [1000000]  REQUESTS_P16 [10000000]  BENCH_THREADS [2]
#        BIN [~/recached-6l-bench/bin]  OUT [~/recached-6nc4-bench/results-<utc>]
#        MAX_LOAD [1.0]  SERVER_CPUS [0-2]  BENCH_CPUS [3-5]  ALLOW_SANDBOX [0]
#        CELLS [the M1 matrix below; names are <kind>-P<pipe> or <kind>-w4]
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
KIT=$(cd "$HERE/.." && pwd)                       # benchmarks/v0.6.0
RC_SHA=${RC_SHA:?set RC_SHA to the tested 40-hex commit}
RC_BIN=${RC_BIN:?set RC_BIN to a recached-server built from RC_SHA}
RC_SHA256=${RC_SHA256:?set RC_SHA256 to the expected sha256 of RC_BIN}
BIN=${BIN:-$HOME/recached-6l-bench/bin}
OUT=${OUT:-$HOME/recached-6nc4-bench/results-$(date -u +%Y%m%dT%H%M%SZ)}
REPS=${REPS:-3}
REQUESTS_P1=${REQUESTS_P1:-1000000}
REQUESTS_P16=${REQUESTS_P16:-10000000}
BENCH_THREADS=${BENCH_THREADS:-2}
MAX_LOAD=${MAX_LOAD:-1.0}
SERVER_CPUS=${SERVER_CPUS:-0-2}
BENCH_CPUS=${BENCH_CPUS:-3-5}
ALLOW_SANDBOX=${ALLOW_SANDBOX:-0}
M1_CELLS="rc-standalone-P1 rc-inline-P1 rc-coalesced-P1 rc-standalone-P16 rc-inline-P16 rc-coalesced-P16 redis-P16 valkey-P16 rc-inline-w4 rc-coalesced-w4"
CELLS=${CELLS:-$M1_CELLS}

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
  echo "perf: $(command -v perf || echo absent) perf_event_paranoid=$(cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null || echo n/a)"
  echo "cpus: server=$SERVER_CPUS bench=$BENCH_CPUS reps=$REPS requests_p1=$REQUESTS_P1 requests_p16=$REQUESTS_P16 bench_threads=$BENCH_THREADS"
  echo "cells: $CELLS"
  for b in redis-server valkey-server redis-benchmark; do echo "$b: $(sha256sum "$BIN/$b" | cut -d' ' -f1) $("$BIN/$b" --version | head -1)"; done
  echo "rustc_of_rc_bin: ${RC_RUSTC:-unknown}"
} | tee "$OUT/environment.txt"

P=6800 PREPL=6810 R=6820
# Whatever happens, leave no server behind to hold the ports of the next run.
trap 'kill -9 $(jobs -p) 2>/dev/null || true' EXIT
tck=$(getconf CLK_TCK)
cpu() { awk -v t="$tck" '{ sub(/^.*\) /, ""); printf "%.3f", ($12 + $13) / t }' "/proc/$1/stat"; }
rss_kb() { awk '/^VmRSS:/ { print $2 }' "/proc/$1/status"; }
# Context switches (voluntary, involuntary) summed over the process's threads.
ctxsw() { cat /proc/"$1"/task/*/status 2>/dev/null | awk '/^voluntary_ctxt_switches/ { v += $2 } /^nonvoluntary_ctxt_switches/ { n += $2 } END { printf "%d %d", v, n }'; }
# read/write syscalls of the whole process.
sysc() { awk '/^syscr/ { r = $2 } /^syscw/ { w = $2 } END { printf "%d %d", r, w }' "/proc/$1/io" 2>/dev/null || echo "n/a n/a"; }
# CPU seconds per thread name: name:threads:seconds ...
threads() {
  local t
  for t in /proc/"$1"/task/*; do
    printf '%s %s\n' "$(tr -c 'A-Za-z0-9_.\n-' '_' <"$t/comm" 2>/dev/null)" "$(cpu "${t#/proc/}" 2>/dev/null || echo 0)"
  done | awk '{ c[$1] += $2; n[$1]++ } END { for (k in c) printf "%s:%d:%.3f ", k, n[k], c[k] }'
}
info_aof() { "$CLI" -p $P info persistence | tr -d '\r' | grep '^recached_aof_' | tr '\n' ' ' || true; }

# run <kind> <pipeline> <workers> <rep>
run() {
  local kind=$1 pipe=$2 workers=$3 rep=$4
  local d="$OUT/$kind-P$pipe-w$workers-r$rep"
  local requests=$REQUESTS_P1
  (( pipe > 1 )) && requests=$REQUESTS_P16
  rm -rf "$d"; mkdir -p "$d/p" "$d/r"
  local ppid rpid="" replicated=1 first=${SERVER_CPUS%%-*} last=${SERVER_CPUS##*-}
  local pcpus=$first
  (( workers > 1 )) && pcpus="$first-$((last - 1))"
  case $kind in
    rc-*)
      local -a penv=(RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0
        RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$d/p/dump.rdb" RECACHED_WORKER_THREADS=$workers)
      local -a aof=(RECACHED_AOF_PATH="$d/p/appendonly.aof" RECACHED_AOF_SYNC=everysec)
      local -a repl=(RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=$PREPL)
      case $kind in
        rc-standalone) replicated=0; penv+=("${aof[@]}") ;;
        rc-inline) penv+=("${aof[@]}" "${repl[@]}" RECACHED_AOF_FANOUT=inline) ;;
        rc-coalesced) penv+=("${aof[@]}" "${repl[@]}" RECACHED_AOF_FANOUT=coalesced) ;;
        rc-noaof) replicated=0 ;;
        rc-repl-noaof) penv+=("${repl[@]}") ;;
        rc-coalesced-norep) replicated=0; penv+=("${aof[@]}" "${repl[@]}" RECACHED_AOF_FANOUT=coalesced) ;;
        *) die "unknown kind $kind" ;;
      esac
      env "${penv[@]}" taskset -c "$pcpus" "$RC_BIN" >"$d/primary.log" 2>&1 & ppid=$! ;;
    redis|valkey)
      taskset -c "$pcpus" "$BIN/$kind-server" --port $P --bind 127.0.0.1 --save '' --appendonly yes \
        --appendfsync everysec --dir "$d/p" --logfile "$d/primary.log" --protected-mode no & ppid=$! ;;
    *) die "unknown kind $kind" ;;
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
  local c0 c1 a0="" a1="" x0 x1 s0 s1 t0 t1 th0 th1
  c0=$(cpu $ppid); x0=$(ctxsw $ppid); s0=$(sysc $ppid); th0=$(threads $ppid)
  [[ $kind == rc-* ]] && a0=$(info_aof)
  t0=$(date +%s%N)
  local TIMEFORMAT='%U %S'
  { time taskset -c "$BENCH_CPUS" "$BENCHB" -p $P -n "$requests" -c 50 -P "$pipe" -d 32 -r 100000 \
      --threads "$BENCH_THREADS" --csv -t set >"$d/bench.csv" 2>"$d/bench.err"; } 2>"$d/gen-time.txt"
  t1=$(date +%s%N)
  c1=$(cpu $ppid); x1=$(ctxsw $ppid); s1=$(sysc $ppid); th1=$(threads $ppid)
  [[ $kind == rc-* ]] && a1=$(info_aof)
  local rss rrss="n/a" conv="n/a" lag="n/a" sample="n/a" gen
  rss=$(rss_kb $ppid)
  gen=$(tail -1 "$d/gen-time.txt")
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
    echo "kind=$kind P=$pipe workers=$workers rep=$rep requests=$requests cpu_s=$(awk -v a="$c0" -v b="$c1" 'BEGIN{printf "%.3f", b - a}') wall_s=$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.3f", (b - a) / 1e9}') gen_user_s=${gen% *} gen_sys_s=${gen#* } rss_kb=$rss replica_rss_kb=$rrss lag_p50_p99_max_ms=\"$lag\" converge=\"$conv\" sample=$sample"
    echo "  ctxsw_before: $x0"; echo "  ctxsw_after:  $x1"
    echo "  syscalls_before: $s0"; echo "  syscalls_after:  $s1"
    echo "  threads_before: $th0"; echo "  threads_after:  $th1"
    echo "  aof_before: $a0"; echo "  aof_after:  $a1"
    echo "  bench: $(tail -1 "$d/bench.csv")"
  } >>"$OUT/results.txt"
  kill -9 $ppid ${rpid:+$rpid} 2>/dev/null || true; wait 2>/dev/null || true; sleep 0.5
  rm -f "$d/p/appendonly.aof" "$d/p"/*.rdb "$d/r"/*.rdb "$d/p"/appendonlydir/* 2>/dev/null || true   # GBs at P=16
  grep -lE 'panicked|ERROR' "$d"/*.log >>"$OUT/log-errors.txt" 2>/dev/null || true
}

want() { [[ " $CELLS " == *" $1 "* ]]; }
KINDS="rc-standalone rc-inline rc-coalesced redis valkey rc-noaof rc-repl-noaof rc-coalesced-norep"
for rep in $(seq 1 "$REPS"); do
  for pipe in 1 16; do
    for kind in $KINDS; do
      want "$kind-P$pipe" && run "$kind" "$pipe" 1 "$rep"
    done
  done
  for kind in $KINDS; do
    want "$kind-w4" && run "$kind" 1 4 "$rep"
  done
done
echo "loadavg_at_end: $(cat /proc/loadavg)" >>"$OUT/environment.txt"
python3 -I "$HERE/summarize-c4.py" "$OUT/results.txt" | tee "$OUT/summary.md"
