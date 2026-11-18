#!/usr/bin/env bash
#
# Recached 6M / 6N-0: dedicated-Linux kit for cross-connection AOF group
# commit and the v0.6.0 release-candidate validation.
#
# Builds, from source and at exact commits (separate CARGO_TARGET_DIRs):
#   6l    Recached dev after Phase 6L (BASE_REF, default 08e5a7d)
#   6m    the build under test: NEW_REF, an explicit commit SHA (required;
#         no default — 6N-0 found the old default, a feature branch, had
#         diverged from dev). Or NEW_BIN: a prebuilt binary, verified
#         against NEW_SHA256. "6m-off" is the same binary with coalescing
#         off: the closest build to "6M before coalescing".
#   redis 7.2.5   valkey 9.1.0
# then runs, server and load generator pinned to disjoint CPUs:
#   A. AOF execution profile, 1 worker: 6l vs 6m vs 6m with coalescing off
#      vs Redis vs Valkey. SET over 1..200 clients, P=1 and P=16, policies
#      no / everysec / always. Throughput, p50/p99, server CPU-seconds, and
#      Recached's AOF counters (batches, frames, write(2)s, waiters, ...).
#   B. the same at 4 workers (no Redis: it has one execution thread)
#   C. command mix at 50 clients, everysec, 1 and 4 workers:
#      SET (existing / new keys), INCR, HSET, MSET, GET
#   D. strace -c syscall counts at 1 and 50 clients (6l, 6m, Redis);
#      perf record call graphs at 50 clients if perf is usable
#   E. production soak (6L harness): 6l then 6m, capped reservations on and
#      off. Long: 4 x SOAK_SECS. Off unless PHASES includes E.
#   F. command CPU attribution (no persistence, 1 worker, 50 clients):
#      perf stat instructions/cycles per request and flat perf profiles for
#      LPUSH, SADD, MSET (and SET as the control), 6m vs Redis
# 6N-0 phases (the default):
#   G. release matrix, SET: 6l / 6m / 6m-off / Redis / Valkey; 1 and 4
#      workers; P=1 and 16; 1, 8, 50, 100 clients; AOF off / everysec /
#      always. Throughput, p50/p95/p99, CPU/op, RSS, AOF counters.
#   H. coverage 6M never measured: HGET and new-key INCR (Redis-comparable);
#      replication-enabled AOF; AOF with WebSocket subscribers and with
#      WATCH / QSUB observers (Recached-specific, reported apart)
#   I. rate-limited, equal-throughput latency (valkey-benchmark --rps;
#      skipped, and said so, if the built valkey-benchmark lacks it)
#   E. (opt-in) the soak, plus a fifth run with a binding memory cap
# and packs everything into one tarball.
#
# Usage (no root needed; nothing is installed system-wide):
#   NEW_REF=<40-hex dev SHA> bash run.sh                 # G H I, ~3-4 h
#   NEW_REF=<sha> PHASES="G" REPS=1 bash run.sh          # quick look
#   NEW_REF=<sha> PHASES="E" SOAK_SECS=2400 bash run.sh  # the soak (5 runs)
#   NEW_REF=<sha> NEW_BIN=./recached-server NEW_SHA256=<hex> bash run.sh
#                                         # test a recorded RC binary as-is
#   SERVER_CPUS=0-2 BENCH_CPUS=3-5 bash run.sh
#
# Before running, for numbers worth publishing: close other workloads,
# `sudo cpupower frequency-set -g performance`, stay on AC power. Run it
# natively, not inside a Flatpak/toolbox sandbox (the 6L runs were inside a
# Flatpak, which the reports flag).
#
# Prerequisites (Fedora):
#   sudo dnf install -y git gcc gcc-c++ make cmake clang perl python3 \
#       util-linux tar curl strace perf nodejs
#   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # if no cargo

set -euo pipefail

WORK=${WORK:-$HOME/recached-6n0-bench}
REPO=${REPO:-https://github.com/recached-sh/recached.git}
BASE_REF=${BASE_REF:-08e5a7d}                                 # dev after 6L (#120)
# The build under test must be named by commit: a branch name moves, and the
# old default (a feature branch) resolved to history dev never had.
NEW_REF=${NEW_REF:-}
if [[ ! $NEW_REF =~ ^[0-9a-f]{7,40}$ ]]; then
  echo "NEW_REF must be the commit SHA under test (7-40 hex digits), got '${NEW_REF:-unset}'." >&2
  echo "Example: NEW_REF=\$(git rev-parse origin/dev) bash run.sh" >&2
  exit 2
fi
NEW_BIN=${NEW_BIN:-}          # optional prebuilt binary of NEW_REF
NEW_SHA256=${NEW_SHA256:-}    # required with NEW_BIN; checked if set otherwise
if [[ -n $NEW_BIN && -z $NEW_SHA256 ]]; then echo "NEW_BIN needs NEW_SHA256" >&2; exit 2; fi
REDIS_VERSION=${REDIS_VERSION:-7.2.5}
VALKEY_VERSION=${VALKEY_VERSION:-9.1.0}
REPS=${REPS:-3}
REQUESTS=${REQUESTS:-1000000}
PHASES=${PHASES:-"G H I"}
RATES=${RATES:-"25000 50000 100000"}   # phase I, requests/s
SOAK_SECS=${SOAK_SECS:-2400}

# The kit's scripts come from this checkout, not from the tree under test, so
# any kit version can measure any commit (an RC cut before a kit change still
# gets the kit's workloads). Its commit is recorded with the results.
KIT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
[[ -f $KIT/6m/aof-profile.sh && -f $KIT/6l/soak/soak.sh && -f $KIT/6n-0/observers.mjs ]] || {
  echo "run.sh must be run from a recached checkout (benchmarks/v0.6.0/6m/linux/run.sh)" >&2; exit 2; }
KIT_SHA=$(git -C "$KIT" rev-parse HEAD 2>/dev/null || echo unknown)
KIT_DIRTY=$(git -C "$KIT" status --porcelain -- . 2>/dev/null | wc -l | tr -d ' ')

STAMP=$(date -u +%Y%m%dT%H%M%SZ)
RES="$WORK/results-$STAMP"
BIN="$WORK/bin"
mkdir -p "$WORK" "$RES" "$BIN"
exec > >(tee -a "$RES/run.log") 2>&1

say() { printf '\n=== %s  %s\n' "$(date -u +%H:%M:%S)" "$*"; }
need() { command -v "$1" >/dev/null || { echo "missing: $1 — see the prerequisites at the top of this script" >&2; exit 1; }; }
for tool in git cargo make gcc python3 taskset curl tar sha256sum lscpu; do need "$tool"; done
has() { [[ " $PHASES " == *" $1 "* ]]; }

NCPU=$(nproc)
if [[ -z ${SERVER_CPUS:-} || -z ${BENCH_CPUS:-} ]]; then
  half=$(( NCPU / 2 ))
  SERVER_CPUS=${SERVER_CPUS:-0-$(( half - 1 ))}
  BENCH_CPUS=${BENCH_CPUS:-$half-$(( NCPU - 1 ))}
fi
export PIN=1 SERVER_CPUS BENCH_CPUS

say "recording host conditions"
{
  echo "date_utc: $STAMP"
  echo "kernel: $(uname -srvm)"
  echo "os: $(. /etc/os-release && echo "$PRETTY_NAME")"
  echo "virtualization: $(systemd-detect-virt 2>/dev/null || echo unknown)"
  echo "sandbox: $([[ -n ${FLATPAK_ID:-} || -f /.flatpak-info ]] && echo flatpak || ([[ -f /run/.toolboxenv ]] && echo toolbox) || echo none)"
  echo "cpu_split: server=$SERVER_CPUS bench=$BENCH_CPUS"
  echo "reps: $REPS phases: $PHASES requests: $REQUESTS"
  echo "kit: $KIT_SHA ($KIT_DIRTY locally modified files under benchmarks/v0.6.0)"
  echo "refs: base=$BASE_REF new=$NEW_REF new_bin=${NEW_BIN:-built} new_sha256=${NEW_SHA256:-unpinned}"
  echo "rustc: $(rustc --version 2>/dev/null)  cargo: $(cargo --version 2>/dev/null)"
  echo "profile: release (--locked), debug=line-tables-only, strip=false (for perf); NEW_BIN as given"
  echo "governors: $(cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor 2>/dev/null | sort | uniq -c | tr '\n' ' ' || echo n/a)"
  echo "boost: $(cat /sys/devices/system/cpu/cpufreq/boost 2>/dev/null || cat /sys/devices/system/cpu/intel_pstate/no_turbo 2>/dev/null | sed 's/^/no_turbo=/' || echo n/a)"
  echo "load_at_start: $(cut -d' ' -f1-3 /proc/loadavg)"
  echo "memory: $(grep MemTotal /proc/meminfo)"
  echo "aof_fs: $(df -T "$WORK" | tail -1 | awk '{print $2, $1}')"
  echo "--- lscpu"; lscpu
  echo "--- top processes by CPU"; ps -eo pcpu,pid,comm --sort=-pcpu | head -15
} > "$RES/conditions.txt"

sha256_of() { sha256sum "$1" | cut -d' ' -f1; }
build_recached() {
  local label=$1 ref=$2 prebuilt=${3:-} tree="$WORK/src-$1"
  if [[ ! -d "$WORK/repo" ]]; then git clone --quiet "$REPO" "$WORK/repo"; fi
  git -C "$WORK/repo" fetch --quiet origin '+refs/heads/*:refs/remotes/origin/*'
  local sha
  sha=$(git -C "$WORK/repo" rev-parse --verify --quiet "$ref^{commit}") || {
    echo "$label: '$ref' does not resolve to a commit in $REPO" >&2; exit 2; }
  if [[ -d $tree ]]; then git -C "$tree" checkout --quiet --force --detach "$sha"
  else git -C "$WORK/repo" worktree add --quiet --detach "$tree" "$sha"; fi
  # Provenance: the tree is exactly that commit, with no local edits.
  [[ $(git -C "$tree" rev-parse HEAD) == "$sha" ]] || { echo "$label: $tree is not at $sha" >&2; exit 2; }
  [[ -z $(git -C "$tree" status --porcelain --untracked-files=no) ]] || {
    echo "$label: $tree has local changes; refusing to measure a dirty tree" >&2; exit 2; }
  if [[ -n $prebuilt ]]; then
    say "using prebuilt recached $label for $sha: $prebuilt"
    cp "$prebuilt" "$BIN/recached-server-$label"
  else
    say "building recached $label at $sha"
    # Its own target dir per label: a shared one can hand back a binary
    # built from another tree. line-tables-only + unstripped for perf.
    (cd "$tree" && CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_STRIP=false \
      CARGO_TARGET_DIR="$WORK/target-$label" cargo build --release --locked -p recached --bin recached-server)
    cp "$WORK/target-$label/release/recached-server" "$BIN/recached-server-$label"
  fi
  local sum; sum=$(sha256_of "$BIN/recached-server-$label")
  if [[ $label == 6m && -n $NEW_SHA256 && $sum != "$NEW_SHA256" ]]; then
    echo "6m: binary sha256 $sum does not match NEW_SHA256 $NEW_SHA256" >&2; exit 2
  fi
  echo "$sum" > "$BIN/recached-server-$label.sha256"
  echo "$label $sha sha256=$sum $("$BIN/recached-server-$label" --version)" >> "$RES/binaries.txt"
}
build_recached 6l "$BASE_REF"
build_recached 6m "$NEW_REF" "$NEW_BIN"
# Before every phase: the binaries are still the ones recorded above.
verify_bins() {
  local label
  for label in 6l 6m; do
    [[ $(sha256_of "$BIN/recached-server-$label") == "$(cat "$BIN/recached-server-$label.sha256")" ]] || {
      echo "recached-server-$label changed since it was recorded; stopping" >&2; exit 3; }
  done
}

build_kv() {
  local name=$1 version=$2 url=$3 dir="$WORK/$1-$2"
  if [[ ! -x "$BIN/$name-server" ]]; then
    say "building $name $version"
    mkdir -p "$dir"
    curl -fsSL "$url" | tar -xz -C "$dir" --strip-components=1
    make -C "$dir" -j"$NCPU" BUILD_TLS=no >"$RES/build-$name.log" 2>&1 || {
      make -C "$dir" distclean >/dev/null 2>&1 || true
      make -C "$dir" -j"$NCPU" BUILD_TLS=no CC="gcc -std=gnu17" >>"$RES/build-$name.log" 2>&1
    }
    cp "$dir/src/$name-server" "$BIN/$name-server"
    if [[ $name == redis ]]; then cp "$dir/src/redis-benchmark" "$dir/src/redis-cli" "$BIN/"; fi
    if [[ $name == valkey ]]; then cp "$dir/src/valkey-benchmark" "$BIN/" 2>/dev/null || true; fi
  fi
  echo "$name $("$BIN/$name-server" --version)" >> "$RES/binaries.txt"
}
build_kv redis "$REDIS_VERSION" "https://download.redis.io/releases/redis-$REDIS_VERSION.tar.gz"
build_kv valkey "$VALKEY_VERSION" "https://github.com/valkey-io/valkey/archive/refs/tags/$VALKEY_VERSION.tar.gz"
(cd "$BIN" && sha256sum ./*) >> "$RES/binaries.txt"
export BENCH="$BIN/redis-benchmark" CLI="$BIN/redis-cli"
PROFILE="$KIT/6m/aof-profile.sh"

# 6m-off: the 6M binary with coalescing switched off — the same code as 6M
# otherwise, so 6m vs 6m-off isolates the change and 6l vs 6m-off isolates
# the instrumentation.
wrap_off="$BIN/recached-server-6m-off"
printf '#!/bin/sh\nRECACHED_AOF_COALESCE=off exec "%s" "$@"\n' "$BIN/recached-server-6m" > "$wrap_off"
chmod +x "$wrap_off"

profile() {
  local out=$1; shift
  env RECACHED_BIN="$BIN/recached-server-6m" BASE_BIN="$BIN/recached-server-6l" \
    REDIS_BIN="$BIN/redis-server" VALKEY_BIN="$BIN/valkey-server" OUT="$out" REPS="$REPS" \
    REQUESTS="$REQUESTS" "$@" bash "$PROFILE"
}
# `off` is a server label the profile script does not know: run it as `base`
# with BASE_BIN pointed at the wrapper.
profile_off() {
  local out=$1; shift
  env RECACHED_BIN="$BIN/recached-server-6m" BASE_BIN="$wrap_off" OUT="$out" REPS="$REPS" \
    REQUESTS="$REQUESTS" SERVERS=base "$@" bash "$PROFILE"
}

if has G; then
  verify_bins
  say "G: 6N-0 release matrix (SET)"
  profile "$RES/G-matrix" SERVERS="base recached redis valkey" WORKERS="1 4" \
    POLICIES="none everysec always" PIPELINES="1 16" CLIENTS="1 8 50 100" TESTS=set
  profile_off "$RES/G-matrix-6m-off" WORKERS="1 4" POLICIES="everysec always" PIPELINES="1 16" \
    CLIENTS="1 8 50 100" TESTS=set
fi
if has H; then
  verify_bins
  say "H: coverage 6M never measured"
  # Redis-comparable commands.
  profile "$RES/H-hget-incrnew" SERVERS="base recached redis valkey" WORKERS="1 4" \
    POLICIES="none everysec" PIPELINES="1 16" CLIENTS="1 8 50 100" TESTS="hget incr-new"
  # Recached-specific: every write on the per-command append path.
  profile "$RES/H-repl-aof" SERVERS="base recached" WORKERS="1 4" POLICIES="everysec always" \
    PIPELINES="1 16" CLIENTS="1 8 50 100" TESTS=set EXTRA_ENV="RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=6710"
  profile "$RES/H-ws-subscribers" SERVERS="base recached" WORKERS="1 4" POLICIES="everysec" \
    PIPELINES="1 16" CLIENTS="8 50" TESTS=set OBSERVER="ws-sync:200:key:*"
  profile "$RES/H-watch-observers" SERVERS="base recached" WORKERS="1 4" POLICIES="everysec" \
    PIPELINES="1 16" CLIENTS="8 50" TESTS=set OBSERVER="tcp-watch:50:200"
  profile "$RES/H-qsub-observers" SERVERS="base recached" WORKERS="1 4" POLICIES="everysec" \
    PIPELINES="1 16" CLIENTS="8 50" TESTS=set OBSERVER="ws-qsub:100:key:0000000000*"
fi
if has I; then
  verify_bins
  say "I: rate-limited, equal-throughput latency"
  if [[ -x $BIN/valkey-benchmark ]] && "$BIN/valkey-benchmark" --help 2>&1 | grep -q -- '--rps'; then
    for rate in $RATES; do
      profile "$RES/I-rate-$rate" SERVERS="base recached redis valkey" WORKERS=1 POLICIES="none everysec" \
        PIPELINES=1 CLIENTS="50 100" TESTS=set RATE="$rate" RATE_BENCH="$BIN/valkey-benchmark"
    done
  else
    echo "valkey-benchmark $VALKEY_VERSION has no --rps: no rate-limited run (not measured)" | tee "$RES/I-skipped.txt"
  fi
fi

if has A; then
  say "A: AOF profile, 1 worker"
  profile "$RES/A-1w" SERVERS="base recached redis valkey" WORKERS=1 \
    POLICIES="everysec no always" PIPELINES="1 16" CLIENTS="1 2 4 8 16 32 50 100 200" TESTS=set
  profile_off "$RES/A-1w-6m-off" WORKERS=1 POLICIES="everysec" PIPELINES="1 16" \
    CLIENTS="1 2 4 8 16 32 50 100 200" TESTS=set
  # With a replication listener every write takes the per-command append
  # path (no connection gate): measured, not changed, by 6M-B.
  profile "$RES/A-1w-repl" SERVERS="base recached" WORKERS="1 4" POLICIES="everysec" \
    PIPELINES="1 16" CLIENTS="1 50" TESTS=set EXTRA_ENV="RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT=6710"
fi
if has B; then
  say "B: AOF profile, 4 workers"
  profile "$RES/B-4w" SERVERS="base recached valkey" WORKERS=4 \
    POLICIES="everysec always" PIPELINES="1 16" CLIENTS="1 8 50 100 200" TESTS=set
fi
if has C; then
  say "C: command mix at 50 clients, everysec"
  profile "$RES/C-mix" SERVERS="base recached redis valkey" WORKERS="1 4" \
    POLICIES="everysec none" PIPELINES="1 16" CLIENTS=50 TESTS="set set-new incr hset mset get"
fi
if has D; then
  say "D: syscall counts and call graphs"
  profile "$RES/D-strace" SERVERS="base recached redis" WORKERS=1 REPS=1 REQUESTS=200000 \
    POLICIES="everysec" PIPELINES=1 CLIENTS="1 50" TESTS=set STRACE=1
  if command -v perf >/dev/null && perf record -o /dev/null -- true >/dev/null 2>&1; then
    for label in 6l 6m; do
      dir="$RES/D-perf/$label"; mkdir -p "$dir/data"
      RECACHED_BIND=127.0.0.1 RECACHED_PORT=6790 RECACHED_WS_PORT=6791 RECACHED_METRICS_PORT=0 \
        RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$dir/data/dump.rdb" RECACHED_WORKER_THREADS=1 \
        RECACHED_AOF_PATH="$dir/data/appendonly.aof" RECACHED_AOF_SYNC=everysec \
        taskset -c "$SERVER_CPUS" "$BIN/recached-server-$label" >"$dir/server.log" 2>&1 &
      spid=$!; sleep 1
      perf record -F 999 -g -p "$spid" -o "$dir/perf.data" -- sleep 12 >/dev/null 2>&1 &
      ppid=$!
      taskset -c "$BENCH_CPUS" "$BENCH" -p 6790 -n 100000000 -c 50 -P 1 -d 32 -r 100000 -t set --threads 2 -q >/dev/null 2>&1 &
      bpid=$!
      wait "$ppid" || true
      kill "$bpid" 2>/dev/null || true; kill -9 "$spid" 2>/dev/null || true
      perf report -i "$dir/perf.data" --no-children --stdio --percent-limit 0.5 > "$dir/report.txt" 2>/dev/null || true
      perf report -i "$dir/perf.data" --no-children --stdio --sort dso,sym -g none --percent-limit 0.3 > "$dir/flat.txt" 2>/dev/null || true
      rm -f "$dir/perf.data"
    done
  else
    echo "perf unavailable or not permitted (kernel.perf_event_paranoid=$(cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null)); skipping call graphs" | tee "$RES/D-perf-skipped.txt"
  fi
fi

if has F; then
  say "F: command CPU attribution (perf)"
  if command -v perf >/dev/null && perf stat -e instructions -- true >/dev/null 2>&1; then
    for server in 6m redis; do
      for test in set lpush sadd mset; do
        for P in 1 16; do
          dir="$RES/F-cmd/$server/$test-P$P"; mkdir -p "$dir/data"
          if [[ $server == redis ]]; then
            taskset -c "$SERVER_CPUS" "$BIN/redis-server" --port 6795 --bind 127.0.0.1 --save "" \
              --appendonly no --dir "$dir/data" --protected-mode no >"$dir/server.log" 2>&1 &
          else
            RECACHED_BIND=127.0.0.1 RECACHED_PORT=6795 RECACHED_WS_PORT=6796 RECACHED_METRICS_PORT=0 \
              RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$dir/data/dump.rdb" RECACHED_WORKER_THREADS=1 \
              taskset -c "$SERVER_CPUS" "$BIN/recached-server-$server" >"$dir/server.log" 2>&1 &
          fi
          spid=$!; sleep 1
          n=2000000; [[ $test == mset ]] && n=400000
          perf stat -x, -e task-clock,instructions,cycles,context-switches -p "$spid" -o "$dir/stat.csv" &
          statpid=$!
          taskset -c "$BENCH_CPUS" "$BENCH" -p 6795 -n "$n" -c 50 -P "$P" -d 32 -r 100000 -t "$test" \
            --threads 2 --csv > "$dir/bench.csv" 2>/dev/null || true
          kill -INT "$statpid" 2>/dev/null || true; wait "$statpid" 2>/dev/null || true
          echo "requests,$n" >> "$dir/stat.csv"
          if [[ $P == 16 ]]; then
            perf record -F 1999 -g -p "$spid" -o "$dir/perf.data" -- sleep 8 >/dev/null 2>&1 &
            ppid=$!
            taskset -c "$BENCH_CPUS" "$BENCH" -p 6795 -n 100000000 -c 50 -P 16 -d 32 -r 100000 -t "$test" \
              --threads 2 -q >/dev/null 2>&1 &
            bpid=$!
            wait "$ppid" || true; kill "$bpid" 2>/dev/null || true
            perf report -i "$dir/perf.data" --no-children --stdio --sort sym -g none --percent-limit 0.3 \
              > "$dir/flat.txt" 2>/dev/null || true
            perf report -i "$dir/perf.data" --children --stdio --sort sym -g none --percent-limit 1 \
              > "$dir/inclusive.txt" 2>/dev/null || true
            rm -f "$dir/perf.data"
          fi
          kill -9 "$spid" 2>/dev/null || true; wait "$spid" 2>/dev/null || true
        done
      done
    done
  else
    echo "perf unavailable or not permitted (kernel.perf_event_paranoid=$(cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null)); try: sudo sysctl kernel.perf_event_paranoid=1" | tee "$RES/F-skipped.txt"
  fi
fi

if has E; then
  verify_bins
  say "E: production soak, 6l and 6m ($NEW_REF), reservations on and off, + binding memory cap (5 x ${SOAK_SECS}s)"
  SOAK="$KIT/6l/soak/soak.sh"
  for label in 6l 6m; do
    for res in on off; do
      out="$RES/E-soak/$label-reservations-$res"
      SERVER_BIN="$BIN/recached-server-$label" RESERVATIONS=$res SOAK_SECS="$SOAK_SECS" OUT="$out" \
        bash "$SOAK" || echo "soak $label reservations=$res: FAIL (see $out/verdict.txt)"
    done
  done
  # Run 5: the memory limit, not the key cap, binds (MEMORY_BINDING=1).
  out="$RES/E-soak/6m-memory-binding-reservations-on"
  SERVER_BIN="$BIN/recached-server-6m" RESERVATIONS=on MEMORY_BINDING=1 SOAK_SECS="$SOAK_SECS" OUT="$out" \
    bash "$SOAK" || echo "soak 6m memory-binding: FAIL (see $out/verdict.txt)"
fi

for d in "$RES"/*/; do
  [[ -f $d/results.csv ]] && python3 "$KIT/6m/summarize.py" "$d" > "$d/summary.md" 2>&1 || true
done
echo "load_at_end: $(cut -d' ' -f1-3 /proc/loadavg)" >> "$RES/conditions.txt"
host=$(hostname -s 2>/dev/null || cat /etc/hostname 2>/dev/null || echo linux)
verify_bins
tarball="$WORK/recached-6n0-linux-${host%%.*}-$STAMP.tar.gz"
tar -czf "$tarball" -C "$WORK" "$(basename "$RES")"
say "done. Send back: $tarball ($(du -h "$tarball" | cut -f1)); raw data belongs under benchmarks/v0.6.0/6n-0/"
