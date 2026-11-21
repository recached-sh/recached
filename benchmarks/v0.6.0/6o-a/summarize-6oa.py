#!/usr/bin/env python3
"""Summarize a 6O-A run-6oa.sh results.txt into markdown tables.

    summarize-6oa.py results.txt [--ratio A:B ...] [--csv out.csv]

Per cell (as named in CELLS): the median and range over interleaved
repetitions of throughput; the server's CPU per operation split into user and
system time; user-space instructions and cycles per operation (when the run
had UPROF); latency p50/p95/p99; AOF write(2) calls per command, frames per
batch and write time per call (Recached); RSS; the generator's CPU (to show it
was not the bottleneck); host pressure at each repetition's start.

Ratios: --ratio A:B prints median(rps A) / median(rps B) and the
per-repetition ratios (rep i of A over rep i of B, interleaved neighbours).
With no --ratio, every Recached rc-everysec cell is compared with the Redis
and Valkey cell of the same test and pipeline depth.
"""
import argparse
import re
import statistics
from collections import OrderedDict
from pathlib import Path


def parse(path):
    cells = OrderedDict()
    cur = None
    for line in Path(path).read_text().splitlines():
        if line.startswith("cell="):
            kv = dict(re.findall(r'(\w+)=("[^"]*"|\S+)', line))
            cur = {k: v.strip('"') for k, v in kv.items()}
            cells.setdefault(cur["cell"], []).append(cur)
        elif cur is None:
            continue
        elif line.startswith(("  aof_before:", "  aof_after:")):
            cur["aof" + ("0" if "before" in line else "1")] = dict(re.findall(r"recached_aof_(\w+):(\S+)", line))
        elif line.startswith("  pressure_before:"):
            cur["pressure"] = line.split(":", 1)[1].strip()
        elif line.startswith(("  ctxsw_", "  syscalls_")):
            name, rest = line.strip().split(":", 1)
            cur[name] = [int(x) if x.isdigit() else None for x in rest.split()]
        elif line.startswith("  uprof:"):
            cur["uprof"] = {k: float(v) for k, v in re.findall(r"(\w+)=([0-9.]+) opened", line)}
        elif line.startswith("  bench_err:"):
            cur["bench_err"] = int(line.split(":", 1)[1].strip() or 0)
        elif line.startswith("  bench:"):
            cols = [c.strip('"') for c in line.split(":", 1)[1].strip().split(",")]
            try:
                cur["rps"] = float(cols[1])
                cur["p50"], cur["p95"], cur["p99"] = float(cols[4]), float(cols[5]), float(cols[6])
            except (IndexError, ValueError):
                cur["rps"] = None
    return cells


def med(xs):
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None


def spread(xs):
    xs = [x for x in xs if x is not None]
    if len(xs) < 2 or not statistics.median(xs):
        return None
    return (max(xs) - min(xs)) / statistics.median(xs)


def fmt(x, nd=0, pct=False):
    if x is None:
        return "—"
    if pct:
        return f"{x * 100:.0f}%"
    return f"{x:,.{nd}f}"


def aof_delta(r, key):
    a0, a1 = r.get("aof0") or {}, r.get("aof1") or {}
    if key not in a1:
        return None
    try:
        return float(a1[key]) - float(a0.get(key, 0))
    except ValueError:
        return None


def per_rep(r):
    n = float(r["requests"])
    out = {
        "rps": r.get("rps"),
        "cpu": (float(r["cpu_user_s"]) + float(r["cpu_sys_s"])) * 1e6 / n,
        "user": float(r["cpu_user_s"]) * 1e6 / n,
        "sys": float(r["cpu_sys_s"]) * 1e6 / n,
        "p50": r.get("p50"),
        "p95": r.get("p95"),
        "p99": r.get("p99"),
        "rss": float(r["rss_kb"]) / 1024 if r.get("rss_kb", "").isdigit() else None,
        "gen": (float(r["gen_user_s"]) + float(r["gen_sys_s"])) / float(r["wall_s"]) if float(r["wall_s"]) else None,
        "err": r.get("bench_err", 0),
    }
    u = r.get("uprof")
    out["instr"] = u["instructions"] / n if u and "instructions" in u else None
    out["cycles"] = u["cycles"] / n if u and "cycles" in u else None
    w = aof_delta(r, "write_calls")
    f = aof_delta(r, "frames")
    b = aof_delta(r, "batches")
    wu = aof_delta(r, "write_usec")
    out["writes_per_cmd"] = w / n if w is not None else None
    out["frames_per_batch"] = f / b if f is not None and b else None
    out["write_us"] = wu / w if wu is not None and w else None
    x0, x1 = r.get("ctxsw_before"), r.get("ctxsw_after")
    out["ctxsw"] = (sum(x1) - sum(x0)) / n if x0 and x1 and None not in x0 + x1 else None
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("results")
    ap.add_argument("--ratio", action="append", default=[])
    ap.add_argument("--csv")
    args = ap.parse_args()
    cells = parse(args.results)
    reps = {c: [per_rep(r) for r in rs] for c, rs in cells.items()}

    print("## Throughput, CPU and latency (median of repetitions; spread = (max - min) / median)\n")
    print("| cell | n | ops/s | spread | CPU µs/op | user | sys | instr/op (user) | cycles/op (user) | p50 ms | p95 ms | p99 ms | RSS MiB | gen CPUs | errors |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for c, rs in reps.items():
        g = lambda k: [r[k] for r in rs]
        print(f"| `{c}` | {len(rs)} | {fmt(med(g('rps')))} | {fmt(spread(g('rps')), pct=True)} | {fmt(med(g('cpu')), 2)} | {fmt(med(g('user')), 2)} | {fmt(med(g('sys')), 2)} | {fmt(med(g('instr')))} | {fmt(med(g('cycles')))} | {fmt(med(g('p50')), 3)} | {fmt(med(g('p95')), 3)} | {fmt(med(g('p99')), 3)} | {fmt(med(g('rss')), 1)} | {fmt(med(g('gen')), 2)} | {sum(g('err'))} |")

    aofcells = [c for c, rs in reps.items() if any(r["writes_per_cmd"] is not None for r in rs)]
    if aofcells:
        print("\n## AOF (Recached INFO persistence deltas over the timed run)\n")
        print("| cell | write(2) / cmd | frames / batch | µs per write(2) | ctx switches / op |")
        print("|---|---:|---:|---:|---:|")
        for c in aofcells:
            rs = reps[c]
            g = lambda k: [r[k] for r in rs]
            print(f"| `{c}` | {fmt(med(g('writes_per_cmd')), 4)} | {fmt(med(g('frames_per_batch')), 1)} | {fmt(med(g('write_us')), 1)} | {fmt(med(g('ctxsw')), 4)} |")

    ratios = list(args.ratio)
    if not ratios:
        for c in reps:
            m = re.match(r"^(.+)/rc-everysec-(\w+)-P(\d+)$", c)
            if m:
                for ref in (f"redis-{m.group(2)}-P{m.group(3)}", f"valkey-{m.group(2)}-P{m.group(3)}"):
                    if ref in reps:
                        ratios.append(f"{c}:{ref}")
    if ratios:
        print("\n## Ratios (throughput)\n")
        print("| A / B | median ratio | per-repetition ratios |")
        print("|---|---:|---|")
        for spec in ratios:
            a, b = spec.split(":")
            if a not in reps or b not in reps:
                print(f"| `{a}` / `{b}` | missing | |")
                continue
            ma, mb = med([r["rps"] for r in reps[a]]), med([r["rps"] for r in reps[b]])
            pr = [ra["rps"] / rb["rps"] for ra, rb in zip(reps[a], reps[b]) if ra["rps"] and rb["rps"]]
            print(f"| `{a}` / `{b}` | **{ma / mb:.3f}** | {' '.join(f'{x:.3f}' for x in pr)} |")

    print("\n## Host pressure at each repetition's start\n")
    print("| cell | rep | pressure |")
    print("|---|---:|---|")
    for c, rs in cells.items():
        for r in rs:
            print(f"| `{c}` | {r['rep']} | {r.get('pressure', '—')} |")

    if args.csv:
        keys = ["rps", "cpu", "user", "sys", "instr", "cycles", "p50", "p95", "p99", "rss", "gen", "writes_per_cmd", "frames_per_batch", "write_us", "ctxsw", "err"]
        with open(args.csv, "w") as f:
            f.write("cell,rep," + ",".join(keys) + "\n")
            for c, rs in reps.items():
                for i, r in enumerate(rs, 1):
                    f.write(f"{c},{i}," + ",".join("" if r[k] is None else f"{r[k]}" for k in keys) + "\n")


if __name__ == "__main__":
    main()
