#!/usr/bin/env python3
"""Summarise Phase 6I benchmark output into Markdown tables.

    summarize.py prod <results.csv> [more.csv ...]     memtier production matrix (prod.sh)
    summarize.py ws <ws-results.csv> [more ...]        WebSocket sync matrix (ws.sh)
    summarize.py inproc <request-path.csv> [more ...]  in-process request path (abba-n.sh)
    summarize.py ttl <ttl-churn.csv>                   expiry backlog (ttl-churn.sh)

prod: one table per config x workers, one row per cell, a column per server:
median ops/s over repetitions [min-max], CPU-s per million ops, p99. A
latency above 60 s is a memtier histogram artefact (seen on fsync-bound cells)
and is reported as "artefact" rather than averaged in.
"""
import csv
import statistics
import sys
from collections import defaultdict

ORDER = ["v051", "6g", "6h", "6i", "6i-rc3", "redis", "valkey"]
LABEL = {"v051": "v0.5.1", "6g": "6G", "6h": "6H", "6i": "6I", "6i-rc3": "6I rc3",
         "redis": "Redis 7.2.5", "valkey": "Valkey 9.1.0"}


def rows(paths):
    for p in paths:
        with open(p, newline="") as f:
            yield from (r for r in csv.DictReader(f) if not r.get("server", "").startswith("#"))


def num(v):
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def med_rng(xs, fmt="{:,.0f}"):
    xs = [x for x in xs if x is not None]
    if not xs:
        return "—"
    m = statistics.median(xs)
    s = fmt.format(m)
    if len(xs) > 1:
        s += f" [{fmt.format(min(xs))}–{fmt.format(max(xs))}]"
    return s + f" n={len(xs)}"


def prod(paths):
    data = defaultdict(lambda: defaultdict(list))
    for r in rows(paths):
        if r.get("errors") == "parse-error" or not r.get("ops_per_sec"):
            continue
        data[(r["config"], r["workers"])][(r["cell"], r["server"])].append(r)
    for (config, workers) in sorted(data):
        cells = data[(config, workers)]
        servers = [s for s in ORDER if any(k[1] == s for k in cells)]
        cell_names = list(dict.fromkeys(k[0] for k in cells))
        print(f"\n#### {config}, {workers} worker(s)\n")
        print("| Cell | " + " | ".join(LABEL.get(s, s) for s in servers) + " |")
        print("|---|" + "---:|" * len(servers))
        for cell in cell_names:
            out = []
            for s in servers:
                rs = cells.get((cell, s), [])
                if not rs:
                    out.append("—")
                    continue
                ops = [num(r["ops_per_sec"]) for r in rs]
                cpu = [num(r["cpu_s_per_mops"]) for r in rs]
                p99 = [num(r["p99_ms"]) for r in rs]
                p99 = [x for x in p99 if x is not None]
                p99s = ("artefact" if p99 and statistics.median(p99) > 60_000
                        else f"{statistics.median(p99):.2f}" if p99 else "—")
                cell_s = f"{med_rng(ops)}<br>CPU {med_rng(cpu, '{:.2f}').split(' n=')[0]} · p99 {p99s}"
                if config == "repl":
                    lag50 = [num(r["lag_p50_ms"]) for r in rs]
                    lag99 = [r["lag_p99_ms"] for r in rs]
                    conv = [r["converged"] for r in rs]
                    l99 = [num(x) for x in lag99 if x not in ("", "timeout")]
                    cell_s += (f"<br>lag p50 {med_rng(lag50, '{:.1f}').split(' [')[0].split(' n=')[0]}"
                               f" p99 {statistics.median(l99):.1f}" if l99 else " p99 timeout")
                    if any(c != "ok" for c in conv):
                        cell_s += f" **{conv}**"
                out.append(cell_s)
            print(f"| {cell} | " + " | ".join(out) + " |")


def ws(paths):
    data = defaultdict(list)
    for r in rows(paths):
        data[(r["workers"], r["clients"], r["cell"], r["server"])].append(r)
    keys = sorted({k[:3] for k in data}, key=lambda k: (int(k[0]), int(k[1]), k[2]))
    servers = [s for s in ORDER if any(k[3] == s for k in data)]
    print("| Workers | Clients | Cell | Server | writes/s | CPU-s/M | KB/client | probe p50 / p95 / p99 ms | lag disconnects | reconnects (fail) | leaks | DEDUP first/dup/wrong/applied |")
    print("|---:|---:|---|---|---:|---:|---:|---|---:|---|---:|---|")
    for k in keys:
        for s in servers:
            rs = data.get((*k, s), [])
            if not rs:
                continue
            g = lambda c: [num(r[c]) for r in rs]
            probe = "/".join(med_rng(g(c), "{:.1f}").split(" [")[0].split(" n=")[0]
                             for c in ("probe_p50_ms", "probe_p95_ms", "probe_p99_ms"))
            dedup = "/".join(str(int(sum(x for x in g(c) if x))) for c in
                             ("dedup_first", "dedup_dup", "dedup_wrong", "dedup_applied"))
            print(f"| {k[0]} | {k[1]} | {k[2]} | {LABEL.get(s, s)} | {med_rng(g('ops_per_sec'))} | "
                  f"{med_rng(g('cpu_s_per_mops'), '{:.1f}').split(' n=')[0]} | "
                  f"{med_rng(g('kb_per_client'), '{:.1f}').split(' n=')[0]} | {probe} | "
                  f"{int(sum(x for x in g('closes_unexpected') if x))} | "
                  f"{int(sum(x for x in g('reconnects') if x))} ({int(sum(x for x in g('reconnect_fail') if x))}) | "
                  f"{int(sum(x for x in g('leaks') if x))} | {dedup} |")


def inproc(paths):
    data = defaultdict(list)
    for p in paths:
        for line in open(p):
            if line.startswith("#"):
                continue
            f = line.strip().split(",")
            if len(f) < 7:
                continue
            data[(f[1], f[0])].append((float(f[3]), float(f[5]), float(f[6])))
    labels = list(dict.fromkeys(k[1] for k in data))
    workloads = list(dict.fromkeys(k[0] for k in data))
    print("| Workload | " + " | ".join(f"{l} min ns (allocs, bytes)" for l in labels) + " |")
    print("|---|" + "---:|" * len(labels))
    for w in workloads:
        cells = []
        for l in labels:
            v = data.get((w, l))
            cells.append("—" if not v else f"{min(x[0] for x in v):,.1f} ({v[0][1]:.2f}, {v[0][2]:.1f}) n={len(v)}")
        print(f"| {w} | " + " | ".join(cells) + " |")


def ttl(paths):
    data = defaultdict(list)
    for r in rows(paths):
        data[r["server"]].append(r)
    print("| Server | keys at end of load | RSS MB end of load | keys after 30 s idle | RSS MB after idle |")
    print("|---|---:|---:|---:|---:|")
    for s in [s for s in ORDER if s in data]:
        rs = data[s]
        load = [r for r in rs if r["phase"] == "load"]
        idle = [r for r in rs if r["phase"] == "idle"]
        start = int(rs[0]["rss_kb"])
        print(f"| {LABEL.get(s, s)} | {int(load[-1]['dbsize']):,} | {int(load[-1]['rss_kb']) / 1024:.1f} (+{(int(load[-1]['rss_kb']) - start) / 1024:.1f}) | "
              f"{int(idle[-1]['dbsize']):,} | {int(idle[-1]['rss_kb']) / 1024:.1f} |")


if __name__ == "__main__":
    {"prod": prod, "ws": ws, "inproc": inproc, "ttl": ttl}[sys.argv[1]](sys.argv[2:])
