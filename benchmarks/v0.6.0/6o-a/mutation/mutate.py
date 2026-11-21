#!/usr/bin/env python3
"""6O-A PR 2 mutation testing: apply each mutation to a scratch copy of the
tree, run the tests that guard it, expect at least one failure, restore.

    mutate.py <tree> <target-dir> <out.txt>
"""
import os
import subprocess
import sys
from pathlib import Path

tree, target, out = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
PROP = "server-native/src/propagation.rs"
RESP = "core-engine/src/resp.rs"
PERS = "server-native/src/persistence.rs"
STATE = "server-native/src/server_state.rs"
FRAMES = "frame_encoding_6oa"

# (name, file, old, new, test filter)
MUTANTS = [
    ("push header counts one part fewer", RESP,
     "write_header(out, b'>', len_i64(len));", "write_header(out, b'>', len_i64(len.saturating_sub(1)));", FRAMES),
    ("bulk length off by one", RESP,
     "write_header(out, b'$', len_i64(data.len()));", "write_header(out, b'$', len_i64(data.len() + 1));", FRAMES),
    ("drop the last part", PROP,
     "    for part in parts {\n        write_bulk(&mut out, Some(part));",
     "    for part in parts.iter().take(parts.len().saturating_sub(1)) {\n        write_bulk(&mut out, Some(part));", FRAMES),
    ("capacity one byte short (correct bytes, extra realloc)", PROP,
     "            total\n                .checked_add(5 + digits(part.len()))?",
     "            total\n                .checked_add(4 + digits(part.len()))?", FRAMES),
    ("digit count wrong at powers of ten", PROP,
     "n.checked_ilog10().map_or(1, |d| d as usize + 1)", "n.checked_ilog10().map_or(1, |d| d as usize)", FRAMES),
    ("HSET field and value swapped", PROP,
     "                parts.push(f.as_bytes());\n                parts.push(v);",
     "                parts.push(v);\n                parts.push(f.as_bytes());", FRAMES),
    ("MSET drops its last pair", PROP,
     "            for (k, v) in pairs {\n                parts.push(k.as_bytes());",
     "            for (k, v) in pairs.iter().take(pairs.len().saturating_sub(1)) {\n                parts.push(k.as_bytes());", FRAMES),
    ("counter loses its sign", PROP,
     "let n = format_i64(*n, &mut digits);", "let n = format_i64(n.saturating_abs(), &mut digits);", FRAMES),
    ("format_i64 drops the minus sign", RESP,
     "    if n < 0 {\n        at -= 1;\n        buf[at] = b'-';\n    }", "    if n < 0 {\n        at -= 1;\n        buf[at] = b' ';\n    }", FRAMES),
    ("DEDUP envelope count wrong", PERS,
     "        count + 3,", "        count + 2,", FRAMES),
    ("skip the AOF append of a deferred client write", PERS,
     "                let done = self.queue_only(false, |batch| batch.push_frame(resp));",
     "                let done = self.queue_only(false, |_batch| ());",
     "no_reply_precedes_the_aof_write_of_its_command"),
    ("gate completes before its batch is written (early ack)", PERS,
     "            let Some((shared, done)) = self.deferred.pop() else {",
     "            let Some((shared, done)) = self.deferred.pop().filter(|_| false) else {",
     "no_reply_precedes_the_aof_write_of_its_command"),
    ("a cancelled gate no longer leads its batch on drop", PERS,
     "        if let Some(shared) = self.waiting_on.take() {\n            shared.lead();\n        }\n        while let Some((shared, _)) = self.deferred.pop() {\n            shared.lead();\n        }",
     "        let _ = self.waiting_on.take();\n        while let Some((_shared, _)) = self.deferred.pop() {}",
     "persistence::"),
]

results = []
env = dict(os.environ, CARGO_TARGET_DIR=target, PATH=os.path.expanduser("~/.cargo/bin") + ":" + os.environ["PATH"])
for name, rel, old, new, flt in MUTANTS:
    path = tree / rel
    src = path.read_text()
    if src.count(old) != 1:
        results.append(f"SKIPPED (pattern matched {src.count(old)} times): {name}")
        continue
    path.write_text(src.replace(old, new))
    try:
        r = subprocess.run(
            ["cargo", "test", "--locked", "-p", "recached", "--bin", "recached-server", "--", flt],
            cwd=tree, env=env, capture_output=True, text=True, timeout=1800)
        tail = [l for l in r.stdout.splitlines() if l.startswith("test result") or "FAILED" in l][-3:]
        compiled = "error[" not in r.stderr and "could not compile" not in r.stderr
        if not compiled:
            verdict = "KILLED (does not compile)"
        elif r.returncode != 0:
            verdict = "KILLED"
        else:
            verdict = "SURVIVED"
        results.append(f"{verdict}: {name} [{flt}] {' | '.join(tail)}")
    finally:
        path.write_text(src)
        os.utime(path, None)   # cargo must see the restore
    print(results[-1], flush=True)

out.write_text("\n".join(results) + "\n")
killed = sum(r.startswith("KILLED") for r in results)
print(f"{killed}/{len(MUTANTS)} killed")
