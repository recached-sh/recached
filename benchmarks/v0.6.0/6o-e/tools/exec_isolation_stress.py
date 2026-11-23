#!/usr/bin/env python3
"""6O-E: read-only MULTI/EXEC against concurrent writers, over TCP.

Extends 6O-D's tools/exec_read_isolation.py (same keys, same check) with a
third writer mode and a clock check.

usage: exec_isolation_stress.py PORT MODE SECONDS
  MODE unordered  2 MSET a g b g writers, nothing observing write order
       watcher    the same, with an unrelated WATCH held (writes ordered)
       sets       1 writer: SET a g, then SET b g (separate commands).
                  Legal states: a == b or a == b + 1; anything else means a
                  write landed between the transaction's two reads.

Each reader sends MULTI GET a GET b EXEC and MGET a b, pipelined, and counts
transactions (and MGETs) that saw an illegal state.
"""
import socket, sys, threading, time

port = int(sys.argv[1]); mode = sys.argv[2]; secs = float(sys.argv[3])
A, B = "iso:a", "iso:b"


def c():
    s = socket.create_connection(("127.0.0.1", port))
    s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    return s


def enc(*a):
    return ("*%d\r\n" % len(a) + "".join("$%d\r\n%s\r\n" % (len(x), x) for x in a)).encode()


def readv(f):
    l = f.readline(); t = l[:1]
    if t in b"+-:":
        return l[1:-2]
    if t == b"$":
        n = int(l[1:-2])
        if n < 0:
            return None
        return f.read(n + 2)[:-2]
    if t == b"*":
        n = int(l[1:-2])
        return None if n < 0 else [readv(f) for _ in range(n)]
    raise RuntimeError("bad reply %r" % l)


def legal(a, b):
    a, b = int(a), int(b)
    return a == b or (mode == "sets" and a == b + 1)


w0 = c(); w0.sendall(enc("MSET", A, "0", B, "0")); w0.makefile("rb").readline()
if mode == "watcher":
    wt = c(); wt.sendall(enc("WATCH", "iso:unrelated")); wt.makefile("rb").readline()
stop = time.time() + secs
counts = {"rounds": 0, "exec_illegal": 0, "mget_illegal": 0, "writes": 0}
lock = threading.Lock()


def writer(start):
    s = c(); f = s.makefile("rb"); g = start; n = 0
    while time.time() < stop:
        g += 1
        if mode == "sets":
            s.sendall(enc("SET", A, str(g))); readv(f)
            s.sendall(enc("SET", B, str(g))); readv(f)
        else:
            s.sendall(enc("MSET", A, str(g), B, str(g))); readv(f)
        n += 1
    with lock:
        counts["writes"] += n


def reader():
    s = c(); f = s.makefile("rb"); r = e = m = 0
    while time.time() < stop:
        s.sendall(enc("MULTI") + enc("GET", A) + enc("GET", B) + enc("EXEC") + enc("MGET", A, B))
        replies = [readv(f) for _ in range(5)]
        ex, mg = replies[3], replies[4]
        r += 1
        if not legal(*ex):
            e += 1
        if not legal(*mg):
            m += 1
    with lock:
        counts["rounds"] += r; counts["exec_illegal"] += e; counts["mget_illegal"] += m


writers = 1 if mode == "sets" else 2
ts = [threading.Thread(target=writer, args=(i * 10**9,)) for i in range(writers)] + \
     [threading.Thread(target=reader) for _ in range(2)]
[t.start() for t in ts]; [t.join() for t in ts]
print("mode=%s writers=%d writes=%d rounds=%d exec_illegal=%d mget_illegal=%d" % (
    mode, writers, counts["writes"], counts["rounds"], counts["exec_illegal"], counts["mget_illegal"]))
