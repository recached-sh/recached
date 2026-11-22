#!/usr/bin/env python3
"""Per-op decomposition of a run-6ob.sh results.txt: the server CPU's budget.

    decomp.py results.txt

Per cell (median of repetitions): ops/s; wall µs/op (the server CPU is
saturated at 50 clients, so wall time per op is its whole budget); the
server task's exact on-CPU time (schedstat); softirq and idle on the
server's CPU (/proc/stat, USER_HZ=100 ticks; with IRQ_TIME_ACCOUNTING the
softirq run inline in the server's send(2) is charged here, not to the
task); the generator's voluntary context switches per op; user-space cycles
and instructions per op (uprof); p99 (ms).
"""
import re,sys,statistics as st
recs=[];cur=None
for l in open(sys.argv[1]):
    if l.startswith('cell='): cur=dict(re.findall(r'(\w+)=(\S+)',l)); recs.append(cur)
    elif ':' in l and cur is not None: k,v=l.strip().split(':',1); cur[k]=v.strip()
agg={}
for r in recs:
    n=float(r['requests']); 
    try: rps=float(r['bench'].split(',')[1].strip('"'))
    except: continue
    c0=list(map(int,r['cpu0_before'].split())); c1=list(map(int,r['cpu0_after'].split()))
    g=dict(re.findall(r'(\w+)=(\S+)',r.get('gen_rusage','')))
    u=dict(re.findall(r'(\w+)=([0-9.]+) opened',r.get('uprof','')))
    p99=float(r['bench'].split(',')[6].strip('"'))
    row=dict(rps=rps,wall=float(r['wall_s'])*1e6/n,task=float(r['oncpu_s'])*1e6/n,si=(c1[6]-c0[6])*1e4/n,idle=(c1[3]-c0[3])*1e4/n,
             vcsw=int(g.get('nvcsw',0))/n,cyc=float(u.get('cycles',0))/n,ins=float(u.get('instructions',0))/n,p99=p99)
    agg.setdefault(r['cell'],[]).append(row)
keys=['rps','wall','task','si','idle','vcsw','cyc','ins','p99']
print(f"{'cell':30} {'n':>2} "+' '.join(f'{k:>8}' for k in keys))
ref={}
for c,rs in agg.items():
    m={k:st.median([x[k] for x in rs]) for k in keys}
    t=c.split('/')[-1].replace('redis-noaof-','rc-noaof-')
    print(f"{c:30} {len(rs):>2} "+' '.join(f'{m[k]:8.0f}' if k in('rps','cyc','ins') else f'{m[k]:8.3f}' for k in keys))
