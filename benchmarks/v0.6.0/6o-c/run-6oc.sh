#!/usr/bin/env bash
# Recached v0.6.0 Phase 6O-C (copy of 6o-b/run-6ob.sh; adds the diagnostic reply_coalesce_diag INFO line to stats).
# Recached v0.6.0 Phase 6O-C: the no-persistence request path against
# standalone Redis 7.2.5 and Valkey 9.1.0, with the clients dimension, the
# collection commands, precise on-CPU time and optional syscall counts.
#
# Derived from 6o-a/run-6oa.sh (itself from 6n-c5/run-c5.sh). The same: one
# server on CPU 0, redis-benchmark with 2 threads on BENCH_CPUS, 32-byte values,
# keys drawn from 100,000, a fresh server per cell populated by its own command
# first, no replication anywhere, uprof user-space counters and optional LBR
# samples, host pressure per cell, the IO-settle wait, ALTERNATE ordering.
#
# What 6O-B adds (benchmark side only):
#   * A clients dimension: cells are <kind>-<test>-c<clients>-P<depth>.
#   * The 6L battery's commands, in redis-benchmark 7.2.5's own shapes:
#       set   SET key:__rand_int__ <32 B>         get  GET key:__rand_int__
#       incr  INCR counter:__rand_int__           hset HSET myhash element:__rand_int__ <32 B>
#       lpush LPUSH mylist <32 B>  (one list, grows by every request); rpush RPUSH mylist <32 B> (6O-C)
#       sadd  SADD myset element:__rand_int__  (one set, at most 100,000 members)
#       zadd  ZADD myzset __rand_int__ element:__rand_int__  (one sorted set)
#       mset  MSET with 10 x (key:__rand_int__ <32 B>), as redis-benchmark's "MSET (10 keys)"
#     Each cell first runs its own command 100,000 times (P=64, 20 clients) on
#     the fresh server, as 6O-A does; get is populated with 300,000 SETs (the
#     6L battery's prefill draw count).
#   * Request counts per (clients, depth), equal for every server:
#     REQ_C1_P1 [500000] REQ_C1_P16 [2000000] REQ_C50_P1 [2000000] REQ_C50_P16 [5000000].
#   * /proc/<pid>/task/*/schedstat: the server's exact on-CPU time over the run
#     (6O-A's /proc utime+stime were tick-sampled and missed about a third of it).
#   * SYSC_SO=<sysc.so> with SYSC_CELLS="<cell> ...": those cells' servers run
#     under the syscall-counting shim (tools/sysc.c) and record per-op syscall
#     counts and time. A diagnostic: never mix shim cells into throughput runs.
#
# Recached kinds: rc-noaof rc-everysec rc-aofno rc-always
# Redis/Valkey kinds: redis-noaof valkey-noaof, redis valkey (AOF everysec), ...
#
# Usage (native Linux; an idle host):
#   RC_BINS="base=/path/rc-base" RC_SHAS="base=<sha>" \
#   CELLS="base/rc-noaof-set-c50-P1 redis-noaof-set-c50-P1" bash run-6ob.sh
# Knobs: REPS [3]  the REQ_* counts above  ALWAYS_REQUESTS [50000]
#        BENCH_THREADS [2]  BIN [~/recached-6l-bench/bin]
#        OUT [~/recached-6ob-bench/results-<utc>]  MAX_LOAD [1.0]
#        SERVER_CPUS [0-2]  BENCH_CPUS [3-5]  ALLOW_SANDBOX [0]
#        UPROF  RECORD_CELLS  RECORD_FREQ [2999]  SYSC_SO  SYSC_CELLS
#        LABEL_ENV_<label>  ALTERNATE [0]  SETTLE_IO [8]  SETTLE_MAX [90]
#        REQ_SCALE_<test> [1]  per-command request multiplier (e.g. REQ_SCALE_mset=0.3)
#        SERVER_PIN [first of SERVER_CPUS]  the server's taskset list, e.g. 0-2 for a
#                   multi-worker check (with LABEL_ENV_x="RECACHED_WORKER_THREADS=3")
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
KIT=$(cd "$HERE/.." && pwd)                       # benchmarks/v0.6.0
RC_BINS=${RC_BINS:-}
RC_SHAS=${RC_SHAS:-}
BIN=${BIN:-$HOME/recached-6l-bench/bin}
OUT=${OUT:-$HOME/recached-6oc-bench/results-$(date -u +%Y%m%dT%H%M%SZ)}
REPS=${REPS:-3}
REQ_C1_P1=${REQ_C1_P1:-500000}
REQ_C1_P16=${REQ_C1_P16:-2000000}
REQ_C50_P1=${REQ_C50_P1:-2000000}
REQ_C50_P16=${REQ_C50_P16:-5000000}
ALWAYS_REQUESTS=${ALWAYS_REQUESTS:-50000}
BENCH_THREADS=${BENCH_THREADS:-2}
KEYSPACE=100000
MAX_LOAD=${MAX_LOAD:-1.0}
SERVER_CPUS=${SERVER_CPUS:-0-2}
BENCH_CPUS=${BENCH_CPUS:-3-5}
ALLOW_SANDBOX=${ALLOW_SANDBOX:-0}
UPROF=${UPROF:-}
RECORD_CELLS=${RECORD_CELLS:-}
RECORD_FREQ=${RECORD_FREQ:-2999}
SYSC_SO=${SYSC_SO:-}
SYSC_CELLS=${SYSC_CELLS:-}
CELLS=${CELLS:?set CELLS to the cells to run, in order}
ALTERNATE=${ALTERNATE:-0}
SETTLE_IO=${SETTLE_IO:-8}
SETTLE_MAX=${SETTLE_MAX:-90}

die() { echo "run-6oc: $*" >&2; exit 2; }
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
[[ -z $SYSC_SO || -r $SYSC_SO ]] || die "SYSC_SO $SYSC_SO is not readable"
if [[ -e /.flatpak-info && $ALLOW_SANDBOX != 1 ]]; then die "inside a Flatpak sandbox; run natively"; fi
load=$(cut -d' ' -f1 /proc/loadavg)
awk -v l="$load" -v m="$MAX_LOAD" 'BEGIN { exit !(l <= m) }' || die "load average $load > MAX_LOAD $MAX_LOAD: the host is not idle"

mkdir -p "$OUT"
CLI=$BIN/redis-cli BENCHB=$BIN/redis-benchmark
# One line of host pressure: load, PSI some/full avg10 for IO and memory,
# swap used (MiB), dirty+writeback (KiB).
pressure() {
  local io mem
  io=$(awk '/^some/ { split($2, a, "="); s = a[2] } /^full/ { split($2, a, "="); f = a[2] } END { printf "%s/%s", s, f }' /proc/pressure/io 2>/dev/null || echo n/a)
  mem=$(awk '/^some/ { split($2, a, "="); s = a[2] } /^full/ { split($2, a, "="); f = a[2] } END { printf "%s/%s", s, f }' /proc/pressure/memory 2>/dev/null || echo n/a)
  printf 'load=%s io_psi10=%s mem_psi10=%s swap_used_mib=%s dirty_wb_kib=%s' \
    "$(cut -d' ' -f1 /proc/loadavg)" "$io" "$mem" \
    "$(awk '/^SwapTotal/ { t = $2 } /^SwapFree/ { f = $2 } END { printf "%d", (t - f) / 1024 }' /proc/meminfo)" \
    "$(awk '/^Dirty:/ { d = $2 } /^Writeback:/ { w = $2 } END { printf "%d", d + w }' /proc/meminfo)"
}
{
  echo "date_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  for label in "${!RCBIN[@]}"; do
    echo "rc_bin[$label]: ${RCBIN[$label]} sha256=${RCSUM[$label]} source=${RCSHA[$label]:-unknown} version=$("${RCBIN[$label]}" --version 2>/dev/null | head -1)"
  done
  echo "kit_sha: $(git -C "$KIT" rev-parse HEAD 2>/dev/null || echo unknown) run-6oc.sh sha256=$(sha256sum "$0" | cut -d' ' -f1)"
  echo "sandboxed: $([[ -e /.flatpak-info ]] && echo yes || echo no)"
  echo "kernel: $(uname -srvm)"
  echo "cpu: $(lscpu | sed -n 's/^Model name:[[:space:]]*//p') nproc=$(nproc)"
  echo "governors: $(cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor 2>/dev/null | sort | uniq -c | tr '\n' ' ')"
  echo "loadavg_at_start: $(cat /proc/loadavg)"
  echo "pressure_at_start: $(pressure)"
  echo "memory: $(free -m | awk 'NR == 2 { printf "total=%s used=%s available=%s", $2, $3, $7 } NR == 3 { printf " swap_total=%s swap_used=%s", $2, $3 }')"
  echo "aof_fs: $(df -T "$OUT" | tail -1 | awk '{ print $1, $2 }') rotational=$(lsblk -dno ROTA "$(df "$OUT" | tail -1 | awk '{ print $1 }' | sed 's/[0-9]*$//')" 2>/dev/null || echo unknown)"
  echo "perf: $(command -v perf || echo absent) perf_event_paranoid=$(cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null || echo n/a) uprof=${UPROF:-off}${UPROF:+ sha256=$(sha256sum "$UPROF" | cut -d' ' -f1)}"
  echo "cpus: server=${SERVER_CPUS%%-*} bench=$BENCH_CPUS reps=$REPS req_c1_p1=$REQ_C1_P1 req_c1_p16=$REQ_C1_P16 req_c50_p1=$REQ_C50_P1 req_c50_p16=$REQ_C50_P16 always_requests=$ALWAYS_REQUESTS bench_threads=$BENCH_THREADS keyspace=$KEYSPACE"
  echo "req_scale: $(env | grep "^REQ_SCALE_" | tr "\n" " ")"
  echo "order: alternate=$ALTERNATE settle_io=$SETTLE_IO settle_max_s=$SETTLE_MAX server_pin=${SERVER_PIN:-${SERVER_CPUS%%-*}}"
  env | grep '^LABEL_ENV_' | sed 's/^/label_env: /' || true
  echo "cells: $CELLS"
  echo "record_cells: ${RECORD_CELLS:-none} freq=$RECORD_FREQ"
  echo "sysc: ${SYSC_SO:-off}${SYSC_SO:+ sha256=$(sha256sum "$SYSC_SO" | cut -d' ' -f1)} cells=${SYSC_CELLS:-none}"
  for b in redis-server valkey-server redis-benchmark; do echo "$b: $(sha256sum "$BIN/$b" | cut -d' ' -f1) $("$BIN/$b" --version | head -1)"; done
  echo "rustc_of_rc_bins: ${RC_RUSTC:-unknown}"
  echo "top_at_start:"; top -bn1 | sed -n '7,14p'
} | tee "$OUT/environment.txt"

P=6900
trap 'kill -9 $(jobs -p) 2>/dev/null || true' EXIT
tck=$(getconf CLK_TCK)
cpu_ut() { awk -v t="$tck" '{ sub(/^.*\) /, ""); printf "%.3f %.3f", $12 / t, $13 / t }' "/proc/$1/stat"; }
rss_kb() { awk '/^VmRSS:/ { print $2 }' "/proc/$1/status"; }
ctxsw() { cat /proc/"$1"/task/*/status 2>/dev/null | awk '/^voluntary_ctxt_switches/ { v += $2 } /^nonvoluntary_ctxt_switches/ { n += $2 } END { printf "%d %d", v, n }'; }
# CPU 0 (the server's) jiffies: user nice system idle iowait irq softirq. With
# IRQ_TIME_ACCOUNTING, softirq run inline in the server's syscalls is charged
# here and not to the task, so on-CPU time alone misses it.
cpu0_stat() { awk -v c="cpu${SERVER_CPUS%%-*}" '$1 == c { print $2, $3, $4, $5, $6, $7, $8 }' /proc/stat; }
# System-wide TCP segments and the ACK counters that tell piggybacked from pure ACKs.
tcp_ctr() {
  awk '/^Tcp: [0-9]/ { printf "InSegs=%s OutSegs=%s RetransSegs=%s ", $11, $12, $13 }' /proc/net/snmp
  awk 'NR % 2 == 1 { split($0, h) } NR % 2 == 0 && $1 == "TcpExt:" { for (i = 2; i <= NF; i++) if (h[i] ~ /^(DelayedACKs|DelayedACKLocked|DelayedACKLost|TCPPureAcks|TCPHPAcks|TCPHPHits|TCPBacklogCoalesce|TCPAutoCorking|TCPOrigDataSent|TCPDelivered)$/) printf "%s=%s ", h[i], $i }' /proc/net/netstat
}
net_rx0() { awk -v c=$((${SERVER_CPUS%%-*} + 2)) '$1 == "NET_RX:" || $1 == "NET_TX:" { printf "%s%s ", $1, $c }' /proc/softirqs; }
sched_ns() { cat /proc/"$1"/task/*/schedstat 2>/dev/null | awk '{ r += $1; w += $2 } END { printf "%d %d", r, w }'; }
sysc() { awk '/^syscr/ { r = $2 } /^syscw/ { w = $2 } END { printf "%d %d", r, w }' "/proc/$1/io" 2>/dev/null || echo "n/a n/a"; }
threads() {
  local t
  for t in /proc/"$1"/task/*; do
    printf '%s %s\n' "$(tr -c 'A-Za-z0-9_.\n-' '_' <"$t/comm" 2>/dev/null)" "$(cpu_ut "${t#/proc/}" 2>/dev/null || echo 0 0)"
  done | awk '{ u[$1] += $2; s[$1] += $3; n[$1]++ } END { for (k in u) printf "%s:%d:%.3f:%.3f ", k, n[k], u[k], s[k] }'
}
info_aof() { "$CLI" -p $P info persistence | tr -d '\r' | grep '^recached_aof_' | tr '\n' ' ' || true; }
info_stats() { "$CLI" -p $P info stats | tr -d '\r' | grep -E '^(total_commands_processed|instantaneous_ops_per_sec|total_net_input_bytes|total_net_output_bytes|rejected_connections|total_error_replies|reply_coalesce_diag):' | tr '\n' ' ' || true; }

populate() {   # populate <test>: the command itself, 100,000 times (get: 300,000 SETs)
  local -a common=(-p $P -c 20 -P 64 -r $KEYSPACE -d 32 -q)
  case $1 in
    get) "$BENCHB" "${common[@]}" -n $((KEYSPACE * 3)) -t set >/dev/null 2>&1 ;;
    set|incr|hset|lpush|rpush|sadd|zadd|mset) "$BENCHB" "${common[@]}" -n $KEYSPACE -t "$1" >/dev/null 2>&1 ;;
    *) die "unknown test $1" ;;
  esac
}

# run <label> <kind> <test> <clients> <pipeline> <rep> <cell>
run() {
  local label=$1 kind=$2 test=$3 clients=$4 pipe=$5 rep=$6 cell=$7
  local d="$OUT/${label:+$label-}$kind-$test-c$clients-P$pipe-r$rep"
  local requests
  case "$clients/$pipe" in
    1/1) requests=$REQ_C1_P1 ;; 1/*) requests=$REQ_C1_P16 ;;
    */1) requests=$REQ_C50_P1 ;; *) requests=$REQ_C50_P16 ;;
  esac
  # REQ_SCALE_<test> (default 1): a per-command multiplier, equal for every
  # server, so a slow command's cells last about as long as a fast one's.
  local scalevar="REQ_SCALE_$test"
  requests=$(awk -v r="$requests" -v s="${!scalevar:-1}" 'BEGIN { printf "%d", r * s }')
  [[ $kind == *always ]] && requests=$ALWAYS_REQUESTS
  rm -rf "$d"; mkdir -p "$d/p"
  local ppid cpu0=${SERVER_CPUS%%-*} rcbin="" binsum=n/a
  local settled=0
  while (( settled < SETTLE_MAX )) && ! awk -v m="$SETTLE_IO" '/^some/ { split($2, a, "="); exit !(a[2] < m) }' /proc/pressure/io; do
    sleep 1; settled=$((settled + 1))
  done
  local pre; pre="$(pressure) settle_wait_s=$settled"
  local -a shim=()
  if [[ -n $SYSC_SO && " $SYSC_CELLS " == *" $cell "* ]]; then shim=(LD_PRELOAD="$SYSC_SO" SYSC_OUT="$d/sysc.bin"); fi
  case $kind in
    rc-*)
      rcbin=${RCBIN[$label]:-}
      [[ -n $rcbin ]] || die "cell $cell: no binary labelled '$label' in RC_BINS"
      binsum=${RCSUM[$label]}
      # No RECACHED_REPL_ENABLE: no replication listener. Metrics exporter
      # off (RECACHED_METRICS_PORT=0), snapshots off (SAVE_INTERVAL=0).
      local -a penv=(RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0
        RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$d/p/dump.rdb" RECACHED_WORKER_THREADS=1)
      case $kind in
        rc-everysec) penv+=(RECACHED_AOF_PATH="$d/p/appendonly.aof" RECACHED_AOF_SYNC=everysec) ;;
        rc-aofno) penv+=(RECACHED_AOF_PATH="$d/p/appendonly.aof" RECACHED_AOF_SYNC=no) ;;
        rc-always) penv+=(RECACHED_AOF_PATH="$d/p/appendonly.aof" RECACHED_AOF_SYNC=always) ;;
        rc-noaof) ;;
        *) die "unknown kind $kind" ;;
      esac
      local labelenv="LABEL_ENV_${label//[^A-Za-z0-9_]/_}"
      # shellcheck disable=SC2206
      local -a extra=(${!labelenv:-})
      env "${penv[@]}" "${extra[@]}" "${shim[@]}" taskset -c "${SERVER_PIN:-$cpu0}" "$rcbin" >"$d/server.log" 2>&1 & ppid=$! ;;
    redis|valkey|redis-aofno|valkey-aofno|redis-always|valkey-always|redis-noaof|valkey-noaof)
      local srv=${kind%%-*}
      binsum=$(sha256sum "$BIN/$srv-server" | cut -d' ' -f1)
      local -a args=(--port $P --bind 127.0.0.1 --save '' --dir "$d/p" --logfile "$d/server.log" --protected-mode no)
      case $kind in
        *-noaof) args+=(--appendonly no) ;;
        *-aofno) args+=(--appendonly yes --appendfsync no) ;;
        *-always) args+=(--appendonly yes --appendfsync always) ;;
        *) args+=(--appendonly yes --appendfsync everysec) ;;
      esac
      env "${shim[@]}" taskset -c "${SERVER_PIN:-$cpu0}" "$BIN/$srv-server" "${args[@]}" & ppid=$! ;;
    *) die "unknown kind $kind" ;;
  esac
  for _ in $(seq 80); do "$CLI" -p $P ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
  "$CLI" -p $P ping 2>/dev/null | grep -q PONG || die "$cell: server did not start (see $d/server.log)"
  populate "$test"
  # Let the populate's AOF writeback leave the page cache before timing.
  sync -f "$d/p" 2>/dev/null || true
  local u0 u1 a0="" a1="" x0 x1 s0 s1 t0 t1 th0 th1 st0 st1 ust="" urec=""
  local sc0="" sc1="" k0 k1 c00 c01 tc0 tc1 nr0 nr1
  [[ -s $d/sysc.bin ]] && sc0=$(python3 -I "$HERE/tools/sysc_read.py" snap "$d/sysc.bin")
  c00=$(cpu0_stat); tc0=$(tcp_ctr); nr0=$(net_rx0); k0=$(sched_ns $ppid); u0=$(cpu_ut $ppid); x0=$(ctxsw $ppid); s0=$(sysc $ppid); th0=$(threads $ppid); st0=$(info_stats)
  [[ $kind == rc-* ]] && a0=$(info_aof)
  if [[ -n $UPROF ]]; then
    "$UPROF" stat $ppid >"$d/uprof-stat.txt" 2>&1 & ust=$!
    if [[ " $RECORD_CELLS " == *" $cell "* ]]; then
      "$UPROF" record $ppid "$RECORD_FREQ" "$d/samples.txt" lbr >"$d/uprof-record.txt" 2>&1 & urec=$!
    fi
    sleep 0.2
  fi
  local -a targ=(-t "$test")
  t0=$(date +%s%N)
  local TIMEFORMAT='%U %S'
  { time python3 -I "$HERE/tools/rusage.py" "$d/gen-rusage.txt" -- taskset -c "$BENCH_CPUS" "$BENCHB" -p $P -n "$requests" -c "$clients" -P "$pipe" -d 32 -r $KEYSPACE \
      --threads "$BENCH_THREADS" --csv "${targ[@]}" >"$d/bench.csv" 2>"$d/bench.err"; } 2>"$d/gen-time.txt"
  t1=$(date +%s%N)
  [[ -n $ust ]] && { kill -INT $ust; wait $ust || true; }
  [[ -n $urec ]] && { kill -INT $urec; wait $urec || true; }
  c01=$(cpu0_stat); tc1=$(tcp_ctr); nr1=$(net_rx0); k1=$(sched_ns $ppid); u1=$(cpu_ut $ppid); x1=$(ctxsw $ppid); s1=$(sysc $ppid); th1=$(threads $ppid); st1=$(info_stats)
  [[ -s $d/sysc.bin ]] && sc1=$(python3 -I "$HERE/tools/sysc_read.py" snap "$d/sysc.bin")
  [[ $kind == rc-* ]] && a1=$(info_aof)
  local rss gen aofsz
  rss=$(rss_kb $ppid)
  gen=$(tail -1 "$d/gen-time.txt")
  aofsz=$(du -sb "$d/p" 2>/dev/null | cut -f1)
  {
    echo "cell=$cell label=${label:-none} kind=$kind test=$test clients=$clients P=$pipe rep=$rep requests=$requests bin_sha256=$binsum oncpu_s=$(awk -v a="${k0% *}" -v b="${k1% *}" 'BEGIN{printf "%.4f", (b - a) / 1e9}') rqwait_s=$(awk -v a="${k0#* }" -v b="${k1#* }" 'BEGIN{printf "%.4f", (b - a) / 1e9}') cpu_user_s=$(awk -v a="${u0% *}" -v b="${u1% *}" 'BEGIN{printf "%.3f", b - a}') cpu_sys_s=$(awk -v a="${u0#* }" -v b="${u1#* }" 'BEGIN{printf "%.3f", b - a}') wall_s=$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.3f", (b - a) / 1e9}') gen_user_s=${gen% *} gen_sys_s=${gen#* } rss_kb=$rss data_dir_bytes=$aofsz"
    echo "  pressure_before: $pre"
    echo "  pressure_after: $(pressure)"
    echo "  ctxsw_before: $x0"; echo "  ctxsw_after:  $x1"
    echo "  cpu0_before: $c00"; echo "  cpu0_after: $c01"
    echo "  tcp_before: $tc0"; echo "  tcp_after: $tc1"
    echo "  softirq0_before: $nr0"; echo "  softirq0_after: $nr1"
    echo "  gen_rusage: $(cat "$d/gen-rusage.txt" 2>/dev/null)"
    echo "  syscalls_before: $s0"; echo "  syscalls_after:  $s1"
    echo "  threads_before: $th0"; echo "  threads_after:  $th1"
    echo "  stats_before: $st0"; echo "  stats_after:  $st1"
    echo "  aof_before: $a0"; echo "  aof_after:  $a1"
    [[ -n $sc0 ]] && { echo "  sysc_before: $sc0"; echo "  sysc_after: $sc1"; }
    [[ -s $d/uprof-stat.txt ]] && echo "  uprof: $(tr '\n' ' ' <"$d/uprof-stat.txt")"
    echo "  bench_err: $(grep -ci 'error' "$d/bench.err" 2>/dev/null || true)"
    echo "  bench: $(tail -1 "$d/bench.csv")"
  } >>"$OUT/results.txt"
  kill -9 $ppid 2>/dev/null || true; wait $ppid 2>/dev/null || true; sleep 0.5
  rm -rf "$d/p" "$d/sysc.bin"   # GBs at P=16; the size is recorded above
  grep -lE 'panicked|ERROR' "$d"/*.log >>"$OUT/log-errors.txt" 2>/dev/null || true
}

for rep in $(seq 1 "$REPS"); do
  order=$CELLS
  if [[ $ALTERNATE == 1 ]] && (( rep % 2 == 0 )); then
    order=$(printf '%s\n' $CELLS | tac | tr '\n' ' ')
  fi
  for cell in $order; do
    label="" spec=$cell
    [[ $cell == */* ]] && { label=${cell%%/*}; spec=${cell#*/}; }
    [[ $spec =~ ^(.+)-(set|get|incr|hset|lpush|rpush|sadd|zadd|mset)-c([0-9]+)-P([0-9]+)$ ]] || die "bad cell name $cell"
    run "$label" "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}" "${BASH_REMATCH[3]}" "${BASH_REMATCH[4]}" "$rep" "$cell"
  done
done
echo "loadavg_at_end: $(cat /proc/loadavg)" >>"$OUT/environment.txt"
echo "pressure_at_end: $(pressure)" >>"$OUT/environment.txt"
python3 -I "$HERE/summarize-6ob.py" "$OUT/results.txt" | tee "$OUT/summary.md"
