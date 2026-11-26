#!/usr/bin/env python3
"""6O-H: a replica applying its primary's frames late, Redis 7.2.5 against
Recached builds. The replica is paused (SIGSTOP) before the writes and
resumed after their deadlines, so it applies the backlog late. Then it is
read, promoted, and restarted from its own files alone.

usage: replica_diff.py NAME=BINARY ...     (NAME 'redis' is Redis)
"""
import os, shutil, signal, subprocess, sys, tempfile, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from expiry_restart_diff import cmd, free_port, ttl_class  # noqa: E402


class Node:
    def __init__(self, name, binary, d, primary_repl=None, extra=None):
        self.name, self.binary, self.d = name, binary, d
        self.extra = extra or {}
        self.port, self.repl = free_port(), free_port()
        self.primary_repl = primary_repl
        self.p = None

    def start(self, standalone=False):
        os.makedirs(self.d, exist_ok=True)
        if self.name == "redis":
            argv = [self.binary, "--port", str(self.port), "--bind", "127.0.0.1",
                    "--dir", self.d, "--appendonly", "yes", "--appendfsync", "always",
                    "--save", "", "--appenddirname", "aof"]
            if self.primary_repl and not standalone:
                argv += ["--replicaof", "127.0.0.1", str(self.primary_repl)]
            env = None
        else:
            argv = [self.binary]
            env = {"RECACHED_BIND": "127.0.0.1", "RECACHED_PORT": str(self.port),
                   "RECACHED_WS_PORT": str(free_port()), "RECACHED_METRICS_PORT": "0",
                   "RECACHED_SAVE_PATH": os.path.join(self.d, "dump.rdb"),
                   "RECACHED_AOF_PATH": os.path.join(self.d, "append.aof"),
                   "RECACHED_AOF_SYNC": "always", "RECACHED_SAVE_INTERVAL": "0",
                   "RECACHED_REPL_ENABLE": "1", "RECACHED_REPL_PORT": str(self.repl)}
            if self.primary_repl and not standalone:
                env["RECACHED_REPLICAOF"] = f"127.0.0.1:{self.primary_repl}"
            env.update(self.extra)
        log = open(os.path.join(self.d, "server.log"), "ab")
        self.p = subprocess.Popen(argv, cwd=self.d, env=env, stdout=log, stderr=log)
        for _ in range(400):
            if self.p.poll() is not None:
                return False
            try:
                if cmd(self.port, "PING") == "PONG":
                    return True
            except OSError:
                time.sleep(0.025)
        return False

    @property
    def repl_port(self):
        return self.port if self.name == "redis" else self.repl

    def kill(self):
        if self.p and self.p.poll() is None:
            self.p.send_signal(signal.SIGCONT)
            self.p.send_signal(signal.SIGKILL)
            self.p.wait()


def wait_key(port, key, want, timeout=10):
    end = time.time() + timeout
    while time.time() < end:
        try:
            if cmd(port, "GET", key) == want:
                return True
        except OSError:
            pass
        time.sleep(0.05)
    return False


def run(name, binary):
    # NAME=PRIMARY[,REPLICA]: a mixed-version pair when two binaries are given.
    # PRIMARY_ENV="K=V,..." is passed to the primary (e.g. RECACHED_REPL_DEDUP=off
    # for a 0.5 replica).
    pbin, _, rbin = binary.partition(",")
    rbin = rbin or pbin
    extra = dict(kv.split("=", 1) for kv in os.environ.get("PRIMARY_ENV", "").split(",") if kv)
    root = tempfile.mkdtemp(prefix=f"6oh-repl-{name}-", dir=os.environ.get("DIFF_TMP"))
    p = Node(name, pbin, os.path.join(root, "p"), extra=extra)
    try:
        assert p.start(), "primary"
        r = Node(name, rbin, os.path.join(root, "r"), primary_repl=p.repl_port)
        assert r.start(), "replica"
        cmd(p.port, "SET", "ready", "1")
        assert wait_key(r.port, "ready", "1"), "replica never synced"
        r.p.send_signal(signal.SIGSTOP)
        # While it is paused: the 6O-F timelines.
        for c in (["SET", "c", "1", "PX", "3000"], ["INCR", "c"],
                  ["SET", "p", "1", "PX", "3000"], ["INCR", "p"], ["PERSIST", "p"],
                  ["SET", "s", "old", "PX", "3000"], ["APPEND", "s", "+"],
                  ["RPUSH", "l", "a"], ["PEXPIRE", "l", "3000"], ["LSET", "l", "0", "z"],
                  ["SET", "x", "1", "PX", "3000"], ["INCR", "x"], ["PEXPIRE", "x", "60000"],
                  ["SET", "done", "1"]):
            cmd(p.port, *c)
        time.sleep(3.3)
        r.p.send_signal(signal.SIGCONT)
        assert wait_key(r.port, "done", "1"), "replica never caught up"
        out = {}
        out["replica reads"] = [cmd(r.port, "GET", k) for k in ("c", "p", "s", "x")] + \
            [cmd(r.port, "LRANGE", "l", "0", "-1"), ttl_class(cmd(r.port, "PTTL", "p"))]
        # Promoted: what it serves as a primary, and after a restart from its
        # own files alone.
        cmd(r.port, "REPLICAOF", "NO", "ONE")
        time.sleep(0.2)
        out["promoted"] = [cmd(r.port, "GET", k) for k in ("c", "p", "s", "x")] + \
            [ttl_class(cmd(r.port, "PTTL", "x"))]
        cmd(r.port, "INCR", "c")  # a write on the promoted node
        r.kill()
        up = r.start(standalone=True)
        out["restarted"] = ([cmd(r.port, "GET", k) for k in ("c", "p", "s", "x")]
                            + [ttl_class(cmd(r.port, "PTTL", "c"))]) if up else ["RESTART REFUSED"]
        r.kill()
        return out
    finally:
        p.kill()
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    rows = {n: run(n, b) for n, b in (a.split("=", 1) for a in sys.argv[1:])}
    names = list(rows)
    print("| check | " + " | ".join(names) + " |")
    print("|---|" + "---|" * len(names))
    for k in next(iter(rows.values())):
        ref = rows.get("redis", {}).get(k)
        cells = [f"{rows[n][k]}" + ("" if n == "redis" or ref is None else
                                     (" ✓" if rows[n][k] == ref else " ✗")) for n in names]
        print(f"| {k} | " + " | ".join(cells) + " |")
