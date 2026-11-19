"""Summarize a run-m1.sh results.txt into the Gate M1 tables (markdown).

Per cell (kind, pipeline, workers): the median and range of each metric over
the interleaved repetitions. Then the M1 ratios: rc-coalesced / rc-standalone
at P=1 and P=16, each against the 0.85 threshold of the 6N-B design decision.
"""
import re
import statistics
import sys
from collections import defaultdict
from pathlib import Path

THRESHOLD = 0.85


def parse(path):
    env = Path(path).with_name("environment.txt").read_text()
    requests = int(re.search(r"requests=(\d+)", env).group(1))
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
        elif line.startswith("  bench:"):
            cols = [c.strip('"') for c in line.split(":", 1)[1].strip().split(",")]
            cur["rps"], cur["avg"], cur["min"], cur["p50"], cur["p95"], cur["p99"], cur["max"] = (
                float(c) for c in cols[1:8]
            )
    return requests, cells


def delta(run, key):
    try:
        return int(run["aof1"][key]) - int(run["aof0"][key])
    except (KeyError, ValueError):
        return None


def med(xs):
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None


def rng(xs):
    xs = [x for x in xs if x is not None]
    return f"{min(xs):,.0f}–{max(xs):,.0f}" if xs else "n/a"


def fmt(x, spec):
    return "n/a" if x is None else format(x, spec)


def main(path):
    requests, cells = parse(path)
    print("| Cell | n | rps median | rps range | CPU µs/op | AOF write(2)/cmd | cmds/batch | p50 ms | p95 ms | p99 ms | lag p50 / p99 ms | converge | RSS MiB | errors |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|---:|---|")
    meds = {}
    for (kind, pipe, workers), runs in sorted(cells.items(), key=lambda kv: (kv[0][2], kv[0][1], kv[0][0])):
        rps = [r["rps"] for r in runs]
        cpu = [float(r["cpu_s"]) * 1e6 / requests for r in runs]
        writes = [(delta(r, "write_calls") or 0) / requests if delta(r, "write_calls") is not None else None for r in runs]
        per_batch = [
            delta(r, "frames") / delta(r, "batches") if delta(r, "batches") else None for r in runs
        ]
        lags = [r["lag_p50_p99_max_ms"].split() for r in runs if r["lag_p50_p99_max_ms"] != "n/a"]
        lag = (
            f"{med([float(l[0]) for l in lags if l[0] != 'timeout']):.2f} / "
            f"{fmt(med([float(l[1]) for l in lags if l[1] != 'timeout']), '.2f')}"
            if lags
            else "n/a"
        )
        conv = ",".join(sorted({r["converge"].split()[-1] for r in runs})) if runs[0]["converge"] != "n/a" else "n/a"
        samples = {r["sample"] for r in runs}
        errors = "none" if samples <= {"ok", "n/a"} else ",".join(sorted(samples))
        m = meds[(kind, pipe, workers)] = med(rps)
        print(
            f"| {kind} P={pipe} w={workers} | {len(runs)} | {m:,.0f} | {rng(rps)} | {fmt(med(cpu), '.2f')} | "
            f"{fmt(med(writes), '.3f')} | {fmt(med(per_batch), '.1f')} | {med([r['p50'] for r in runs]):.3f} | "
            f"{med([r['p95'] for r in runs]):.3f} | {med([r['p99'] for r in runs]):.3f} | {lag} | {conv} | "
            f"{med([int(r['rss_kb']) for r in runs]) / 1024:.0f} | {errors} |"
        )
    print()
    print("| Ratio (medians) | P=1 | P=16 |")
    print("|---|---:|---:|")
    def ratio(a, b, p):
        num, den = meds.get((a, p, 1)), meds.get((b, p, 1))
        return num / den if num and den else None

    for a, b in [
        ("rc-coalesced", "rc-standalone"),
        ("rc-inline", "rc-standalone"),
        ("rc-coalesced", "rc-inline"),
        ("rc-coalesced", "redis"),
        ("rc-coalesced", "valkey"),
    ]:
        print(f"| {a} / {b} | {fmt(ratio(a, b, 1), '.3f')} | {fmt(ratio(a, b, 16), '.3f')} |")
    print()
    r1, r16 = ratio("rc-coalesced", "rc-standalone", 1), ratio("rc-coalesced", "rc-standalone", 16)
    if r1 is None or r16 is None:
        print("**M1: NOT VERIFIED** (a required cell is missing)")
    elif r1 >= THRESHOLD and r16 >= THRESHOLD:
        print(f"**M1 throughput: PASS** (P=1 {r1:.3f}, P=16 {r16:.3f} ≥ {THRESHOLD})")
    else:
        print(f"**M1 throughput: FAIL** (P=1 {r1:.3f}, P=16 {r16:.3f}; threshold {THRESHOLD})")


if __name__ == "__main__":
    main(sys.argv[1])
