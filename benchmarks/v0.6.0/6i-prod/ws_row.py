#!/usr/bin/env python3
"""One ws-results.csv row from a ws.sh cell's JSON outputs."""
import glob, json, sys
d, tag, name, workers, clients, cell, rep, c0, c1, idle, withc, endrss = sys.argv[1:13]
def load(p):
    try:
        return json.load(open(p))
    except Exception:
        return {}
mt = load(f"{d}/{tag}-memtier.json").get("ALL STATS", {}).get("Totals", {})
ops = mt.get("Ops/sec", 0.0); count = mt.get("Count", 0) or 0
p99 = mt.get("Percentile Latencies", {}).get("p99.00", "")
cpu = float(c1) - float(c0)
per = f"{cpu / (count / 1e6):.3f}" if count else ""
parts = [load(p) for p in sorted(glob.glob(f"{d}/{tag}-clients-*.json"))]
w = load(f"{d}/{tag}-writer.json")
s = lambda k: sum(p.get(k, 0) or 0 for p in parts)
# Probe quantiles: the per-process quantiles cannot be merged exactly; report
# the max of each across processes (conservative) and the summed count.
def q(k):
    vals = [p["probe"][k] for p in parts if p.get("probe")]
    return f"{max(vals):.3f}" if vals else ""
rq = [p["reconnect"]["p99"] for p in parts if p.get("reconnect")]
n = int(clients)
kbpc = f"{(int(withc) - int(idle)) / n:.1f}" if n else ""
print(",".join(str(x) for x in [
    name, workers, clients, cell, rep, f"{ops:.0f}", p99, f"{cpu:.3f}", per, idle, withc, kbpc, endrss,
    s("connected"), s("pushes"), sum((p.get("probe") or {}).get("n", 0) for p in parts),
    q("p50"), q("p95"), q("p99"), q("max"), s("leaks"), s("reconnects"), s("reconnectFailures"),
    f"{max(rq):.1f}" if rq else "", s("resumedAfterReconnect"), s("closesUnexpected"),
    w.get("dedupFirst", ""), w.get("dedupDup", ""), w.get("dedupWrong", ""), w.get("dedupApplied", ""),
    s("errors") + (w.get("errors", 0) or 0)]))
