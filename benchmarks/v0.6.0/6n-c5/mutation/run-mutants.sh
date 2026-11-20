#!/usr/bin/env bash
# 6N-C5 PR 2 mutants of the replica sender's deferred flush. Each must make
# `cargo test -p recached --bins repl_sender_batching` fail. Run from the
# repository root on a clean tree; every mutant is reverted afterwards.
set -uo pipefail
F=server-native/src/replication.rs
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-target}
LOGS=${LOGS:-$(mktemp -d)}
mutate() { # name, old, new
  local name=$1 old=$2 new=$3
  cp "$F" "$F.orig"
  python3 - "$F" "$old" "$new" <<'PY'
import sys
p, old, new = sys.argv[1:]
s = open(p).read()
assert s.count(old) == 1, f"pattern not unique: {old!r}"
open(p, "w").write(s.replace(old, new))
PY
  if cargo test -q -p recached --bins repl_sender_batching >"$LOGS/$name.log" 2>&1; then
    echo "SURVIVED $name"
  else
    echo "killed   $name by: $(grep -oE '^    tests::repl_sender_batching::[a-z_]+$' "$LOGS/$name.log" | sed 's/.*:://' | sort -u | tr '\n' ' ')"
  fi
  mv "$F.orig" "$F"
  # The restored copy predates the mutant's build; without a fresh mtime
  # cargo keeps running the last mutant's binary.
  touch "$F"
}
# Flush on every wake (the pre-6N-C5 sender).
mutate flush-every-wake "                } else if !flush_pending {" "                } else if { wr.flush().await?; replicas.record_send(unflushed, false); unflushed = 0; false } {"
# Stop draining while a flush is held (the first 6N-C5 design, review R1).
mutate no-drain-while-held "            frame = rx.recv() => {" "            frame = rx.recv(), if !flush_pending => {"
mutate no-drain "                    match rx.try_recv() {" "                    match Err::<QueuedReplFrame, ()>(()) {"
mutate uncapped-buffer "                while unflushed < REPL_SEND_BATCH_FRAMES {" "                while unflushed < usize::MAX {"
mutate never-flush-at-round-end "                wr.flush().await?;
                replicas.record_send(unflushed, true);" "                replicas.record_send(unflushed, true);"
# Equivalent: re-arming the round-end wait on every drain is the mitigation
# review S1 suggested; it changes when a held flush goes out, not what any
# test can observe (and did not help the R1 reproduction). Expected to survive.
mutate re-arm-each-wake "                } else if !flush_pending {" "                } else if true {"
mutate no-flush-count "                replicas.record_send(unflushed, true);" "                let _ = unflushed;"
mutate frame-count-off-by-one "                replicas.record_send(unflushed, false);" "                replicas.record_send(unflushed + 1, false);"
mutate no-small-channel-guard "                    || rx.max_capacity() < REPL_SEND_DEFER_MIN_CAPACITY" "                    || rx.max_capacity() < 1"
