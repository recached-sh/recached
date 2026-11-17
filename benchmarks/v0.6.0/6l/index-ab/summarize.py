"""Median per configuration, base vs new, with min-max spread."""
import collections, json, statistics as st, sys
d = collections.defaultdict(list)
for line in open(sys.argv[1]):
    r = json.loads(line)
    k = (r["mode"], r.get("count"), r.get("threads"), r.get("capped"), r.get("pattern"), r.get("scatter"))
    d[(k, r["build"])].append(r)
field = {"scan": "full_scan_s", "create": "creates_per_s", "qsub": "snapshot_us"}
for k in sorted({k for k, _ in d}, key=str):
    b, n = d[(k, "base")], d[(k, "new")]
    f = field[k[0]]
    if k[0] == "scan":
        label = f"SCAN COUNT {k[1]} (s, lower is better)"
    elif k[0] == "create":
        label = f"create {'capped+reserved' if k[3] == 1 else 'uncapped'} {k[2]}t (/s)"
    else:
        label = f"QSUB {k[4]} {b[0]['matched']} keys{' scattered' if k[5] == 1 else ''} (us)"
    mb, mn = st.median(x[f] for x in b), st.median(x[f] for x in n)
    sp = lambda xs: f"{min(x[f] for x in xs):.4g}-{max(x[f] for x in xs):.4g}"
    print(f"{label:44} base {mb:>11.4g} [{sp(b)}]  new {mn:>11.4g} [{sp(n)}]  new/base {mn / mb:.2f}")
