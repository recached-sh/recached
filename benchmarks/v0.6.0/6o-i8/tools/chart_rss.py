#!/usr/bin/env python3
"""6O-I8: memory over time, one line per run, per build (static SVG).

    chart_rss.py RUNS_DIR CELL OUT.svg TITLE [--who primary|replica] [--key VmRSS]
                 [--diag allocated|resident]   (jemalloc, from the diag builds' I8MEM lines)

x: seconds since the load started (warm-up shaded, the post-load window
shaded); y: MiB. Thin lines are runs, the thick line is the median across
runs at each second. Colors: the reference palette's first three
categorical slots (validated all-pairs), H blue, I orange, P aqua; every
series is also labelled directly and in the legend.
"""
import json
import os
import re
import statistics
import sys

COLORS = {"h": "#2a78d6", "i": "#eb6834", "p": "#1baf7a"}
NAMES = {"h": "H (6O-H)", "i": "I (6O-I, before #200)", "p": "P (dev, after #200)"}
INK, INK2, GRID, SURF = "#0b0b0b", "#52514e", "#e1e0d9", "#fcfcfb"
ANSI = re.compile(r"\x1b\[[0-9;]*m")

args = sys.argv[1:]
d, cell, out, title = args[:4]
who = args[args.index("--who") + 1] if "--who" in args else "primary"
key = args[args.index("--key") + 1] if "--key" in args else "VmRSS"
diag = args[args.index("--diag") + 1] if "--diag" in args else None

runs = [json.loads(l) for l in open(f"{d}/runs.jsonl")]
runs = [r for r in runs if r["cell"] == cell and r["valid"] and "load" in r]
lines = {}
warm = measure = post_end = None
for r in runs:
    rd = f"{d}/{r['rep']}-{r['cell']}-{r['label']}"
    t0 = r["load_start"]
    pts = []
    if diag:
        for raw in open(f"{rd}/{who}.log.txt", errors="replace"):
            s = ANSI.sub("", raw)
            if s.startswith("I8MEM "):
                p = s.split()
                vals = dict(x.split("=") for x in p[2:])
                if vals.get(diag, "na") != "na":
                    pts.append((int(p[1]) / 1e3 - t0, int(vals[diag]) / 2**20))
    else:
        for line in open(f"{rd}/series.jsonl"):
            x = json.loads(line)
            v = x.get(who, {}).get("st", {}).get(key)
            if v is not None:
                pts.append((x["t"] - t0, v / 1024))
    lines.setdefault(r["label"], []).append(pts)
    ms = r["load"]["measure_start_ms"] / 1e3 - t0
    me = r["load"]["measure_end_ms"] / 1e3 - t0
    warm, measure = ms, me
    post_end = max(post_end or 0, pts[-1][0] if pts else 0)

W, H = 760, 400
L, R, T, B = 64, 170, 48, 52
xs = [t for ls in lines.values() for pts in ls for t, _ in pts]
ys = [v for ls in lines.values() for pts in ls for _, v in pts]
x0, x1 = max(-5, min(xs)), max(xs)
ymax = max(ys) * 1.08
ymin = 0
step = 10 if ymax < 80 else 20 if ymax < 160 else 50
ymax = (int(ymax / step) + 1) * step


def X(t):
    return L + (t - x0) / (x1 - x0) * (W - L - R)


def Y(v):
    return T + (1 - (v - ymin) / (ymax - ymin)) * (H - T - B)


o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" '
     f'font-family="system-ui, -apple-system, Segoe UI, sans-serif" font-size="12">',
     f'<title>{title}</title>', f'<rect width="{W}" height="{H}" fill="{SURF}"/>',
     f'<text x="{L}" y="22" font-size="15" font-weight="600" fill="{INK}">{title}</text>']
# Phases.
for a, b, label in ((x0, warm, "warm-up"), (measure, x1, "after the load")):
    if a is not None and b is not None and b > a:
        o.append(f'<rect x="{X(a):.1f}" y="{T}" width="{X(b) - X(a):.1f}" height="{H - T - B}" fill="#efeee9"/>')
        o.append(f'<text x="{(X(a) + X(b)) / 2:.1f}" y="{T + 14}" text-anchor="middle" fill="{INK2}" font-size="11">{label}</text>')
if warm is not None and measure is not None:
    o.append(f'<text x="{(X(warm) + X(measure)) / 2:.1f}" y="{T + 14}" text-anchor="middle" fill="{INK2}" font-size="11">measured window ({measure - warm:.0f} s)</text>')
# Grid and axes.
v = ymin
while v <= ymax + 1e-9:
    o.append(f'<line x1="{L}" x2="{W - R}" y1="{Y(v):.1f}" y2="{Y(v):.1f}" stroke="{GRID}" stroke-width="1"/>')
    o.append(f'<text x="{L - 8}" y="{Y(v) + 4:.1f}" text-anchor="end" fill="{INK2}">{v:g}</text>')
    v += step
tstep = 20 if x1 - x0 < 260 else 30
t = 0
while t <= x1:
    o.append(f'<text x="{X(t):.1f}" y="{H - B + 18}" text-anchor="middle" fill="{INK2}">{t:g}</text>')
    t += tstep
o.append(f'<text x="{(L + W - R) / 2}" y="{H - 12}" text-anchor="middle" fill="{INK2}">seconds since the load started</text>')
o.append(f'<text transform="translate(16,{(T + H - B) / 2}) rotate(-90)" text-anchor="middle" fill="{INK2}">MiB</text>')
# Lines: runs thin, the median thick.
ends = []
for label in ("h", "i", "p"):
    if label not in lines:
        continue
    c = COLORS[label]
    for n, pts in enumerate(lines[label]):
        path = " ".join(f"{X(t):.1f},{Y(v):.1f}" for t, v in pts)
        o.append(f'<polyline points="{path}" fill="none" stroke="{c}" stroke-width="1" stroke-opacity="0.35">'
                 f'<title>{NAMES[label]}, run {n + 1}</title></polyline>')
    # Median per whole second.
    buckets = {}
    for pts in lines[label]:
        for t, v in pts:
            buckets.setdefault(round(t), []).append(v)
    med = sorted((t, statistics.median(v)) for t, v in buckets.items() if len(v) >= max(1, len(lines[label]) // 2))
    path = " ".join(f"{X(t):.1f},{Y(v):.1f}" for t, v in med)
    o.append(f'<polyline points="{path}" fill="none" stroke="{c}" stroke-width="2.25" stroke-linejoin="round">'
             f'<title>{NAMES[label]}: median of {len(lines[label])} runs</title></polyline>')
    win = [v for t, v in med if warm <= t <= measure]
    ends.append((Y(med[-1][1]), label, statistics.mean(win) if win else med[-1][1]))
# Direct labels at the right edge, nudged apart.
ends.sort()
last = -99
for y, label, mean in ends:
    y = max(y, last + 16)
    last = y
    o.append(f'<circle cx="{W - R + 10}" cy="{y - 4:.1f}" r="4" fill="{COLORS[label]}"/>')
    o.append(f'<text x="{W - R + 18}" y="{y:.1f}" fill="{INK}">{NAMES[label].split(" (")[0]} · window mean {mean:.0f}</text>')
# Legend.
lx = L
for label in ("h", "i", "p"):
    if label in lines:
        o.append(f'<line x1="{lx}" x2="{lx + 18}" y1="36" y2="36" stroke="{COLORS[label]}" stroke-width="2.25"/>')
        o.append(f'<text x="{lx + 24}" y="40" fill="{INK2}">{NAMES[label]} ({len(lines[label])} runs)</text>')
        lx += 210
o.append("</svg>")
open(out, "w").write("\n".join(o))
print(f"wrote {out}: {sum(len(v) for v in lines.values())} runs")
