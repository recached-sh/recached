#!/usr/bin/env python3
"""A replica's own AOF does not record a full synchronization.

P primary, R replica with an AOF. R full-syncs, applies SET a/SET b; R is
killed; P deletes a and sets c; R restarts and full-syncs again (store
replaced, AOF untouched); P sets d; R is killed and restarted with no
reachable primary. Its AOF replay then holds a (deleted on P) and lacks c.
usage: replica_aof.py BINARY WORKDIR
"""
import os, shutil, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rc import Server, cmd, free_port

binary, work = sys.argv[1], sys.argv[2]
shutil.rmtree(work, ignore_errors=True)
rport = free_port()
P = Server(binary, os.path.join(work, "p"), RECACHED_REPL_ENABLE="1", RECACHED_REPL_PORT=str(rport),
           RECACHED_AOF_PATH=None)
assert P.start()
R = Server(binary, os.path.join(work, "r"), RECACHED_REPLICAOF="127.0.0.1:%d" % rport,
           RECACHED_REPL_PORT=str(free_port()))
assert R.start()


def wait(port, key, want, t=10):
    end = time.time() + t
    while time.time() < end:
        if cmd(port, "GET", key) == want:
            return True
        time.sleep(0.1)
    return False


time.sleep(1)
cmd(P.port, "SET", "a", "1"); cmd(P.port, "SET", "b", "2")
print("R has a,b:", wait(R.port, "a", "1"), wait(R.port, "b", "2"))
R.kill9()
cmd(P.port, "DEL", "a"); cmd(P.port, "SET", "c", "3")
assert R.start()
print("R after 2nd full sync: c =", wait(R.port, "c", "3") and "3", " a =", cmd(R.port, "GET", "a"))
cmd(P.port, "SET", "d", "4")
print("R has d:", wait(R.port, "d", "4"))
print("R aof:", R.aof())
R.kill9()
P.kill9()
assert R.start()  # primary now unreachable
print("R restarted, primary down: a=%r b=%r c=%r d=%r   (primary had a=None b=2 c=3 d=4)" %
      tuple(cmd(R.port, "GET", k) for k in "abcd"))
R.kill9()
