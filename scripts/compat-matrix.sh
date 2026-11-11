#!/usr/bin/env bash
# Cross-version compatibility matrix: this checkout ("new") against a released
# tag ("old", default v0.4.0).
#
#   scripts/compat-matrix.sh            # OLD_REF=v0.4.0
#   OLD_REF=v0.4.0 COMPAT_PORT_BASE=7700 scripts/compat-matrix.sh
#
# What it runs, each as real processes on loopback:
#
#   clients   the recached-embed and recached-mobile live suites of each version
#             against a server of each version (old/old is the control: a
#             failure there is the old release's own, not a compatibility one)
#   replicas  new primary -> old replica, and old primary -> new replica
#   AOF       old AOF replayed by the new server (upgrade); a new AOF holding
#             DEDUP records offered to the old server (must refuse to start,
#             not skip them); the same after SAVE (the documented downgrade)
#
# Client failures are compared with scripts/compat-expected-failures.txt. A
# failure not listed there fails the run; a listed one that passes is reported
# as stale so the list cannot quietly outlive the limitation it records.
#
# The old ref is built in a git worktree under $COMPAT_DIR (default
# target/compat), so repeated runs reuse its build. Needs redis-cli and nc.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OLD_REF="${OLD_REF:-v0.4.0}"
COMPAT_DIR="${COMPAT_DIR:-$ROOT/target/compat}"
EXPECTED="$ROOT/scripts/compat-expected-failures.txt"
PORT_BASE="${COMPAT_PORT_BASE:-7700}"
OLD_DIR="$COMPAT_DIR/$OLD_REF"
RUN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/recached-compat.XXXXXX")"
LOG_DIR="${COMPAT_LOG_DIR:-$RUN_DIR/logs}"
mkdir -p "$LOG_DIR"

for tool in redis-cli nc cargo git; do
  command -v "$tool" >/dev/null || { echo "compat-matrix: '$tool' is required" >&2; exit 2; }
done

# Servers start inside $(...), so their pids go to a file, not an array the
# subshell could not hand back.
PIDFILE="$RUN_DIR/pids"
: >"$PIDFILE"
cleanup() {
  while read -r pid; do kill "$pid" 2>/dev/null || true; done <"$PIDFILE"
}
trap cleanup EXIT

say() { printf '\n== %s\n' "$*"; }

# ── builds ───────────────────────────────────────────────────────────────────

if [ ! -d "$OLD_DIR" ]; then
  say "worktree $OLD_REF -> $OLD_DIR"
  git -C "$ROOT" worktree add --detach "$OLD_DIR" "$OLD_REF" >/dev/null
fi

build() {
  local dir="$1"
  say "build $(git -C "$dir" describe --tags --always --dirty)"
  (cd "$dir" &&
    cargo build --locked -q -p recached --bin recached-server &&
    cargo test --locked -q --no-run -p recached-embed --test live -p recached-mobile --test live 2>&1 | tail -n 3)
}
build "$OLD_DIR"
build "$ROOT"

server_bin() { if [ "$1" = old ]; then echo "$OLD_DIR/target/debug/recached-server"; else echo "$ROOT/target/debug/recached-server"; fi; }
src_dir() { if [ "$1" = old ]; then echo "$OLD_DIR"; else echo "$ROOT"; fi; }

# start_server <version> <name> <tcp> <ws> [VAR=value...]: starts it in its own
# directory, waits for both ports, and prints its pid.
start_server() {
  local version="$1" name="$2" tcp="$3" ws="$4"
  shift 4
  local dir="$RUN_DIR/$name"
  mkdir -p "$dir"
  # exec, so $! is the server itself; every fd redirected, so the $(...) this
  # runs in is not held open until the server exits.
  (cd "$dir" && exec env RECACHED_BIND=127.0.0.1 RECACHED_PORT="$tcp" RECACHED_WS_PORT="$ws" \
    RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$dir/dump.rdb" "$@" \
    "$(server_bin "$version")") >>"$LOG_DIR/server-$name.log" 2>&1 </dev/null &
  local pid=$!
  echo "$pid" >>"$PIDFILE"
  for _ in $(seq 1 100); do
    if nc -z 127.0.0.1 "$tcp" 2>/dev/null && nc -z 127.0.0.1 "$ws" 2>/dev/null; then
      echo "$pid"
      return 0
    fi
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.1
  done
  echo "server $name ($version) did not start; log:" >&2
  cat "$LOG_DIR/server-$name.log" >&2
  return 1
}

# Not our child when started inside $(...), so poll rather than `wait`.
wait_gone() { for _ in $(seq 1 100); do kill -0 "$1" 2>/dev/null || return 0; sleep 0.05; done; }
stop_server() { kill "$1" 2>/dev/null || true; wait_gone "$1"; }
kill9_server() { kill -9 "$1" 2>/dev/null || true; wait_gone "$1"; }
cli() { local port="$1"; shift; redis-cli -h 127.0.0.1 -p "$port" "$@"; }

FAILURES=0
fail() { echo "FAIL  $*"; FAILURES=$((FAILURES + 1)); }
ok() { echo "ok    $*"; }

# ── client suites ────────────────────────────────────────────────────────────

expected_failure() { # <client> <server> <suite> <test>
  [ -f "$EXPECTED" ] && grep -qE "^$1[[:space:]]+$2[[:space:]]+$3[[:space:]]+$4([[:space:]]|$)" "$EXPECTED"
}

SUMMARY="$RUN_DIR/summary.txt"
: >"$SUMMARY"

run_suite() { # <client> <server> <suite> <ws-url>
  local client="$1" server="$2" suite="$3" url="$4" var log
  case "$suite" in
    embed) var=RECACHED_EMBED_TEST_URL ;;
    mobile) var=RECACHED_MOBILE_TEST_URL ;;
  esac
  log="$LOG_DIR/client-$client-server-$server-$suite.log"
  (cd "$(src_dir "$client")" && env "$var=$url" \
    cargo test --locked -q -p "recached-$suite" --test live -- --test-threads=4 >"$log" 2>&1) || true
  local result passed failed
  result="$(grep -E '^test result:' "$log" | tail -n 1 || true)"
  passed="$(sed -nE 's/.* ([0-9]+) passed.*/\1/p' <<<"$result")"
  failed="$(sed -nE 's/.* ([0-9]+) failed.*/\1/p' <<<"$result")"
  if [ -z "$result" ]; then
    fail "client=$client server=$server $suite: suite did not run (see $log)"
    printf '%-6s %-6s %-6s %s\n' "$client" "$server" "$suite" "did not run" >>"$SUMMARY"
    return
  fi
  printf '%-6s %-6s %-6s passed=%s failed=%s\n' "$client" "$server" "$suite" "${passed:-0}" "${failed:-0}" >>"$SUMMARY"
  local name
  # Rust prints each failure as `test <name> ... FAILED`.
  while read -r name; do
    [ -n "$name" ] || continue
    if expected_failure "$client" "$server" "$suite" "$name"; then
      ok "client=$client server=$server $suite: $name failed, as expected"
    else
      fail "client=$client server=$server $suite: $name (unexpected; see $log)"
    fi
  done < <(sed -nE 's/^test ([A-Za-z0-9_:]+) \.\.\. FAILED$/\1/p' "$log")
  # Listed failures for this cell that passed.
  if [ -f "$EXPECTED" ]; then
    while read -r c s su t _; do
      [ "$c" = "$client" ] && [ "$s" = "$server" ] && [ "$su" = "$suite" ] || continue
      if grep -qE "^test $t \.\.\. ok$" "$log"; then
        echo "STALE client=$client server=$server $suite: $t is listed as an expected failure but passed"
      fi
    done < <(grep -vE '^[[:space:]]*(#|$)' "$EXPECTED")
  fi
}

port=$PORT_BASE
for server in old new; do
  tcp=$port ws=$((port + 1))
  port=$((port + 10))
  say "server=$server on ws://127.0.0.1:$ws"
  pid="$(start_server "$server" "clients-$server" "$tcp" "$ws")"
  for client in old new; do
    for suite in embed mobile; do
      run_suite "$client" "$server" "$suite" "ws://127.0.0.1:$ws"
    done
  done
  stop_server "$pid"
done

# ── replication ──────────────────────────────────────────────────────────────

replicate() { # <primary-version> <replica-version>
  local p="$1" r="$2"
  local ptcp=$port pws=$((port + 1)) prepl=$((port + 2)) rtcp=$((port + 3)) rws=$((port + 4))
  port=$((port + 10))
  local ppid rpid key="compat:repl:$p-to-$r"
  ppid="$(start_server "$p" "repl-primary-$p" "$ptcp" "$pws" RECACHED_REPL_ENABLE=1 RECACHED_REPL_PORT="$prepl")"
  rpid="$(start_server "$r" "repl-replica-$r" "$rtcp" "$rws" RECACHED_REPLICAOF="127.0.0.1:$prepl")"
  cli "$ptcp" SET "$key" before >/dev/null
  cli "$ptcp" INCRBY "$key:n" 5 >/dev/null
  local got="" n=""
  for _ in $(seq 1 50); do
    got="$(cli "$rtcp" GET "$key")"
    n="$(cli "$rtcp" GET "$key:n")"
    [ "$got" = before ] && [ "$n" = 5 ] && break
    sleep 0.1
  done
  if [ "$got" = before ] && [ "$n" = 5 ]; then
    ok "replication $p primary -> $r replica"
    printf 'replication %s->%s ok\n' "$p" "$r" >>"$SUMMARY"
  else
    fail "replication $p primary -> $r replica: replica has '$got' / '$n' (logs: $LOG_DIR/server-repl-*)"
    printf 'replication %s->%s FAILED\n' "$p" "$r" >>"$SUMMARY"
  fi
  # A replica refuses writes whatever the version mix.
  if cli "$rtcp" SET "$key" from-replica 2>&1 | grep -q READONLY; then
    ok "$r replica of a $p primary refuses writes"
  else
    fail "$r replica of a $p primary accepted a write"
  fi
  stop_server "$rpid"
  stop_server "$ppid"
}

say "replication"
replicate new old
replicate old new

# ── AOF upgrade and downgrade ────────────────────────────────────────────────

say "AOF"
aof_env() { echo "RECACHED_AOF_PATH=$RUN_DIR/$1/appendonly.aof RECACHED_AOF_SYNC=always"; }

# Upgrade: writes held only in an old AOF (no snapshot) replay on the new server.
tcp=$port ws=$((port + 1)); port=$((port + 10))
mkdir -p "$RUN_DIR/aof-up"
# shellcheck disable=SC2046
pid="$(start_server old aof-up "$tcp" "$ws" $(aof_env aof-up))"
cli "$tcp" SET compat:aof:up v >/dev/null
cli "$tcp" INCRBY compat:aof:up:n 3 >/dev/null
kill9_server "$pid"
# shellcheck disable=SC2046
pid="$(start_server new aof-up "$tcp" "$ws" $(aof_env aof-up))"
if [ "$(cli "$tcp" GET compat:aof:up)" = v ] && [ "$(cli "$tcp" GET compat:aof:up:n)" = 3 ]; then
  ok "an $OLD_REF AOF replays on the new server"
  echo "aof old->new ok" >>"$SUMMARY"
else
  fail "an $OLD_REF AOF did not replay on the new server"
  echo "aof old->new FAILED" >>"$SUMMARY"
fi
stop_server "$pid"

# Downgrade without SAVE: the new AOF holds DEDUP records the old server cannot
# replay. It must refuse to start, not start without them.
tcp=$port ws=$((port + 1)); port=$((port + 10))
mkdir -p "$RUN_DIR/aof-down"
# shellcheck disable=SC2046
pid="$(start_server new aof-down "$tcp" "$ws" $(aof_env aof-down))"
cli "$tcp" DEDUP compat-client 1 SET compat:aof:down v >/dev/null
cli "$tcp" DEDUP compat-client 2 INCRBY compat:aof:down:n 7 >/dev/null
kill9_server "$pid"
dir="$RUN_DIR/aof-down"
# shellcheck disable=SC2046
(cd "$dir" && exec env RECACHED_BIND=127.0.0.1 RECACHED_PORT="$tcp" RECACHED_WS_PORT="$ws" RECACHED_METRICS_PORT=0 \
  RECACHED_SAVE_INTERVAL=0 RECACHED_SAVE_PATH="$dir/dump.rdb" $(aof_env aof-down) \
  "$(server_bin old)") >"$LOG_DIR/server-aof-down-old.log" 2>&1 </dev/null &
pid=$!
echo "$pid" >>"$PIDFILE"
started=0
for _ in $(seq 1 50); do
  if ! kill -0 "$pid" 2>/dev/null; then break; fi
  if nc -z 127.0.0.1 "$tcp" 2>/dev/null; then started=1; break; fi
  sleep 0.1
done
if [ "$started" = 1 ]; then
  fail "$OLD_REF started on an AOF holding DEDUP records (got '$(cli "$tcp" GET compat:aof:down)')"
  echo "aof new->old (no SAVE) STARTED — unsafe" >>"$SUMMARY"
  stop_server "$pid"
else
  ok "$OLD_REF refuses to start on an AOF holding DEDUP records"
  echo "aof new->old (no SAVE) refused to start" >>"$SUMMARY"
fi

# Downgrade with SAVE, as documented: SAVE truncates the AOF, and the old
# server starts from the snapshot.
# shellcheck disable=SC2046
pid="$(start_server new aof-down "$tcp" "$ws" $(aof_env aof-down))"
cli "$tcp" SAVE >/dev/null
stop_server "$pid"
# shellcheck disable=SC2046
pid="$(start_server old aof-down "$tcp" "$ws" $(aof_env aof-down))"
if [ "$(cli "$tcp" GET compat:aof:down)" = v ] && [ "$(cli "$tcp" GET compat:aof:down:n)" = 7 ]; then
  ok "after SAVE, $OLD_REF starts with the new server's data"
  echo "aof new->old (after SAVE) ok" >>"$SUMMARY"
else
  fail "after SAVE, $OLD_REF is missing the new server's data"
  echo "aof new->old (after SAVE) FAILED" >>"$SUMMARY"
fi
stop_server "$pid"

say "summary (old=$OLD_REF, new=$(git -C "$ROOT" rev-parse --short HEAD)$(git -C "$ROOT" diff --quiet || echo -dirty)); logs in $LOG_DIR"
echo "client server suite  result"
cat "$SUMMARY"
if [ "$FAILURES" -gt 0 ]; then
  echo "$FAILURES failure(s)"
  exit 1
fi
echo "all checks passed"
