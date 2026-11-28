#!/usr/bin/env python3
"""6O-I8: resynchronizations forced under load (r5, review C F4).

    bounces_i8.py RUNS_DIR [SS_LOG]

For each run and each bounce (the replica killed and restarted on its own
files during the load): the restart's time to PONG, the time until its full
synchronization was installed, and whether the replica then stayed attached
(no queue-full disconnect) until the next bounce or the load's end. With
SS_LOG (`ssmon.sh`'s once-a-second `ss -tmi`), the new link's receive buffer
at its start and 30 s later.
"""
import datetime
import glob
import json
import re
import sys

ANSI = re.compile(r"\x1b\[[0-9;]*m")


def ts(line):
    return datetime.datetime.fromisoformat(line.split()[0].replace("Z", "+00:00")).timestamp()


def events(path, key):
    out = []
    try:
        for raw in open(path, errors="replace"):
            s = ANSI.sub("", raw)
            if key in s:
                out.append(ts(s))
    except FileNotFoundError:
        pass
    return out


ss = {}
if len(sys.argv) > 2:
    t = None
    lines = open(sys.argv[2]).read().splitlines()
    for i, line in enumerate(lines):
        if line.startswith("T "):
            t = float(line.split()[1])
            continue
        m = re.match(r"\S+\s+\S+\s+127\.0\.0\.1:(\d+)\s+127\.0\.0\.1:(\d+)\b", line)
        if m and i + 1 < len(lines):
            rb = re.search(r"rb(\d+)", lines[i + 1])
            if rb:
                ss.setdefault((m.group(2), m.group(1)), []).append((t, int(rb.group(1))))

d = sys.argv[1]
print("| run | bounce at s | PONG s | full sync installed after s | disconnects until the next bounce / the end | new link's receive buffer: at start → 30 s later |")
print("|---|---|---|---|---|---|")
for line in open(f"{d}/runs.jsonl"):
    r = json.loads(line)
    rd = f"{d}/{r['rep']}-{r['cell']}-{r['label']}"
    ls, le = r["load_start"], r["load_end"]
    full = events(f"{rd}/primary.stdout.txt", "queue full")
    installs = []
    for f in sorted(glob.glob(f"{rd}/replica*.stdout.txt")):
        if "restart" in f:
            continue
        installs += events(f, "full snapshot loaded")
    installs.sort()
    bs = r.get("bounces", [])
    before = [round(t - ls) for t in full if ls <= t < (bs[0]["killed"] if bs else le)]
    print(f"| {r['cell']} {r['label']} | before the first bounce | | | {len(before)} {before if before else ''} | |")
    repl_port = str(r["port"] + 2)
    for i, b in enumerate(bs):
        end = bs[i + 1]["killed"] if i + 1 < len(bs) else le
        inst = [t for t in installs if t > b["killed"]]
        after = [round(t - ls) for t in full if b["killed"] < t < end]
        links = [v for (peer, _), v in ss.items() if peer == repl_port and v and b["killed"] < v[0][0] < end]
        rb = ""
        if links:
            v = links[0]
            later = [x for x in v if x[0] - v[0][0] >= 30]
            rb = f"{v[0][1] / 1e6:.2f} MB → {later[0][1] / 1e6:.2f} MB" if later else f"{v[0][1] / 1e6:.2f} MB → (closed)"
        print(f"| {r['cell']} {r['label']} | {b['at']} | {b['pong_s']} | {round(inst[0] - b['killed'], 1) if inst else '–'} | "
              f"{len(after)} {after if after else ''} | {rb} |")
    checks = {k[7:]: (r.get(k) or {}).get("ok") for k in ("verify_primary", "verify_replica", "verify_restart")}
    print(f"| {r['cell']} {r['label']} | after the load | restart {r.get('restart_s')} s | | checks {checks} | |")
