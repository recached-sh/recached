"""Summarise abba.sh CSVs.

request_path rows: label,workload,sync,min_ns,median_ns,allocs,bytes,samples
hot rows:          label,shape,workers,best_ops,median_ops,samples

For each label x workload: the lowest per-run minimum (the run least disturbed
by the host), the median of per-run medians, and the change from the first
label to each other one. Usage: summarize.py <csv>...
"""
import csv, statistics, sys
from collections import defaultdict

ns = defaultdict(lambda: defaultdict(list))   # wl -> label -> [(min, med)]
alloc = defaultdict(dict)
hot = defaultdict(lambda: defaultdict(list))
labels = []
for path in sys.argv[1:]:
    for row in csv.reader(l for l in open(path) if not l.startswith('#')):
        if len(row) == 8:
            label, wl, sync, mn, med, al, by, _ = row
            key = wl + (' [sync]' if sync == 'true' else '')
            ns[key][label].append((float(mn), float(med)))
            alloc[key][label] = (float(al), float(by))
        elif len(row) == 6:
            label, shape, w, best, med, _ = row
            hot[f'{shape} w={w}'][label].append((float(best), float(med)))
        else:
            continue
        if label not in labels:
            labels.append(label)

def pct(a, b):
    return f'{(b - a) / a * 100:+.1f}%'

if ns:
    print('| workload | ' + ' | '.join(f'{l} min ns / med ns / allocs' for l in labels) + ' | Δ min |')
    print('|---|' + '---:|' * (len(labels) + 1))
    for wl, by in ns.items():
        cells, mins = [], []
        for l in labels:
            if l not in by:
                cells.append('—'); continue
            mn = min(m for m, _ in by[l]); md = statistics.median(m for _, m in by[l])
            mins.append(mn)
            a = alloc[wl].get(l, (0, 0))[0]
            cells.append(f'{mn:.0f} / {md:.0f} / {a:.2f} (n={len(by[l])})')
        d = ' '.join(pct(mins[0], m) for m in mins[1:]) if len(mins) > 1 else ''
        print(f'| {wl} | ' + ' | '.join(cells) + f' | {d} |')
if hot:
    print('\n| shape | ' + ' | '.join(f'{l} best / median ops/s' for l in labels) + ' | Δ best |')
    print('|---|' + '---:|' * (len(labels) + 1))
    for sh, by in hot.items():
        cells, bests = [], []
        for l in labels:
            if l not in by:
                cells.append('—'); continue
            b = max(x for x, _ in by[l]); md = statistics.median(m for _, m in by[l])
            bests.append(b)
            cells.append(f'{b:,.0f} / {md:,.0f} (n={len(by[l])})')
        d = ' '.join(pct(bests[0], x) for x in bests[1:]) if len(bests) > 1 else ''
        print(f'| {sh} | ' + ' | '.join(cells) + f' | {d} |')
