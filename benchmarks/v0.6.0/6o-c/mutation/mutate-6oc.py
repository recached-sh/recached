#!/usr/bin/env python3
"""6O-C mutation testing (derived from 6O-B's mutate-6ob.py): apply each
mutation to the tree, run the tests that guard it, expect at least one
failure, restore.

    mutate-6oc.py <c1|c2|watch> <tree> <target-dir> <out.txt>
"""
import os
import subprocess
import sys
from pathlib import Path

which, tree, target, out = sys.argv[1], Path(sys.argv[2]), sys.argv[3], Path(sys.argv[4])
CONN = "server-native/src/connection.rs"
RC = "server-native/src/reply_coalesce.rs"
PERS = "server-native/src/persistence.rs"
REQ = "core-engine/src/request.rs"
STORE = "core-engine/src/store.rs"
CLIENTS = "server-native/src/clients.rs"
COAL = "reply_coalesce"
GATE = "no_reply_precedes_the_aof_write_of_its_command"
BIN = ("recached", ["--bin", "recached-server"])
CORE = ("core-engine", [])

# (name, file, old, new, (package, extra cargo args), test filter)
C1 = [
    ("yield after every multi-command batch", RC,
     "    commands == 1\n", "    commands >= 1\n", BIN, COAL),
    ("a batch leaving part of a frame still yields", CONN,
     "                        let eligible = reply_coalesce::batch_eligible(commands)\n                            && !partial_left\n",
     "                        let eligible = reply_coalesce::batch_eligible(commands)\n", BIN, COAL),
    ("a pipeline's tail is never recognised", CONN,
     "                            pipeline_tail = partial_left;", "                            pipeline_tail = false;", BIN, COAL),
    ("a batch counts as company before it ran anything", CONN,
     "                        let mut commands: u32 = 0;\n                        'parse: loop {",
     "                        reply_coalesce::note_batch(conn_id);\n                        let mut commands: u32 = 0;\n                        'parse: loop {", BIN, COAL),
    ("a migration counts as a gain", RC,
     "        if migrated {\n            return false;\n        }",
     "        if migrated {\n            w.misses.set(0);\n            return true;\n        }", BIN, COAL),
    ("a migration counts as a miss", RC,
     "        if migrated {\n            return false;\n        }", "", BIN, COAL),
    ("a miss counts as a gain", RC,
     "        if w.batches.get() != probe.batches {", "        if w.batches.get() == probe.batches {", BIN, COAL),
    ("misses never back off", RC,
     "            w.skip.set(period);", "            w.skip.set(0);", BIN, COAL),
    ("a gain does not reset the escalation", RC,
     "            w.misses.set(0);\n            w.period.set(BACKOFF);\n            return true;",
     "            w.misses.set(0);\n            return true;", BIN, COAL),
    ("the back-off escalates with company", RC,
     "            if w.company.replace(false) {", "            if false && w.company.replace(false) {", BIN, COAL),
    ("the back-off has no maximum", RC,
     "            w.period.set(period.saturating_mul(2).min(MAX_BACKOFF));",
     "            w.period.set(period.saturating_mul(2));", BIN, COAL),
    ("skip the AOF gate: its empty check ignores queued tickets", PERS,
     "        self.queued.load(std::sync::atomic::Ordering::Acquire) == 0",
     "        self.queued.load(std::sync::atomic::Ordering::Acquire) < usize::MAX", BIN, GATE),
    ("release a reply before its ticket: a push leaves the queued count unset", PERS,
     "        tickets.push_back((Arc::clone(shared), done));\n        self.queued\n            .store(tickets.len(), std::sync::atomic::Ordering::Release);",
     "        tickets.push_back((Arc::clone(shared), done));", BIN, GATE),
]
C2 = [
    ("borrowed LPUSH reverses its values", REQ,
     "    for i in from..args.len() {\n        out.push(args.get(i)?);",
     "    for i in (from..args.len()).rev() {\n        out.push(args.get(i)?);", CORE, ""),
    ("borrowed push drops its final value", REQ,
     "    for i in from..args.len() {\n        out.push(args.get(i)?);",
     "    for i in from..args.len().saturating_sub(1) {\n        out.push(args.get(i)?);", CORE, ""),
    ("borrowed SADD accepts invalid UTF-8", REQ,
     "        out.push(args.text(i)?);", "        out.push(args.text(i).unwrap_or(\"?\"));", CORE, ""),
    ("borrowed SADD never adds a new member", STORE,
     "        !set.contains(self) && set.insert(self.to_owned())",
     "        set.contains(self) && set.insert(self.to_owned())", CORE, ""),
    ("borrowed RPUSH pushes to the front", STORE,
     "                    store.push(*key, vals.iter().copied(), End::Back, probe)",
     "                    store.push(*key, vals.iter().copied(), End::Front, probe)", CORE, ""),
    ("borrowed LPUSH bypasses its probe and stripe (lost update, unaccounted)", STORE,
     "            CommandRef::LPush(key, vals) => write(\n                self.run_probed_write(key, (), |store, (), probe| {\n                    store.push(*key, vals.iter().copied(), End::Front, probe)\n                }),",
     "            CommandRef::LPush(key, vals) => write(\n                self.push(*key, vals.iter().copied(), End::Front, None),", CORE, ""),
    ("a capped store runs borrowed writes (no reservation)", STORE,
     "        if cmd.is_write() && self.enforces_caps() {\n            return None;\n        }",
     "", CORE, ""),
    ("SADD is not a write", REQ,
     "            | CommandRef::SAdd(..) => true,", "            => true,\n            CommandRef::SAdd(..) => false,", CORE, ""),
    ("LPUSH and RPUSH labels swapped", CLIENTS,
     "        CommandRef::LPush(..) => \"lpush\",\n        CommandRef::RPush(..) => \"rpush\",",
     "        CommandRef::LPush(..) => \"rpush\",\n        CommandRef::RPush(..) => \"lpush\",", BIN, "every_borrowed_shape_agrees"),
]
W = "watch_overflow_exec"
WATCH = [
    ("TCP EXEC does not check the overflow", CONN,
     "                                                    if overflow_rx.try_recv().is_ok() {\n                                                        warn!(\"TCP conn",
     "                                                    if false && overflow_rx.try_recv().is_ok() {\n                                                        warn!(\"TCP conn", BIN, W),
    ("WS EXEC does not check the overflow", CONN,
     "                                        if overflow_rx.try_recv().is_ok() {\n                                            warn!(\"WS conn",
     "                                        if false && overflow_rx.try_recv().is_ok() {\n                                            warn!(\"WS conn", BIN, W),
    ("TCP EXEC consumes the overflow and commits anyway", CONN,
     "                                                        let _ = writer.flush().await;\n                                                        break 'outer;\n",
     "                                                        let _ = writer.flush().await;\n", BIN, W),
    ("the earlier replies are dropped at the EXEC-time disconnect", CONN,
     "                                                        drop(_write_guards);\n                                                        let _ = writer.flush().await;\n",
     "", BIN, W),
]
MUTANTS = {"c1": C1, "c2": C2, "watch": WATCH}[which]
if os.environ.get("ONLY"):
    MUTANTS = [m for m in MUTANTS if os.environ["ONLY"] in m[0]]

results = []
env = dict(os.environ, CARGO_TARGET_DIR=target, PATH=os.path.expanduser("~/.cargo/bin") + ":" + os.environ["PATH"])
for name, rel, old, new, (pkg, extra), flt in MUTANTS:
    path = tree / rel
    src = path.read_text()
    if src.count(old) != 1:
        results.append(f"NOT APPLIED: {name} ({src.count(old)} matches)")
        print(results[-1], flush=True)
        continue
    path.write_text(src.replace(old, new))
    try:
        cmd = ["cargo", "test", "--locked", "-p", pkg, *extra, "--", flt] if flt else ["cargo", "test", "--locked", "-p", pkg, *extra]
        p = subprocess.run(cmd, cwd=tree, env=env, capture_output=True, text=True, timeout=2400)
        text = p.stdout + p.stderr
        fails = [l for l in text.splitlines() if l.endswith("FAILED") or l.startswith("test result: FAILED")]
        if "error[E" in text or "could not compile" in text:
            results.append(f"KILLED (compile): {name} [{flt or 'all'}]")
        elif p.returncode != 0:
            results.append(f"KILLED: {name} [{flt or 'all'}] " + " | ".join(fails[:4]))
        else:
            summary = [l for l in text.splitlines() if l.startswith("test result:")]
            results.append(f"SURVIVED: {name} [{flt or 'all'}] " + " | ".join(summary[-1:]))
    finally:
        path.write_text(src)
    print(results[-1], flush=True)
out.write_text("\n".join(results) + "\n")
