#!/usr/bin/env bash
# 6N-C6 PR 3 mutants of the replica sender's acknowledgement read. Each must
# make `cargo test -p recached --all-features --bins` over the ACK tests fail.
# Run from the repository root on a clean tree; every mutant is reverted.
set -uo pipefail
F=server-native/src/replication.rs
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-target}
LOGS=${LOGS:-$(mktemp -d)}
FILTERS=(repl_ack_read ack_reader_keeps_partial_acknowledgements)
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
  if timeout 600 cargo test -q -p recached --all-features --bins -- "${FILTERS[@]}" >"$LOGS/$name.log" 2>&1; then
    echo "SURVIVED $name"
  else
    echo "killed   $name by: $(grep -oE '^    (tests::repl_ack_read|replication::write_order_tests)::[a-z_]+$' "$LOGS/$name.log" | sed 's/.*:://' | sort -u | tr '\n' ' ')"
  fi
  mv "$F.orig" "$F"
  # The restored copy predates the mutant's build; without a fresh mtime
  # cargo keeps running the last mutant's binary.
  touch "$F"
}
# The pre-6N-C6 read: `read_exact` into a fresh buffer each iteration, so a
# partial acknowledgement is lost when another branch wins (review F4).
mutate read-exact "            res = rd.read(acks.spare()) => {
                match res {" "            res = async { let mut b = [0u8; 8]; rd.read_exact(&mut b).await.map(|_| { acks.spare()[..8].copy_from_slice(&b); 8 }) } => {
                match res {"
# Discard a trailing partial acknowledgement instead of keeping it.
mutate drop-partial "        self.buf.copy_within(complete..self.filled, 0);
        self.filled -= complete;" "        self.filled = 0;"
# The first complete acknowledgement of a read, not the highest.
mutate first-not-highest "            .max();" "            .next();"
# A closed acknowledgement side does not end the stream.
mutate eof-continues "                    Ok(0) | Err(_) => break," "                    Err(_) => break,"
# Store instead of keeping the maximum.
mutate store-not-max "                            acked.fetch_max(applied, Ordering::Relaxed);" "                            acked.store(applied, Ordering::Relaxed);"
echo "logs: $LOGS"
