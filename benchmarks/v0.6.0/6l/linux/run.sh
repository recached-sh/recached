#!/usr/bin/env bash
#
# Recached 6L: dedicated-Linux benchmark kit (written for Fedora; any modern
# x86_64/aarch64 Linux works).
#
# Builds, from source and at exact commits:
#   6k    Recached after Phase 6K (dev 10c6854)
#   6l    Recached with the 6L branches (reservations off by default)
#   redis 7.2.5   valkey 9.1.0
# then runs, pinned, server and load generator on disjoint CPUs:
#   A. in-process capped-write microbenchmark (gated vs reserved, 1/2/4/8 threads)
#   B. capped SET/INCR worker scaling, 1/2/4/8 workers: 6k vs 6l (reservations
#      off) vs 6l (RECACHED_CAPPED_RESERVATIONS=on), interleaved repetitions
#   C. standard battery, one execution worker: 6k, 6l, Redis, Valkey
#      (throughput, p50/p95/p99, server CPU-seconds), no persistence
#   D. the same with AOF everysec
#   E. RSS per key at 100k and 1M keys
# and packs everything — raw CSVs, server logs, host conditions, binaries'
# SHA-256 — into one tarball to send back.
#
# Usage (no root needed; nothing is installed system-wide):
#   bash run.sh                       # everything, ~1.5-3 h depending on CPU
#   PHASES="A B" bash run.sh          # a subset
#   REPS=3 bash run.sh                # fewer repetitions (default 5)
#   SERVER_CPUS=0-3 BENCH_CPUS=8-11 bash run.sh   # choose the CPU split
#
# Before running, for numbers worth publishing:
#   - close other workloads (browsers, builds, VMs); the kit records load
#   - sudo cpupower frequency-set -g performance    (recorded either way)
#   - keep the machine on AC power
#
# Prerequisites (Fedora):
#   sudo dnf install -y git gcc gcc-c++ make cmake clang perl python3 \
#       util-linux tar curl
#   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # if no cargo
#   (the repo pins Rust 1.98.0; rustup installs it on first build)

set -euo pipefail

WORK=${WORK:-$HOME/recached-6l-bench}
REPO=${REPO:-https://github.com/recached-sh/recached.git}
BASE_REF=${BASE_REF:-10c6854}                                   # 6K: dev after #101-#104
NEW_REF=${NEW_REF:-origin/dev}                                  # 6L as merged
REDIS_VERSION=${REDIS_VERSION:-7.2.5}
VALKEY_VERSION=${VALKEY_VERSION:-9.1.0}
REPS=${REPS:-5}
# Requests per battery run. The first run used the profile default of
# 100,000: rates came out quantized and CPU-seconds resolved to only +/-0.1
# per million.
REQUESTS=${REQUESTS:-2000000}
PHASES=${PHASES:-"A B C D E"}

STAMP=$(date -u +%Y%m%dT%H%M%SZ)
RES="$WORK/results-$STAMP"
BIN="$WORK/bin"
mkdir -p "$WORK" "$RES" "$BIN"
exec > >(tee -a "$RES/run.log") 2>&1

say() { printf '\n=== %s  %s\n' "$(date -u +%H:%M:%S)" "$*"; }
need() { command -v "$1" >/dev/null || { echo "missing: $1 — see the prerequisites at the top of this script" >&2; exit 1; }; }
for tool in git cargo make gcc python3 taskset curl tar sha256sum lscpu; do need "$tool"; done

# ── CPU split: server and load generator on disjoint CPUs ─────────────────────
# Default: the first half of the logical CPUs for the server, the second half
# for redis-benchmark. On SMT machines lscpu numbers siblings either 0/N or
# 0/1; the split below is recorded and can be overridden.
NCPU=$(nproc)
if [[ -z ${SERVER_CPUS:-} || -z ${BENCH_CPUS:-} ]]; then
  if (( NCPU < 8 )); then
    echo "only $NCPU CPUs: the 8-worker runs will share CPUs with the load generator" >&2
  fi
  half=$(( NCPU / 2 ))
  SERVER_CPUS=${SERVER_CPUS:-0-$(( half - 1 ))}
  BENCH_CPUS=${BENCH_CPUS:-$half-$(( NCPU - 1 ))}
fi
export PIN=1 SERVER_CPUS BENCH_CPUS

# ── conditions ────────────────────────────────────────────────────────────────
say "recording host conditions"
{
  echo "date_utc: $STAMP"
  echo "kernel: $(uname -srvm)"
  echo "os: $(. /etc/os-release && echo "$PRETTY_NAME")"
  echo "virtualization: $(systemd-detect-virt 2>/dev/null || echo unknown)"
  echo "cpu_split: server=$SERVER_CPUS bench=$BENCH_CPUS"
  echo "reps: $REPS phases: $PHASES"
  echo "governors: $(cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor 2>/dev/null | sort | uniq -c | tr '\n' ' ' || echo n/a)"
  echo "boost: $(cat /sys/devices/system/cpu/cpufreq/boost 2>/dev/null || cat /sys/devices/system/cpu/intel_pstate/no_turbo 2>/dev/null | sed 's/^/no_turbo=/' || echo n/a)"
  echo "kernel_cmdline: $(cat /proc/cmdline)"
  echo "load_at_start: $(cut -d' ' -f1-3 /proc/loadavg)"
  echo "memory: $(grep MemTotal /proc/meminfo)"
  echo "--- lscpu"; lscpu
  echo "--- lscpu -e"; lscpu -e
  echo "--- top processes by CPU"; ps -eo pcpu,pid,comm --sort=-pcpu | head -15
} > "$RES/conditions.txt"

# ── Recached: two exact trees, two target dirs ────────────────────────────────
# Separate CARGO_TARGET_DIRs on purpose: worktrees sharing one can run a
# binary built from the other tree.
build_recached() {
  local label=$1 ref=$2 tree="$WORK/src-$1"
  if [[ ! -d "$WORK/repo" ]]; then git clone --quiet "$REPO" "$WORK/repo"; fi
  git -C "$WORK/repo" fetch --quiet origin '+refs/heads/*:refs/remotes/origin/*'
  local sha; sha=$(git -C "$WORK/repo" rev-parse "$ref^{commit}")
  if [[ -d $tree ]]; then git -C "$tree" checkout --quiet --detach "$sha"
  else git -C "$WORK/repo" worktree add --quiet --detach "$tree" "$sha"; fi
  say "building recached $label at $sha"
  (cd "$tree" && CARGO_TARGET_DIR="$WORK/target-$label" cargo build --release --locked -p recached --bin recached-server)
  cp "$WORK/target-$label/release/recached-server" "$BIN/recached-server-$label"
  echo "$label $sha $("$BIN/recached-server-$label" --version)" >> "$RES/binaries.txt"
}
build_recached 6k "$BASE_REF"
build_recached 6l "$NEW_REF"

# ── Redis and Valkey from source ──────────────────────────────────────────────
build_kv() {
  local name=$1 version=$2 url=$3 dir="$WORK/$1-$2"
  if [[ ! -x "$BIN/$name-server" ]]; then
    say "building $name $version"
    mkdir -p "$dir"
    curl -fsSL "$url" | tar -xz -C "$dir" --strip-components=1
    # Older C sources can trip a newer GCC's default standard; retry with
    # gnu17 before giving up.
    make -C "$dir" -j"$NCPU" BUILD_TLS=no >"$RES/build-$name.log" 2>&1 || {
      make -C "$dir" distclean >/dev/null 2>&1 || true
      make -C "$dir" -j"$NCPU" BUILD_TLS=no CC="gcc -std=gnu17" >>"$RES/build-$name.log" 2>&1
    }
    cp "$dir/src/$name-server" "$BIN/$name-server"
    if [[ $name == redis ]]; then cp "$dir/src/redis-benchmark" "$dir/src/redis-cli" "$BIN/"; fi
  fi
  echo "$name $("$BIN/$name-server" --version)" >> "$RES/binaries.txt"
}
build_kv redis "$REDIS_VERSION" "https://download.redis.io/releases/redis-$REDIS_VERSION.tar.gz"
build_kv valkey "$VALKEY_VERSION" "https://github.com/valkey-io/valkey/archive/refs/tags/$VALKEY_VERSION.tar.gz"
(cd "$BIN" && sha256sum ./*) >> "$RES/binaries.txt"
export BENCH="$BIN/redis-benchmark" CLI="$BIN/redis-cli"
export PATH="$BIN:$PATH"
SCRIPTS="$WORK/src-6l/scripts"

has() { [[ " $PHASES " == *" $1 "* ]]; }

# ── A. in-process microbenchmark ──────────────────────────────────────────────
if has A; then
  say "A: in-process gated vs reserved capped writes"
  (cd "$WORK/src-6l" && CARGO_TARGET_DIR="$WORK/target-6l" \
    taskset -c "$SERVER_CPUS" cargo test --release --locked -p core-engine --lib bench_reserved_writes -- --ignored --nocapture) \
    | tee "$RES/A-inprocess.log" | { grep -E '^(workload|new_keys|overwrite|incr)' || true; } > "$RES/A-inprocess.csv"
  if [[ -s "$RES/A-inprocess.csv" ]]; then cat "$RES/A-inprocess.csv"; else echo "A produced no rows — see A-inprocess.log"; fi
fi

# ── B. capped worker scaling, server level ────────────────────────────────────
# Caps far above the workload, so every write is admitted: this measures the
# admission path. Repetitions are interleaved across the three builds so slow
# drift on the host lands on all of them alike.
if has B; then
  say "B: capped SET/INCR worker scaling (6k, 6l off, 6l on) x $REPS"
  for rep in $(seq 1 "$REPS"); do
    for mode in 6k 6l-off 6l-on; do
      bin="$BIN/recached-server-${mode%%-*}"
      reservations=$([[ $mode == 6l-on ]] && echo on || echo off)
      out="$RES/B-capped/$mode/rep$rep"
      mkdir -p "$out"
      RECACHED_MAX_KEYS=50000000 RECACHED_MAX_MEMORY=8gb RECACHED_CAPPED_RESERVATIONS=$reservations \
        BIN="$bin" OUT="$out" THREADS="1 2 4 8" TESTS=set,incr N=1000000 CLIENTS=64 PIPELINE=16 \
        KEYSPACE=1000000 DATA=32 bash "$SCRIPTS/bench-scaling.sh" > "$out/stdout.txt" 2>&1 \
        || echo "B $mode rep$rep FAILED — see $out/stdout.txt"
    done
  done
fi

# ── C/D. standard battery vs Redis and Valkey ─────────────────────────────────
battery() {
  local persist=$1 out=$2
  RECACHED_BIN="$BIN/recached-server-6l" BASELINE_BIN="$BIN/recached-server-6k" \
    REDIS_BIN="$BIN/redis-server" VALKEY_BIN="$BIN/valkey-server" \
    SERVERS="baseline recached redis valkey" PROFILE=quick REPS="$REPS" SINGLE=1 REQUESTS="$REQUESTS" \
    PERSIST="$persist" OUT="$out" MEM_KEYS="100000 1000000" MEM_SIZES="32" \
    bash "$SCRIPTS/bench-redis-valkey.sh"
  python3 "$SCRIPTS/bench-summarize.py" "$out" > "$out/summary.md" 2>&1 || true
}
if has C; then say "C: standard battery, no persistence"; battery none "$RES/C-battery"; fi
if has D; then say "D: standard battery, AOF everysec"; battery aof-everysec "$RES/D-battery-aof"; fi

# ── E. memory ─────────────────────────────────────────────────────────────────
# Phase C already records RSS at 100k and 1M keys (memory.csv). E repeats it
# alone, with no other phase's allocations behind it, for the small-key figure.
if has E; then
  say "E: RSS per small key"
  RECACHED_BIN="$BIN/recached-server-6l" BASELINE_BIN="$BIN/recached-server-6k" \
    REDIS_BIN="$BIN/redis-server" VALKEY_BIN="$BIN/valkey-server" \
    SERVERS="baseline recached redis valkey" PROFILE=quick REPS=3 SINGLE=1 \
    TESTS="get" PIPELINES="1" CLIENT_COUNTS="1" REQUESTS=1000 \
    MEM_KEYS="1000000" MEM_SIZES="32" OUT="$RES/E-memory" \
    bash "$SCRIPTS/bench-redis-valkey.sh"
fi

echo "load_at_end: $(cut -d' ' -f1-3 /proc/loadavg)" >> "$RES/conditions.txt"
host=$(hostname -s 2>/dev/null || hostname 2>/dev/null || cat /etc/hostname 2>/dev/null || echo linux)
tarball="$WORK/recached-6l-linux-${host%%.*}-$STAMP.tar.gz"
tar -czf "$tarball" -C "$WORK" "$(basename "$RES")"
say "done. Send back: $tarball ($(du -h "$tarball" | cut -f1))"
