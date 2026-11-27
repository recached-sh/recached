#!/usr/bin/env python3
"""6O-I8: summarize an i8run.py campaign.

    summarize_i8.py RUNS_DIR [--base h] [--json OUT.json]

Per run (valid runs only; invalid ones are listed with their reason):
  * load: tx/s, latency and writer numbers over the measured window
    (contend), or offered/accepted (paced);
  * CPU: each server's on-CPU time (schedstat, interpolated from the 1 Hz
    series to the generator's measured window) per transaction, and
    instructions per transaction from perf stat;
  * memory: VmRSS / RssAnon at the load's start, the end of warm-up, the
    measured window's mean and maximum, the slope over the measured
    window (MiB/min, least squares), the mean of each 30 s window, at the
    load's end and at the end of the post-load window; VmHWM;
  * replication: queue depth, lag, queue disconnects, syncs (full,
    partial) during the load and after, catch-up time, digests;
  * diag builds: the I8DIAG lock and phase counters and I8MEM jemalloc
    statistics over the measured window.
Paired ratios per rep against the base label, median and range.
"""
import json
import math
import os
import re
import statistics
import sys

ANSI = re.compile(r"\x1b\[[0-9;]*m")


def load_runs(d):
    runs = []
    for line in open(f"{d}/runs.jsonl"):
        runs.append(json.loads(line))
    return runs


def series(path):
    rows = []
    if os.path.exists(path):
        for line in open(path):
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return rows


def interp(rows, t, f):
    """f(row) linearly interpolated at time t."""
    pts = [(r["t"], f(r)) for r in rows if f(r) is not None]
    if not pts:
        return None
    if t <= pts[0][0]:
        return pts[0][1]
    for (t0, v0), (t1, v1) in zip(pts, pts[1:]):
        if t0 <= t <= t1:
            return v0 + (v1 - v0) * (t - t0) / (t1 - t0) if t1 > t0 else v0
    return pts[-1][1]


def g(row, *path):
    for p in path:
        if not isinstance(row, dict) or p not in row:
            return None
        row = row[p]
    return row


def slope(points):
    """Least-squares slope of (t, v)."""
    if len(points) < 3:
        return None
    n = len(points)
    mt = sum(t for t, _ in points) / n
    mv = sum(v for _, v in points) / n
    den = sum((t - mt) ** 2 for t, _ in points)
    return sum((t - mt) * (v - mv) for t, v in points) / den if den else None


def mem_stats(rows, who, key, r):
    ls, ms, me = r["load_start"], r["load"]["measure_start_ms"] / 1e3, r["load"]["measure_end_ms"] / 1e3
    le = r["load_end"]
    vals = [(x["t"], g(x, who, "st", key) / 1024) for x in rows if g(x, who, "st", key) is not None]
    if not vals:
        return {}
    win = [(t, v) for t, v in vals if ms <= t <= me]
    post = [(t, v) for t, v in vals if t > le]
    first = [v for t, v in vals if t >= ls][:1]
    out = {
        "at_load_start": first[0] if first else None,
        "at_measure_start": interp([{"t": t, "v": v} for t, v in vals], ms, lambda x: x["v"]),
        "window_mean": statistics.mean(v for _, v in win) if win else None,
        "window_max": max(v for _, v in win) if win else None,
        "load_max": max((v for t, v in vals if ls <= t <= le), default=None),
        "at_load_end": interp([{"t": t, "v": v} for t, v in vals], le, lambda x: x["v"]),
        "post_end": post[-1][1] if post else None,
        "post_min": min((v for _, v in post), default=None),
        "slope_mib_per_min": (s * 60 if (s := slope(win)) is not None else None),
    }
    # 30 s windows over the measured window.
    w30 = []
    t = ms
    while t + 30 <= me + 0.5:
        seg = [v for tt, v in win if t <= tt < t + 30]
        if seg:
            w30.append(round(statistics.mean(seg), 1))
        t += 30
    out["w30_means"] = w30
    return out


def cpu_per(rows, who, ms, me, n):
    a = interp(rows, ms, lambda x: g(x, who, "sched_ns"))
    b = interp(rows, me, lambda x: g(x, who, "sched_ns"))
    if a is None or b is None or not n:
        return None, None
    return (b - a) / n / 1e3, (b - a) / 1e9 / (me - ms)  # µs per unit, CPUs


def perf_stat(path):
    d = {}
    if not os.path.exists(path):
        return d
    for line in open(path):
        p = line.strip().split(",")
        if len(p) > 3 and p[0].replace(".", "").isdigit():
            d[p[2].split(":")[0]] = float(p[0])
    return d


def info_delta(rows, who, key, t0, t1):
    a = interp(rows, t0, lambda x: g(x, who, "info", key))
    b = interp(rows, t1, lambda x: g(x, who, "info", key))
    return None if a is None or b is None else b - a


def info_max(rows, who, key, t0, t1):
    v = [g(x, who, "info", key) for x in rows if t0 <= x["t"] <= t1 and g(x, who, "info", key) is not None]
    return max(v) if v else None


def diag(path, ms, me):
    """I8DIAG counters and I8MEM over [ms, me] (unix seconds)."""
    if not os.path.exists(path):
        return None
    lines = []
    mems = []
    for raw in open(path, errors="replace"):
        line = ANSI.sub("", raw).strip()
        if line.startswith("I8DIAG "):
            parts = line.split()
            t = int(parts[1]) / 1e3
            stats = {}
            for tok in parts[2:]:
                name, _, v = tok.partition("=")
                c, tot, mx, b = v.split(",", 3)
                buckets = {}
                if b:
                    for kv in b.split(";"):
                        k, n = kv.split(":")
                        buckets[int(k)] = int(n)
                stats[name] = (int(c), int(tot), int(mx), buckets)
            lines.append((t, stats))
        elif line.startswith("I8MEM "):
            parts = line.split()
            t = int(parts[1]) / 1e3
            m = {}
            for tok in parts[2:]:
                k, _, v = tok.partition("=")
                m[k] = None if v == "na" else int(v)
            mems.append((t, m))
    if not lines:
        return None
    inwin = [x for x in lines if ms <= x[0] <= me]
    if len(inwin) < 2:
        return None
    first, last = inwin[0][1], inwin[-1][1]
    out = {}
    for name in last:
        c0, t0, _, b0 = first.get(name, (0, 0, 0, {}))
        c1, t1, _, b1 = last[name]
        n = c1 - c0
        mx = max(s[1][name][2] for s in inwin[1:] if name in s[1])
        bd = {k: b1.get(k, 0) - b0.get(k, 0) for k in b1}
        tot_b = sum(bd.values())

        def q(p):
            acc = 0
            for k in sorted(bd):
                acc += bd[k]
                if tot_b and acc >= p * tot_b:
                    return 2 ** (k + 1)
            return None
        out[name] = {"n": n, "per_s": n / (inwin[-1][0] - inwin[0][0]), "mean": (t1 - t0) / n if n else None,
                     "total": t1 - t0, "max": mx, "p50": q(0.5), "p99": q(0.99), "p999": q(0.999)}
    mem = {}
    win = [m for t, m in mems if ms <= t <= me]
    if win:
        for k in ("allocated", "active", "resident", "retained", "mapped", "metadata", "pdirty"):
            v = [m[k] for m in win if m.get(k) is not None]
            if v:
                mem[k] = {"mean_mib": statistics.mean(v) / 2**20, "max_mib": max(v) / 2**20, "min_mib": min(v) / 2**20}
        a = [m for t, m in mems if ms <= t <= me and m.get("small_nmalloc") is not None]
        if len(a) >= 2:
            dt = len(a) - 1
            for k in ("small_nrequests", "large_nrequests", "small_nfills", "large_nfills", "small_nflushes",
                      "large_nflushes", "bin_mutex_ops", "bin_mutex_waits", "bin_mutex_wait_ns"):
                if a[0].get(k) is not None and a[-1].get(k) is not None:
                    mem[f"{k}_per_s"] = (a[-1][k] - a[0][k]) / dt
            dt = len(a) - 1
            mem["mallocs_per_s"] = ((a[-1]["small_nmalloc"] + a[-1]["large_nmalloc"]) - (a[0]["small_nmalloc"] + a[0]["large_nmalloc"])) / dt
            mem["frees_per_s"] = ((a[-1]["small_ndalloc"] + a[-1]["large_ndalloc"]) - (a[0]["small_ndalloc"] + a[0]["large_ndalloc"])) / dt
    out["_mem"] = mem
    out["_mem_series"] = [(round(t, 1), m.get("allocated"), m.get("active"), m.get("resident"), m.get("retained"), m.get("pdirty")) for t, m in mems]
    return out


def events(path):
    """Timestamped replication events from a server log."""
    out = []
    if not os.path.exists(path):
        return out
    pat = re.compile(r"^(\S+Z)\s+(\w+)\s+\S+:\s+(.*)$")
    keys = ("fell too far behind", "byte queue full", "sync ended", "full snapshot loaded",
            "partial synchronization reached", "connecting to primary", "resynchroniz", "Refusing", "group",
            "Replica connected", "disconnected:", "Snapshot saved from a full")
    for raw in open(path, errors="replace"):
        line = ANSI.sub("", raw).rstrip()
        m = pat.match(line)
        if m and any(k in m.group(3) for k in keys):
            out.append((m.group(1), m.group(2), m.group(3)[:200]))
    return out


def summarize_run(d, r):
    rows = series(f"{d}/series.jsonl")
    s = {"rep": r["rep"], "cell": r["cell"], "label": r["label"], "valid": r["valid"], "why": r["why"]}
    if not r["valid"] or "load" not in r:
        return s
    L = r["load"]
    ms, me = L["measure_start_ms"] / 1e3, L["measure_end_ms"] / 1e3
    s["measure_s"] = me - ms
    paced = L.get("mode") == "paced"
    if paced:
        n = L["accepted"]
        for k in ("rate", "offered_per_s", "accepted_per_s", "accepted_ratio", "frames_per_s", "due_p99_us",
                  "due_max_us", "sent_p99_us", "errors"):
            s[k] = L.get(k)
    else:
        # A writers-only cell: per writer operation.
        n = L["tx"] or L.get("wops")
        for k in ("tx_per_s", "p50_us", "p99_us", "max_us", "writer_ops_per_s", "w_p50_us", "w_p99_us", "w_max_us", "aborts", "errors"):
            s[k] = L.get(k)
    s["n_tx"] = n
    for who in ("primary", "replica"):
        cpu, cpus = cpu_per(rows, who, ms, me, n)
        s[f"{who}_cpu_us_per_tx"], s[f"{who}_cpus"] = cpu, cpus
        ps = perf_stat(f"{d}/perf-stat-{who}.csv")
        if ps.get("instructions"):
            s[f"{who}_instr_per_tx"] = ps["instructions"] / n
            s[f"{who}_ipc"] = ps["instructions"] / ps["cycles"] if ps.get("cycles") else None
            s[f"{who}_cache_miss_per_tx"] = ps.get("cache-misses", 0) / n
        for key in ("VmRSS", "RssAnon"):
            s[f"{who}_{key}"] = mem_stats(rows, who, key, r)
        st = r.get(f"{who}_status", {})
        s[f"{who}_hwm_mib"] = st.get("VmHWM", 0) / 1024 if st else None
        a = interp(rows, ms, lambda x: g(x, who, "st", "voluntary_ctxt_switches"))
        b = interp(rows, me, lambda x: g(x, who, "st", "voluntary_ctxt_switches"))
        a2 = interp(rows, ms, lambda x: g(x, who, "st", "nonvoluntary_ctxt_switches"))
        b2 = interp(rows, me, lambda x: g(x, who, "st", "nonvoluntary_ctxt_switches"))
        if a is not None and b is not None:
            s[f"{who}_vcsw_per_tx"] = (b - a) / n
            s[f"{who}_ivcsw_per_tx"] = (b2 - a2) / n
        for f in ("utime", "stime", "minflt", "majflt"):
            a = interp(rows, ms, lambda x, f=f: g(x, who, "ps", f))
            b = interp(rows, me, lambda x, f=f: g(x, who, "ps", f))
            if a is not None and b is not None and n:
                # ticks are 1/100 s: µs per transaction; faults per transaction
                s[f"{who}_{f}_per_tx"] = (b - a) * (1e4 if f.endswith("time") else 1) / n
        thr = [g(x, who, "st", "Threads") for x in rows if g(x, who, "st", "Threads")]
        s[f"{who}_threads_max"] = max(thr) if thr else None
    busy = interp(rows, me, lambda x: x["busy_ns"]) - interp(rows, ms, lambda x: x["busy_ns"])
    own = sum((s.get(f"{w}_cpus") or 0) for w in ("primary", "replica")) * (me - ms) * 1e9
    gen = None
    s["foreign_cpus"] = None if busy is None else (busy - own) / 1e9 / (me - ms)  # includes the generator
    ls, le = r["load_start"], r["load_end"]
    for key, short in (("recached_replication_queue_depth", "qdepth"), ("recached_replication_lag_frames", "lag"),
                       ("recached_replication_send_queue_bytes_peak", "qbytes_peak")):
        s[f"{short}_max"] = info_max(rows, "primary", key, ls, le)
    for key, short in (("recached_replication_queue_disconnects", "disconnects"), ("recached_replication_syncs_full", "full_syncs"),
                       ("recached_replication_syncs_partial", "partial_syncs")):
        s[f"{short}_load"] = info_delta(rows, "primary", key, ls, le)
        s[f"{short}_measure"] = info_delta(rows, "primary", key, ms, me)
        fin = g(r, "final", "primary", "replication", key)
        pre = g(r, "prefill_info", key)
        s[f"{short}_after_prefill_total"] = (fin - pre) if fin is not None and pre is not None else None
    s["catchup_s"] = r.get("catchup_s")
    s["converged"] = r.get("converged")
    s["digest_equal"] = r.get("digest_equal")
    for k in ("verify_primary", "verify_replica", "verify_restart"):
        v = r.get(k)
        if v:
            s[k] = {x: v.get(x) for x in ("ok", "mismatched", "missing", "ttl_set", "dbsize", "digest")}
    s["restart_s"] = r.get("restart_s")
    # Replica AOF and disk over the measured window.
    for key, short in (("recached_aof_write_calls", "aof_writes"), ("recached_aof_write_usec", "aof_write_us"),
                       ("recached_aof_fsyncs", "aof_fsyncs"), ("recached_aof_fsync_usec", "aof_fsync_us"),
                       ("recached_aof_bytes", "aof_bytes"), ("recached_aof_frames", "aof_frames")):
        dv = info_delta(rows, "replica", key, ms, me)
        if dv is not None:
            s[f"r_{short}"] = dv
    if s.get("r_aof_writes"):
        s["r_aof_us_per_write"] = s["r_aof_write_us"] / s["r_aof_writes"]
        s["r_aof_mb_per_s"] = s["r_aof_bytes"] / (me - ms) / 1e6
        s["r_aof_frames_per_s"] = s["r_aof_frames"] / (me - ms)
        s["r_aof_write_us_per_mb"] = s["r_aof_write_us"] / (s["r_aof_bytes"] / 1e6) if s["r_aof_bytes"] else None
        s["r_aof_write_share"] = s["r_aof_write_us"] / 1e6 / (me - ms)
    if s.get("r_aof_fsyncs"):
        s["r_aof_ms_per_fsync"] = s["r_aof_fsync_us"] / s["r_aof_fsyncs"] / 1e3
    io = interp(rows, me, lambda x: g(x, "disk", "io_ms"))
    io0 = interp(rows, ms, lambda x: g(x, "disk", "io_ms"))
    ws = interp(rows, me, lambda x: g(x, "disk", "w_sect"))
    ws0 = interp(rows, ms, lambda x: g(x, "disk", "w_sect"))
    if io is not None and io0 is not None:
        s["disk_util"] = (io - io0) / 1e3 / (me - ms)
        s["disk_mb_per_s"] = (ws - ws0) * 512 / 1e6 / (me - ms)
    psi = [x.get("psi_io") for x in rows if ms <= x["t"] <= me and x.get("psi_io") is not None]
    s["psi_io_mean"] = statistics.mean(psi) if psi else None
    s["load_at_start"] = r.get("load_at_start")
    s["mhz_before"] = g(r, "host_before", "mhz")
    s["swapfree_mib"] = (g(r, "host_before", "mem", "SwapFree") or 0) / 1024
    # Diagnostic builds.
    for who in ("primary", "replica"):
        dg = diag(f"{d}/{who}.log.txt", ms, me)
        if dg:
            s[f"{who}_diag"] = dg
    # Generator CPU and foreign CPU without it.
    # The generator's CPU from /proc/<pid>/stat (utime + stime, ticks): its
    # threads exit at the window's end, which empties a per-thread schedstat
    # sum (review A round 2, N3). Older series without it: schedstat.
    tick = lambda x: (g(x, "gen", "ps", "utime") + g(x, "gen", "ps", "stime")) * 1e7 if g(x, "gen", "ps", "utime") is not None else None
    if any(tick(x) is not None for x in rows):
        ga, gb = interp(rows, ms, tick), interp(rows, me - 1.5, tick)
        me_g = me - 1.5
    else:
        ga, gb = interp(rows, ms, lambda x: g(x, "gen", "sched_ns")), interp(rows, me, lambda x: g(x, "gen", "sched_ns"))
        me_g = me
    if ga is not None and gb is not None and s.get("foreign_cpus") is not None:
        s["gen_cpus"] = (gb - ga) / 1e9 / (me_g - ms)
        s["foreign_cpus_net"] = s["foreign_cpus"] - s["gen_cpus"]
    # Comparable: completed, every check passed, and (a contention cell) the
    # replica never resynchronized during the load (review A M1, B F1).
    s["checks_failed"] = list(r.get("why") or [])
    if not paced:
        rs = (s.get("full_syncs_load") or 0) + (s.get("partial_syncs_load") or 0)
        dc = s.get("disconnects_load") or 0
        if rs > 0 or dc > 0:
            s["checks_failed"].append(f"replica disconnected {dc:.0f}x / resynchronized {rs:.0f}x during the load")
    s["comparable"] = r["valid"] and not s["checks_failed"]
    s["events_primary"] = events(f"{d}/primary.stdout.txt")
    s["events_replica"] = events(f"{d}/replica.stdout.txt")
    return s


def fmt(v, nd=2):
    if v is None:
        return "–"
    if isinstance(v, float):
        if math.isnan(v):
            return "nan"
        return f"{v:.{nd}f}"
    return str(v)


def paired(summ, base, key, f=lambda s, k: s.get(k)):
    """{label: [ratio per rep]} against base, for valid pairs."""
    out = {}
    by = {}
    for s in summ:
        if s["valid"] and s.get("comparable", True):
            by[(s["cell"], s["rep"], s["label"])] = s
    for (cell, rep, label), s in by.items():
        b = by.get((cell, rep, base))
        if b is None or label == base:
            continue
        x, y = f(s, key), f(b, key)
        if x is not None and y:
            out.setdefault((cell, label), []).append(x / y)
    return out


def med_range(v):
    if not v:
        return "–"
    v = sorted(v)
    return f"{statistics.median(v):.3f} [{v[0]:.3f}–{v[-1]:.3f}]"


def main():
    d = sys.argv[1]
    base = "h"
    if "--base" in sys.argv:
        base = sys.argv[sys.argv.index("--base") + 1]
    runs = load_runs(d)
    summ = []
    for r in runs:
        name = f"{r['rep']}-{r['cell']}-{r['label']}"
        summ.append(summarize_run(f"{d}/{name}", r))
    if "--json" in sys.argv:
        json.dump(summ, open(sys.argv[sys.argv.index("--json") + 1], "w"), indent=1, default=str)
    cells = sorted({s["cell"] for s in summ}, key=lambda c: [s["cell"] for s in summ].index(c))
    labels = []
    for s in summ:
        if s["label"] not in labels:
            labels.append(s["label"])
    print(f"# {os.path.basename(os.path.abspath(d))}: {len(runs)} runs, "
          f"{sum(1 for s in summ if s['valid'])} valid\n")
    bad = [s for s in summ if not s["valid"] or s.get("checks_failed") or s["why"]]
    if bad:
        print("Runs excluded from paired ratios (invalid, a failed check, or a replica resync during a contention load):\n")
        for s in bad:
            print(f"- rep {s['rep']} {s['cell']} {s['label']}: valid={s['valid']} {s.get('checks_failed') or s['why']}")
        print()
    for cell in cells:
        ss = [s for s in summ if s["cell"] == cell and s["valid"]]
        if not ss:
            continue
        paced = "accepted_per_s" in ss[0]
        print(f"## {cell}\n")
        if paced:
            cols = [("rate", "offered/s", "offered_per_s", 0), ("acc", "accepted/s", "accepted_per_s", 0),
                    ("ratio", "acc ratio", "accepted_ratio", 4), ("fr", "frames/s", "frames_per_s", 0),
                    ("pc", "primary µs/tx", "primary_cpu_us_per_tx", 1), ("rc", "replica µs/tx", "replica_cpu_us_per_tx", 1),
                    ("rcpus", "replica CPUs", "replica_cpus", 2),
                    ("fs", "full syncs (after prefill)", "full_syncs_after_prefill_total", 0),
                    ("dc", "queue disconnects (after prefill)", "disconnects_after_prefill_total", 0),
                    ("lag", "lag max", "lag_max", 0), ("cu", "catch-up s", "catchup_s", 2),
                    ("aw", "r AOF µs/write", "r_aof_us_per_write", 1), ("awm", "r write(2) µs/MB", "r_aof_write_us_per_mb", 0),
                    ("aws", "r write(2) share", "r_aof_write_share", 3), ("af", "r ms/fsync", "r_aof_ms_per_fsync", 0),
                    ("amb", "r AOF MB/s", "r_aof_mb_per_s", 1), ("du", "disk util", "disk_util", 2),
                    ("due99", "due p99 µs", "due_p99_us", 0)]
        else:
            cols = [("tx", "tx/s", "tx_per_s", 2), ("p99", "tx p99 ms", "p99_us", 0), ("w", "writer ops/s", "writer_ops_per_s", 0),
                    ("wp99", "writer p99 µs", "w_p99_us", 0), ("wmax", "writer max µs", "w_max_us", 0),
                    ("pc", "primary µs/tx", "primary_cpu_us_per_tx", 0), ("pi", "primary Minstr/tx", "primary_instr_per_tx", 0),
                    ("rc", "replica µs/tx", "replica_cpu_us_per_tx", 0), ("lag", "lag max", "lag_max", 0),
                    ("fs", "full syncs (after prefill)", "full_syncs_after_prefill_total", 0),
                    ("cu", "catch-up s", "catchup_s", 2), ("gc", "generator CPUs", "gen_cpus", 2), ("fc", "foreign CPUs", "foreign_cpus_net", 2)]
        print("| rep | label | " + " | ".join(c[1] for c in cols) + " | checks |")
        print("|" + "---|" * (len(cols) + 3))
        for s in sorted(ss, key=lambda s: (s["rep"], labels.index(s["label"]))):
            vals = []
            for _, _, k, nd in cols:
                v = s.get(k)
                if k == "primary_instr_per_tx" and v:
                    v = v / 1e6
                if k == "p99_us" and v and not paced:
                    v = v / 1e3
                vals.append(fmt(v, nd))
            checks = []
            if s.get("digest_equal") is not None:
                checks.append("digest=" + ("eq" if s["digest_equal"] else "DIFF"))
            for k in ("verify_primary", "verify_replica", "verify_restart"):
                if s.get(k) and s[k].get("ok") is not None and paced:
                    checks.append(f"{k.split('_')[1]}={s[k]['ok']}")
            print(f"| {s['rep']} | {s['label']} | " + " | ".join(vals) + f" | {' '.join(checks)} |")
        print()
        # Memory.
        print("| rep | label | RSS start | RSS measure start | RSS window mean | RSS window max | slope MiB/min | 30 s means | RSS load end | RSS post end | HWM | anon mean | replica RSS max |")
        print("|" + "---|" * 13)
        for s in sorted(ss, key=lambda s: (s["rep"], labels.index(s["label"]))):
            m, a, rm = s.get("primary_VmRSS", {}), s.get("primary_RssAnon", {}), s.get("replica_VmRSS", {})
            print(f"| {s['rep']} | {s['label']} | {fmt(m.get('at_load_start'), 1)} | {fmt(m.get('at_measure_start'), 1)} | "
                  f"{fmt(m.get('window_mean'), 1)} | {fmt(m.get('window_max'), 1)} | {fmt(m.get('slope_mib_per_min'), 2)} | "
                  f"{' '.join(str(x) for x in m.get('w30_means', []))} | {fmt(m.get('at_load_end'), 1)} | {fmt(m.get('post_end'), 1)} | "
                  f"{fmt(s.get('primary_hwm_mib'), 1)} | {fmt(a.get('window_mean'), 1)} | {fmt(rm.get('load_max'), 1)} |")
        print()
        # Paired ratios.
        keys = (["accepted_per_s", "primary_cpu_us_per_tx", "replica_cpu_us_per_tx", "r_aof_us_per_write"] if paced else
                ["tx_per_s", "primary_cpu_us_per_tx", "primary_instr_per_tx", "writer_ops_per_s", "w_p99_us", "replica_cpu_us_per_tx"])
        mkeys = [("RSS window mean", lambda s, k: g(s, "primary_VmRSS", "window_mean")),
                 ("RSS window max", lambda s, k: g(s, "primary_VmRSS", "window_max")),
                 ("RSS post end", lambda s, k: g(s, "primary_VmRSS", "post_end"))]
        print(f"Paired per rep against {base} (median [range]):\n")
        print("| label | " + " | ".join(keys + [m[0] for m in mkeys]) + " |")
        print("|" + "---|" * (len(keys) + len(mkeys) + 1))
        for label in labels:
            if label == base:
                continue
            row = []
            for k in keys:
                row.append(med_range(paired([s for s in summ if s["cell"] == cell], base, k).get((cell, label), [])))
            for _, f in mkeys:
                row.append(med_range(paired([s for s in summ if s["cell"] == cell], base, None, f).get((cell, label), [])))
            print(f"| {label} | " + " | ".join(row) + " |")
        print()
        # Diag.
        dg = [s for s in ss if s.get("primary_diag")]
        if dg:
            names = ["stream_hold_g", "stream_hold_s16", "stream_wait_g", "stream_wait_s", "stream_free_s", "stripe_wait_big",
                     "stripe_hold_big", "stripe_wait_small", "stripe_hold_small", "stripe_wait_all", "stripe_hold_all", "stripe_gap", "waiters_at_release", "gate_close", "tx_apply", "tx_log"]
            print("Diag (primary, measured window; times in µs (waiters_at_release: a count): mean / upper bound of the log2 bucket holding the 99th percentile / max; n per s):\n")
            print("| rep | label | " + " | ".join(names) + " |")
            print("|" + "---|" * (len(names) + 2))
            for s in sorted(dg, key=lambda s: (s["rep"], labels.index(s["label"]))):
                cells_ = []
                for n in names:
                    x = s["primary_diag"].get(n)
                    if not x or not x["n"]:
                        cells_.append("–")
                    else:
                        div = 1 if n in ("waiters_at_release",) else 1e3
                        cells_.append(f"{fmt(x['mean'] / div if x['mean'] else None, 2 if div == 1 else 1)} / {fmt((x['p99'] or 0) / div, 0)} / "
                                      f"{fmt(x['max'] / div, 0)} ({x['per_s']:.0f}/s)")
                print(f"| {s['rep']} | {s['label']} | " + " | ".join(cells_) + " |")
            print()
            print("jemalloc (primary, measured window, MiB mean/max): allocated, active, resident, retained, dirty pages; mallocs/s\n")
            print("| rep | label | allocated | active | resident | retained | pdirty (pages) | mallocs/s | frees/s |")
            print("|---|---|---|---|---|---|---|---|---|")
            for s in sorted(dg, key=lambda s: (s["rep"], labels.index(s["label"]))):
                m = s["primary_diag"]["_mem"]
                def mm(k):
                    x = m.get(k)
                    return f"{x['mean_mib']:.1f} / {x['max_mib']:.1f}" if x else "–"
                pd = m.get("pdirty")
                print(f"| {s['rep']} | {s['label']} | {mm('allocated')} | {mm('active')} | {mm('resident')} | {mm('retained')} | "
                      f"{(pd['mean_mib'] * 2**20):.0f} / {(pd['max_mib'] * 2**20):.0f} | {fmt(m.get('mallocs_per_s'), 0)} | {fmt(m.get('frees_per_s'), 0)} |"
                      if pd else f"| {s['rep']} | {s['label']} | {mm('allocated')} | {mm('active')} | {mm('resident')} | {mm('retained')} | – | {fmt(m.get('mallocs_per_s'), 0)} | {fmt(m.get('frees_per_s'), 0)} |")
            print()
            rd = [s for s in dg if s.get("replica_diag")]
            if rd:
                names = ["r_group_apply", "r_frame_apply16", "r_read_gap", "r_read_bytes", "r_apply_run", "r_apply_run_frames"]
                print("Diag (replica, measured window; mean / p99 bucket / max, µs or a value; n per s):\n")
                print("| rep | label | " + " | ".join(names) + " |")
                print("|" + "---|" * (len(names) + 2))
                for s in sorted(rd, key=lambda s: (s["rep"], labels.index(s["label"]))):
                    cells_ = []
                    for n in names:
                        x = s["replica_diag"].get(n)
                        div = 1 if n in ("r_read_bytes", "r_apply_run_frames") else 1e3
                        if not x or not x["n"]:
                            cells_.append("–")
                        else:
                            cells_.append(f"{fmt(x['mean'] / div, 1)} / {fmt((x['p99'] or 0) / div, 0)} / {fmt(x['max'] / div, 0)} ({x['per_s']:.0f}/s)")
                    print(f"| {s['rep']} | {s['label']} | " + " | ".join(cells_) + " |")
                print()


if __name__ == "__main__":
    main()
