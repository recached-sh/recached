#!/usr/bin/env bash
# Recached v0.6.0 Phase 6N-C5: replication fan-out profiling and targeted
# validation.
#
# Derived from benchmarks/v0.6.0/6n-c4/run-m1.sh. The workload, affinity,
# replica setup, population, lag probe, convergence and sample checks are
# unchanged: AOF everysec where enabled, one primary worker (four for the w4
# cells), 50 clients, SET, 32-byte values, keys drawn from 100,000, the
# keyspace pre-populated and the replica synced before the timed run.
#
# What changed against the 6N-C4 kit (benchmark side only):
#   * Several Recached binaries in one interleaved run, so a baseline and a
#     candidate are measured under the same host conditions. RC_BINS lists
#     label=path pairs; a Recached cell is named <label>/<kind>-P<pipe> or
#     <label>/<kind>-w4 (Redis/Valkey cells take no label). Every binary's
#     sha256 is recorded; RC_SHAS (label=sha) records the source commit.
#   * CELLS is run in the order given, once per repetition, so the caller
#     chooses the interleaving.
#   * The primary's user and system CPU are recorded separately (cpu_user_s,
#     cpu_sys_s), per process and per thread name. System time is where the
#     kernel's share of a replicated write (socket send, the loopback TCP
#     stack, receiving the replica's acks) shows up.
#   * With UPROF=<path to tools/uprof>, user-space hardware counters over the
#     timed window: cycles, instructions, branch and cache misses, task clock
#     and context switches (perf_event_open with exclude_kernel, which
#     kernel.perf_event_paranoid=2 allows for one's own processes; nothing is
#     installed and no sysctl changed). With RECORD_CELLS naming a cell, that
#     cell also samples user call stacks (LBR) into <run>/samples.txt.
#   * The replication socket's TCP counters (ss -ti on the primary's side of
#     the replication port) before and after: data segments out (≈ sender
#     flushes) and in (≈ replica acknowledgements), bytes.
#   * The replica's CPU over the timed window.
#
# Usage (native Linux; not a Flatpak/toolbox; an idle host):
#   RC_BINS="base=/path/recached-server opt=/path/recached-server" \
#   RC_SHAS="base=<40-hex> opt=<40-hex>" CELLS="base/rc-coalesced-P1 opt/rc-coalesced-P1" \
#     bash benchmarks/v0.6.0/6n-c5/run-c5.sh
# Knobs: REPS [3]  REQUESTS_P1 [1000000]  REQUESTS_P16 [10000000]  BENCH_THREADS [2]
#        BIN [~/recached-6l-bench/bin]  OUT [~/recached-6nc5-bench/results-<utc>]
#        MAX_LOAD [1.0]  SERVER_CPUS [0-2]  BENCH_CPUS [3-5]  ALLOW_SANDBOX [0]
#        UPROF [unset]  RECORD_CELLS [unset]  RECORD_FREQ [2999]
#        PRIMARY_ENV [extra VAR=value words for every Recached primary]
#        LABEL_ENV_<label> [extra VAR=value words for that label's primaries,
#                           so one binary can run as several labels]
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
KIT=$(cd "$HERE/.." && pwd)                       # benchmarks/v0.6.0
RC_BINS=${RC_BINS:?set RC_BINS to label=path pairs}
RC_SHAS=${RC_SHAS:-}
BIN=${BIN:-$HOME/recached-6l-bench/bin}
OUT=${OUT:-$HOME/recached-6nc5-bench/results-$(date -u +%Y%m%dT%H%M%SZ)}
REPS=${REPS:-3}
REQUESTS_P1=${REQUESTS_P1:-1000000}
REQUESTS_P16=${REQUESTS_P16:-10000000}
BENCH_THREADS=${BENCH_THREADS:-2}
MAX_LOAD=${MAX_LOAD:-1.0}
SERVER_CPUS=${SERVER_CPUS:-0-2}
BENCH_CPUS=${BENCH_CPUS:-3-5}
ALLOW_SANDBOX=${ALLOW_SANDBOX:-0}
UPROF=${UPROF:-}
RECORD_CELLS=${RECORD_CELLS:-}
RECORD_FREQ=${RECORD_FREQ:-2999}
PRIMARY_ENV=${PRIMARY_ENV:-}
CELLS=${CELLS:?set CELLS to the cells to run, in order}

die() { echo "run-c5: $*" >&2; exit 2; }
declare -A RCBIN RCSUM RCSHA
for pair in $RC_BINS; do
  label=${pair%%=*} path=${pair#*=}
  [[ -x $path ]] || die "binary $path for $label is not executable"
  RCBIN[$label]=$path
  RCSUM[$label]=$(sha256sum "$path" | cut -d' ' -f1)
done
for pair in $RC_SHAS; do RCSHA[${pair%%=*}]=${pair#*=}; done
for b in redis-server valkey-server redis-benchmark redis-cli; do [[ -x $BIN/$b ]] || die "missing $BIN/$b"; done
[[ -z $UPROF || -x $UPROF ]] || die "UPROF $UPROF is not executable"
if [[ -e /.flatpak-info && $ALLOW_SANDBOX != 1 ]]; then die "inside a Flatpak sandbox; run natively"; fi
load=$(cut -d' ' -f1 /proc/loadavg)
awk -v l="$load" -v m="$MAX_LOAD" 'BEGIN { exit !(l <= m) }' || die "load average $load > MAX_LOAD $MAX_LOAD: the host is not idle"

mkdir -p "$OUT"
CLI=$BIN/redis-cli BENCHB=$BIN/redis-benchmark
{
  echo "date_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  for label in "${!RCBIN[@]}"; do
    echo "rc_bin[$label]: ${RCBIN[$label]} sha256=${RCSUM[$label]} source=${RCSHA[$label]:-unknown} version=$("${RCBIN[$label]}" --version 2>/dev/null | head -1)"
  done
  echo "kit_sha: $(git -C "$KIT" rev-parse HEAD 2>/dev/null || echo unknown) run-c5.sh sha256=$(sha256sum "$0" | cut -d' ' -f1)"
  echo "sandboxed: $([[ -e /.flatpak-info ]] && echo yes || echo no)"
  echo "kernel: $(uname -srvm)"
  echo "cpu: $(lscpu | sed -n 's/^Model name:[[:space:]]*//p') nproc=$(nproc)"
  echo "governors: $(cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor 2>/dev/null | sort | uniq -c | tr '\n' ' ')"
  echo "loadavg_at_start: $(cat /proc/loadavg)"
  echo "perf: $(command -v perf || echo absent) perf_event_paranoid=$(cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null || echo n/a) uprof=${UPROF:-off}${UPROF:+ sha256=$(sha256sum "$UPROF" | cut -d' ' -f1)}"
  echo "cpus: server=$SERVER_CPUS bench=$BENCH_CPUS reps=$REPS requests_p1=$REQUESTS_P1 requests_p16=$REQUESTS_P16 bench_threads=$BENCH_THREADS"
  echo "primary_env: ${PRIMARY_ENV:-none}"
  env | grep '^LABEL_ENV_' | sed 's/^/label_env: /' || true
  echo "cells: $CELLS"
  echo "record_cells: ${RECORD_CELLS:-none} freq=$RECORD_FREQ"
  for b in redis-server valkey-server redis-benchmark; do echo "$b: $(sha256sum "$BIN/$b" | cut -d' ' -f1) $("$BIN/$b" --version | head -1)"; done
  echo "rustc_of_rc_bins: ${RC_RUSTC:-unknown}"
  echo "top_at_start:"; top -bn1 | sed -n '7,14p'
} | tee "$OUT/environment.txt"

P=6800 PREPL=6810 R=6820
# Whatever happens, leave no server behind to hold the ports of the next run.
trap 'kill -9 $(jobs -p) 2>/dev/null || true' EXIT
tck=$(getconf CLK_TCK)
cpu() { awk -v t="$tck" '{ sub(/^.*\) /, ""); printf "%.3f", ($12 + $13) / t }' "/proc/$1/stat"; }
cpu_ut() { awk -v t="$tck" '{ sub(/^.*\) /, ""); printf "%.3f %.3f", $12 / t, $13 / t }' "/proc/$1/stat"; }
rss_kb() { awk '/^VmRSS:/ { print $2 }' "/proc/$1/status"; }
ctxsw() { cat /proc/"$1"/task/*/status 2>/dev/null | awk '/^voluntary_ctxt_switches/ { v += $2 } /^nonvoluntary_ctxt_switches/ { n += $2 } END { printf "%d %d", v, n }'; }
sysc() { awk '/^syscr/ { r = $2 } /^syscw/ { w = $2 } END { printf "%d %d", r, w }' "/proc/$1/io" 2>/dev/null || echo "n/a n/a"; }
# CPU seconds per thread name: name:threads:user:sys ...
threads() {
  local t
  for t in /proc/"$1"/task/*; do
    printf '%s %s\n' "$(tr -c 'A-Za-z0-9_.\n-' '_' <"$t/comm" 2>/dev/null)" "$(cpu_ut "${t#/proc/}" 2>/dev/null || echo 0 0)"
  done | awk '{ u[$1] += $2; s[$1] += $3; n[$1]++ } END { for (k in u) printf "%s:%d:%.3f:%.3f ", k, n[k], u[k], s[k] }'
}
info_aof() { "$CLI" -p $P info persistence | tr -d '\r' | grep '^recached_aof_' | tr '\n' ' ' || true; }
info_repl() { "$CLI" -p $P info replication | tr -d '\r' | grep -E '^(recached_|master_repl|connected_)' | tr '\n' ' ' || true; }
# The primary's side of every replication connection, TCP_INFO fields.
ss_repl() { ss -tinH state established "( sport = :$PREPL )" 2>/dev/null | tr -s ' \n' ' ' || true; }

# run <label> <kind> <pipeline> <workers> <rep> <cell>
run() {
  local label=$1 kind=$2 pipe=$3 workers=$4 rep=$5 cell=$6
  local d="$OUT/${label:+$label-}$kind-P$pipe-w$workers-r$rep"
  local requests=$REQUESTS_P1
  (( pipe > 1 )) && requests=$REQUESTS_P16
  rm -rf "$d"; mkdir -p "$d/p" "$d/r"
  local ppid rpid="" replicated=1 first=${SERVER_CPUS%%-*} last=${SERVER_CPUS##*-}
  local pcpus=$first rcbin="" binsum=n/a
  (( workers > 1 )) && pcpus="$first-$((last - 1))"
  case $kind in
    rc-*)
      rcbin=${RCBIN[$label]:-}
      [[ -n $rcbin ]] || die "cell $cell: no binary labelled '$label' in RC_BINS"
      binsum=${RCSUM[$label]}
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
      local labelenv="LABEL_ENV_${label//[^A-Za-z0-9_]/_}"
      # shellcheck disable=SC2206
      local -a extra=($PRIMARY_ENV ${!labelenv:-})
      env "${penv[@]}" "${extra[@]}" taskset -c "$pcpus" "$rcbin" >"$d/primary.log" 2>&1 & ppid=$! ;;
    redis|valkey)
      taskset -c "$pcpus" "$BIN/$kind-server" --port $P --bind 127.0.0.1 --save '' --appendonly yes \
        --appendfsync everysec --dir "$d/p" --logfile "$d/primary.log" --protected-mode no & ppid=$! ;;
    redis-noaof|valkey-noaof)
      taskset -c "$pcpus" "$BIN/${kind%-noaof}-server" --port $P --bind 127.0.0.1 --save '' --appendonly no \
        --dir "$d/p" --logfile "$d/primary.log" --protected-mode no & ppid=$! ;;
    *) die "unknown kind $kind" ;;
  esac
  for _ in $(seq 80); do "$CLI" -p $P ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
  "$BENCHB" -p $P -n 100000 -c 20 -P 64 -r 100000 -d 32 -t set -q >/dev/null 2>&1   # populate
  local prb="" rserver=${kind%-noaof}
  if (( replicated )); then
    case $kind in
      rc-*) env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$R RECACHED_WS_PORT=$((R + 1)) RECACHED_METRICS_PORT=0 \
              RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$d/r/dump.rdb" RECACHED_REPLICAOF=127.0.0.1:$PREPL \
              RECACHED_WORKER_THREADS=1 taskset -c "$last" "$rcbin" >"$d/replica.log" 2>&1 & rpid=$! ;;
      *) taskset -c "$last" "$BIN/$rserver-server" --port $R --bind 127.0.0.1 --save '' --appendonly no \
           --replicaof 127.0.0.1 $P --dir "$d/r" --logfile "$d/replica.log" --protected-mode no & rpid=$! ;;
    esac
    local n=0 want; want=$("$CLI" -p $P dbsize | tr -cd 0-9)
    for _ in $(seq 240); do n=$("$CLI" -p $R dbsize 2>/dev/null | tr -cd 0-9 || true); (( ${n:-0} >= want )) && break; sleep 0.25; done
    (( ${n:-0} >= want )) || { echo "$cell: replica has $n of $want keys after its first sync" >&2; kill -9 $ppid $rpid; exit 1; }
    taskset -c "$BENCH_CPUS" python3 -I "$KIT/6l/soak/repl_check.py" probe $P $R 100000 >"$d/lag-probe.txt" 2>&1 & prb=$!
  fi
  local c0 c1 u0 u1 a0="" a1="" i0="" i1="" x0 x1 s0 s1 t0 t1 th0 th1 ss0 ss1 rc0="" rc1="" ust="" urec=""
  c0=$(cpu $ppid); u0=$(cpu_ut $ppid); x0=$(ctxsw $ppid); s0=$(sysc $ppid); th0=$(threads $ppid)
  [[ -n $rpid ]] && rc0=$(cpu_ut $rpid)
  [[ $kind == rc-* ]] && { a0=$(info_aof); i0=$(info_repl); }
  ss0=$(ss_repl)
  if [[ -n $UPROF ]]; then
    "$UPROF" stat $ppid >"$d/uprof-stat.txt" 2>&1 & ust=$!
    if [[ " $RECORD_CELLS " == *" $cell "* ]]; then
      "$UPROF" record $ppid "$RECORD_FREQ" "$d/samples.txt" lbr >"$d/uprof-record.txt" 2>&1 & urec=$!
    fi
    sleep 0.2
  fi
  t0=$(date +%s%N)
  local TIMEFORMAT='%U %S'
  { time taskset -c "$BENCH_CPUS" "$BENCHB" -p $P -n "$requests" -c 50 -P "$pipe" -d 32 -r 100000 \
      --threads "$BENCH_THREADS" --csv -t set >"$d/bench.csv" 2>"$d/bench.err"; } 2>"$d/gen-time.txt"
  t1=$(date +%s%N)
  [[ -n $ust ]] && { kill -INT $ust; wait $ust || true; }
  [[ -n $urec ]] && { kill -INT $urec; wait $urec || true; }
  c1=$(cpu $ppid); u1=$(cpu_ut $ppid); x1=$(ctxsw $ppid); s1=$(sysc $ppid); th1=$(threads $ppid)
  [[ -n $rpid ]] && rc1=$(cpu_ut $rpid)
  [[ $kind == rc-* ]] && { a1=$(info_aof); i1=$(info_repl); }
  ss1=$(ss_repl)
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
    echo "cell=$cell label=${label:-none} kind=$kind P=$pipe workers=$workers rep=$rep requests=$requests bin_sha256=$binsum cpu_s=$(awk -v a="$c0" -v b="$c1" 'BEGIN{printf "%.3f", b - a}') cpu_user_s=$(awk -v a="${u0% *}" -v b="${u1% *}" 'BEGIN{printf "%.3f", b - a}') cpu_sys_s=$(awk -v a="${u0#* }" -v b="${u1#* }" 'BEGIN{printf "%.3f", b - a}') wall_s=$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.3f", (b - a) / 1e9}') gen_user_s=${gen% *} gen_sys_s=${gen#* } rss_kb=$rss replica_rss_kb=$rrss lag_p50_p99_max_ms=\"$lag\" converge=\"$conv\" sample=$sample"
    echo "  replica_cpu_before: $rc0"; echo "  replica_cpu_after:  $rc1"
    echo "  ctxsw_before: $x0"; echo "  ctxsw_after:  $x1"
    echo "  syscalls_before: $s0"; echo "  syscalls_after:  $s1"
    echo "  threads_before: $th0"; echo "  threads_after:  $th1"
    echo "  aof_before: $a0"; echo "  aof_after:  $a1"
    echo "  repl_before: $i0"; echo "  repl_after:  $i1"
    echo "  ss_before: $ss0"; echo "  ss_after:  $ss1"
    [[ -s $d/uprof-stat.txt ]] && echo "  uprof: $(tr '\n' ' ' <"$d/uprof-stat.txt")"
    echo "  bench: $(tail -1 "$d/bench.csv")"
  } >>"$OUT/results.txt"
  kill -9 $ppid ${rpid:+$rpid} 2>/dev/null || true; wait 2>/dev/null || true; sleep 0.5
  rm -f "$d/p/appendonly.aof" "$d/p"/*.rdb "$d/r"/*.rdb "$d/p"/appendonlydir/* 2>/dev/null || true   # GBs at P=16
  grep -lE 'panicked|ERROR' "$d"/*.log >>"$OUT/log-errors.txt" 2>/dev/null || true
}

# <label>/<kind>-P<n> | <label>/<kind>-w<n> | <kind>-P<n> (Redis/Valkey)
for rep in $(seq 1 "$REPS"); do
  for cell in $CELLS; do
    label="" spec=$cell
    [[ $cell == */* ]] && { label=${cell%%/*}; spec=${cell#*/}; }
    if [[ $spec =~ ^(.+)-P([0-9]+)$ ]]; then run "$label" "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}" 1 "$rep" "$cell"
    elif [[ $spec =~ ^(.+)-w([0-9]+)$ ]]; then run "$label" "${BASH_REMATCH[1]}" 1 "${BASH_REMATCH[2]}" "$rep" "$cell"
    else die "bad cell name $cell"; fi
  done
done
echo "loadavg_at_end: $(cat /proc/loadavg)" >>"$OUT/environment.txt"
python3 -I "$HERE/summarize-c5.py" "$OUT/results.txt" | tee "$OUT/summary.md"
