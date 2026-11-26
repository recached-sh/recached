#!/usr/bin/env python3
"""AOF frames/op, bytes/op, write(2)/op, fsyncs per cell (median over reps),
from a run-6od results.txt."""
import re, statistics, sys
cells = {}
cur = None
for line in open(sys.argv[1]):
    if line.startswith("cell="):
        f = dict(kv.split("=", 1) for kv in line.split() if "=" in kv)
        cur = {"cell": f["cell"], "req": int(f["requests"])}
    elif line.strip().startswith("aof_before:") and cur:
        cur["b"] = dict(re.findall(r"recached_aof_(\w+):(\d+)", line))
    elif line.strip().startswith("aof_after:") and cur:
        a = dict(re.findall(r"recached_aof_(\w+):(\d+)", line))
        b = cur.get("b", {})
        d = lambda k: int(a.get(k, 0)) - int(b.get(k, 0))
        if a:
            cells.setdefault(cur["cell"], []).append(
                (d("frames") / cur["req"], d("bytes") / cur["req"], d("write_calls") / cur["req"], d("fsyncs")))
print("| cell | frames/op | bytes/op | write(2)/op | fsyncs/run |")
print("|---|---:|---:|---:|---:|")
for c, v in cells.items():
    m = [statistics.median(x[i] for x in v) for i in range(4)]
    print(f"| `{c}` | {m[0]:.3f} | {m[1]:.1f} | {m[2]:.4f} | {m[3]:.0f} |")
