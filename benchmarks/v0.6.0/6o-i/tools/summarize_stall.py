#!/usr/bin/env python3
"""Summarize an i-stall2.sh log: readers of a replica (GET c10 P1) while the
primary commits TX_N-write transactions. Per replica worker count and TX_N, the
median over reps of the reader's ops/s, p50, p99 and max latency, the primary's
transaction rate and the replica's VmHWM, each label next to the first.

usage: summarize_stall.py stall.log [LABEL ...]   (default: base p3)
"""
import re, statistics, sys
from collections import defaultdict

labels = sys.argv[2:] or ["base", "p3"]
rows = defaultdict(lambda: defaultdict(list))
order = []
for line in open(sys.argv[1]):
    if not line.startswith("label="):
        continue
    kv = dict(re.findall(r"(\w+)=(\S*)", line))
    key = (int(kv["replica_workers"]), int(kv["tx_n"]))
    if key not in order:
        order.append(key)
    rows[key][kv["label"]].append(kv)

def med(key, l, k, scale=1.0):
    vals = [float(r[k]) * scale for r in rows[key][l] if r.get(k)]
    return statistics.median(vals) if vals else float("nan")

def worst(key, l, k):
    return max(float(r[k]) for r in rows[key][l])

arrow = lambda key, k, f, s=1.0: " → ".join(f.format(med(key, l, k, s)) for l in labels)
print(f"| replica workers | TX_N | reps | reader ops/s {' → '.join(labels)} | reader p50 ms | reader p99 ms | "
      f"reader max ms (median) | reader max ms (worst rep) | primary tx/s | replica RSS MiB | converged |")
print("|---|---|---|---|---|---|---|---|---|---|---|")
for key in sorted(order):
    reps = "/".join(str(len(rows[key][l])) for l in labels)
    conv = all(r["replica_dbsize"] == r["primary_dbsize"] for l in labels for r in rows[key][l])
    print(f"| {key[0]} | {key[1]} | {reps} | {arrow(key, 'reader_rps', '{:,.0f}')} | {arrow(key, 'reader_p50_ms', '{:.3f}')} | "
          f"{arrow(key, 'reader_p99_ms', '{:.3f}')} | {arrow(key, 'reader_max_ms', '{:.2f}')} | "
          f"{' → '.join(f'{worst(key, l, chr(114)+"eader_max_ms"):.2f}' for l in labels)} | "
          f"{arrow(key, 'tx_per_s', '{:,.0f}')} | {arrow(key, 'replica_hwm_kb', '{:.0f}', 1 / 1024)} | {'yes' if conv else 'NO'} |")
