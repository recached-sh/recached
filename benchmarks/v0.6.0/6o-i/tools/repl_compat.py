#!/usr/bin/env python3
"""6O-I: replication between the 6O-I binary and older ones, as real
processes. For each pairing (primary binary -> replica binary): the replica
follows, the primary runs single writes and multi-write EXECs (some on keys
that expired), the replica must converge to the primary's keyspace, and a
reader of the replica records whether it ever saw an EXEC part-applied
(expected only where the replica does not apply groups atomically).

usage: repl_compat.py 'LABEL|PRIMARY_BINARY|REPLICA_BINARY|atomic|perframe' ...
  (atomic: the replica must never be seen part-applied; perframe: recorded)
Env: RECACHED_REPL_DEDUP is set to off on the primary when a replica is a
0.5 release (0.5 replicas cannot read DEDUP frames; unchanged since 6B).
"""
import os, shutil, signal, socket, subprocess, sys, tempfile, threading, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from txn_compat import cmd, enc, free_port  # noqa: E402


class Node:
    def __init__(self, binary, d, primary_repl=None, env=None):
        self.binary, self.d = binary, d
        self.port, self.repl, self.ws = free_port(), free_port(), free_port()
        self.primary_repl, self.extra = primary_repl, env or {}
        os.makedirs(d, exist_ok=True)

    def start(self):
        env = dict(os.environ)
        env.update({"RECACHED_BIND": "127.0.0.1", "RECACHED_PORT": str(self.port),
                    "RECACHED_WS_PORT": str(self.ws), "RECACHED_METRICS_PORT": "0",
                    "RECACHED_SAVE_PATH": os.path.join(self.d, "dump.rdb"),
                    "RECACHED_SAVE_INTERVAL": "0", "RECACHED_REPL_ENABLE": "1",
                    "RECACHED_REPL_PORT": str(self.repl)})
        env.update(self.extra)
        if self.primary_repl:
            env["RECACHED_REPLICAOF"] = f"127.0.0.1:{self.primary_repl}"
        self.log = open(os.path.join(self.d, "server.log"), "ab")
        self.p = subprocess.Popen([self.binary], cwd=self.d, env=env, stdout=self.log,
                                  stderr=self.log, stdin=subprocess.DEVNULL)
        for _ in range(200):
            try:
                if cmd(self.port, "PING") == "PONG":
                    return True
            except OSError:
                if self.p.poll() is not None:
                    return False
            time.sleep(0.05)
        return False

    def kill(self):
        if self.p.poll() is None:
            self.p.send_signal(signal.SIGKILL)
            self.p.wait()


def exec_tx(port, writes):
    with socket.create_connection(("127.0.0.1", port), timeout=10) as c:
        c.sendall(enc(["MULTI"]) + b"".join(enc(w) for w in writes) + enc(["EXEC"]))
        buf = b""
        deadline = time.time() + 5
        while buf.count(b"\r\n") < len(writes) + 3 and time.time() < deadline:
            buf += c.recv(65536)


def dump(port, keys):
    return [cmd(port, "GET", k) for k in keys]


RESULTS = []


def report(ok, label, case, detail):
    print(f"{'ok  ' if ok else 'FAIL'}  {label:16s} {case:44s} {detail}", flush=True)
    RESULTS.append(ok)


def run(label, primary_bin, replica_bin, atomic_expected, root):
    d = os.path.join(root, label.replace(">", "").replace("/", "-"))
    old_replica = "v0.5" in replica_bin or "v0.4" in replica_bin
    p = Node(primary_bin, os.path.join(d, "p"), env={"RECACHED_REPL_DEDUP": "off"} if old_replica else {})
    if not p.start():
        report(False, label, "primary starts", "")
        return
    r = Node(replica_bin, os.path.join(d, "r"), primary_repl=p.repl)
    if not r.start():
        report(False, label, "replica starts", "")
        p.kill()
        return
    cmd(p.port, "SET", "rc:seed", "1")
    for _ in range(200):
        if cmd(r.port, "GET", "rc:seed") == "1":
            break
        time.sleep(0.05)
    linked = cmd(r.port, "GET", "rc:seed") == "1"
    keys = [f"rc:k{i}" for i in range(8)]
    stop = threading.Event()
    mixed = []

    def reader():
        while not stop.is_set():
            try:
                vals = cmd(r.port, "MGET", *keys)
            except OSError:
                continue
            if isinstance(vals, list) and len(set(vals)) > 1:
                mixed.append(vals)

    t = threading.Thread(target=reader)
    t.start()
    cmd(p.port, "SET", "rc:x", "old", "PX", "50")
    time.sleep(0.1)
    for rnd in range(200):
        exec_tx(p.port, [["SET", k, str(rnd)] for k in keys])
        cmd(p.port, "SET", f"rc:single{rnd % 5}", str(rnd))
    exec_tx(p.port, [["APPEND", "rc:x", "new"], ["INCR", "rc:n"]])
    time.sleep(0.5)
    stop.set()
    t.join()
    watch = keys + [f"rc:single{i}" for i in range(5)] + ["rc:x", "rc:n"]
    want = dump(p.port, watch)
    got = None
    for _ in range(100):
        got = dump(r.port, watch)
        if got == want:
            break
        time.sleep(0.05)
    converged = got == want
    report(linked and converged, label, "links, follows and converges",
           f"linked={linked} converged={converged} {'' if converged else (want, got)}")
    if atomic_expected:
        report(not mixed, label, "replica readers never see part of an EXEC",
               f"part-applied MGETs seen: {len(mixed)}")
    else:
        report(True, label, "replica readers (per-frame replica, recorded)",
               f"part-applied MGETs seen: {len(mixed)} (an older replica applies frame by frame)")
    r.kill()
    p.kill()


def main():
    root = tempfile.mkdtemp(prefix="rc6oi-repl-", dir=os.environ.get("TXN_TMP"))
    try:
        for spec in sys.argv[1:]:
            label, primary, replica, atomic = spec.split("|")
            run(label, primary, replica, atomic == "atomic", root)
    finally:
        shutil.rmtree(root, ignore_errors=True)
    print(f"{sum(RESULTS)}/{len(RESULTS)} as expected")
    sys.exit(0 if all(RESULTS) else 1)


if __name__ == "__main__":
    main()
