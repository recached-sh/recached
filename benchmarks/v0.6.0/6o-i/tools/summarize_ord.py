#!/usr/bin/env python3
"""Summarize an i-ord.sh log: per kind and test, the median over reps of ops/s,
the primary's CPU ns per request and its (and the replica's) VmHWM, with each
label's ratio to the first, paired by rep.

usage: summarize_ord.py ord.log [LABEL ...]   (default: base p3)
"""
import re, statistics, sys
from collections import defaultdict

labels = sys.argv[2:] or ["base", "p3"]
rows = defaultdict(lambda: defaultdict(dict))
order = []
for line in open(sys.argv[1]):
    if not line.startswith("label="):
        continue
    kv = dict(re.findall(r"(\w+)=(\S*)", line))
    key = (kv["kind"], kv["test"])
    if key not in order:
        order.append(key)
    rows[key][kv["label"]][int(kv["rep"])] = {
        "rps": float(kv["rps"]), "ns": float(kv["oncpu_s"]) * 1e9 / float(kv["requests"]),
        "p99": float(kv["p99_ms"]), "phwm": float(kv["primary_hwm_kb"]) / 1024,
        "rhwm": float(kv["replica_hwm_kb"]) / 1024}

med = lambda key, l, k: statistics.median(r[k] for r in rows[key][l].values())
print(f"| kind | test | ops/s {labels[0]} | {' / '.join(labels[1:])} ratio (paired median, range) | "
      f"CPU ns/req {' → '.join(labels)} | CPU ratio (paired median) | p99 ms | primary RSS MiB | replica RSS MiB |")
print("|---|---|---|---|---|---|---|---|---|")
for key in order:
    b = rows[key][labels[0]]
    rr, cr = [], []
    for l in labels[1:]:
        reps = sorted(set(b) & set(rows[key][l]))
        t = [rows[key][l][r]["rps"] / b[r]["rps"] for r in reps]
        c = [rows[key][l][r]["ns"] / b[r]["ns"] for r in reps]
        rr.append(f"{statistics.median(t):.3f} [{min(t):.3f}–{max(t):.3f}]")
        cr.append(f"{statistics.median(c):.3f}")
    arrow = lambda k, f: " → ".join(f.format(med(key, l, k)) for l in labels)
    print(f"| {key[0]} | {key[1]} | {med(key, labels[0], 'rps'):,.0f} | {' / '.join(rr)} | {arrow('ns', '{:.0f}')} | "
          f"{' / '.join(cr)} | {arrow('p99', '{:.3f}')} | {arrow('phwm', '{:.0f}')} | {arrow('rhwm', '{:.0f}')} |")
