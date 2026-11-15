#!/usr/bin/env python3
"""Best repetition per engine and cell: the repetition with the lowest server CPU per
operation, as "ops/s · CPU-s per M ops · p99 ms". Used only on runs disturbed by host
load (see docs/reports/v0.6.0/V0.6I-PERFORMANCE-COMPARISON.md 1a); it does not recover an
undisturbed figure. Usage: best_rep.py <dir with rep-*/results.csv> "<server labels>"
"""
import csv,sys,glob,collections
rows=collections.defaultdict(list)
for f in sorted(glob.glob(sys.argv[1]+'/rep-*/results.csv')):
    for r in csv.DictReader(open(f)):
        rows[(r['config'],r['workers'],r['cell'],r['server'])].append(r)
servers=sys.argv[2].split()
keys=sorted({k[:3] for k in rows},key=lambda k:(k[0],int(k[1]),k[2]))
print('Best repetition: ops/s · CPU-s per M ops · p99 ms\n')
print('| Config, workers, cell | '+' | '.join(servers)+' |')
print('|---|'+'---:|'*len(servers))
for cfg,w,cell in keys:
    out=[]
    for s in servers:
        rs=rows.get((cfg,w,cell,s))
        if not rs: out.append('—'); continue
        b=min(rs,key=lambda r:float(r['cpu_s_per_mops'] or 1e9))
        out.append(f"{float(b['ops_per_sec']):,.0f} · {float(b['cpu_s_per_mops']):.2f} · {float(b['p99_ms']):.2f}")
    print(f"| {cfg} {w}w {cell} | "+" | ".join(out)+" |")
