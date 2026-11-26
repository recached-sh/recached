#!/usr/bin/env python3
"""Summarize an i-tx.sh log: per cell, the median over reps of each label,
and each label's ratio to the first. Columns: tx/s, server CPU µs per
transaction, p99 and max latency (µs), peak RSS (MiB); for replica cells, the
replica's CPU per transaction, lag, full syncs and queue disconnects.

usage: summarize_tx.py tx.log [LABEL ...]   (default: base i)
"""
import re, statistics, sys
from collections import defaultdict

path = sys.argv[1]
labels = sys.argv[2:] or ["base", "i"]
cells = defaultdict(lambda: defaultdict(list))
order = []
for line in open(path):
    if not line.startswith("label="):
        continue
    kv = dict(re.findall(r"(\w+)=(\S*)", line))
    cell = f"{kv['mode']} {kv.get('txenv') or ''} {kv.get('srvenv') or 'noaof'} c{kv['clients']}".replace(
        "RECACHED_AOF_SYNC=", "aof=").replace("  ", " ")
    if cell not in order:
        order.append(cell)
    tx = float(kv["tx"]) or 1.0
    num = lambda k: float(kv.get(k) or "nan")
    cells[cell][kv["label"]].append({
        "tps": float(kv["tx_per_s"]), "cpu_us": float(kv["oncpu_s"]) * 1e6 / tx,
        "p99": float(kv["p99_us"]), "max": float(kv["max_us"]), "rss": float(kv["hwm_kb"]) / 1024,
        "rcpu": num("replica_oncpu_s") * 1e6 / tx, "lag": num("repl_lag_frames"),
        "full": num("repl_syncs_full"), "qdisc": num("repl_queue_disconnects"),
        "foreign": num("foreign_cpus"),
        "aof_b": num("aof_bytes") / tx, "aof_w": num("aof_write_calls") / tx,
        "aof_wus": num("aof_write_usec") / tx, "aof_fs": num("aof_fsyncs"),
        "aof_fsus": num("aof_fsync_usec") / max(num("aof_fsyncs"), 1),
        "rhwm": num("replica_hwm_kb") / 1024,
    })


def med(rows, k):
    vals = [r[k] for r in rows if r[k] == r[k]]
    return statistics.median(vals) if vals else float("nan")


arrow = lambda cell, k, fmt: " → ".join(fmt.format(med(cells[cell][l], k)) for l in labels)
print(f"| cell | reps | tx/s {' / '.join(labels)} | ratio to {labels[0]} | CPU µs/tx | p99 µs | max µs | RSS MiB |")
print("|---|---|---|---|---|---|---|---|")
for cell in order:
    tps = [med(cells[cell][l], "tps") for l in labels]
    ratios = " / ".join(f"{t / tps[0]:.3f}" for t in tps[1:])
    reps = "/".join(str(len(cells[cell][l])) for l in labels)
    print(f"| {cell} | {reps} | {' / '.join(f'{t:,.0f}' for t in tps)} | {ratios} | "
          f"{arrow(cell, 'cpu_us', '{:.2f}')} | {arrow(cell, 'p99', '{:,.0f}')} | {arrow(cell, 'max', '{:,.0f}')} | "
          f"{arrow(cell, 'rss', '{:.0f}')} |")
repl = [c for c in order if "REPLICA" in c]
if repl:
    print()
    print("| replica cell | replica CPU µs/tx | lag frames at end | full syncs | queue disconnects |")
    print("|---|---|---|---|---|")
    for cell in repl:
        print(f"| {cell} | {arrow(cell, 'rcpu', '{:.2f}')} | {arrow(cell, 'lag', '{:.0f}')} | "
              f"{arrow(cell, 'full', '{:.0f}')} | {arrow(cell, 'qdisc', '{:.0f}')} |")
aofc = [c for c in order if any(r["aof_w"] == r["aof_w"] for l in labels for r in cells[c][l])]
if aofc:
    print()
    print("| AOF cell | AOF bytes/tx | write(2) calls/tx | µs in write(2) per tx | fsyncs per run | µs per fsync |")
    print("|---|---|---|---|---|---|")
    for cell in aofc:
        print(f"| {cell} | {arrow(cell, 'aof_b', '{:.0f}')} | {arrow(cell, 'aof_w', '{:.3f}')} | "
              f"{arrow(cell, 'aof_wus', '{:.2f}')} | {arrow(cell, 'aof_fs', '{:.0f}')} | {arrow(cell, 'aof_fsus', '{:,.0f}')} |")
rh = [c for c in order if any(r["rhwm"] == r["rhwm"] for l in labels for r in cells[c][l])]
if rh:
    print()
    print("| replica cell | replica RSS MiB |")
    print("|---|---|")
    for cell in rh:
        print(f"| {cell} | {arrow(cell, 'rhwm', '{:.0f}')} |")
print()
print(f"paired ratios per rep (each label / {labels[0]}), and the host's foreign CPUs:")
for cell in order:
    parts = []
    for l in labels[1:]:
        pairs = [f"{y['tps'] / x['tps']:.3f}" for x, y in zip(cells[cell][labels[0]], cells[cell][l])]
        cpu = [y['cpu_us'] / x['cpu_us'] for x, y in zip(cells[cell][labels[0]], cells[cell][l])]
        if not cpu:
            continue
        parts.append(f"{l}: tx/s {' '.join(pairs)} | CPU/tx median {statistics.median(cpu):.3f} "
                     f"[{min(cpu):.3f}-{max(cpu):.3f}]")
    allrows = [r for l in labels for r in cells[cell][l]]
    print(f"  {cell}: {'; '.join(parts)}; foreign CPUs median {med(allrows, 'foreign'):.2f}")
