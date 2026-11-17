"""Median per build/ws/writers from run.sh's raw.csv, with min-max spread."""
import csv, statistics, sys
from collections import defaultdict
rows = defaultdict(list)
builds = []
for r in csv.DictReader(open(sys.argv[1])):
    if r["build"] not in builds:
        builds.append(r["build"])
    rows[(r["ws"], int(r["writers"]), r["build"])].append((float(r["ops_s"]), float(r["cpu_ns_per_op"]), int(r["ws_gaps"])))
print("| WS subscribers | writers | build | ops/s (median) | spread | CPU ns/op (median) | spread | vs base ops | vs base CPU | WS gaps |")
print("|---|---:|---|---:|---|---:|---|---:|---:|---:|")
for ws in ("false", "true"):
    for w in (1, 4, 8):
        base = rows.get((ws, w, "base"))
        if not base:
            continue
        bo, bc = statistics.median(x[0] for x in base), statistics.median(x[1] for x in base)
        for b in builds:
            v = rows.get((ws, w, b))
            if not v:
                continue
            o, c = statistics.median(x[0] for x in v), statistics.median(x[1] for x in v)
            os_ = f"{min(x[0] for x in v)/1e3:.0f}k-{max(x[0] for x in v)/1e3:.0f}k"
            cs = f"{min(x[1] for x in v):.0f}-{max(x[1] for x in v):.0f}"
            print(f"| {'8' if ws=='true' else '0'} | {w} | {b} | {o:,.0f} | {os_} | {c:,.0f} | {cs} | {o/bo-1:+.1%} | {c/bc-1:+.1%} | {sum(x[2] for x in v)} |")
