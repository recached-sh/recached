#!/usr/bin/env python3
"""6O-I8: summarize the `prof` campaign's perf recordings (host only).

    perf_i8.py RUNS_DIR HOSTPERF

For every perf-<who>.data: the top symbols by self cycles, and the share of
user cycles in jemalloc (its symbols carry the `_rjem_` prefix in these
builds), split into its fast path (malloc/free entry points), the
thread-cache refill/flush and arena/bin paths, and page purging/extent
management. Writes <run>/perf-<who>.top.txt and prints a table.
"""
import glob
import os
import re
import subprocess
import sys

d, hp = sys.argv[1], sys.argv[2]
GROUPS = [
    ("tcache refill/flush", re.compile(r"tcache_(bin_flush|alloc_small_hard|gc|event|bin_fill)|cache_bin|arena_cache_bin_fill")),
    ("arena bins/slabs", re.compile(r"arena_(slab|bin|ralloc|dalloc_bin|malloc_hard)|bin_(malloc|dalloc)|slab|bitmap")),
    ("extents/purge/decay", re.compile(r"extent|pac_|decay|purge|ecache|eset|emap|ehooks|san_|pages_|hpa|sec_")),
    ("jemalloc other", re.compile(r"_rjem_")),
]
print("| run | who | samples | jemalloc share | " + " | ".join(g for g, _ in GROUPS[:-1]) + " | top self symbols |")
print("|---|---|---|---|" + "---|" * (len(GROUPS) - 1) + "---|")
for f in sorted(glob.glob(f"{d}/*/perf-*.data")):
    run = os.path.basename(os.path.dirname(f))
    who = os.path.basename(f)[5:-5]
    out = subprocess.run([hp, "report", "-i", f, "--stdio", "--no-children", "--sort", "symbol",
                          "--percent-limit", "0.05", "-g", "none"], capture_output=True, text=True).stdout
    open(f"{os.path.dirname(f)}/perf-{who}.top.txt", "w").write(out)
    samples = re.search(r"# Samples: (\S+)", out)
    rows = []
    for line in out.splitlines():
        m = re.match(r"\s+([\d.]+)%\s+\[\.\]\s+(.*)$", line)
        if m:
            rows.append((float(m.group(1)), m.group(2).strip()))
    shares = {g: 0.0 for g, _ in GROUPS}
    for pct, sym in rows:
        if "_rjem_" in sym or "je_" in sym:
            for g, pat in GROUPS:
                if pat.search(sym):
                    shares[g] += pct
                    break
    total_je = sum(shares.values())
    top = "; ".join(f"{s[:40]} {p:.1f}%" for p, s in rows[:5])
    print(f"| {run} | {who} | {samples.group(1) if samples else '?'} | {total_je:.1f}% | "
          + " | ".join(f"{shares[g]:.1f}%" for g, _ in GROUPS[:-1]) + f" | {top} |")
