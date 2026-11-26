#!/usr/bin/env python3
"""Paired per-rep ratio between any two labels of a g-tx.sh log (6O-G campaign 3: gcap-jcc vs base-jcc).
Usage: pair_ratio.py tx.log <base-label> <candidate-label> [METRIC ...]"""
import sys,collections,statistics as st
path,a,b=sys.argv[1:4]; mets=sys.argv[4:] or ["tx_per_s","p99_us","oncpu_s"]
by=collections.defaultdict(dict)
for l in open(path):
    if not l.startswith("label="): continue
    kv=dict(t.split("=",1) for t in l.split() if "=" in t)
    c=f"{kv['mode']} c{kv['clients']} {kv.get('txenv','')} {kv.get('srvenv','')}"
    by[(c,kv['rep'])][kv['label']]=kv
cells=collections.defaultdict(lambda: collections.defaultdict(list))
for (c,r),d in by.items():
    if a in d and b in d:
        for m in mets:
            x,y=float(d[a][m]),float(d[b][m])
            if x: cells[c][m].append(y/x)
for c,ms in cells.items():
    print(c, " ".join(f"{m}={st.median(v):.3f}[{min(v):.3f}-{max(v):.3f}]n{len(v)}" for m,v in ms.items()))
