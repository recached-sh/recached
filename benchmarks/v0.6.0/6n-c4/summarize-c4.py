"""Summarize a 6N-C4 run-m1.sh results.txt into markdown tables.

Per cell (kind, pipeline, workers), the median (and range) over interleaved
repetitions of: throughput as redis-benchmark reports it and as requests over
an independent wall clock; the load generator's CPU; the primary's CPU,
context switches and syscalls per operation; AOF batching and the coalesced
fan-out counters; latency; replica lag and convergence; RSS. Then CPU per
thread name, the ratios Gate M1 reads, and the M1 throughput verdict
(rc-coalesced / rc-standalone >= 0.85 at both P=1 and P=16).
"""
import re
import statistics
import sys
from collections import defaultdict
from pathlib import Path

THRESHOLD = 0.85


def parse(path):
    cells = defaultdict(list)
    cur = None
    for line in Path(path).read_text().splitlines():
        if line.startswith("kind="):
            kv = dict(re.findall(r'(\w+)=("[^"]*"|\S+)', line))
            cur = {k: v.strip('"') for k, v in kv.items()}
            cells[(cur["kind"], int(cur["P"]), int(cur["workers"]))].append(cur)
        elif line.startswith("  aof_before:"):
            cur["aof0"] = dict(re.findall(r"recached_aof_(\w+):(\S+)", line))
        elif line.startswith("  aof_after:"):
            cur["aof1"] = dict(re.findall(r"recached_aof_(\w+):(\S+)", line))
        elif line.startswith(("  ctxsw_", "  syscalls_")):
            name, rest = line.strip().split(":", 1)
            cur[name] = [int(x) if x.isdigit() else None for x in rest.split()]
        elif line.startswith("  threads_"):
            name, rest = line.strip().split(":", 1)
            cur[name] = {
                t.split(":")[0]: float(t.split(":")[2]) for t in rest.split() if t.count(":") == 2
            }
        elif line.startswith("  bench:"):
            cols = [c.strip('"') for c in line.split(":", 1)[1].strip().split(",")]
            cur["rps"], cur["avg"], cur["min"], cur["p50"], cur["p95"], cur["p99"], cur["max"] = (
                float(c) for c in cols[1:8]
            )
    return cells


def delta(run, key):
    try:
        return int(run["aof1"][key]) - int(run["aof0"][key])
    except (KeyError, ValueError):
        return None


def pair_delta(run, name):
    a, b = run.get(name + "_before"), run.get(name + "_after")
    if not a or not b or None in a or None in b:
        return None
    return sum(b) - sum(a)


def med(xs):
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None


def rng(xs):
    xs = [x for x in xs if x is not None]
    return f"{min(xs):,.0f}–{max(xs):,.0f}" if xs else "n/a"


def fmt(x, spec):
    return "n/a" if x is None else format(x, spec)


def per_op(x, run):
    return None if x is None else x / int(run["requests"])


def main(path):
    cells = parse(path)
    order = sorted(cells.items(), key=lambda kv: (kv[0][2], kv[0][1], kv[0][0]))
    meds, p99s = {}, {}

    print("## Throughput, CPU and generator\n")
    print("| Cell | n | requests | rps (redis-benchmark) | rps range | rps (wall clock) | run s | generator cores | CPU µs/op | ctx switches/op | syscalls/op | RSS MiB |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for (kind, pipe, workers), runs in order:
        rps = [r["rps"] for r in runs]
        wall = [int(r["requests"]) / float(r["wall_s"]) for r in runs]
        gen = [(float(r["gen_user_s"]) + float(r["gen_sys_s"])) / float(r["wall_s"]) for r in runs]
        cpu = [float(r["cpu_s"]) * 1e6 / int(r["requests"]) for r in runs]
        ctx = [per_op(pair_delta(r, "ctxsw"), r) for r in runs]
        sysc = [per_op(pair_delta(r, "syscalls"), r) for r in runs]
        meds[(kind, pipe, workers)] = med(rps)
        p99s[(kind, pipe, workers)] = med([r["p99"] for r in runs])
        print(
            f"| {kind} P={pipe} w={workers} | {len(runs)} | {int(runs[0]['requests']):,} | {med(rps):,.0f} | {rng(rps)} | "
            f"{med(wall):,.0f} | {med([float(r['wall_s']) for r in runs]):.1f} | {med(gen):.2f} | {fmt(med(cpu), '.2f')} | "
            f"{fmt(med(ctx), '.3f')} | {fmt(med(sysc), '.2f')} | {med([int(r['rss_kb']) for r in runs]) / 1024:.0f} |"
        )

    print("\n## AOF batching and coalescing (Recached cells with an AOF)\n")
    print("| Cell | AOF write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | gate yields/cmd | gate gained share |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
    for (kind, pipe, workers), runs in order:
        if not kind.startswith("rc-") or not any(delta(r, "batches") for r in runs):
            continue
        writes = [per_op(delta(r, "write_calls"), r) for r in runs]
        fpb = [delta(r, "frames") / delta(r, "batches") if delta(r, "batches") else None for r in runs]
        fg = [delta(r, "fanout_coalesce_gained") for r in runs]
        fm = [delta(r, "fanout_coalesce_missed") for r in runs]
        fy = [per_op(g + m, r) if g is not None and m is not None else None for g, m, r in zip(fg, fm, runs)]
        fshare = [g / (g + m) if g is not None and m is not None and g + m else None for g, m in zip(fg, fm)]
        gg = [delta(r, "coalesce_gained") for r in runs]
        gm = [delta(r, "coalesce_missed") for r in runs]
        gy = [per_op(g + m, r) if g is not None and m is not None else None for g, m, r in zip(gg, gm, runs)]
        gshare = [g / (g + m) if g is not None and m is not None and g + m else None for g, m in zip(gg, gm)]
        print(
            f"| {kind} P={pipe} w={workers} | {fmt(med(writes), '.4f')} | {fmt(med(fpb), '.1f')} | "
            f"{fmt(med(fy), '.3f')} | {fmt(med(fshare), '.3f')} | {fmt(med([delta(r, 'fanout_coalesce_backoffs') for r in runs]), ',.0f')} | "
            f"{fmt(med([per_op(delta(r, 'fanout_found_written'), r) for r in runs]), '.3f')} | "
            f"{fmt(med(gy), '.3f')} | {fmt(med(gshare), '.3f')} |"
        )

    print("\n## Latency, replica lag, convergence\n")
    print("| Cell | p50 ms | p95 ms | p99 ms | lag p50 / p99 ms | converge | sample | replica RSS MiB |")
    print("|---|---:|---:|---:|---|---|---|---:|")
    for (kind, pipe, workers), runs in order:
        lags = [r["lag_p50_p99_max_ms"].split() for r in runs if r["lag_p50_p99_max_ms"] != "n/a"]
        lag = (
            f"{fmt(med([float(l[0]) for l in lags if l[0] != 'timeout']), '.2f')} / "
            f"{fmt(med([float(l[1]) for l in lags if l[1] != 'timeout']), '.2f')}"
            if lags
            else "n/a"
        )
        conv = ",".join(sorted({r["converge"].split()[-1] for r in runs})) if runs[0]["converge"] != "n/a" else "n/a"
        samples = ",".join(sorted({r["sample"] for r in runs}))
        rrss = [int(r["replica_rss_kb"]) / 1024 for r in runs if r["replica_rss_kb"].isdigit()]
        print(
            f"| {kind} P={pipe} w={workers} | {med([r['p50'] for r in runs]):.3f} | {med([r['p95'] for r in runs]):.3f} | "
            f"{med([r['p99'] for r in runs]):.3f} | {lag} | {conv} | {samples} | {fmt(med(rrss), '.0f')} |"
        )

    print("\n## CPU by thread name (µs/op, median)\n")
    names = sorted({n for _, runs in order for r in runs for n in r.get("threads_after", {})})
    print("| Cell | " + " | ".join(names) + " |")
    print("|---|" + "---:|" * len(names))
    for (kind, pipe, workers), runs in order:
        vals = []
        for n in names:
            xs = [
                (r.get("threads_after", {}).get(n, 0) - r.get("threads_before", {}).get(n, 0)) * 1e6 / int(r["requests"])
                for r in runs
                if "threads_after" in r
            ]
            vals.append(fmt(med(xs), ".2f"))
        print(f"| {kind} P={pipe} w={workers} | " + " | ".join(vals) + " |")

    def ratio(a, b, p, w=1):
        num, den = meds.get((a, p, w)), meds.get((b, p, w))
        return num / den if num and den else None

    print("\n## Ratios (redis-benchmark rps medians)\n")
    print("| Ratio | P=1 | P=16 |")
    print("|---|---:|---:|")
    for a, b in [
        ("rc-coalesced", "rc-standalone"),
        ("rc-inline", "rc-standalone"),
        ("rc-coalesced", "rc-inline"),
        ("rc-coalesced", "redis"),
        ("rc-coalesced", "valkey"),
        ("rc-standalone", "redis"),
    ]:
        print(f"| {a} / {b} | {fmt(ratio(a, b, 1), '.3f')} | {fmt(ratio(a, b, 16), '.3f')} |")
    print(f"| rc-coalesced / rc-inline, 4 workers | {fmt(ratio('rc-coalesced', 'rc-inline', 1, 4), '.3f')} | — |")
    for p in (1, 16):
        a, b = p99s.get(("rc-coalesced", p, 1)), p99s.get(("rc-inline", p, 1))
        if a and b:
            print(f"\np99 at P={p}: coalesced {a:.3f} ms vs inline {b:.3f} ms ({(a / b - 1) * 100:+.1f}%)")

    r1, r16 = ratio("rc-coalesced", "rc-standalone", 1), ratio("rc-coalesced", "rc-standalone", 16)
    print()
    if r1 is None or r16 is None:
        print("**M1 throughput: NOT VERIFIED** (a required cell is missing)")
    elif r1 >= THRESHOLD and r16 >= THRESHOLD:
        print(f"**M1 throughput: PASS** (P=1 {r1:.3f}, P=16 {r16:.3f} ≥ {THRESHOLD})")
    else:
        print(f"**M1 throughput: below threshold** (P=1 {r1:.3f}, P=16 {r16:.3f}; threshold {THRESHOLD})")


if __name__ == "__main__":
    main(sys.argv[1])
