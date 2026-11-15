#!/usr/bin/env python3
"""One ws-route-results.csv row from a ws-route.sh cell's outputs."""
import glob, json, sys

(out, tag, name, scenario, workers, clients, rep, offered,
 c0, c1, idle, withc, endrss, gaps) = sys.argv[1:15]


def load(p):
    try:
        with open(p) as f:
            return json.load(f)
    except Exception:
        return {}


def info(p):
    """INFO fields as a dict; absent fields (older servers) stay absent."""
    fields = {}
    try:
        with open(p) as f:
            for line in f:
                if ":" in line and not line.startswith("#"):
                    k, v = line.strip().split(":", 1)
                    fields[k] = v
    except Exception:
        pass
    return fields


d = f"{out}/ws"
mt = load(f"{d}/{tag}-memtier.json").get("ALL STATS", {}).get("Totals", {})
ops = mt.get("Ops/sec", 0.0)
count = mt.get("Count", 0) or 0
p99 = mt.get("Percentile Latencies", {}).get("p99.00", "")
cpu = float(c1) - float(c0)
per = f"{cpu / (count / 1e6):.3f}" if count else ""
parts = [load(p) for p in sorted(glob.glob(f"{d}/{tag}-clients-*.json"))]
w = load(f"{d}/{tag}-writer.json")
s = lambda k: sum(p.get(k, 0) or 0 for p in parts)


def q(k):
    # Per-process quantiles cannot be merged exactly: report the max across
    # processes (conservative).
    vals = [p["probe"][k] for p in parts if p.get("probe")]
    return f"{max(vals):.3f}" if vals else ""


rq = [p["reconnect"]["p99"] for p in parts if p.get("reconnect")]
n = int(clients)
kbpc = f"{(int(withc) - int(idle)) / n:.1f}" if n else ""
before = info(f"{out}/info/{tag}-before.txt")
after = info(f"{out}/info/{tag}-after.txt")


def delta(k):
    if k not in after:
        return ""
    try:
        return str(int(after[k]) - int(before.get(k, 0)))
    except ValueError:
        return ""


print(",".join(str(x) for x in [
    name, scenario, workers, clients, rep, offered, f"{ops:.0f}", p99, f"{cpu:.3f}", per,
    idle, withc, kbpc, endrss,
    s("connected"), s("pushes"), sum((p.get("probe") or {}).get("n", 0) for p in parts),
    q("p50"), q("p95"), q("p99"), q("max"), s("leaks"), s("reconnects"), s("reconnectFailures"),
    f"{max(rq):.1f}" if rq else "", s("renewals"), s("renewFailures"), s("churns"), s("churnFailures"),
    s("slowOpened"), s("slowClosedByServer"), s("closesUnexpected"), gaps,
    w.get("dedupFirst", ""), w.get("dedupDup", ""), w.get("dedupWrong", ""), w.get("dedupApplied", ""),
    s("errors") + (w.get("errors", 0) or 0),
    delta("sync_route_events"), delta("sync_route_candidates"), delta("sync_route_skipped"),
    delta("sync_pushes_forwarded"), delta("sync_pushes_filtered")]))
