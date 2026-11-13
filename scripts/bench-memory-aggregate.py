#!/usr/bin/env python3
"""Aggregate repeated memory runs from scripts/bench-redis-valkey.sh.

Usage: scripts/bench-memory-aggregate.py DIR [--compare BEFORE AFTER]

DIR holds one sub-directory per run, named <label>-<rep> (e.g.
v06opt-small-2), each with a memory.csv. For every (label, kind, keys, value
size) this prints the median of the repetitions with the min–max range, so a
difference smaller than the spread is visible as such. Bytes/key is
(RSS after load - idle RSS) / keys stored; "retained" is RSS after FLUSHDB
above idle, as a share of what loading added.
"""
import csv
import statistics
import sys
from collections import defaultdict
from pathlib import Path


def main():
    root = Path(sys.argv[1])
    compare = None
    if "--compare" in sys.argv:
        i = sys.argv.index("--compare")
        compare = (sys.argv[i + 1], sys.argv[i + 2])

    cells = defaultdict(lambda: {"per_key": [], "retained": [], "loaded_mib": []})
    labels = []
    for run in sorted(p for p in root.iterdir() if p.is_dir()):
        label = run.name.rsplit("-", 1)[0]
        if label not in labels:
            labels.append(label)
        mem = run / "memory.csv"
        if not mem.exists():
            continue
        for r in csv.DictReader(open(mem)):
            idle, loaded = int(r["rss_idle_kb"]), int(r["rss_loaded_kb"])
            flushed = int(r["rss_after_flush_kb"])
            keys = int(r["dbsize"] or 0) or int(r["keys"])
            key = (label, r["kind"], int(r["keys"]), int(r["value_size"]))
            cells[key]["per_key"].append((loaded - idle) * 1024 / keys)
            cells[key]["loaded_mib"].append(loaded / 1024)
            if loaded > idle:
                cells[key]["retained"].append((flushed - idle) / (loaded - idle) * 100)

    print("| run | kind | keys | value | bytes/key (median) | range | n | RSS loaded MiB | retained after FLUSHDB |")
    print("|---|---|---|---|---:|---|---:|---:|---:|")
    order = {label: i for i, label in enumerate(labels)}
    for key in sorted(cells, key=lambda k: (k[2], k[3], k[1], order[k[0]])):
        c = cells[key]
        pk = c["per_key"]
        print(
            f"| {key[0]} | {key[1]} | {key[2]:,} | {key[3]} | {statistics.median(pk):,.0f} | "
            f"{min(pk):,.0f}–{max(pk):,.0f} | {len(pk)} | {statistics.median(c['loaded_mib']):,.1f} | "
            f"{statistics.median(c['retained']):.0f}% |"
        )

    if compare:
        a, b = compare
        print(f"\n### {b} vs {a} (median bytes/key)\n")
        print("| kind | keys | value | before | after | change |")
        print("|---|---|---|---:|---:|---:|")
        for key in sorted(k for k in cells if k[0] == a):
            other = (b,) + key[1:]
            if other not in cells:
                continue
            before = statistics.median(cells[key]["per_key"])
            after = statistics.median(cells[other]["per_key"])
            print(
                f"| {key[1]} | {key[2]:,} | {key[3]} | {before:,.0f} | {after:,.0f} | "
                f"{(after / before - 1) * 100:+.1f}% |"
            )


if __name__ == "__main__":
    main()
