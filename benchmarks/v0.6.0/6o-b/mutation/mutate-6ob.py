#!/usr/bin/env python3
"""6O-B mutation testing (derived from 6O-A's mutate.py): apply each mutation
to a scratch copy of the tree, run the tests that guard it, expect at least
one failure, restore.

    mutate-6ob.py <pr2|pr3> <tree> <target-dir> <out.txt>
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
T6OB = "reply_coalesce_6ob"
GATE = "no_reply_precedes_the_aof_write_of_its_command"

# (name, file, old, new, package, test filter)
PR2 = [
    ("coalescing never yields", RC,
     "        if skip > 0 {\n            w.skip.set(skip - 1);\n            return None;\n        }",
     "        if skip > 0 || true {\n            w.skip.set(skip.saturating_sub(1));\n            return None;\n        }", "recached", T6OB),
    ("the connection loop never records its batches", CONN,
     "                        reply_coalesce::note_batch();\n", "", "recached", T6OB),
    ("misses never back off", RC,
     "            w.skip.set(BACKOFF);", "            w.skip.set(0);", "recached", T6OB),
    ("a gain is counted as a miss", RC,
     "        if w.batches.get() != probe.batches {", "        if w.batches.get() == u64::MAX {", "recached", T6OB),
    ("the flush skips its yield even with nothing queued", CONN,
     "if !writer.buffer().is_empty() && aof_pending.is_empty() {",
     "if !writer.buffer().is_empty() && !aof_pending.is_empty() {", "recached", T6OB),
    ("SUBSCRIBE does not arm the overflow arm", CONN,
     "                                                    overflow_armed = true;\n                                                    pubsub.lock().await.subscribe(",
     "                                                    pubsub.lock().await.subscribe(", "recached",
     "a_subscriber_whose_queue_overflows_is_disconnected"),
    ("PSUBSCRIBE does not arm the overflow arm", CONN,
     "                                                    overflow_armed = true;\n                                                    pubsub.lock().await.psubscribe(",
     "                                                    pubsub.lock().await.psubscribe(", "recached",
     "a_pattern_subscriber_whose_queue_overflows_is_disconnected"),
    ("WATCH does not arm the overflow arm", CONN,
     "                                                    overflow_armed = true;\n                                                    for key in &keys {",
     "                                                    for key in &keys {", "recached",
     "an_overflow_signalled_and_unwatched_within_one_batch_still_disconnects"),
    ("the gate's empty check ignores queued tickets", PERS,
     "        self.queued.load(std::sync::atomic::Ordering::Acquire) == 0",
     "        self.queued.load(std::sync::atomic::Ordering::Acquire) < usize::MAX", "recached", GATE),
    ("a push does not update the queued count", PERS,
     "        tickets.push_back((Arc::clone(shared), done));\n        self.queued\n            .store(tickets.len(), std::sync::atomic::Ordering::Release);",
     "        tickets.push_back((Arc::clone(shared), done));", "recached", GATE),
    ("a pop leaves the queued count stale at zero", PERS,
     "        let ticket = tickets.pop_front();\n        self.queued\n            .store(tickets.len(), std::sync::atomic::Ordering::Release);",
     "        let ticket = tickets.pop_front();\n        self.queued\n            .store(0, std::sync::atomic::Ordering::Release);", "recached", "persistence::gate_tests"),
]
PR3 = [
    ("LPUSH builder reverses its values", REQ,
     "    (from..args.len()).map(|i| args.bytes(i)).collect()\n",
     "    (from..args.len()).rev().map(|i| args.bytes(i)).collect()\n", "core-engine", "request::"),
    ("RPUSH builder builds an LPUSH", REQ,
     "=> Command::RPush(args.key(1)?, values(args, 2)?),", "=> Command::LPush(args.key(1)?, values(args, 2)?),",
     "core-engine", "request::"),
    ("LPUSH key not validated", REQ,
     "=> Command::LPush(args.key(1)?, values(args, 2)?),",
     "=> Command::LPush(args.text(1)?.to_owned(), values(args, 2)?),", "core-engine", "request::"),
    ("SADD member converted lossily instead of refused", REQ,
     "        .map(|i| args.text(i).map(str::to_owned))",
     "        .map(|i| args.get(i).map(|b| String::from_utf8_lossy(b).into_owned()))", "core-engine", "request::"),
    ("SADD builder drops its last member", REQ,
     "    (from..args.len())\n        .map(|i| args.text(i).map(str::to_owned))",
     "    (from..args.len().saturating_sub(1))\n        .map(|i| args.text(i).map(str::to_owned))", "core-engine", "request::"),
    ("borrowed SADD never adds a new member", STORE,
     "        !set.contains(self) && set.insert(self.to_owned())",
     "        set.contains(self) && set.insert(self.to_owned())", "core-engine", ""),
    ("RPUSH pushes to the front", STORE,
     "                End::Back => list.push_back(v.into()),", "                End::Back => list.push_front(v.into()),",
     "core-engine", ""),
    ("a push takes its slot without the probe (unaccounted)", STORE,
     "            Box::new(CompactList::new()),\n            probe\n        );\n        for v in vals {",
     "            Box::new(CompactList::new()),\n            None\n        );\n        for v in vals {", "core-engine", "probe_tests"),
    ("SADD takes its slot without the probe (unaccounted)", STORE,
     "            Box::new(CompactSet::new()),\n            probe\n        );\n        let added",
     "            Box::new(CompactSet::new()),\n            None\n        );\n        let added", "core-engine", "probe_tests"),
    ("LPUSH not probe-eligible (generic path again)", STORE,
     "            | Command::LPush(..)\n            | Command::RPush(..)\n            | Command::SAdd(..)\n    )",
     "            | Command::RPush(..)\n            | Command::SAdd(..)\n    )", "core-engine", "reservation_tests"),
    ("LPUSH builder accepts a missing value", REQ,
     "(5, _) if n >= 3 && args.name_is(\"LPUSH\")", "(5, _) if n >= 2 && args.name_is(\"LPUSH\")",
     "core-engine", "request::"),
]
MUTANTS = PR2 if which == "pr2" else PR3
if os.environ.get("ONLY"):
    MUTANTS = [m for m in MUTANTS if os.environ["ONLY"] in m[0]]

results = []
env = dict(os.environ, CARGO_TARGET_DIR=target, PATH=os.path.expanduser("~/.cargo/bin") + ":" + os.environ["PATH"])
for name, rel, old, new, pkg, flt in MUTANTS:
    path = tree / rel
    src = path.read_text()
    if src.count(old) != 1:
        results.append(f"NOT APPLIED: {name} ({src.count(old)} matches)")
        continue
    path.write_text(src.replace(old, new))
    try:
        p = subprocess.run(["cargo", "test", "--locked", "-p", pkg, "--", flt],
                           cwd=tree, env=env, capture_output=True, text=True, timeout=1800)
        text = p.stdout + p.stderr
        fails = [l for l in text.splitlines() if l.endswith("FAILED") or l.startswith("test result: FAILED")]
        if "error[E" in text or "could not compile" in text:
            results.append(f"KILLED (compile): {name} [{flt}]")
        elif p.returncode != 0:
            results.append(f"KILLED: {name} [{flt}] " + " | ".join(fails[:4]))
        else:
            summary = [l for l in text.splitlines() if l.startswith("test result:")]
            results.append(f"SURVIVED: {name} [{flt}] " + " | ".join(summary[-1:]))
    finally:
        path.write_text(src)
    print(results[-1], flush=True)
out.write_text("\n".join(results) + "\n")
