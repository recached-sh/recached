"""Summarize a 6N-C5 run-c5.sh results.txt into markdown tables.

    summarize-c5.py results.txt [--ratio A:B ...]

Per cell (as named in CELLS), the median and range over interleaved
repetitions of: throughput; the primary's CPU per operation, split into user
and system time; user-space instructions, cycles, IPC, branch and cache misses
per operation (when the run had UPROF); the replication socket's data segments
per operation in each direction (≈ sender flushes and replica
acknowledgements); latency; replica lag, CPU and convergence; AOF batching and
coalescing counters; RSS. Then CPU per thread name and the requested ratios.
With no --ratio, every <label>/rc-coalesced-P<n> is compared with every
<label>/rc-standalone-P<n>, which is the M1 ratio (threshold 0.85).
"""
import argparse
import re
import statistics
from collections import OrderedDict
from pathlib import Path

THRESHOLD = 0.85


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
        elif line.startswith(("  repl_before:", "  repl_after:")):
            cur["repl" + ("0" if "before" in line else "1")] = dict(re.findall(r"(\w+):(\S+)", line.split(":", 1)[1]))
        elif line.startswith(("  ss_before:", "  ss_after:")):
            rest = line.split(":", 1)[1]
            cur["ss" + ("0" if "before" in line else "1")] = {
                k: int(v) for k, v in re.findall(r"\b(data_segs_out|data_segs_in|segs_out|segs_in|bytes_sent|bytes_received):(\d+)", rest)
            }
        elif line.startswith(("  replica_cpu_before:", "  replica_cpu_after:")):
            xs = line.split(":", 1)[1].split()
            cur["rcpu" + ("0" if "before" in line else "1")] = [float(x) for x in xs] if len(xs) == 2 else None
        elif line.startswith(("  ctxsw_", "  syscalls_")):
            name, rest = line.strip().split(":", 1)
            cur[name] = [int(x) if x.isdigit() else None for x in rest.split()]
        elif line.startswith("  threads_"):
            name, rest = line.strip().split(":", 1)
            cur[name] = {
                t.split(":")[0]: (float(t.split(":")[2]), float(t.split(":")[3]))
                for t in rest.split()
                if t.count(":") == 3
            }
        elif line.startswith("  uprof:"):
            cur["uprof"] = {k: float(v) for k, v in re.findall(r"(\w+)=([0-9.]+) opened", line)}
        elif line.startswith("  bench:"):
            cols = [c.strip('"') for c in line.split(":", 1)[1].strip().split(",")]
            try:
                cur["rps"], cur["avg"], cur["min"], cur["p50"], cur["p95"], cur["p99"], cur["max"] = (
                    float(c) for c in cols[1:8]
                )
            except ValueError:
                cur["rps"] = None
    return cells


def med(xs):
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None


def rng(xs, spec=",.0f"):
    xs = [x for x in xs if x is not None]
    return f"{format(min(xs), spec)}–{format(max(xs), spec)}" if xs else "n/a"


def fmt(x, spec):
    return "n/a" if x is None else format(x, spec)


def req(r):
    return int(r["requests"])


def d_aof(r, key):
    try:
        return int(r["aof1"][key]) - int(r["aof0"][key])
    except (KeyError, ValueError):
        return None


def d_ss(r, key):
    a, b = r.get("ss0") or {}, r.get("ss1") or {}
    if key not in a or key not in b:
        return None
    return b[key] - a[key]


def pair_delta(r, name):
    a, b = r.get(name + "_before"), r.get(name + "_after")
    if not a or not b or None in a or None in b:
        return None
    return sum(b) - sum(a)


def up(r, key):
    u = r.get("uprof")
    return None if not u or key not in u else u[key] / req(r)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("results")
    ap.add_argument("--ratio", action="append", default=[])
    a = ap.parse_args()
    cells = parse(a.results)
    meds = {}

    print("## Throughput and CPU\n")
    print("| Cell | n | requests | rps | rps range | rps (wall) | run s | gen cores | CPU µs/op | user µs/op | sys µs/op | ctx sw/op | RSS MiB |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for cell, runs in cells.items():
        rps = [r.get("rps") for r in runs]
        meds[cell] = med(rps)
        gen = [(float(r["gen_user_s"]) + float(r["gen_sys_s"])) / float(r["wall_s"]) for r in runs]
        print(
            f"| {cell} | {len(runs)} | {req(runs[0]):,} | {fmt(med(rps), ',.0f')} | {rng(rps)} | "
            f"{med([req(r) / float(r['wall_s']) for r in runs]):,.0f} | {med([float(r['wall_s']) for r in runs]):.1f} | {med(gen):.2f} | "
            f"{med([float(r['cpu_s']) * 1e6 / req(r) for r in runs]):.2f} | "
            f"{med([float(r['cpu_user_s']) * 1e6 / req(r) for r in runs]):.2f} | "
            f"{med([float(r['cpu_sys_s']) * 1e6 / req(r) for r in runs]):.2f} | "
            f"{fmt(med([None if pair_delta(r, 'ctxsw') is None else pair_delta(r, 'ctxsw') / req(r) for r in runs]), '.4f')} | "
            f"{med([int(r['rss_kb']) for r in runs]) / 1024:.0f} |"
        )

    if any("uprof" in r for runs in cells.values() for r in runs):
        print("\n## User-space hardware counters per operation (uprof; kernel excluded)\n")
        print("| Cell | instructions/op | range | cycles/op | IPC | branch misses/op | cache misses/op | L1d read misses/op |")
        print("|---|---:|---:|---:|---:|---:|---:|---:|")
        for cell, runs in cells.items():
            if not any("uprof" in r for r in runs):
                continue
            ins = [up(r, "instructions") for r in runs]
            cyc = [up(r, "cycles") for r in runs]
            ipc = [i / c if i and c else None for i, c in zip(ins, cyc)]
            print(
                f"| {cell} | {fmt(med(ins), ',.0f')} | {rng(ins)} | {fmt(med(cyc), ',.0f')} | {fmt(med(ipc), '.2f')} | "
                f"{fmt(med([up(r, 'branch_misses') for r in runs]), '.2f')} | {fmt(med([up(r, 'cache_misses') for r in runs]), '.2f')} | "
                f"{fmt(med([up(r, 'l1d_read_misses') for r in runs]), '.2f')} |"
            )

    print("\n## Replication socket and replica\n")
    print("| Cell | frames/segment out | data segs out/op | data segs in/op (acks) | bytes out/op | replica CPU µs/op | lag p50 / p99 ms | converge | sample | replica RSS MiB |")
    print("|---|---:|---:|---:|---:|---:|---|---|---|---:|")
    for cell, runs in cells.items():
        if all(r["converge"] == "n/a" for r in runs):
            continue
        out = [d_ss(r, "data_segs_out") for r in runs]
        fps = [req(r) / o if o else None for r, o in zip(runs, out)]
        rcpu = [
            (sum(r["rcpu1"]) - sum(r["rcpu0"])) * 1e6 / req(r) if r.get("rcpu0") and r.get("rcpu1") else None
            for r in runs
        ]
        lags = [r["lag_p50_p99_max_ms"].split() for r in runs if r["lag_p50_p99_max_ms"] != "n/a"]
        lag = (
            f"{fmt(med([float(l[0]) for l in lags if l[0] != 'timeout']), '.2f')} / "
            f"{fmt(med([float(l[1]) for l in lags if l[1] != 'timeout']), '.2f')}"
            if lags else "n/a"
        )
        conv = ",".join(sorted({r["converge"].split()[-1] for r in runs}))
        samples = ",".join(sorted({r["sample"] for r in runs}))
        rrss = [int(r["replica_rss_kb"]) / 1024 for r in runs if r["replica_rss_kb"].isdigit()]
        print(
            f"| {cell} | {fmt(med(fps), '.2f')} | {fmt(med([o / req(r) if o is not None else None for r, o in zip(runs, out)]), '.3f')} | "
            f"{fmt(med([d_ss(r, 'data_segs_in') / req(r) if d_ss(r, 'data_segs_in') is not None else None for r in runs]), '.3f')} | "
            f"{fmt(med([d_ss(r, 'bytes_sent') / req(r) if d_ss(r, 'bytes_sent') is not None else None for r in runs]), '.1f')} | "
            f"{fmt(med(rcpu), '.2f')} | {lag} | {conv} | {samples} | {fmt(med(rrss), '.0f')} |"
        )

    print("\n## Latency (ms)\n")
    print("| Cell | p50 | p95 | p99 | p99 range | max |")
    print("|---|---:|---:|---:|---:|---:|")
    for cell, runs in cells.items():
        print(
            f"| {cell} | {fmt(med([r.get('p50') for r in runs]), '.3f')} | {fmt(med([r.get('p95') for r in runs]), '.3f')} | "
            f"{fmt(med([r.get('p99') for r in runs]), '.3f')} | {rng([r.get('p99') for r in runs], '.3f')} | "
            f"{fmt(med([r.get('max') for r in runs]), '.3f')} |"
        )

    aof_cells = [(c, rs) for c, rs in cells.items() if any(d_aof(r, "batches") for r in rs)]
    if aof_cells:
        print("\n## AOF batching and coalescing\n")
        extra = sorted({k for _, rs in aof_cells for r in rs for k in (r.get("aof1") or {}) if k.startswith("fanout_") and k not in (
            "fanout_coalesce_gained", "fanout_coalesce_missed", "fanout_coalesce_backoffs", "fanout_found_written")})
        print("| Cell | write(2)/cmd | frames/batch | fan-out yields/cmd | gained share | back-offs | found written/cmd | lock contended/cmd | lead busy/cmd | "
              + " | ".join(f"{k}/cmd" for k in extra) + (" |" if extra else ""))
        print("|---|" + "---:|" * (8 + len(extra)))
        for cell, runs in aof_cells:
            def per(key):
                return med([d_aof(r, key) / req(r) if d_aof(r, key) is not None else None for r in runs])
            fpb = [d_aof(r, "frames") / d_aof(r, "batches") if d_aof(r, "batches") else None for r in runs]
            g = [d_aof(r, "fanout_coalesce_gained") for r in runs]
            m = [d_aof(r, "fanout_coalesce_missed") for r in runs]
            fy = [(x + y) / req(r) if x is not None and y is not None else None for x, y, r in zip(g, m, runs)]
            share = [x / (x + y) if x is not None and y is not None and x + y else None for x, y in zip(g, m)]
            print(
                f"| {cell} | {fmt(per('write_calls'), '.4f')} | {fmt(med(fpb), '.1f')} | {fmt(med(fy), '.3f')} | {fmt(med(share), '.3f')} | "
                f"{fmt(med([d_aof(r, 'fanout_coalesce_backoffs') for r in runs]), ',.0f')} | {fmt(per('fanout_found_written'), '.3f')} | "
                f"{fmt(per('lock_contended'), '.4f')} | {fmt(per('lead_busy'), '.4f')} | "
                + " | ".join(fmt(per(k), ".4f") for k in extra) + (" |" if extra else "")
            )

    print("\n## CPU by thread name (user + sys µs/op, median)\n")
    names = sorted({n for runs in cells.values() for r in runs for n in r.get("threads_after", {})})
    print("| Cell | " + " | ".join(names) + " |")
    print("|---|" + "---:|" * len(names))
    for cell, runs in cells.items():
        vals = []
        for n in names:
            us, ss = [], []
            for r in runs:
                if "threads_after" not in r:
                    continue
                b, e = r["threads_before"].get(n, (0, 0)), r["threads_after"].get(n, (0, 0))
                us.append((e[0] - b[0]) * 1e6 / req(r))
                ss.append((e[1] - b[1]) * 1e6 / req(r))
            vals.append(f"{fmt(med(us), '.2f')} + {fmt(med(ss), '.2f')}")
        print(f"| {cell} | " + " | ".join(vals) + " |")

    ratios = [tuple(x.split(":", 1)) for x in a.ratio]
    if not ratios:
        for c in cells:
            mm = re.match(r"(?:(.+)/)?rc-coalesced-P(\d+)$", c)
            if mm:
                for s in cells:
                    if re.match(rf"(?:.+/)?rc-standalone-P{mm.group(2)}$", s):
                        ratios.append((c, s))
    if ratios:
        print("\n## Ratios (rps medians)\n")
        print("| Ratio | value |")
        print("|---|---:|")
        for x, y in ratios:
            v = meds.get(x) / meds.get(y) if meds.get(x) and meds.get(y) else None
            flag = ""
            if v is not None and "rc-coalesced" in x and "rc-standalone" in y:
                flag = " (M1 ≥ 0.85: " + ("pass" if v >= THRESHOLD else "below") + ")"
            print(f"| {x} / {y} | {fmt(v, '.3f')}{flag} |")


if __name__ == "__main__":
    main()
