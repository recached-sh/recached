#!/usr/bin/env python3
"""6O-H: expiring keys through SIGKILL restarts, Redis 7.2.5 against Recached
builds, end to end. Each server runs with an AOF and fsync always (Recached
also with a snapshot path; SAVE is used where a scenario says so). A scenario
is a list of steps; `restart` SIGKILLs the server and starts it again on the
same files. Every check's reply is recorded (PTTL as -2 / -1 / "live"), and a
server that fails to restart is recorded as such.

usage: expiry_restart_diff.py NAME=BINARY ...     (NAME 'redis' is Redis)
       env SCENARIOS=a,b to run a subset
"""
import os, shutil, signal, socket, subprocess, sys, tempfile, time


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def enc(args):
    out = b"*%d\r\n" % len(args)
    for a in args:
        a = a.encode() if isinstance(a, str) else a
        out += b"$%d\r\n%s\r\n" % (len(a), a)
    return out


def parse(buf):
    t, rest = buf[:1], buf[1:]
    line, sep, after = rest.partition(b"\r\n")
    if not sep:
        raise IndexError
    if t == b"+":
        return line.decode(), after
    if t == b"-":
        return "ERR", after
    if t == b":":
        return int(line), after
    if t == b"$":
        n = int(line)
        if n < 0:
            return None, after
        if len(after) < n + 2:
            raise IndexError
        return after[:n].decode(), after[n + 2:]
    if t == b"*":
        n = int(line)
        if n < 0:
            return None, after
        items = []
        for _ in range(n):
            v, after = parse(after)
            items.append(v)
        return items, after
    raise ValueError(buf[:40])


def cmd(port, *args):
    with socket.create_connection(("127.0.0.1", port), timeout=10) as s:
        s.sendall(enc(args))
        buf = b""
        while True:
            chunk = s.recv(65536)
            if not chunk:
                raise ConnectionError
            buf += chunk
            try:
                return parse(buf)[0]
            except IndexError:
                continue


def pipeline(port, *commands):
    """Several commands on one connection (MULTI/EXEC)."""
    with socket.create_connection(("127.0.0.1", port), timeout=10) as s:
        s.sendall(b"".join(enc(c) for c in commands))
        buf, out = b"", []
        while len(out) < len(commands):
            chunk = s.recv(65536)
            if not chunk:
                raise ConnectionError
            buf += chunk
            while len(out) < len(commands):
                try:
                    v, buf = parse(buf)
                    out.append(v)
                except IndexError:
                    break
        return out


class Server:
    def __init__(self, name, binary, d):
        self.name, self.binary, self.d = name, binary, d
        self.port = free_port()
        self.p = None

    def start(self):
        if self.name == "redis":
            argv = [self.binary, "--port", str(self.port), "--bind", "127.0.0.1",
                    "--dir", self.d, "--appendonly", "yes", "--appendfsync", "always",
                    "--save", "", "--appenddirname", "aof"]
            env = None
        else:
            argv = [self.binary]
            env = {"RECACHED_BIND": "127.0.0.1", "RECACHED_PORT": str(self.port),
                   "RECACHED_WS_PORT": str(free_port()), "RECACHED_METRICS_PORT": "0",
                   "RECACHED_SAVE_PATH": os.path.join(self.d, "dump.rdb"),
                   "RECACHED_AOF_PATH": os.path.join(self.d, "append.aof"),
                   "RECACHED_AOF_SYNC": "always", "RECACHED_SAVE_INTERVAL": "0"}
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

    def kill(self):
        if self.p and self.p.poll() is None:
            self.p.send_signal(signal.SIGKILL)
            self.p.wait()


def ttl_class(v):
    if isinstance(v, int) and v >= 0:
        return "live"
    return v


# Steps: ("c", *args) run; ("tx", [args], ...) MULTI/EXEC; ("sleep", ms);
# ("restart",); ("check", *args) record the reply; ("ttl", key) record the
# PTTL class.
S = {
    "fixed-window": [("c", "SET", "c", "1", "PX", "400"), ("c", "INCR", "c"), ("c", "INCR", "c"),
                     ("restart",), ("check", "GET", "c"), ("ttl", "c"),
                     ("sleep", 500), ("restart",), ("check", "GET", "c"), ("ttl", "c")],
    "sliding-window": [("c", "INCR", "c"), ("c", "PEXPIRE", "c", "600"), ("sleep", 200),
                       ("c", "INCR", "c"), ("c", "PEXPIRE", "c", "600"), ("sleep", 200),
                       ("c", "INCR", "c"), ("c", "PEXPIRE", "c", "600"), ("sleep", 450),
                       ("restart",), ("check", "GET", "c"), ("ttl", "c"),
                       ("sleep", 300), ("restart",), ("check", "GET", "c")],
    "incr-then-extend": [("c", "SET", "c", "1", "PX", "300"), ("c", "INCR", "c"),
                         ("c", "PEXPIRE", "c", "5000"), ("sleep", 400), ("restart",),
                         ("check", "GET", "c"), ("ttl", "c")],
    "incr-then-persist": [("c", "SET", "c", "1", "PX", "300"), ("c", "INCR", "c"),
                          ("c", "PERSIST", "c"), ("sleep", 400), ("restart",),
                          ("check", "GET", "c"), ("ttl", "c")],
    "incr-then-rename": [("c", "SET", "c", "1", "PX", "300"), ("c", "INCR", "c"),
                         ("c", "RENAME", "c", "d"), ("sleep", 400), ("restart",),
                         ("check", "GET", "d"), ("check", "EXISTS", "c")],
    "recreated-after-expiry": [("c", "SET", "c", "5", "PX", "200"), ("sleep", 300),
                               ("c", "INCR", "c"), ("restart",), ("check", "GET", "c"),
                               ("ttl", "c")],
    "counter-in-multi": [("c", "SET", "c", "1", "PX", "300"),
                         ("tx", ["INCR", "c"], ["INCR", "c"]), ("sleep", 400), ("restart",),
                         ("check", "GET", "c")],
    "multi-after-expiry": [("c", "SET", "c", "1", "PX", "200"), ("sleep", 300),
                           ("tx", ["INCR", "c"], ["PEXPIRE", "c", "5000"]), ("restart",),
                           ("check", "GET", "c"), ("ttl", "c")],
    "append-live": [("c", "SET", "s", "old", "PX", "300"), ("c", "APPEND", "s", "+"),
                    ("sleep", 400), ("restart",), ("check", "GET", "s")],
    "append-after-expiry": [("c", "SET", "s", "old", "PX", "200"), ("sleep", 300),
                            ("c", "APPEND", "s", "new"), ("restart",), ("check", "GET", "s"),
                            ("ttl", "s")],
    "hincrby-live": [("c", "HSET", "h", "f", "1"), ("c", "PEXPIRE", "h", "300"),
                     ("c", "HINCRBY", "h", "f", "2"), ("sleep", 400), ("restart",),
                     ("check", "HGETALL", "h")],
    "hincrby-after-expiry": [("c", "HSET", "h", "f", "1", "g", "x"), ("c", "PEXPIRE", "h", "200"),
                             ("sleep", 300), ("c", "HINCRBY", "h", "f", "2"), ("restart",),
                             ("check", "HGETALL", "h"), ("ttl", "h")],
    "rpush-after-expiry": [("c", "RPUSH", "l", "a", "b"), ("c", "PEXPIRE", "l", "200"),
                           ("sleep", 300), ("c", "RPUSH", "l", "z"), ("restart",),
                           ("check", "LRANGE", "l", "0", "-1"), ("ttl", "l")],
    "lpush-live": [("c", "RPUSH", "l", "a"), ("c", "PEXPIRE", "l", "300"),
                   ("c", "LPUSH", "l", "z"), ("sleep", 400), ("restart",),
                   ("check", "LRANGE", "l", "0", "-1")],
    "sadd-after-expiry": [("c", "SADD", "s", "a", "b"), ("c", "PEXPIRE", "s", "200"),
                          ("sleep", 300), ("c", "SADD", "s", "z"), ("restart",),
                          ("check", "SMEMBERS", "s"), ("ttl", "s")],
    "zadd-after-expiry": [("c", "ZADD", "z", "1", "a", "2", "b"), ("c", "PEXPIRE", "z", "200"),
                          ("sleep", 300), ("c", "ZADD", "z", "9", "q"), ("restart",),
                          ("check", "ZRANGE", "z", "0", "-1"), ("ttl", "z")],
    "lset-volatile": [("c", "RPUSH", "l", "a", "b"), ("c", "PEXPIRE", "l", "300"),
                      ("c", "LSET", "l", "0", "z"), ("sleep", 400), ("restart",),
                      ("check", "LRANGE", "l", "0", "-1")],
    "rename-volatile-source": [("c", "SET", "a", "v", "PX", "300"), ("c", "RENAME", "a", "b"),
                               ("sleep", 400), ("restart",), ("check", "GET", "b")],
    # Deadlines well past SAVE's duration (an fsync on a slow disk), so the
    # writes after it land before them on every server.
    "snapshot-then-aof": [("c", "SET", "c", "1", "PX", "3000"), ("c", "INCR", "c"),
                          ("c", "SET", "s", "old", "PX", "3000"), ("c", "SAVE"),
                          ("c", "INCR", "c"), ("c", "APPEND", "s", "+"), ("sleep", 3200),
                          ("restart",), ("check", "GET", "c"), ("check", "GET", "s"),
                          ("c", "INCR", "c"), ("restart",), ("check", "GET", "c"), ("ttl", "c")],
    "multiple-restarts": [("c", "SET", "s", "a", "PX", "300"), ("restart",),
                          ("c", "APPEND", "s", "b"), ("restart",), ("sleep", 400),
                          ("c", "APPEND", "s", "c"), ("restart",), ("check", "GET", "s"),
                          ("ttl", "s"), ("restart",), ("check", "GET", "s")],
}


def run(name, binary, steps):
    d = tempfile.mkdtemp(prefix=f"6oh-diff-{name}-", dir=os.environ.get("DIFF_TMP"))
    s = Server(name, binary, d)
    out = []
    try:
        if not s.start():
            return ["did not start"]
        for step in steps:
            kind = step[0]
            if kind == "c":
                cmd(s.port, *step[1:])
            elif kind == "tx":
                pipeline(s.port, ["MULTI"], *step[1:], ["EXEC"])
            elif kind == "sleep":
                time.sleep(step[1] / 1000)
            elif kind == "restart":
                s.kill()
                if not s.start():
                    out.append("RESTART REFUSED")
                    return out
            elif kind == "check":
                v = cmd(s.port, *step[1:])
                out.append(sorted(v) if step[1] in ("SMEMBERS", "HGETALL") and v else v)
            elif kind == "ttl":
                out.append(ttl_class(cmd(s.port, "PTTL", step[1])))
        return out
    finally:
        s.kill()
        shutil.rmtree(d, ignore_errors=True)


if __name__ == "__main__":
    servers = [a.split("=", 1) for a in sys.argv[1:]]
    only = os.environ.get("SCENARIOS")
    names = only.split(",") if only else list(S)
    print("| scenario | " + " | ".join(n for n, _ in servers) + " |")
    print("|---|" + "---|" * len(servers))
    differs = 0
    for sc in names:
        row = {n: run(n, b, S[sc]) for n, b in servers}
        ref = row.get("redis")
        cells = []
        for n, _ in servers:
            mark = "" if ref is None or n == "redis" else (" ✓" if row[n] == ref else " ✗")
            if ref is not None and n != "redis" and row[n] != ref:
                differs += 1
            cells.append(f"{row[n]}{mark}")
        print(f"| {sc} | " + " | ".join(cells) + " |", flush=True)
    print(f"\n{differs} cell(s) differ from redis")
