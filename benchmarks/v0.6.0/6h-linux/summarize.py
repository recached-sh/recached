#!/usr/bin/env python3
"""Summarise the 6H memtier matrix (gated.sh output: <dir>/rep-*/results.csv).

Per mode x cell x server: median ops/s with [min-max] and n, median p50/p99,
median server CPU-seconds per million ops, and each server's ops/s as a
percentage of Redis's in single mode. A latency above the cell's test time is
a memtier histogram artefact and is dropped. Usage: summarize.py <dir>
"""
import csv, glob, statistics, sys
from collections import defaultdict

ORDER = ["v051", "6g", "6h", "redis", "valkey"]
LABEL = {"v051": "v0.5.1", "6g": "v0.6 after 6G", "6h": "v0.6 after 6H",
         "redis": "Redis 7.2.5", "valkey": "Valkey 9.1.0"}
rows = defaultdict(list)
for path in sorted(glob.glob(f"{sys.argv[1]}/rep-*/results.csv")):
    for r in csv.DictReader(open(path)):
        if r["ops_per_sec"]:
            rows[(r["mode"], r["cell"], r["server"])].append(r)

def med(rs, k, cap=None):
    v = [float(r[k]) for r in rs if r[k] not in ("", None)]
    if cap is not None:
        v = [x for x in v if x <= cap]
    return statistics.median(v) if v else float("nan")

cells = sorted({(m, c) for m, c, _ in rows}, key=lambda mc: (mc[0] != "single", mc[1]))
for mode, cell in cells:
    print(f"\n#### {cell} — {mode}\n")
    print("| Server | ops/s median [min–max] | p50 ms | p99 ms | CPU-s / M ops | vs Redis (1 thread) |")
    print("|---|---:|---:|---:|---:|---:|")
    redis = rows.get(("single", cell, "redis"))
    rmed = med(redis, "ops_per_sec") if redis else None
    for s in ORDER:
        rs = rows.get((mode, cell, s))
        if not rs:
            continue
        ops = [float(r["ops_per_sec"]) for r in rs]
        m = statistics.median(ops)
        vs = f"{m / rmed * 100:.0f}%" if rmed else ""
        print(f"| {LABEL[s]} | {m:,.0f} [{min(ops):,.0f}–{max(ops):,.0f}] n={len(ops)} | "
              f"{med(rs, 'p50_ms', 10000):.3f} | {med(rs, 'p99_ms', 10000):.3f} | "
              f"{med(rs, 'cpu_s_per_mops'):.2f} | {vs} |")
