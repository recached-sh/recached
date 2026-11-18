#!/usr/bin/env python3
"""Summarize an aof-profile.sh results.csv into Markdown.

One row per (policy, workers, test, pipeline, clients); per server the median
throughput, the median server CPU in microseconds per request, and the median
p99. For Recached builds, the AOF counters as rates: frames per batch,
write(2) calls per million requests, and tasks sharing each write.

Usage: summarize.py <dir-with-results.csv> [server ...]
"""
import csv
import statistics
import sys
from collections import defaultdict
from pathlib import Path


def counters(text):
    out = {}
    for kv in text.split():
        k, _, v = kv.partition("=")
        out[k] = v
    return out


def med(xs):
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None


def num(s):
    try:
        return float(s)
    except (TypeError, ValueError):
        return None


def main():
    d = Path(sys.argv[1])
    rows = list(csv.DictReader(open(d / "results.csv")))
    servers = sys.argv[2:] or sorted({r["server"] for r in rows}, key=lambda s: ["base", "recached", "redis", "valkey"].index(s) if s in ["base", "recached", "redis", "valkey"] else 9)
    cells = defaultdict(lambda: defaultdict(list))
    for r in rows:
        key = (r["policy"], int(r["workers"]), r["test"], int(r["pipeline"]), int(r["clients"]))
        cells[key][r["server"]].append(r)
    cond = (d / "conditions.txt")
    if cond.exists():
        print("<details><summary>Conditions</summary>\n\n```\n" + cond.read_text() + "```\n</details>\n")
    hdr = ["policy", "workers", "test", "P", "clients"]
    for s in servers:
        hdr += [f"{s} rps", f"{s} µs CPU/req", f"{s} p99 ms"]
    for s in servers:
        if s in ("base", "recached"):
            hdr += [f"{s} frames/write", f"{s} writes/M req", f"{s} waiters/write"]
    print("| " + " | ".join(hdr) + " |")
    print("|" + "---|" * len(hdr))
    for key in sorted(cells):
        line = [key[0], str(key[1]), key[2], str(key[3]), str(key[4])]
        for s in servers:
            rs = cells[key].get(s, [])
            rps = med([num(r["rps"]) for r in rs])
            cpu = med([num(r["cpu_us_per_op"]) for r in rs])
            p99 = med([num(r["p99_ms"]) for r in rs])
            n = len(rs)
            line += [f"{rps:,.0f} n={n}" if rps else "—", f"{cpu:.2f}" if cpu else "—", f"{p99:.3f}" if p99 else "—"]
        for s in servers:
            if s not in ("base", "recached"):
                continue
            rs = cells[key].get(s, [])
            fpb, wpm, wpb = [], [], []
            for r in rs:
                c = counters(r.get("counters", ""))
                batches, frames, calls, waiters = (num(c.get(k)) for k in ("batches", "frames", "write_calls", "batch_waiters"))
                req = num(r["requests"])
                if batches:
                    fpb.append(frames / batches)
                    wpb.append(waiters / batches)
                if calls is not None and req:
                    wpm.append(calls * 1e6 / req)
            a, b, c3 = med(fpb), med(wpm), med(wpb)
            line += [f"{a:.1f}" if a else "—", f"{b:,.0f}" if b is not None else "—", f"{c3:.1f}" if c3 else "—"]
        print("| " + " | ".join(line) + " |")


if __name__ == "__main__":
    main()
