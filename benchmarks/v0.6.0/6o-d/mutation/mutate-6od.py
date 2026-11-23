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
     [("""            let mut before = held.multi.load(Ordering::Acquire);
            if before & MULTI_ACTIVE != 0 {""", """            let mut before = held.multi.load(Ordering::Acquire);
            if false {""")]),
    ("M2 multi-key read ignores writes in progress (validates only)",
     [("busy || word & SEQ_ACTIVE != 0", "busy || word & 0 != 0")]),
    ("M3 multi-key read skips its after-validation",
     [("""            if load().0 == before {
                return out;""", """            if true {
                return out;""")]),
    ("M4 locked fallback takes stripes in descending order",
     [("""let _held: SmallVec<[std::sync::MutexGuard<'_, ()>; 16]> = stripes
            .iter()""", """let _held: SmallVec<[std::sync::MutexGuard<'_, ()>; 16]> = stripes
            .iter()
            .rev()""")]),
    ("M5 multi-key write marks only its first stripe",
     [("""        order.mark(flags);
        order""", """        match &order {
            WriteOrder::One(hold) => hold.mark(flags),
            WriteOrder::Many(holds) => holds[0].mark(flags),
        }
        order""")]),
    ("M6 multi-key write not marked multi-key",
     [("let flags = if several_steps || keys.len() > 1 {", "let flags = if false {")]),
    ("M7 writers never mark a write in progress",
     [(".store(seq | (flags & SEQ_ACTIVE), Ordering::Release);",
       ".store(seq | (flags & 0), Ordering::Release);")]),
    ("M8 release clears the flags without advancing the version",
     [(".store((seq | (SEQ_STEP - 1)) + 1, Ordering::Release);",
       ".store(seq & !(SEQ_STEP - 1), Ordering::Release);")]),
    ("M9 MGET reads the clock per key",
     [("self.read_str(k, now, |read| {", "self.read_str(k, now_ms(), |read| {")]),
    ("M10 owned one-key reads bypass the protocol",
     [("ReadScope::One(key) => self.read_one(key, || self.execute_read_inner(&cmd)),",
       "ReadScope::One(key) => { let _ = key; self.execute_read_inner(&cmd) }")]),
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
     [("ReadScope::One(key) => self.read_one(key, || self.execute_read_inner(cmd)),",
       "ReadScope::One(key) => { let _ = key; self.execute_read_inner(cmd) }")]),
    ("M14 a retried borrowed MGET does not restart its reply",
     [("""                    out.truncate(start);
                    let now = now_ms();""", """                    let _ = start;
                    let now = now_ms();""")]),
    ("M15 owned multi-key reads bypass the protocol",
     [("ReadScope::Many(keys) => self.read_many(keys, || self.execute_read_inner(&cmd)),",
       "ReadScope::Many(keys) => { let _ = keys; self.execute_read_inner(&cmd) }")]),
    ("M16 a one-key write naming its key twice is not marked multi-key",
     [("let several_steps = keys.len() > 1 || leaves_steps_visible(cmd.borrow());",
       "let several_steps = leaves_steps_visible(cmd.borrow());")]),
    ("M17 a write that may be rolled back is not marked multi-key",
     [("""                if rollback.is_some() {
                    order.mark(SEQ_ACTIVE | SEQ_MULTI);""", """                if false {
                    order.mark(SEQ_ACTIVE | SEQ_MULTI);""")]),
    ("M18 SINTER/SUNION/SDIFF bypass the protocol",
     [("""        | Command::SInter(k)
        | Command::SUnion(k)
        | Command::SDiff(k) => keys(k),""", """        => keys(k),
        Command::SInter(_) | Command::SUnion(_) | Command::SDiff(_) => ReadScope::Unkeyed,""")]),
    ("M19 a DEDUP-wrapped multi-key read runs (as before review round 1)",
     [("Command::Dedup(..) => ReadScope::Unkeyed,", "Command::Dedup(_, _, inner) => read_scope(inner),"),
      ("            other => {\n                // Only reads `execute_inner` does not delegate here may",
       "            Command::Dedup(_, _, inner) => self.execute_read_inner(inner),\n            other => {\n                // Only reads `execute_inner` does not delegate here may")]),
    ("M20 borrowed HGET bypasses the protocol",
     [(UNCHECKED, UNCHECKED_FNS),
      ("CommandRef::HGet(key, field) => self.read_one(key, || {",
       "CommandRef::HGet(key, field) => self.read_one_unchecked(key, || {")]),
    ("M21 a pop that may empty a collection is not marked multi-key",
     [("        | Command::ZAdd(..) => true,", "        | Command::ZAdd(..) => false,")]),
    ("M23 a single-key read skips its after-validation",
     [("if held.multi.load(Ordering::Acquire) == before {", "if true {")]),
    ("M24 multi-step writes never mark the multi word",
     [(".store(multi | MULTI_ACTIVE, Ordering::Release);", ".store(multi, Ordering::Release);")]),
    ("M25 a multi-step release does not advance the epoch",
     [("self.stripe.multi.store(multi + 1, Ordering::Release);",
       "self.stripe.multi.store(multi & !MULTI_ACTIVE, Ordering::Release);")]),
    ("M26 a retried borrowed GET does not restart its reply",
     [("""                // A retried read starts its reply over (`read_one_at`).
                out.truncate(start);""", """                let _ = start;""")]),
    ("M27 HINCRBYFLOAT creates the hash before refusing a non-finite increment",
     [("""    if !delta.is_finite() {
        return Value::Error("ERR value is NaN or Infinity".to_string());
    }""", "")]),
    ("M28 LPUSH with nothing to add creates the list",
     [("""        let mut vals = vals.peekable();
        if vals.peek().is_none() {""", """        let mut vals = vals.peekable();
        if false {""")]),
    ("M22 a multi-key EXISTS is read as its first key only",
     [("""        Command::MGet(k)
        | Command::Exists(k)""", """        Command::Exists(k) => ReadScope::One(k[0].as_str()),
        Command::MGet(k)""")]),
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
