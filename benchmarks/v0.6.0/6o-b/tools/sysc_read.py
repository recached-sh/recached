#!/usr/bin/env python3
"""Snapshot or diff the counters of sysc.so (see sysc.c).

  sysc_read.py snap <counters-file>              one line: k=v ... (raw)
  sysc_read.py diff "<snap-before>" "<snap-after>" <ops>   per-op table

The snapshot line is what the kit stores before and after a timed run; the
diff divides each counter by the operations completed in between.
"""
import struct
import sys

NAMES = ["read", "write", "recv", "send", "readv", "writev",
         "epoll_wait", "epoll_wait_t0", "epoll_ctl", "futex", "syscall_other", "accept4"]
FIELDS = ["calls", "pos_sum", "eagain", "zero", "err", "tsc"]


def snap(path):
    with open(path, "rb") as f:
        raw = f.read()
    magic, version, tsc_hz, pid = struct.unpack_from("<8sQQQ", raw, 0)
    if magic != b"SYSC6OB1" or version != 1:
        raise SystemExit(f"{path}: not a sysc counters file")
    out = [f"tsc_hz={tsc_hz}", f"pid={pid}"]
    off = 32
    for name in NAMES:
        vals = struct.unpack_from("<6Q", raw, off)
        off += 48
        out += [f"{name}.{fld}={v}" for fld, v in zip(FIELDS, vals)]
    return " ".join(out)


def parse(line):
    return {k: int(v) for k, v in (w.split("=", 1) for w in line.split())}


def diff(before, after, ops):
    a, b = parse(before), parse(after)
    hz = b["tsc_hz"]
    print(f"| syscall | calls/op | bytes or events/call | EAGAIN/op | zero/op | ns/call | ns/op |")
    print("|---|---:|---:|---:|---:|---:|---:|")
    total_ns = 0.0
    total_calls = 0.0
    for name in NAMES:
        d = {f: b[f"{name}.{f}"] - a[f"{name}.{f}"] for f in FIELDS}
        if d["calls"] == 0:
            continue
        ns = d["tsc"] * 1e9 / hz
        total_ns += ns
        total_calls += d["calls"]
        ok = d["calls"] - d["eagain"] - d["zero"] - d["err"]
        print(f"| {name} | {d['calls'] / ops:.4f} | {d['pos_sum'] / ok if ok else 0:.1f} | "
              f"{d['eagain'] / ops:.4f} | {d['zero'] / ops:.4f} | {ns / d['calls']:.0f} | {ns / ops:.0f} |")
    print(f"| **all** | {total_calls / ops:.4f} | | | | {total_ns / total_calls if total_calls else 0:.0f} | {total_ns / ops:.0f} |")


if __name__ == "__main__":
    if sys.argv[1] == "snap":
        print(snap(sys.argv[2]))
    elif sys.argv[1] == "diff":
        diff(sys.argv[2], sys.argv[3], float(sys.argv[4]))
    else:
        raise SystemExit(__doc__)
