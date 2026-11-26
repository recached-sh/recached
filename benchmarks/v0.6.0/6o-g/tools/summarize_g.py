#!/usr/bin/env python3
"""Summarize g-tx.sh logs (6O-G): per cell, per label, the median and range of
each metric over repetitions, and for every candidate label the per-repetition
candidate/base ratio (paired by rep, so host drift between reps cancels) with
its median and range.

Usage: summarize_g.py tx.log [METRIC ...]
Default metrics: tx_per_s p50_us p99_us max_us writer_ops_per_s w_p99_us
w_max_us reader_ops_per_s r_p99_us r_max_us oncpu_s gen_cpus foreign_cpus evicted hwm_kb
"""
import collections, statistics, sys

DEFAULT = "tx_per_s p50_us p99_us max_us writer_ops_per_s w_p99_us w_max_us reader_ops_per_s r_p99_us r_max_us oncpu_s gen_cpus foreign_cpus evicted hwm_kb".split()
path = sys.argv[1]
metrics = sys.argv[2:] or DEFAULT
rows = []
for line in open(path):
    if not line.startswith("label="):
        continue
    kv = dict(t.split("=", 1) for t in line.split() if "=" in t)
    rows.append(kv)

def cell(kv):
    return f"w{kv['workers']} {kv['mode']} {kv['keyset']} c{kv['clients']} wr{kv['writers']} {kv.get('txenv','')} {kv.get('srvenv','')}".strip()

by = collections.defaultdict(lambda: collections.defaultdict(dict))  # cell -> label -> rep -> kv
for kv in rows:
    by[cell(kv)][kv["label"]][kv["rep"]] = kv

def num(kv, m):
    try:
        return float(kv.get(m, "nan"))
    except ValueError:
        return float("nan")

def fmt(x):
    if x != x:
        return "-"
    return f"{x:.0f}" if abs(x) >= 100 else f"{x:.2f}"

for c, labels in by.items():
    print(f"\n## {c}")
    cands = [l for l in sorted(labels) if l != "base"] if "base" in labels else []
    print("| metric | " + " | ".join(f"{l} median [min–max]" for l in sorted(labels)) + "".join(f" | {c}/base per-rep median [min–max]" for c in cands) + " |")
    print("|---|" + "---|" * (len(labels) + len(cands)))
    for m in metrics:
        cols = []
        for l in sorted(labels):
            vals = [num(kv, m) for kv in labels[l].values()]
            vals = [v for v in vals if v == v]
            cols.append(f"{fmt(statistics.median(vals))} [{fmt(min(vals))}–{fmt(max(vals))}]" if vals else "-")
        ratios = []
        for cand in cands:
            rs = []
            for rep, kvb in labels["base"].items():
                kvg = labels[cand].get(rep)
                if kvg:
                    b, g = num(kvb, m), num(kvg, m)
                    if b and b == b and g == g:
                        rs.append(g / b)
            ratios.append(f"{statistics.median(rs):.3f} [{min(rs):.3f}–{max(rs):.3f}] n={len(rs)}" if rs else "-")
        if all(col == "-" for col in cols):
            continue
        print(f"| {m} | " + " | ".join(cols) + "".join(f" | {r}" for r in ratios) + " |")
    alive = [kv.get("alive") for l in labels.values() for kv in l.values()]
    errs = [kv.get("errors") for l in labels.values() for kv in l.values()]
    if any(a == "0" for a in alive) or any(e not in (None, "0") for e in errs):
        print(f"\n**alive={alive} errors={errs}**")
