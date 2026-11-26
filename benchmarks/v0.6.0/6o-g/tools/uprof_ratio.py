#!/usr/bin/env python3
"""Per-transaction user-space counters from g-uprof.sh output: instructions, cycles, IPC, branch, L1D and
cache misses per tx, as paired candidate/base ratios per mode (6O-G campaign 3). Usage: uprof_ratio.py uprof.log"""
import re,collections,statistics as st,sys
rows=collections.defaultdict(dict)
for l in open(sys.argv[1]):
    m=re.match(r"label=(\S+) mode=(\S+) rep=(\d+) tx=(\d+) tx_per_s=(\d+) uprof=\[(.*)\]",l)
    if not m: continue
    lab,mode,rep,tx,tps,u=m.groups(); tx=int(tx)
    kv={}
    for k,v in re.findall(r"(\w+)=([\d.]+)",u):
        kv.setdefault(k,float(v))
    d=dict(tps=int(tps),ins=kv['instructions']/tx,cyc=kv['cycles']/tx,ipc=kv['instructions']/kv['cycles'],
           bm=kv['branch_misses']/tx,l1d=kv['l1d_read_misses']/tx,cm=kv['cache_misses']/tx)
    rows[(mode,rep)][lab]=d
for mode in ["get-get","exists-get","set-set"]:
    for a,b in [("base","gcap"),("base-jcc","gcap-jcc")]:
        out={}
        for k in ["tps","ins","cyc","ipc","bm","l1d","cm"]:
            ra=[rows[(mode,r)][b][k]/rows[(mode,r)][a][k] for r in "123" if a in rows[(mode,r)] and b in rows[(mode,r)]]
            out[k]=f"{st.median(ra):.3f}[{min(ra):.3f}-{max(ra):.3f}]"
        absa={k:st.median(rows[(mode,r)][a][k] for r in "123") for k in ["ins","cyc","ipc"]}
        absb={k:st.median(rows[(mode,r)][b][k] for r in "123") for k in ["ins","cyc","ipc"]}
        print(mode,f"{b}/{a}",out, "abs",{k:round(v,2) for k,v in absa.items()},{k:round(v,2) for k,v in absb.items()})
