#!/usr/bin/env python3
"""Summarise a scripts/bench-redis-valkey.sh run as Markdown.

Every throughput/latency cell is the MEDIAN of the repetitions, with the
interquartile range alongside it, so a reader can see when two servers are
within each other's noise. Nothing is dropped: a workload one server failed
shows as a gap rather than disappearing from the table.

Usage: scripts/bench-summarize.py OUT_DIR [--compare A B]

--compare A B adds a column with B's change relative to A (e.g. baseline vs
recached for a before/after table).
"""
import csv
import statistics
import sys
from collections import defaultdict
from pathlib import Path


def quartiles(xs):
    xs = sorted(xs)
    if len(xs) < 2:
        return xs[0], xs[0]
    q = statistics.quantiles(xs, n=4, method="inclusive")
    return q[0], q[2]


def fmt_int(x):
    return f"{x:,.0f}"


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(2)
    out = Path(sys.argv[1])
    compare = None
    if "--compare" in sys.argv:
        i = sys.argv.index("--compare")
        compare = (sys.argv[i + 1], sys.argv[i + 2])

    rows = list(csv.DictReader(open(out / "results.csv")))
    servers = []
    for r in rows:
        if r["server"] not in servers:
            servers.append(r["server"])

    cells = defaultdict(lambda: defaultdict(list))
    for r in rows:
        try:
            rps = float(r["rps"])
        except (TypeError, ValueError):
            continue
        key = (r["test"], r["pipeline"], r["clients"], r["size"], r["keyspace"])
        c = cells[key][r["server"]]
        cpu = float(r["server_cpu_s"] or 0)
        c.append(
            {
                "rps": rps,
                "p50": float(r["p50_ms"]),
                "p99": float(r["p99_ms"]),
                "max": float(r["max_ms"]),
                "cpu": cpu,
                "ops_per_cpu_s": (float(r["requests"]) / cpu) if cpu > 0 else float("nan"),
                "cpu_s_per_mops": cpu / float(r["requests"]) * 1e6,
            }
        )

    print(f"# Benchmark summary — `{out}`\n")
    cond = out / "conditions.txt"
    if cond.exists():
        print("<details><summary>Conditions</summary>\n\n```")
        print(cond.read_text().strip())
        print("```\n</details>\n")

    print("## Throughput (median rps, [IQR]) and p99 latency (median ms)\n")
    hdr = ["test", "P", "clients", "size", "keys"]
    for s in servers:
        hdr += [f"{s} rps", f"{s} p99"]
    if compare:
        hdr += [f"Δ rps {compare[1]} vs {compare[0]}", "Δ p99"]
    print("| " + " | ".join(hdr) + " |")
    print("|" + "---|" * len(hdr))
    for key in sorted(cells, key=lambda k: (k[4], int(k[3]), int(k[1]), int(k[2]), k[0])):
        line = list(key[:1]) + [key[1], key[2], key[3], key[4]]
        med = {}
        for s in servers:
            runs = cells[key].get(s, [])
            if not runs:
                line += ["—", "—"]
                continue
            rps = [x["rps"] for x in runs]
            q1, q3 = quartiles(rps)
            m = statistics.median(rps)
            p99 = statistics.median(x["p99"] for x in runs)
            med[s] = (m, p99)
            line += [f"{fmt_int(m)} [{fmt_int(q1)}–{fmt_int(q3)}] n={len(runs)}", f"{p99:.3f}"]
        if compare:
            a, b = compare
            if a in med and b in med and med[a][0] > 0:
                line += [
                    f"{(med[b][0] / med[a][0] - 1) * 100:+.1f}%",
                    f"{(med[b][1] / med[a][1] - 1) * 100:+.1f}%" if med[a][1] > 0 else "—",
                ]
            else:
                line += ["—", "—"]
        print("| " + " | ".join(line) + " |")

    print("\n## CPU efficiency (median server CPU-seconds per million requests)\n")
    print("Lower is better. Measured from the server process's own user+system time,")
    print("so it is comparable across threading models.\n")
    hdr = ["test", "P", "clients", "size", "keys"] + servers
    print("| " + " | ".join(hdr) + " |")
    print("|" + "---|" * len(hdr))
    for key in sorted(cells, key=lambda k: (k[4], int(k[3]), int(k[1]), int(k[2]), k[0])):
        line = list(key[:1]) + [key[1], key[2], key[3], key[4]]
        for s in servers:
            runs = cells[key].get(s, [])
            line.append(f"{statistics.median(x['cpu_s_per_mops'] for x in runs):.2f}" if runs else "—")
        print("| " + " | ".join(line) + " |")

    mem_path = out / "memory.csv"
    if mem_path.exists():
        mrows = list(csv.DictReader(open(mem_path)))
        if mrows:
            print("\n## Process memory (RSS)\n")
            print("`bytes/key` = (RSS after load − idle RSS) / keys actually stored (DBSIZE).")
            print("RSS includes allocator overhead and fragmentation; it is not any server's")
            print("logical `used_memory` figure.\n")
            print("| server | kind | keys | value | idle MiB | loaded MiB | after overwrite MiB | after FLUSHDB MiB | bytes/key | retained after flush |")
            print("|---|---|---|---|---|---|---|---|---|---|")
            for r in mrows:
                idle, loaded = int(r["rss_idle_kb"]), int(r["rss_loaded_kb"])
                churn, flushed = int(r["rss_after_overwrite_kb"]), int(r["rss_after_flush_kb"])
                keys = int(r["dbsize"] or 0) or int(r["keys"])
                per_key = (loaded - idle) * 1024 / keys if keys else float("nan")
                retained = (flushed - idle) / (loaded - idle) * 100 if loaded > idle else float("nan")
                print(
                    f"| {r['server']} | {r['kind']} | {fmt_int(keys)} | {r['value_size']} | "
                    f"{idle / 1024:.1f} | {loaded / 1024:.1f} | {churn / 1024:.1f} | {flushed / 1024:.1f} | "
                    f"{per_key:,.0f} | {retained:.0f}% |"
                )


if __name__ == "__main__":
    main()
