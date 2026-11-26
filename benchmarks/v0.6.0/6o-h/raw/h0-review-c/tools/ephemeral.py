#!/usr/bin/env python3
"""Ephemeral seed and startup discard: two pre-existing gaps in "every removal
is logged / every frame applies to the state its command saw".

E1 (seed drops the deadline): ESET k v; PEXPIRE k 600000; SAVE (truncate +
    seed `ESET k v`); SET k v2 KEEPTTL; SIGKILL; restart. Primary had v2 with
    a deadline; expected PTTL > 0.
E2 (startup discard is not logged): ESET e v; SIGKILL; restart (replay
    discards e in memory only); APPEND e x (live: "x", durable); SIGKILL;
    restart. Expected GET e == "x".
usage: ephemeral.py BINARY WORKDIR
"""
import os, shutil, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rc import Server, Conn, cmd

binary, work = sys.argv[1], sys.argv[2]
shutil.rmtree(work, ignore_errors=True)

d = os.path.join(work, "e1")
s = Server(binary, d)
assert s.start()
c = Conn(s.port)
print("E1 ESET", c.cmd("ESET", "k", "v"), "PEXPIRE", c.cmd("PEXPIRE", "k", "600000"),
      "PTTL", c.cmd("PTTL", "k"))
print("E1 SAVE", cmd(s.port, "SAVE"), "aof after save:", s.aof())
print("E1 SET KEEPTTL", cmd(s.port, "SET", "k", "v2", "KEEPTTL"), "live PTTL", cmd(s.port, "PTTL", "k"))
c.close()
s.kill9()
assert s.start()
print("E1 after restart: GET", cmd(s.port, "GET", "k"), "PTTL", cmd(s.port, "PTTL", "k"),
      "(expected v2 with PTTL > 0)")
s.kill9()

d = os.path.join(work, "e2")
s = Server(binary, d)
assert s.start()
c = Conn(s.port)
print("E2 ESET", c.cmd("ESET", "e", "v"))
s.kill9()
c.close()
assert s.start()
print("E2 restart 1: GET", cmd(s.port, "GET", "e"), "APPEND", cmd(s.port, "APPEND", "e", "x"),
      "GET", cmd(s.port, "GET", "e"))
s.kill9()
assert s.start()
print("E2 restart 2: GET", cmd(s.port, "GET", "e"), "(expected 'x', the acknowledged durable value)")
print("E2 aof:", s.aof())
s.kill9()
