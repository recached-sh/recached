#!/usr/bin/env python3
"""6O-F3: summarise stripes.sh results (tx.log).

Per cell and label: median tx/s and writer ops/s over the repetitions, each
repetition's ratio against `base` in the same repetition, the spread
(max/min of the ratios), median p99, generator CPU share, server on-CPU per
transaction, RSS, and, for instrumented labels (x*), the medians of the
INFO exp6of deltas over the cell.

usage: summarize_stripes.py tx.log [BASE_LABEL]
"""
import collections
import re
import statistics as st
import sys


def kv(s):
    return dict(re.findall(r"(\w+):(\d+)", s))


def parse(path):
    rows = []
    for line in open(path):
        if not line.startswith("label="):
            continue
        d = dict(re.findall(r"(\w+)=([^\s\[\]]+)", line.split(" uprof=")[0]))
        before = kv(re.search(r"exp_before=\[([^\]]*)\]", line).group(1))
        after = kv(re.search(r"exp_after=\[([^\]]*)\]", line).group(1))
        d["exp"] = {k: int(after[k]) - int(before.get(k, 0)) for k in after} if after else {}
        rows.append(d)
    return rows


def main():
    rows = parse(sys.argv[1])
    base = sys.argv[2] if len(sys.argv) > 2 else "base"
    cells = collections.OrderedDict()
    for r in rows:
        cell = f"{r['workers']}w {r['mode']} {r['keyset']} c{r['clients']} w{r['writers']}"
        cells.setdefault(cell, collections.defaultdict(dict))[r["label"]][int(r["rep"])] = r
    for cell, labels in cells.items():
        print(f"\n### {cell}\n")
        print("| label | tx/s | ratio (per rep) | writer/s | writer ratio | p99 µs | CPU µs/tx | gen CPU | RSS MiB | contended/takes | wait ms | gate refused | tx lock µs |")
        print("|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
        b = labels.get(base, {})
        for label, reps in labels.items():
            tx = [float(reps[i]["tx_per_s"]) for i in sorted(reps)]
            wo = [float(reps[i]["writer_ops_per_s"]) for i in sorted(reps)]
            ratios = [float(reps[i]["tx_per_s"]) / float(b[i]["tx_per_s"])
                      for i in sorted(reps) if i in b and float(b[i]["tx_per_s"]) > 0]
            wratios = [float(reps[i]["writer_ops_per_s"]) / float(b[i]["writer_ops_per_s"])
                       for i in sorted(reps) if i in b and float(b[i]["writer_ops_per_s"]) > 0]
            p99 = st.median(float(reps[i]["p99_us"]) for i in reps)
            ops = [(float(reps[i]["oncpu_s"]) * 1e6 / max(1, int(reps[i]["tx"]))) for i in reps]
            gen = max(float(reps[i]["gen_cpu_share"]) for i in reps)
            rss = st.median(int(reps[i]["rss_kb"]) for i in reps) / 1024
            exps = [reps[i]["exp"] for i in reps if reps[i]["exp"]]
            if exps:
                med = lambda k: st.median(e.get(k, 0) for e in exps)
                cont = f"{med('stripe_contended'):.0f}/{med('stripe_takes'):.0f}"
                wait = f"{med('stripe_wait_ns') / 1e6:.0f}"
                gate = f"{med('gate_refused'):.0f}"
                txl = f"{st.median(e.get('tx_lock_ns', 0) / max(1, e.get('tx_calls', 0)) for e in exps) / 1e3:.2f}"
            else:
                cont = wait = gate = txl = "—"
            r = (f"{st.median(ratios):.3f} ({' '.join(f'{x:.3f}' for x in ratios)})" if ratios and label != base else "—")
            wr = f"{st.median(wratios):.3f}" if wratios and label != base else "—"
            cpu = f"{st.median(ops):.2f}" if any(int(reps[i]['tx']) for i in reps) else "—"
            print(f"| {label} | {st.median(tx):,.0f} | {r} | {st.median(wo):,.0f} | {wr} | {p99:.0f} | {cpu} | {gen:.2f} | {rss:.1f} | {cont} | {wait} | {gate} | {txl} |")


if __name__ == "__main__":
    main()
