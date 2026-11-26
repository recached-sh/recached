#!/usr/bin/env python3
"""Do writes reach the AOF after the shutdown save? (Where SEAL would go.)

Several clients pipeline INCRs; SIGTERM arrives mid-stream. After exit, any
record in the AOF was appended after the shutdown save's truncation (no
ephemeral keys, so the seed is empty). Also counts acknowledged INCRs vs the
restarted value.
usage: shutdown_race.py BINARY WORKDIR ROUNDS
"""
import os, shutil, signal, socket, sys, threading, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rc import Server, Conn, cmd

binary, work, rounds = sys.argv[1], sys.argv[2], int(sys.argv[3])
hits = 0
for r in range(rounds):
    d = os.path.join(work, "r%d" % r)
    shutil.rmtree(d, ignore_errors=True)
    s = Server(binary, d, RECACHED_AOF_SYNC="everysec")
    assert s.start()
    acked = [0] * 8
    stop = threading.Event()

    def worker(i):
        try:
            c = Conn(s.port)
            while not stop.is_set():
                v = c.cmd("INCR", "n%d" % i)
                if isinstance(v, int):
                    acked[i] = v
        except Exception:
            pass

    ts = [threading.Thread(target=worker, args=(i,)) for i in range(8)]
    for t in ts:
        t.start()
    time.sleep(0.3)
    s.p.send_signal(signal.SIGTERM)
    s.p.wait(15)
    stop.set()
    for t in ts:
        t.join(5)
    tail = s.aof()
    if tail:
        hits += 1
        print("round %d: %d bytes in the AOF after the shutdown save: %r" % (r, len(tail), tail[:120]))
print("%d/%d rounds had records appended after the shutdown save" % (hits, rounds))
