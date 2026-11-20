#!/usr/bin/env python3
"""Symbolize `uprof record` samples and print flat and inclusive profiles.

    symbolize.py <samples> <binary-with-symbols> [--ops N] [--top K] [--tid-comm FILE]
                 [--focus REGEX ...]

Each sample line is `S <tid> <ip> [fp-chain...] [| lbr-from...]`. The
`<samples>.maps` file written next to it maps addresses to files. Symbols come
from `nm -n` on the profiling binary (built with symbols and frame pointers
from the same source as the release binary) and from `nm -D` on shared
libraries, so libc frames resolve only to their nearest exported symbol.

Self time is the leaf IP. Inclusive time counts each distinct function once
per sample, over the leaf and the LBR call stack (call sites of calls not yet
returned, newest first); samples without LBR entries fall back to the frame-
pointer chain. LBR holds at most 32 calls, so the outermost frames of deep
stacks are missing: inclusive numbers for leaf-side functions (fan_out,
try_send, malloc, the sender's flush) are exact in coverage, those of outer
async frames are lower bounds.

With --ops, sample counts are converted to cycles/op using the total user
cycles, which the caller passes as --cycles (from `uprof stat` of the same run).
"""
import argparse
import bisect
import collections
import re
import subprocess
import sys

HASH = re.compile(r"::h[0-9a-f]{16}$")


def load_maps(path):
    maps = []
    for line in open(path):
        parts = line.split()
        if len(parts) < 6 or "x" not in parts[1]:
            continue
        lo, hi = (int(x, 16) for x in parts[0].split("-"))
        maps.append((lo, hi, int(parts[2], 16), parts[5]))
    return maps


def load_segments(binary):
    """(file_offset, vaddr, filesz) of each LOAD segment, from objdump -p."""
    out = subprocess.run(["objdump", "-p", binary], capture_output=True, text=True).stdout
    segs = []
    for m in re.finditer(r"LOAD off\s+(0x[0-9a-f]+) vaddr (0x[0-9a-f]+).*\n\s+filesz (0x[0-9a-f]+)", out):
        segs.append((int(m.group(1), 16), int(m.group(2), 16), int(m.group(3), 16)))
    return segs


def load_symbols(binary, dynamic):
    args = ["nm", "-n", "-C", "--defined-only"] + (["-D"] if dynamic else []) + [binary]
    out = subprocess.run(args, capture_output=True, text=True).stdout
    addrs, names = [], []
    for line in out.splitlines():
        parts = line.split(" ", 2)
        if len(parts) < 3 or parts[1] not in "tTwWiu":
            continue
        try:
            a = int(parts[0], 16)
        except ValueError:
            continue
        addrs.append(a)
        names.append(HASH.sub("", parts[2]))
    return addrs, names


class Resolver:
    def __init__(self, maps, binary):
        self.maps = maps
        self.binary = binary
        self.cache = {}
        self.files = {}

    def _file(self, path):
        if path not in self.files:
            is_main = path == self.binary or path.endswith("/" + self.binary.split("/")[-1])
            target = self.binary if is_main else path
            try:
                syms = load_symbols(target, dynamic=not is_main)
                segs = load_segments(target)
            except Exception:
                syms, segs = ([], []), []
            self.files[path] = (syms, segs, path.split("/")[-1])
        return self.files[path]

    def __call__(self, addr):
        if addr in self.cache:
            return self.cache[addr]
        name = "[unknown]"
        for lo, hi, off, path in self.maps:
            if lo <= addr < hi:
                (addrs, names), segs, short = self._file(path)
                foff = addr - lo + off
                vaddr = foff
                for s_off, s_vaddr, s_sz in segs:
                    if s_off <= foff < s_off + s_sz:
                        vaddr = foff - s_off + s_vaddr
                        break
                i = bisect.bisect_right(addrs, vaddr) - 1
                if i >= 0:
                    name = names[i] if short == self.binary.split("/")[-1] else f"{short}:{names[i]}"
                else:
                    name = f"{short}:+{vaddr:x}"
                break
        self.cache[addr] = name
        return name


# Innermost frame matching one of these names the sample's context. The order
# only breaks ties within one frame name; nesting is resolved by the stack.
CONTEXTS = [(lab, re.compile(rx)) for lab, rx in [
    ("repl fan_out: per-replica enqueue", r"retain::<<recached_server::replication::ReplHub>::fan_out"),
    ("repl fan_out: backlog, offset, lock", r"ReplHub>::fan_out"),
    ("repl sender: write_repl_frame", r"replication::write_repl_frame"),
    ("repl sender: task (recv, flush, ack read)", r"replication::handle_replica|run_repl_server::\{closure#0\}::\{closure#0\}"),
    ("on_write_logged (frame copy, dispatch)", r"ServerState>::on_write_logged"),
    ("frame build (broadcast_for_at)", r"propagation::broadcast_for"),
    ("AOF append/lead/gate", r"persistence::|AofGatedWriter|DeferredAof"),
    ("stripe locks", r"ReplHub>::lock_(write|stripes|commands)"),
    ("store execute", r"core_engine::store::|execute_and_record"),
    ("ordered write path (rest)", r"execute_ordered_write|execute_locked_write|apply_write_effects"),
    ("connection (decode, reply, socket)", r"connection::handle_tcp|RequestDecoder|clients::"),
    ("tokio io driver", r"io::driver|Driver>::turn"),
    ("tokio scheduler", r"tokio::runtime::"),
]]
LEAF_CLASSES = [(lab, re.compile(rx)) for lab, rx in [
    ("allocator", r"^(_rjem|do_rallocx|je_|rjem|mallocx|sdallocx|alloc::alloc|<alloc::raw_vec)"),
    ("libc (memcpy, syscall wrappers)", r"^(libc\.so|\[vdso\])"),
    ("tokio wake/schedule", r"wake|schedule|Notify|notify|AtomicWaker|Inject|Unparker|unpark"),
    ("tokio mpsc", r"tokio::sync::mpsc|chan::|semaphore"),
    ("tokio io/split", r"tokio::io::|Registration|PollEvented|mio::"),
    ("atomics/Arc drop", r"drop_in_place|Arc<.*>>::drop_slow"),
]]


def short(name, width=110):
    name = re.sub(r"<impl [^>]*>", "<impl>", name)
    return name if len(name) <= width else name[: width - 1] + "…"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("samples")
    ap.add_argument("binary")
    ap.add_argument("--ops", type=float)
    ap.add_argument("--cycles", type=float, help="total user cycles of the run, from uprof stat")
    ap.add_argument("--top", type=int, default=40)
    ap.add_argument("--focus", action="append", default=[],
                    help="LABEL=REGEX: report the share of samples whose stack contains REGEX")
    ap.add_argument("--breakdown", action="store_true",
                    help="assign each sample once: innermost context frame x leaf class")
    ap.add_argument("--tids", help="comma-separated tids to keep (default: all)")
    a = ap.parse_args()

    res = Resolver(load_maps(a.samples + ".maps"), a.binary)
    keep = set(int(t) for t in a.tids.split(",")) if a.tids else None
    self_c, incl_c = collections.Counter(), collections.Counter()
    # LABEL=TERM&&TERM...: every term must match; a term starting with ^ must
    # match the leaf, any other term any frame of the stack.
    focus = [
        (lab, [(t.startswith("^"), re.compile(t.lstrip("^"))) for t in rx.split("&&")])
        for lab, rx in (f.split("=", 1) for f in a.focus)
    ]
    focus_c = collections.Counter()
    total = 0
    brk = collections.Counter()
    for line in open(a.samples):
        parts = line.split()
        if not parts or parts[0] != "S":
            continue
        tid = int(parts[1])
        if keep is not None and tid not in keep:
            continue
        rest = parts[2:]
        if "|" in rest:
            k = rest.index("|")
            fp, lbr = rest[:k], rest[k + 1:]
        else:
            fp, lbr = rest, []
        leaf = int(fp[0], 16)
        chain = [leaf] + ([int(x, 16) for x in lbr] if lbr else [int(x, 16) for x in fp[1:]])
        names = [res(x) for x in chain]
        total += 1
        self_c[names[0]] += 1
        for n in set(names):
            incl_c[n] += 1
        stack = "\n".join(names)
        if a.breakdown:
            ctx = next((lab for n in names for lab, rx in CONTEXTS if rx.search(n)), "other")
            leafc = next((lab for lab, rx in LEAF_CLASSES if rx.search(names[0])), "code")
            brk[(ctx, leafc)] += 1
        for lab, terms in focus:
            if all(rx.search(names[0]) if leaf_only else rx.search(stack) for leaf_only, rx in terms):
                focus_c[lab] += 1
    if not total:
        print("no samples")
        return
    per_op = None
    if a.ops and a.cycles:
        per_op = a.cycles / a.ops / total  # user cycles per op carried by one sample

    def row(n, c):
        pct = 100.0 * c / total
        extra = f" {c * per_op:8.1f}" if per_op else ""
        return f"| {pct:5.1f}%{extra} | `{short(n)}` |"

    hdr = "| share | cycles/op | function |" if per_op else "| share | function |"
    sep = "|---:|---:|---|" if per_op else "|---:|---|"
    print(f"samples: {total}" + (f", user cycles/op {a.cycles / a.ops:.0f}" if per_op else ""))
    if focus:
        print("\n### Stack contains (inclusive)\n")
        print(hdr.replace("function", "label")); print(sep)
        for lab, _ in focus:
            print(row(lab, focus_c[lab]))
    if a.breakdown:
        print("\n### Breakdown: innermost context x leaf class (each sample once)\n")
        print("| context | leaf class | share | cycles/op |" if per_op else "| context | leaf class | share |")
        print("|---|---|---:|---:|" if per_op else "|---|---|---:|")
        ctx_tot = collections.Counter()
        for (ctx, lc), c in brk.items():
            ctx_tot[ctx] += c
        for ctx, ct in ctx_tot.most_common():
            for (cx, lc), c in sorted(brk.items(), key=lambda kv: -kv[1]):
                if cx == ctx:
                    print(f"| {ctx} | {lc} | {100.0 * c / total:5.1f}% |" + (f" {c * per_op:7.0f} |" if per_op else ""))
            print(f"| **{ctx}** | **all** | **{100.0 * ct / total:5.1f}%** |" + (f" **{ct * per_op:7.0f}** |" if per_op else ""))
    print("\n### Self (leaf)\n")
    print(hdr); print(sep)
    for n, c in self_c.most_common(a.top):
        print(row(n, c))
    print("\n### Inclusive\n")
    print(hdr); print(sep)
    for n, c in incl_c.most_common(a.top):
        print(row(n, c))


if __name__ == "__main__":
    sys.exit(main())
