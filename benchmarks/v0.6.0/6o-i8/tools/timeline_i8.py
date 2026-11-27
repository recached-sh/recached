#!/usr/bin/env python3
"""6O-I8: the replication timeline of each run of a cell.

    timeline_i8.py RUNS_DIR CELL [LABEL]

Every replication event from both servers' logs (queue-full disconnects,
the replica noticing the link end, reconnects, full-sync installs and
checkpoints, partial syncs), in seconds since the load started, with the
primary's lag and queue depth from the 1 Hz series at that moment.
"""
import datetime
import gzip
import json
import os
import re
import sys

ANSI = re.compile(r"\x1b\[[0-9;]*m")
PAT = re.compile(r"^(\S+Z)\s+(\w+)\s+\S+:\s+(.*)$")
KEYS = {"byte queue full": "P queue full (bytes)", "fell too far behind": "P queue full (frames)",
        "disconnected:": "P replica unregistered", "Replica connected": "P replica connected",
        "sync ended": "R link ended", "connecting to primary": "R connecting",
        "Snapshot saved from a full": "R checkpoint written", "full snapshot loaded": "R full sync installed",
        "partial synchronization reached": "R partial sync"}

d, cell = sys.argv[1], sys.argv[2]
only = sys.argv[3] if len(sys.argv) > 3 else None
for line in open(f"{d}/runs.jsonl"):
    r = json.loads(line)
    if r["cell"] != cell or (only and r["label"] != only) or "load_start" not in r:
        continue
    rd = f"{d}/{r['rep']}-{r['cell']}-{r['label']}"
    t0 = r["load_start"]
    ms = r["load"]["measure_start_ms"] / 1e3 - t0 if r.get("load") else None
    me = r["load"]["measure_end_ms"] / 1e3 - t0 if r.get("load") else None
    sp = f"{rd}/series.jsonl"
    rows = [json.loads(x) for x in (open(sp) if os.path.exists(sp) else gzip.open(sp + ".gz", "rt"))]
    ev = []
    import glob
    for path in [f"{rd}/primary.stdout.txt", f"{rd}/replica.stdout.txt"] + sorted(glob.glob(f"{rd}/replica-b*.stdout.txt")):
        if not os.path.exists(path):
            continue
        for raw in open(path, errors="replace"):
            m = PAT.match(ANSI.sub("", raw).rstrip())
            if not m:
                continue
            for k, label in KEYS.items():
                if k in m.group(3):
                    t = datetime.datetime.fromisoformat(m.group(1).replace("Z", "+00:00")).timestamp() - t0
                    extra = m.group(3)[m.group(3).find(":") + 1:].strip()[:70] if label == "R link ended" else ""
                    ev.append((t, label, extra))
    ev.sort()
    print(f"\n== rep {r['rep']} {r['label']}  measured {ms:.0f}–{me:.0f} s; load ends {r['load_end'] - t0:.0f} s; "
          f"catch-up {r.get('catchup_s')} s; valid={r['valid']} {r['why']}")
    for t, label, extra in ev:
        if t < -5:
            continue
        near = min(rows, key=lambda x: abs(x["t"] - t0 - t))
        info = near.get("primary", {}).get("info", {}) if abs(near["t"] - t0 - t) <= 1.5 else {}
        print(f"  {t:8.2f}  {label:24s} lag={info.get('recached_replication_lag_frames')} "
              f"qdepth={info.get('recached_replication_queue_depth')} {extra}")
