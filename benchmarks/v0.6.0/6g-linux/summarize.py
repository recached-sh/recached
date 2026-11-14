#!/usr/bin/env python3
"""Summarise 6G Linux benchmark CSVs into Markdown tables.

    summarize.py memory <memory.csv> [more.csv ...]
    summarize.py throughput <results.csv> [more.csv ...]

Memory: bytes per key = (phase RSS - idle RSS) / keys, per run; the table
shows the median across reps and the [min-max] range. "settled" (+14 s after
the load) is the headline; "loaded" (+2 s) is comparable with the 6E numbers.

Throughput: median ops/s and latency across reps, [min-max] for ops/s, and
server CPU-seconds per million operations. A latency above the cell's test
time is a memtier histogram artefact (seen once in smoke tests: p99.9 =
604 s in a 2 s run) and is dropped rather than averaged in.
"""
import csv
import statistics
import sys
from collections import defaultdict

ORDER = ["v051", "dev", "opt", "opt-bg", "opt-sys", "final", "final-nobg", "redis", "valkey"]
LABEL = {
    "v051": "v0.5.1",
    "dev": "v0.6 current",
    "opt": "v0.6 optimized",
    "opt-bg": "v0.6 opt + jemalloc bg thread",
    "opt-sys": "v0.6 opt, system malloc",
    "final": "v0.6 final",
    "final-nobg": "v0.6 final, no bg thread",
    "redis": "Redis 7.2.5",
    "valkey": "Valkey 9.1.0",
}


def med(xs):
    return statistics.median(xs) if xs else None


def rng(xs, fmt="{:.0f}"):
    return f"[{fmt.format(min(xs))}–{fmt.format(max(xs))}]" if len(xs) > 1 else ""


def rows(paths):
    for p in paths:
        with open(p, newline="") as f:
            yield from csv.DictReader(f)


def memory(paths):
    phases = ["loaded", "settled", "peak", "overwritten", "deleted", "reclaimed", "flushed"]
    data = defaultdict(lambda: defaultdict(list))
    for r in rows(paths):
        key = (r["kind"], int(r["keys"]), int(r["value_bytes"]))
        idle = int(r["idle_kb"])
        n = int(r["keys"])
        for ph in phases:
            v = r.get(f"{ph}_kb", "")
            if v:
                data[key][(r["server"], ph)].append((int(v) - idle) * 1024 / n)
    servers = [s for s in ORDER if any(s == k[0] for d in data.values() for k in d)]
    for key in sorted(data, key=lambda k: (k[0] != "string", k[0], -k[1], k[2])):
        kind, n, size = key
        print(f"\n#### {kind}, {n:,} keys × {size} B — bytes/key over idle RSS (median, [range], n)\n")
        print("| Server | loaded (+2 s) | **settled (+14 s)** | peak | overwritten | after DEL all | reclaimed (+40 s) | after FLUSHDB |")
        print("|---|---:|---:|---:|---:|---:|---:|---:|")
        for s in servers:
            cells = []
            for ph in phases:
                xs = data[key].get((s, ph), [])
                if not xs:
                    cells.append("—")
                    continue
                m = f"{med(xs):.1f}"
                if ph == "settled":
                    m = f"**{m}** {rng(xs, '{:.1f}')} n={len(xs)}"
                cells.append(m)
            if any(c != "—" for c in cells):
                print(f"| {LABEL.get(s, s)} | " + " | ".join(cells) + " |")


def throughput(paths, test_time_ms=10_000):
    data = defaultdict(lambda: defaultdict(list))
    for r in rows(paths):
        if not r["ops_per_sec"]:
            continue
        k = (r["mode"], r["cell"])
        rec = data[k][r["server"]]
        lat = {}
        for f in ("p50_ms", "p99_ms", "p999_ms"):
            try:
                v = float(r[f])
                lat[f] = v if v <= test_time_ms else None
            except ValueError:
                lat[f] = None
        rec.append(
            dict(
                ops=float(r["ops_per_sec"]),
                cpu=float(r["cpu_s_per_mops"]) if r["cpu_s_per_mops"] else None,
                errors=int(r["errors"] or 0),
                **lat,
            )
        )
    for (mode, cell) in sorted(data):
        print(f"\n#### {cell} — {mode} (median of n; ops/s [min–max])\n")
        print("| Server | ops/s | p50 ms | p99 ms | p99.9 ms | CPU-s / M ops | errors |")
        print("|---|---:|---:|---:|---:|---:|---:|")
        for s in ORDER:
            recs = data[(mode, cell)].get(s)
            if not recs:
                continue
            ops = [x["ops"] for x in recs]
            def m(field, fmt="{:.3f}"):
                xs = [x[field] for x in recs if x[field] is not None]
                return fmt.format(med(xs)) if xs else "—"
            errs = sum(x["errors"] for x in recs)
            print(
                f"| {LABEL.get(s, s)} | {med(ops):,.0f} {rng(ops, '{:,.0f}')} n={len(recs)} | "
                f"{m('p50_ms')} | {m('p99_ms')} | {m('p999_ms')} | {m('cpu', '{:.2f}')} | {errs} |"
            )


if __name__ == "__main__":
    what, paths = sys.argv[1], sys.argv[2:]
    {"memory": memory, "throughput": throughput}[what](paths)
