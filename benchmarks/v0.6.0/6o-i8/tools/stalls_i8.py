#!/usr/bin/env python3
"""6O-I8: failure rate and stall distribution of paced replica-with-AOF runs.

    stalls_i8.py RUNS_DIR [RUNS_DIR ...]

Per (build, offered rate), over every valid run's measured window:
  * disconnects (the primary's queue-full events in its log) per hour of
    load, from the load's start (warm-up included; a run with forced
    resynchronizations only until the first), with an exact 95% Poisson interval;
  * the replica's AOF write(2) time accumulated in each 1 s sample interval
    (`recached_aof_write_usec` deltas): the largest, and how many intervals
    held more than 0.3 / 0.5 / 1.0 s of write time: a proxy for the replica
    being blocked in its AOF write (review C, F2).
"""
import collections
import datetime
import glob
import gzip
import json
import math
import os
import re
import sys

ANSI = re.compile(r"\x1b\[[0-9;]*m")


def chi2_ppf(p, k):
    """Wilson–Hilferty approximation, enough for a Poisson interval."""
    if k <= 0:
        return 0.0
    z = {0.025: -1.959964, 0.975: 1.959964}[p]
    return k * (1 - 2 / (9 * k) + z * math.sqrt(2 / (9 * k))) ** 3


def poisson_ci(n, hours):
    lo = 0.0 if n == 0 else chi2_ppf(0.025, 2 * n) / 2
    hi = chi2_ppf(0.975, 2 * n + 2) / 2
    return lo / hours, hi / hours


def series(rd):
    p = f"{rd}/series.jsonl"
    f = open(p) if os.path.exists(p) else gzip.open(p + ".gz", "rt")
    return [json.loads(x) for x in f]


def log_lines(path):
    if os.path.exists(path):
        return open(path, errors="replace")
    if os.path.exists(path + ".gz"):
        return gzip.open(path + ".gz", "rt", errors="replace")
    return []


agg = collections.defaultdict(lambda: {"secs": 0.0, "disc": 0, "runs": 0, "w": []})
for d in sys.argv[1:]:
    for line in open(f"{d}/runs.jsonl"):
        r = json.loads(line)
        if not r.get("valid") or "load" not in r or r["load"].get("mode") != "paced":
            continue
        if r["cell"].startswith(("noaof", "tmpfs")):
            continue
        rd = f"{d}/{r['rep']}-{r['cell']}-{r['label']}"
        # From the load's start, warm-up included (review C round 2, N2); a
        # run with forced resynchronizations only until its first one.
        ms, me = r["load_start"], r["load_end"]
        if r.get("bounces"):
            me = r["bounces"][0]["killed"]
        key = (r["label"], r["load"]["rate"], os.path.basename(d.rstrip("/")))
        a = agg[key]
        a["secs"] += me - ms
        a["runs"] += 1
        for raw in log_lines(f"{rd}/primary.stdout.txt"):
            s = ANSI.sub("", raw)
            if "queue full" in s or "fell too far behind" in s:
                t = datetime.datetime.fromisoformat(s.split()[0].replace("Z", "+00:00")).timestamp()
                if ms <= t <= me:
                    a["disc"] += 1
        rows = [x for x in series(rd) if ms <= x["t"] <= me]
        prev = None
        for x in rows:
            w = x.get("replica", {}).get("info", {}).get("recached_aof_write_usec")
            if w is None:
                continue
            if prev is not None and w >= prev[1]:
                # Write time accumulated in this sample interval (about 1 s;
                # longer when the sampler's INFO waited on a stalled replica).
                a["w"].append((w - prev[1]) / 1e6)
                a["wmax"] = max(a.get("wmax", 0), (w - prev[1]) / 1e6)
            prev = (x["t"], w)

print("| build | offered tx/s | campaign | runs | measured min | disconnects | per hour [95%] | max write(2) s in one interval | intervals with > 0.3 / 0.5 / 1.0 s of write(2) |")
print("|---|---|---|---|---|---|---|---|---|")
for (label, rate, camp), a in sorted(agg.items(), key=lambda kv: (kv[0][1], kv[0][0], kv[0][2])):
    h = a["secs"] / 3600
    lo, hi = poisson_ci(a["disc"], h)
    w = a["w"]
    c = lambda th: sum(1 for v in w if v > th)
    print(f"| {label.upper()} | {rate} | {camp} | {a['runs']} | {a['secs'] / 60:.1f} | {a['disc']} | {a['disc'] / h:.0f} [{lo:.0f}–{hi:.0f}] | "
          f"{a.get('wmax', 0):.2f} | {c(0.3)} / {c(0.5)} / {c(1.0)} |")
