#!/usr/bin/env bash
# 6N-C6 PR 2 mutants of the byte-sized replica queue. Each must make
# `cargo test -p recached --all-features --bins` over the queue tests fail.
# Run from the repository root on a clean tree; every mutant is reverted.
set -uo pipefail
F=server-native/src/replication.rs
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-target}
LOGS=${LOGS:-$(mktemp -d)}
FILTERS=(repl_sender_batching info_replication_reports_both_redis_and_recached_spellings replica_queue_enforces_bytes)
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
  if timeout 900 cargo test -q -p recached --all-features --bins -- "${FILTERS[@]}" >"$LOGS/$name.log" 2>&1; then
    echo "SURVIVED $name"
  else
    echo "killed   $name by: $(grep -oE '^    (tests|replication)::[a-z_0-9:]+$' "$LOGS/$name.log" | sed 's/.*:://' | sort -u | tr '\n' ' ')"
  fi
  mv "$F.orig" "$F"
  # The restored copy predates the mutant's build; without a fresh mtime
  # cargo keeps running the last mutant's binary.
  touch "$F"
}
# The pre-6N-C6 default: a 4096-frame cap whatever the budget.
mutate old-default-cap "        (queue_bytes / REPL_QUEUE_FRAME_OVERHEAD).clamp(1, REPL_QUEUE_MAX_DERIVED_FRAMES)" "        4096"
# The pre-6N-C6 charge: the payload only.
mutate no-overhead-charge "    len.saturating_add(REPL_QUEUE_FRAME_OVERHEAD)" "    len"
# A wrapping charge: a huge frame would charge almost nothing.
mutate wrapping-charge "    len.saturating_add(REPL_QUEUE_FRAME_OVERHEAD)" "    len.wrapping_add(REPL_QUEUE_FRAME_OVERHEAD)"
# An explicit RECACHED_REPL_BUFFER is ignored.
mutate explicit-ignored "    explicit.unwrap_or_else(|| {" "    None::<usize>.or(explicit.filter(|_| false)).unwrap_or_else(|| {"
mutate no-min-clamp "(queue_bytes / REPL_QUEUE_FRAME_OVERHEAD).clamp(1, REPL_QUEUE_MAX_DERIVED_FRAMES)" "(queue_bytes / REPL_QUEUE_FRAME_OVERHEAD).min(REPL_QUEUE_MAX_DERIVED_FRAMES)"
mutate no-max-clamp "(queue_bytes / REPL_QUEUE_FRAME_OVERHEAD).clamp(1, REPL_QUEUE_MAX_DERIVED_FRAMES)" "(queue_bytes / REPL_QUEUE_FRAME_OVERHEAD).max(1)"
# The budget admits one charge too few.
mutate budget-off-by-one "                        .filter(|next| *next <= r.max_queue_bytes)" "                        .filter(|next| *next < r.max_queue_bytes)"
# Dropping a queued frame releases only its payload, leaking the overhead.
mutate release-payload-only "            .fetch_sub(self.charged_bytes, Ordering::AcqRel);" "            .fetch_sub(self.frame.bytes.len(), Ordering::AcqRel);"
# The observability the release gate reads.
mutate no-bytes-disconnect-count "                warn!(\"Replica byte queue full — disconnecting so it can resync\");
                self.queue_disconnects.fetch_add(1, Ordering::Relaxed);" "                warn!(\"Replica byte queue full — disconnecting so it can resync\");"
mutate no-items-disconnect-count "                );
                self.queue_disconnects.fetch_add(1, Ordering::Relaxed);" "                );"
mutate no-bytes-peak "        self.send_queue_bytes_peak
            .fetch_max(queued_bytes as u64, Ordering::Relaxed);" "        let _ = queued_bytes;"
mutate no-partial-sync-count "        replicas.syncs_partial.fetch_add(1, Ordering::Relaxed);" ""
mutate no-full-sync-count "        replicas.syncs_full.fetch_add(1, Ordering::Relaxed);" ""
# Review M1: an unregistered replica's writes are never refused, so its
# sender stays blocked on a peer that stopped reading.
mutate unregistered-never-closed "            if std::pin::Pin::new(handle).poll(cx).is_pending() {" "            if { let _ = std::pin::Pin::new(handle).poll(cx); true } {"
echo "logs: $LOGS"
