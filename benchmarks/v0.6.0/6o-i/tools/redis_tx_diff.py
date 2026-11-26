#!/usr/bin/env python3
"""6O-I: the brief's transaction scenarios against Redis 7.2.5.

Each scenario runs its setup, then one MULTI/EXEC, on Redis 7.2.5
(appendonly yes, appendfsync always) and on a Recached binary (AOF always).
Compared: the EXEC reply, the keyspace after it, and the keyspace after each
server restarts from its AOF (SIGKILL, then start on the same files).

usage: redis_tx_diff.py REDIS_SERVER RECACHED_BINARY
"""
import os, shutil, signal, subprocess, sys, tempfile, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from txn_compat import Server, cmd, enc, free_port  # noqa: E402
import socket

SCENARIOS = [
    ("two SETs", [], [["SET", "a", "1"], ["SET", "b", "2"]]),
    ("SET then GET", [], [["SET", "a", "1"], ["GET", "a"]]),
    ("MSET then SET", [], [["MSET", "a", "1", "b", "2"], ["SET", "a", "3"]]),
    ("repeated writes to one key", [], [["SET", "a", "1"], ["APPEND", "a", "2"], ["INCR", "n"], ["INCR", "n"]]),
    ("duplicate keys", [], [["MSET", "a", "1", "a", "2"], ["DEL", "a", "a"]]),
    ("RENAME", [["SET", "a", "1"]], [["RENAME", "a", "b"], ["SET", "a", "2"]]),
    ("COPY", [["SET", "a", "1"]], [["COPY", "a", "b"], ["APPEND", "b", "x"]]),
    ("DEL", [["SET", "a", "1"], ["SET", "b", "1"]], [["DEL", "a"], ["DEL", "b", "c"]]),
    ("expiration and PERSIST", [], [["SET", "a", "1", "EX", "1000"], ["PERSIST", "a"], ["SET", "b", "1"], ["EXPIRE", "b", "1000"]]),
    ("a failing command inside EXEC", [["SET", "s", "text"]], [["SET", "a", "1"], ["INCR", "s"], ["SET", "b", "1"]]),
    ("SET NX", [["SET", "a", "old"]], [["SET", "a", "new", "NX"], ["SET", "b", "new", "NX"]]),
    ("WRONGTYPE", [["LPUSH", "l", "x"]], [["SET", "a", "1"], ["GET", "l"], ["SET", "b", "1"]]),
    ("FLUSHDB", [["SET", "x", "1"]], [["SET", "a", "1"], ["FLUSHDB"], ["SET", "b", "1"]]),
    ("read-only EXEC", [["SET", "a", "1"]], [["GET", "a"], ["EXISTS", "a"]]),
    ("PUBLISH-only EXEC", [], [["PUBLISH", "ch", "m"]]),
    ("no generated writes", [], [["DEL", "nope"], ["GET", "nope"]]),
    ("lists, sets, hashes", [], [["RPUSH", "l", "1", "2"], ["SADD", "s", "x"], ["HSET", "h", "f", "v"], ["LPOP", "l"]]),
]


class Redis:
    def __init__(self, binary, d):
        self.binary, self.d, self.port = binary, d, free_port()

    def start(self):
        self.p = subprocess.Popen([self.binary, "--port", str(self.port), "--bind", "127.0.0.1",
                                   "--dir", self.d, "--appendonly", "yes", "--appendfsync", "always",
                                   "--save", "", "--protected-mode", "no"],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(100):
            try:
                if cmd(self.port, "PING") == "PONG":
                    return True
            except OSError:
                time.sleep(0.05)
        return False

    def kill9(self):
        self.p.send_signal(signal.SIGKILL)
        self.p.wait()


def run_exec(port, writes):
    """MULTI, the writes, EXEC on one connection; the EXEC reply."""
    from txn_compat import parse
    with socket.create_connection(("127.0.0.1", port), timeout=10) as c:
        c.sendall(enc(["MULTI"]) + b"".join(enc(w) for w in writes) + enc(["EXEC"]))
        buf, replies = b"", []
        deadline = time.time() + 5
        while len(replies) < len(writes) + 2 and time.time() < deadline:
            buf += c.recv(65536)
            while True:
                try:
                    v, rest = parse(buf)
                except (IndexError, ValueError):
                    break
                replies.append(v)
                buf = rest
        return replies[-1] if replies else None


def state(port):
    keys = sorted(cmd(port, "KEYS", "*") or [])
    out = {}
    for k in keys:
        t = cmd(port, "TYPE", k)
        v = {"string": lambda: cmd(port, "GET", k),
             "list": lambda: cmd(port, "LRANGE", k, "0", "-1"),
             "set": lambda: sorted(cmd(port, "SMEMBERS", k) or []),
             "hash": lambda: sorted(cmd(port, "HGETALL", k) or [])}.get(t, lambda: None)()
        ttl = cmd(port, "PTTL", k)
        out[k] = (t, v, "none" if ttl == -1 else "ttl")
    return out


def norm(reply):
    """Error replies compared by their first word (Redis and Recached word
    the rest differently); integers and strings as they are."""
    if isinstance(reply, list):
        return [norm(r) for r in reply]
    if isinstance(reply, str) and reply.startswith("ERR:"):
        return "ERR:" + reply[4:].split()[0]
    return reply


def main():
    redis_bin, rc_bin = sys.argv[1:3]
    root = tempfile.mkdtemp(prefix="rc6oi-rdiff-", dir=os.environ.get("TXN_TMP"))
    ok_all = []
    try:
        for i, (name, setup, writes) in enumerate(SCENARIOS):
            res = {}
            for kind in ("redis", "recached"):
                d = os.path.join(root, f"{i}-{kind}")
                os.makedirs(d)
                s = Redis(redis_bin, d) if kind == "redis" else Server(rc_bin, d)
                assert s.start(), kind
                for args in setup:
                    cmd(s.port, *args)
                reply = run_exec(s.port, writes)
                time.sleep(0.05)
                before = state(s.port)
                s.kill9()
                assert s.start(), kind
                after = state(s.port)
                s.kill9()
                res[kind] = (norm(reply), before, after)
            r, c = res["redis"], res["recached"]
            same = r == c
            ok_all.append(same)
            print(f"{'ok  ' if same else 'DIFF'}  {name:34s} reply={c[0]!r} "
                  f"keys={len(c[1])} restart_same={c[1] == c[2]} "
                  f"{'' if same else f'redis={r!r} recached={c!r}'}", flush=True)
    finally:
        shutil.rmtree(root, ignore_errors=True)
    print(f"{sum(ok_all)}/{len(ok_all)} scenarios identical to Redis 7.2.5 (reply, state, state after AOF restart)")
    sys.exit(0 if all(ok_all) else 1)


if __name__ == "__main__":
    main()
