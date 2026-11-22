#!/usr/bin/env python3
"""6O-D1 mutation run: each mutant breaks one piece of the read-side protocol
in core-engine/src/store.rs; the focused guards must fail on it.

Usage: mutate-6od.py <source tree copy> <out dir>

The tree is a scratch copy (never the working tree): each mutant is applied,
the guards run, and the original file is restored. A mutant is KILLED when a
guard fails, SURVIVED when every guard passes, and INAPPLICABLE when its
anchor text is not found (a stale mutant). The store guards run first; the
server guards (TCP) run only for a mutant the store guards did not kill.
"""
import os
import subprocess
import sys
import time

TREE, OUT = sys.argv[1], sys.argv[2]
STORE = os.path.join(TREE, "core-engine/src/store.rs")
ENV = dict(os.environ, CARGO_TARGET_DIR=os.path.join(TREE, "target"))
ENV["PATH"] = os.path.expanduser("~/.cargo/bin") + ":" + ENV["PATH"]

UNCHECKED = """    /// [`read_one_at`](Self::read_one_at) for `key`'s stripe."""
UNCHECKED_FNS = """    fn read_one_unchecked<R>(&self, _key: &str, f: impl FnOnce() -> R) -> R {
        f()
    }

    fn read_many_unchecked<K, R>(&self, _keys: &[K], mut f: impl FnMut() -> R) -> R {
        f()
    }

""" + UNCHECKED

MUTANTS = [
    ("M1 single-key read never waits for a multi-key write",
     [("if held.seq.load(Ordering::Acquire) & SEQ_MULTI == 0 {", "if true {")]),
    ("M2 multi-key read ignores writes in progress (validates only)",
     [("busy |= word & SEQ_ACTIVE != 0;", "busy |= word & 0 != 0;")]),
    ("M3 multi-key read skips its after-validation",
     [("if after == before {", "if true || after == before {")]),
    ("M4 locked fallback takes stripes in descending order",
     [("""        stripes.sort_unstable();
        stripes.dedup();
        #[cfg(any(test, feature = "test-hooks"))]
        {
            write_pause::LOCKED_READS""", """        stripes.sort_unstable_by(|x, y| y.cmp(x));
        stripes.dedup();
        #[cfg(any(test, feature = "test-hooks"))]
        {
            write_pause::LOCKED_READS""")]),
    ("M5 multi-key write marks only its first stripe",
     [("WriteOrder::Many(stripes.into_iter().map(lock).collect())",
       "WriteOrder::Many(stripes.into_iter().enumerate().map(|(i, s)| if i == 0 { lock(s) } "
       "else { StripeHold::take(&self.write_order[s], 0) }).collect())")]),
    ("M6 multi-key write not marked multi-key",
     [("StripeHold::take(&self.write_order[stripe], SEQ_ACTIVE | SEQ_MULTI)",
       "StripeHold::take(&self.write_order[stripe], SEQ_ACTIVE)")]),
    ("M7 writers never mark a write in progress",
     [("stripe.seq.store(seq | flags, Ordering::Relaxed);",
       "stripe.seq.store(seq | (flags & 0), Ordering::Relaxed);")]),
    ("M8 release clears the flags without advancing the version",
     [(".store((seq | (SEQ_STEP - 1)) + 1, Ordering::Release);",
       ".store(seq & !(SEQ_STEP - 1), Ordering::Release);")]),
    ("M9 MGET reads the clock per key",
     [("self.read_str(k, now, |read| {", "self.read_str(k, now_ms(), |read| {")]),
    ("M10 owned one-key reads bypass the protocol",
     [("self.read_one_at(stripe, || self.execute_inner(cmd))",
       "{ let _ = stripe; self.execute_inner(cmd) }")]),
    ("M11 borrowed GET bypasses the protocol",
     [(UNCHECKED, UNCHECKED_FNS),
      ("CommandRef::Get(key) => self.read_one(key, || {",
       "CommandRef::Get(key) => self.read_one_unchecked(key, || {")]),
    ("M12 borrowed MGET bypasses the protocol",
     [(UNCHECKED, UNCHECKED_FNS),
      ("""self.read_many(keys, || {
                    out.truncate(start);""", """self.read_many_unchecked(keys, || {
                    out.truncate(start);""")]),
    ("M13 by-reference one-key reads bypass the protocol",
     [("ReadScope::One(key) => self.read_one(key, || self.execute_inner(cmd.clone())),",
       "ReadScope::One(key) => { let _ = key; self.execute_inner(cmd.clone()) }")]),
    ("M14 a retried borrowed MGET does not restart its reply",
     [("""                    out.truncate(start);
                    let now = now_ms();""", """                    let _ = start;
                    let now = now_ms();""")]),
    ("M15 owned multi-key reads bypass the protocol",
     [("ReadScope::Many(keys) => self.read_many(keys, || self.execute_multi_read(&cmd)),",
       "ReadScope::Many(keys) => { let _ = keys; self.execute_multi_read(&cmd) }")]),
]

STORE_GUARD = ["cargo", "test", "-q", "-p", "core-engine", "--lib", "atomicity_tests"]
SERVER_GUARD = ["cargo", "test", "-q", "-p", "recached", "--bin", "recached-server", "mset_atomicity"]


def run(cmd, log):
    started = time.time()
    with open(log, "w") as f:
        code = subprocess.run(cmd, cwd=TREE, env=ENV, stdout=f, stderr=subprocess.STDOUT,
                              timeout=1800).returncode
    return code, time.time() - started


def main():
    os.makedirs(OUT, exist_ok=True)
    original = open(STORE).read()
    rows = []
    try:
        for i, (name, edits) in enumerate(MUTANTS, 1):
            text = original
            applicable = all(text.count(old) >= 1 for old, _ in edits)
            if not applicable:
                rows.append((name, "INAPPLICABLE", "-"))
                continue
            for old, new in edits:
                text = text.replace(old, new, 1)
            open(STORE, "w").write(text)
            code, secs = run(STORE_GUARD, os.path.join(OUT, f"m{i:02d}-store.log"))
            if code != 0:
                rows.append((name, "KILLED", f"store guards ({secs:.0f} s)"))
                continue
            code, secs = run(SERVER_GUARD, os.path.join(OUT, f"m{i:02d}-server.log"))
            rows.append((name, "KILLED" if code != 0 else "SURVIVED",
                         f"server guards ({secs:.0f} s)" if code != 0 else "all guards passed"))
            print(rows[-1], flush=True)
    finally:
        open(STORE, "w").write(original)
    with open(os.path.join(OUT, "summary.md"), "w") as f:
        f.write("| Mutant | Outcome | Killed by |\n|---|---|---|\n")
        for name, outcome, by in rows:
            f.write(f"| {name} | **{outcome}** | {by} |\n")
    print(open(os.path.join(OUT, "summary.md")).read())


main()
