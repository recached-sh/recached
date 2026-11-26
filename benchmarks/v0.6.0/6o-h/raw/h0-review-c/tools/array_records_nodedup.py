#!/usr/bin/env python3
"""Do released binaries replay AOF records encoded as RESP2 arrays (`*N`)
as well as RESP3 pushes (`>N`)? If so, the record encoding could mark
authorship (new writer = `*`, old writer = `>`) without relying on SEAL.

Also: does the old binary's own appended record stay `>`?
usage: array_records.py WORKDIR NAME=BINARY...
"""
import os, shutil, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rc import Server, cmd


def rec(kind, *parts):
    out = b"%s%d\r\n" % (kind, len(parts))
    for p in parts:
        p = p.encode() if isinstance(p, str) else p
        out += b"$%d\r\n%s\r\n" % (len(p), p)
    return out


work = sys.argv[1]
FMT = str((0xEC00 << 48) | (1 << 40) | (1 << 32) | 1)
for pair in sys.argv[2:]:
    name, binary = pair.split("=", 1)
    d = os.path.join(work, name)
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    body = (rec(b"*", "RECACHED-CHECKPOINT", FMT) + rec(b"*", "SET", "a", "1") +
            rec(b"*", "SET", "b", "2") + rec(b"*", "APPEND", "a", "x") +
            rec(b"*", "DEL", "zz", "yy"))
    with open(os.path.join(d, "appendonly.aof"), "wb") as f:
        f.write(body)
    s = Server(binary, d)
    up = s.start()
    got = [cmd(s.port, "GET", k) for k in ("a", "b")] if up else None
    if up:
        cmd(s.port, "SET", "c", "3")
        tail = s.aof()[len(body):]
    s.kill9()
    print("%-10s up=%s a,b=%s appended=%r" % (name, up, got, tail[:40] if up else None))
