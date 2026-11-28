#!/usr/bin/env python3
"""6O-I8: the resynchronization cycle of each run, from the servers' logs.

    cycles_i8.py RUNS_DIR [CELL_PREFIX]

For every queue-full disconnect during the load: the time the replica had
streamed since its last install (survival), the time it took to notice the
end of the link (applying what it had read), the reconnect wait, and the
install (full-sync transfer, load and checkpoint). Medians per run.
"""
import datetime
import json
import re
import statistics
import sys

ANSI = re.compile(r"\x1b\[[0-9;]*m")
PAT = re.compile(r"^(\S+Z)\s+\w+\s+\S+:\s+(.*)$")


def ts(s):
    return datetime.datetime.fromisoformat(s.replace("Z", "+00:00")).timestamp()


def evs(path, keys):
    out = []
    for raw in open(path, errors="replace"):
        m = PAT.match(ANSI.sub("", raw).rstrip())
        if m:
            for k, name in keys.items():
                if k in m.group(2):
                    out.append((ts(m.group(1)), name))
    return out


d = sys.argv[1]
pref = sys.argv[2] if len(sys.argv) > 2 else ""
print("| cell | label | disconnects in the measured window | survival s (median, installs during the load only) | notice s | reconnect wait s | install s | cycle s |")
print("|---|---|---|---|---|---|---|---|")
for line in open(f"{d}/runs.jsonl"):
    r = json.loads(line)
    if not r["cell"].startswith(pref) or not r.get("valid") or "load_start" not in r:
        continue
    rd = f"{d}/{r['rep']}-{r['cell']}-{r['label']}"
    p = evs(f"{rd}/primary.stdout.txt", {"queue full": "full", "fell too far behind": "full", "Replica connected": "conn"})
    import glob
    q = []
    for f in [f"{rd}/replica.stdout.txt"] + sorted(glob.glob(f"{rd}/replica-b*.stdout.txt")):
        q += evs(f, {"sync ended": "ended", "full snapshot loaded": "installed", "partial synchronization reached": "installed"})
    ls, le = r["load_start"], r["load_end"]
    # Disconnects during the measured window only (not the warm-up), and the
    # survival of each only from an install that itself happened during the
    # load (review A round 2, N8).
    ms = r["load"]["measure_start_ms"] / 1e3
    fulls = [t for t, n in p if n == "full" and ms <= t <= le]
    surv, notice, wait, inst, cyc = [], [], [], [], []
    installs = sorted(t for t, n in q if n == "installed")
    ends = sorted(t for t, n in q if n == "ended")
    conns = sorted(t for t, n in p if n == "conn")
    for f in fulls:
        prev = [t for t in installs if ls <= t < f]
        if prev:
            surv.append(f - prev[-1])
        e = [t for t in ends if t >= f]
        if e:
            notice.append(e[0] - f)
            c = [t for t in conns if t >= e[0]]
            if c:
                wait.append(c[0] - e[0])
                i = [t for t in installs if t >= c[0]]
                if i:
                    inst.append(i[0] - c[0])
                    cyc.append(i[0] - f + (surv[-1] if prev else 0))
    m = lambda v: f"{statistics.median(v):.2f}" if v else "–"
    print(f"| {r['cell']} | {r['label']} | {len(fulls)} | {m(surv)} | {m(notice)} | {m(wait)} | {m(inst)} | {m(cyc)} |")
