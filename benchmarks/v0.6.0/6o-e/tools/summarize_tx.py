#!/usr/bin/env python3
"""Summarise tx.sh results: per cell, medians over reps per label, ratios to base."""
import re, sys, statistics as st
from collections import defaultdict
rows = defaultdict(lambda: defaultdict(list))
for line in open(sys.argv[1]):
    if not line.startswith("label="):
        continue
    kv = dict(re.findall(r"(\w+)=([^\s]+)", line))
    cell = (kv["workers"], kv["mode"], kv["keyset"], kv["clients"], kv["writers"])
    tx = int(kv["tx"])
    r = {"tps": float(kv["tx_per_s"]), "p50": float(kv["p50_us"]), "p99": float(kv["p99_us"]),
         "cpu_us": float(kv["oncpu_s"]) * 1e6 / tx, "ins": int(kv["instructions"]) / tx,
         "cyc": int(kv["cycles"]) / tx, "aborts": int(kv["aborts"]) / tx * 100, "wops": float(kv["writer_ops_per_s"]),
         "rss": int(kv["rss_kb"])}
    rows[cell][kv["label"]].append(r)
med = lambda xs, k: st.median(x[k] for x in xs)
labels = ["base", "e1", "fix"]
print("| workers | mode | keys | clients | writers | label | tx/s | vs base (reps) | p50 µs | p99 µs | CPU µs/tx | instr/tx | cycles/tx | abort % | writer ops/s |")
print("|---|---|---|---|---|---|---:|---|---:|---:|---:|---:|---:|---:|---:|")
for cell in rows:
    base = rows[cell]["base"]
    bt = med(base, "tps")
    for lab in labels:
        xs = rows[cell].get(lab)
        if not xs:
            continue
        t = med(xs, "tps")
        reps = " ".join(f"{x['tps']/bt:.3f}" for x in xs) if lab != "base" else ""
        ratio = f"**{t/bt:.3f}** ({reps})" if lab != "base" else "1"
        print(f"| {cell[0]} | {cell[1]} | {cell[2]} | {cell[3]} | {cell[4]} | {lab} | {t:,.0f} | {ratio} | {med(xs,'p50'):.1f} | {med(xs,'p99'):.1f} | {med(xs,'cpu_us'):.2f} | {med(xs,'ins'):,.0f} | {med(xs,'cyc'):,.0f} | {med(xs,'aborts'):.3f} | {med(xs,'wops'):,.0f} |")
