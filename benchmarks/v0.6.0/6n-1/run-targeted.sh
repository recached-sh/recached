#!/usr/bin/env bash
#
# Recached v0.6.0 Phase 6N-1: the targeted validation runs, and nothing else.
#
#   A  Cross-connection AOF coalescing: the 7-cell matrix, coalesce off / on /
#      Redis 7.2.5 / Valkey 9.1.0, everysec, REPS interleaved (aof-profile.sh).
#   B  Replication-enabled AOF: one primary + one connected replica vs the same
#      primary with no replication listener. 1 worker, 50 clients, SET, P=1.
#   D  One production soak: the 6L soak harness, run once, reservations off.
#
# Usage (native Linux; not a Flatpak/toolbox; an idle host):
#   RC_SHA=<40-hex> RC_BIN=<recached-server built from RC_SHA> RC_SHA256=<hex> \
#     bash benchmarks/v0.6.0/6n-1/run-targeted.sh            # A then B
#   RUN=D SOAK_SECS=2400 ... bash benchmarks/v0.6.0/6n-1/run-targeted.sh
#
# Knobs: RUN ["A B"]  REPS [3]  REQUESTS [1000000]  BIN [~/recached-6l-bench/bin]
#        (redis-server valkey-server redis-benchmark redis-cli, the 6L builds)
#        OUT [~/recached-6n1-bench/results-<utc>]  MAX_LOAD [1.0]
#        SERVER_CPUS [0-2]  BENCH_CPUS [3-5]  ALLOW_SANDBOX [0]
#
# It refuses to start when the 1-minute load average is above MAX_LOAD or
# when run inside a Flatpak sandbox: neither gives usable throughput numbers.

set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
KIT=$(cd "$HERE/.." && pwd)                       # benchmarks/v0.6.0
RC_SHA=${RC_SHA:?set RC_SHA to the release candidate's 40-hex commit}
RC_BIN=${RC_BIN:?set RC_BIN to a recached-server built from RC_SHA}
RC_SHA256=${RC_SHA256:?set RC_SHA256 to RC_BIN's expected sha256}
BIN=${BIN:-$HOME/recached-6l-bench/bin}
OUT=${OUT:-$HOME/recached-6n1-bench/results-$(date -u +%Y%m%dT%H%M%SZ)}
RUN=${RUN:-"A B"}
REPS=${REPS:-3}
REQUESTS=${REQUESTS:-1000000}
MAX_LOAD=${MAX_LOAD:-1.0}
SERVER_CPUS=${SERVER_CPUS:-0-2}
BENCH_CPUS=${BENCH_CPUS:-3-5}
SOAK_SECS=${SOAK_SECS:-2400}
ALLOW_SANDBOX=${ALLOW_SANDBOX:-0}

die() { echo "run-targeted: $*" >&2; exit 2; }
[[ $RC_SHA =~ ^[0-9a-f]{40}$ ]] || die "RC_SHA must be 40 hex digits"
[[ -x $RC_BIN ]] || die "RC_BIN $RC_BIN is not executable"
got=$(sha256sum "$RC_BIN" | cut -d' ' -f1)
[[ $got == "$RC_SHA256" ]] || die "RC_BIN sha256 $got != RC_SHA256 $RC_SHA256"
for b in redis-server valkey-server redis-benchmark redis-cli; do [[ -x $BIN/$b ]] || die "missing $BIN/$b"; done
if [[ -e /.flatpak-info && $ALLOW_SANDBOX != 1 ]]; then die "inside a Flatpak sandbox; run natively (or ALLOW_SANDBOX=1 and label the results)"; fi
load=$(cut -d' ' -f1 /proc/loadavg)
awk -v l="$load" -v m="$MAX_LOAD" 'BEGIN { exit !(l <= m) }' || die "load average $load > MAX_LOAD $MAX_LOAD: the host is not idle"

mkdir -p "$OUT"
CLI=$BIN/redis-cli BENCHB=$BIN/redis-benchmark
{
  echo "date_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "rc_sha: $RC_SHA"
  echo "rc_bin: $RC_BIN sha256=$got version=$("$RC_BIN" --version 2>/dev/null | head -1)"
  echo "kit_sha: $(git -C "$KIT" rev-parse HEAD 2>/dev/null || echo unknown) dirty=$(git -C "$KIT" status --porcelain -- . 2>/dev/null | wc -l)"
  echo "sandboxed: $([[ -e /.flatpak-info ]] && echo yes || echo no)"
  echo "kernel: $(uname -srvm)"
  echo "cpu: $(lscpu | sed -n 's/^Model name:[[:space:]]*//p') nproc=$(nproc)"
  echo "governors: $(cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor 2>/dev/null | sort | uniq -c | tr '\n' ' ')"
  echo "load_at_start: $(cat /proc/loadavg)"
  echo "cpus: server=$SERVER_CPUS bench=$BENCH_CPUS reps=$REPS requests=$REQUESTS run=[$RUN]"
  for b in redis-server valkey-server redis-benchmark; do echo "$b: $(sha256sum "$BIN/$b" | cut -d' ' -f1)"; done
} >"$OUT/conditions.txt"
cat "$OUT/conditions.txt"

# "off" = the same binary with the pre-6M gate (RECACHED_AOF_COALESCE=off).
wrap_off=$OUT/recached-coalesce-off
printf '#!/bin/sh\nRECACHED_AOF_COALESCE=off exec "%s" "$@"\n' "$RC_BIN" >"$wrap_off"; chmod +x "$wrap_off"

profile() {   # profile <out-subdir> VAR=value...
  local sub=$1; shift
  env PIN=1 SERVER_CPUS="$SERVER_CPUS" BENCH_CPUS="$BENCH_CPUS" \
    RECACHED_BIN="$RC_BIN" BASE_BIN="$wrap_off" REDIS_BIN="$BIN/redis-server" VALKEY_BIN="$BIN/valkey-server" \
    BENCH="$BENCHB" CLI="$CLI" POLICIES=everysec REQUESTS="$REQUESTS" REPS="$REPS" \
    OUT="$OUT/$sub" "$@" bash "$KIT/6m/aof-profile.sh"
  python3 -I "$KIT/6m/summarize.py" "$OUT/$sub/results.csv" >"$OUT/$sub/summary.md" 2>&1 || true
}

# ── A: the 7 cells. "base" is coalesce off, "recached" is coalesce on. ────────
if [[ " $RUN " == *" A "* ]]; then
  profile A1-w1-set-c50  SERVERS="base recached redis valkey" WORKERS=1 TESTS=set PIPELINES="1 16" CLIENTS=50
  profile A2-w1-set-c100 SERVERS="base recached redis valkey" WORKERS=1 TESTS=set PIPELINES=1 CLIENTS=100
  # Redis has one command thread: its 1-worker rows (A1) are the reference.
  profile A3-w4-set-c50  SERVERS="base recached valkey" WORKERS=4 TESTS=set PIPELINES="1 16" CLIENTS=50
  profile A4-w1-incr-hset-c50 SERVERS="base recached redis valkey" WORKERS=1 TESTS="incr hset" PIPELINES=1 CLIENTS=50
  if command -v perf >/dev/null && perf stat -e instructions -- true >/dev/null 2>&1; then
    echo "perf available: instructions/op come from the kit's phase F method; not run by this script" >"$OUT/A-perf.txt"
  else
    echo "perf not available on this host: instructions per op NOT MEASURED" >"$OUT/A-perf.txt"
  fi
fi

# ── B: replication-enabled AOF vs no listener ─────────────────────────────────
P=6700 PREPL=6710 R=6720
b_run() {   # b_run <mode standalone|replicated> <rep>
  local mode=$1 rep=$2 d="$OUT/B/$mode-r$rep"
  rm -rf "$d"; mkdir -p "$d/p" "$d/r"
  local -a penv=(RECACHED_BIND=127.0.0.1 RECACHED_PORT=$P RECACHED_WS_PORT=$((P + 1)) RECACHED_METRICS_PORT=0
    RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$d/p/dump.rdb" RECACHED_AOF_PATH="$d/p/appendonly.aof"
    RECACHED_AOF_SYNC=everysec RECACHED_WORKER_THREADS=1)
  [[ $mode == replicated ]] && penv+=(RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=$PREPL)
  env "${penv[@]}" taskset -c "${SERVER_CPUS%%-*}" "$RC_BIN" >"$d/primary.log" 2>&1 & local ppid=$!
  local rpid=""
  for _ in $(seq 80); do "$CLI" -p $P ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
  "$BENCHB" -p $P -n 100000 -c 20 -P 64 -r 100000 -d 32 -t set -q >/dev/null 2>&1   # populate
  if [[ $mode == replicated ]]; then
    env RECACHED_BIND=127.0.0.1 RECACHED_PORT=$R RECACHED_WS_PORT=$((R + 1)) RECACHED_METRICS_PORT=0 \
      RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$d/r/dump.rdb" RECACHED_REPLICAOF=127.0.0.1:$PREPL \
      RECACHED_WORKER_THREADS=1 taskset -c "${SERVER_CPUS##*-}" "$RC_BIN" >"$d/replica.log" 2>&1 & rpid=$!
    local n=0
    for _ in $(seq 240); do n=$("$CLI" -p $R dbsize 2>/dev/null || echo 0); (( ${n:-0} >= 100000 )) && break; sleep 0.25; done
    (( ${n:-0} >= 100000 )) || { echo "B: replica did not complete its first sync" >&2; kill -9 $ppid $rpid; exit 1; }
    taskset -c "$BENCH_CPUS" python3 -I "$KIT/6l/soak/repl_check.py" probe $P $R 100000 >"$d/lag-probe.txt" 2>&1 & local prb=$!
  fi
  local c0 a0 c1 a1 tck; tck=$(getconf CLK_TCK)
  cpu() { awk -v t="$tck" '{ sub(/^.*\) /, ""); printf "%.3f", ($12 + $13) / t }' "/proc/$1/stat"; }
  c0=$(cpu $ppid); a0=$("$CLI" -p $P info persistence | tr -d '\r' | grep '^recached_aof_' | tr '\n' ' ')
  taskset -c "$BENCH_CPUS" "$BENCHB" -p $P -n "$REQUESTS" -c 50 -P 1 -d 32 -r 100000 --threads 2 --csv -t set >"$d/bench.csv"
  c1=$(cpu $ppid); a1=$("$CLI" -p $P info persistence | tr -d '\r' | grep '^recached_aof_' | tr '\n' ' ')
  local conv="n/a" lag="n/a" sample="n/a"
  if [[ $mode == replicated ]]; then
    kill $prb 2>/dev/null || true; wait $prb 2>/dev/null || true
    lag=$(python3 -I "$KIT/6l/soak/repl_check.py" summarize "$d/lag-probe.txt")
    conv=$(python3 -I "$KIT/6l/soak/repl_check.py" converge $P $R 1 120)
    sample=ok   # 300 benchmark keys must read the same on both nodes
    for i in $(shuf -i 0-99999 -n 300 --random-source=<(yes)); do
      k=$(printf 'key:%012d' "$i")
      [[ $("$CLI" -p $P get "$k") == "$("$CLI" -p $R get "$k")" ]] || { sample="DIVERGED:$k"; break; }
    done
  fi
  echo "mode=$mode rep=$rep cpu_s=$(awk -v a="$c0" -v b="$c1" 'BEGIN{printf "%.3f", b - a}') lag_p50_p99_max_ms=\"$lag\" converge=\"$conv\" sample=$sample" >>"$OUT/B/results.txt"
  echo "  aof_before: $a0" >>"$OUT/B/results.txt"; echo "  aof_after:  $a1" >>"$OUT/B/results.txt"
  echo "  bench: $(tail -1 "$d/bench.csv")" >>"$OUT/B/results.txt"
  kill -9 $ppid ${rpid:+$rpid} 2>/dev/null || true; wait 2>/dev/null || true; sleep 0.5
  grep -lE 'panicked|ERROR' "$d"/*.log >>"$OUT/B/log-errors.txt" 2>/dev/null || true
}
if [[ " $RUN " == *" B "* ]]; then
  mkdir -p "$OUT/B"
  for rep in $(seq 1 "$REPS"); do b_run standalone "$rep"; b_run replicated "$rep"; done
  cat "$OUT/B/results.txt"
fi

# ── D: one production soak, reservations off (the v0.6.0 default) ────────────
if [[ " $RUN " == *" D "* ]]; then
  SERVER_BIN="$RC_BIN" RESERVATIONS=off SOAK_SECS="$SOAK_SECS" OUT="$OUT/D-soak" \
    bash "$KIT/6l/soak/soak.sh"
fi

sha256sum "$RC_BIN" | grep -q "^$RC_SHA256 " || { echo "RC_BIN changed during the run" >&2; exit 3; }
echo "done: $OUT"
