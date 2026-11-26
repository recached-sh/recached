#!/usr/bin/env python3
"""6O-I (review B R2-L3): does the 6O-I binary start on, and replay exactly,
the AOF every older binary writes for every write command?

For each older binary: a fresh server with `appendfsync always` runs a corpus
of write commands (each attempted; a command an older release does not know
just errors), the keyspace it serves is recorded (every key's type and value,
and its TTL class), it is SIGKILLed, and the 6O-I binary is started on the
same files. Each key must read back the same, and the 6O-I binary must start
(the nested-record refusal, D4, must never refuse a file an older release
wrote). Then the same corpus inside MULTI/EXEC, and on the 6O-I binary itself
(its own grouped file).

usage: corpus_compat.py NEW=BINARY NAME=BINARY ...
"""
import os, shutil, signal, socket, subprocess, sys, tempfile, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from txn_compat import Server, cmd  # noqa: E402

FAR = str(int(time.time()) + 86400)
CORPUS = [
    ["SET", "s:plain", "v"], ["SET", "s:px", "v", "PX", "86400000"], ["SET", "s:ex", "v", "EX", "86400"],
    ["SET", "s:exat", "v", "EXAT", FAR], ["SET", "s:nx", "v", "NX"], ["SET", "s:plain", "w", "XX"],
    ["SET", "s:get", "a"], ["SET", "s:get", "b", "GET"], ["SETNX", "s:setnx", "v"], ["SETEX", "s:setex", "86400", "v"],
    ["PSETEX", "s:psetex", "86400000", "v"], ["MSET", "m:a", "1", "m:b", "2"], ["MSETNX", "m:c", "3", "m:d", "4"],
    ["APPEND", "s:app", "x"], ["APPEND", "s:app", "y"], ["INCR", "n:i"], ["INCRBY", "n:i", "5"], ["DECR", "n:d"],
    ["DECRBY", "n:d", "3"], ["INCRBYFLOAT", "n:f", "1.5"], ["SETRANGE", "s:range", "2", "xy"], ["GETSET", "s:gs", "v"],
    ["GETDEL", "s:get"], ["GETEX", "s:plain", "PX", "86400000"], ["SET", "s:persist", "v", "EX", "86400"],
    ["PERSIST", "s:persist"], ["SET", "s:exp", "v"], ["EXPIRE", "s:exp", "86400"], ["PEXPIRE", "s:exp", "86400000"],
    ["EXPIREAT", "s:exp", FAR], ["SET", "s:del", "v"], ["DEL", "s:del"], ["SET", "s:unl", "v"], ["UNLINK", "s:unl"],
    ["SET", "s:ren", "v"], ["RENAME", "s:ren", "s:ren2"], ["SET", "s:rnx", "v"], ["RENAMENX", "s:rnx", "s:rnx2"],
    ["COPY", "s:plain", "s:copy"],
    ["HSET", "h:a", "f1", "v1", "f2", "v2"], ["HSETNX", "h:a", "f3", "v3"], ["HMSET", "h:b", "f", "v"],
    ["HINCRBY", "h:a", "n", "2"], ["HINCRBYFLOAT", "h:a", "fl", "0.5"], ["HDEL", "h:a", "f2"],
    ["LPUSH", "l:a", "1", "2"], ["RPUSH", "l:a", "3"], ["LPUSHX", "l:a", "0"], ["RPUSHX", "l:a", "4"],
    ["LSET", "l:a", "0", "z"], ["LINSERT", "l:a", "BEFORE", "3", "2.5"], ["LREM", "l:a", "1", "2"],
    ["LTRIM", "l:a", "0", "10"], ["LPOP", "l:a"], ["RPOP", "l:a"], ["RPUSH", "l:b", "x", "y"],
    ["LMOVE", "l:b", "l:c", "LEFT", "RIGHT"], ["RPOPLPUSH", "l:b", "l:c"],
    ["SADD", "t:a", "x", "y", "z"], ["SREM", "t:a", "z"], ["SADD", "t:b", "y", "w"], ["SMOVE", "t:a", "t:b", "x"],
    ["SINTERSTORE", "t:i", "t:a", "t:b"], ["SUNIONSTORE", "t:u", "t:a", "t:b"], ["SDIFFSTORE", "t:d", "t:b", "t:a"],
    ["SPOP", "t:u"],
    ["ZADD", "z:a", "1", "a", "2", "b", "3", "c"], ["ZINCRBY", "z:a", "5", "a"], ["ZREM", "z:a", "c"],
    ["ZPOPMIN", "z:a"], ["ZADD", "z:b", "1", "x"],
    ["ESET", "e:a", "v"], ["EADD", "e:s", "m"],
]


def snapshot(port):
    """Every key's type, value and TTL class, read through ordinary commands."""
    keys = sorted(cmd(port, "KEYS", "*") or [])
    out = {}
    for k in keys:
        t = cmd(port, "TYPE", k)
        if t == "string":
            v = cmd(port, "GET", k)
        elif t == "hash":
            flat = cmd(port, "HGETALL", k) or []
            v = sorted(zip(flat[::2], flat[1::2]))
        elif t == "list":
            v = cmd(port, "LRANGE", k, "0", "-1")
        elif t == "set":
            v = sorted(cmd(port, "SMEMBERS", k) or [])
        elif t == "zset":
            v = cmd(port, "ZRANGE", k, "0", "-1", "WITHSCORES")
        else:
            v = None
        pttl = cmd(port, "PTTL", k)
        out[k] = (t, v, "none" if pttl == -1 else "ttl" if isinstance(pttl, int) and pttl > 0 else pttl)
    return out


RESULTS = []


def report(ok, name, case, detail):
    print(f"{'ok  ' if ok else 'FAIL'}  {name:14s} {case:40s} {detail}", flush=True)
    RESULTS.append(ok)


def run(new_bin, name, binary, root, in_exec):
    case = "corpus in MULTI/EXEC" if in_exec else "corpus, one by one"
    d = os.path.join(root, f"{name}-{'exec' if in_exec else 'plain'}")
    os.makedirs(d)
    s = Server(binary, d)
    if not s.start():
        report(False, name, case, f"older binary did not start: {s.tail()}")
        return
    errors = 0
    if in_exec:
        # Several transactions, each a handful of commands, over one connection.
        with socket.create_connection(("127.0.0.1", s.port), timeout=10) as c:
            from txn_compat import enc
            for i in range(0, len(CORPUS), 6):
                batch = [enc(["MULTI"])] + [enc(a) for a in CORPUS[i:i + 6]] + [enc(["EXEC"])]
                c.sendall(b"".join(batch))
                time.sleep(0.05)
            time.sleep(0.3)
        # Ephemeral keys belong to their connection: let the server's
        # disconnect cleanup remove them before recording what it serves.
        time.sleep(1.0)
    else:
        for args in CORPUS:
            try:
                r = cmd(s.port, *args)
                if isinstance(r, str) and r.startswith("ERR:"):
                    errors += 1
            except OSError:
                errors += 1
    served = snapshot(s.port)
    s.kill9()
    # The older binary on its own file, in a copy: what its own restart
    # recovers, to tell a 6O-I difference from one the older release has
    # with itself.
    own = d + "-own"
    shutil.copytree(d, own, ignore=shutil.ignore_patterns("server.log"))
    o = Server(binary, own)
    self_recovered = snapshot(o.port) if o.start() else None
    o.kill9()
    n = Server(new_bin, d)
    up = n.start()
    recovered = snapshot(n.port) if up else None
    n.kill9()
    diff = [] if recovered is None else [
        k for k in sorted(set(served) | set(recovered)) if served.get(k) != recovered.get(k)]
    detail = [(k, served.get(k), recovered.get(k)) for k in diff[:3]] if diff else []
    # A difference the older binary also has with itself is the older
    # release's own (pre-existing), not 6O-I's: recorded, not counted.
    own_too = bool(diff) and recovered == self_recovered
    report(up and (not diff or own_too), name, case,
           f"keys={len(served)} refused-by-older={errors} 6O-I up={up} "
           f"6O-I==older's own restart: {recovered == self_recovered} "
           f"{'PRE-EXISTING (older binary differs from itself) ' if own_too else ''}differing={detail} "
           f"{'' if up else n.tail()}")


def main():
    pairs = [a.split("=", 1) for a in sys.argv[1:]]
    new_bin = dict(pairs)["NEW"]
    root = tempfile.mkdtemp(prefix="rc6oi-corpus-", dir=os.environ.get("TXN_TMP"))
    try:
        for name, binary in pairs:
            for in_exec in (False, True):
                run(new_bin, name, binary, root, in_exec)
    finally:
        shutil.rmtree(root, ignore_errors=True)
    print(f"{sum(RESULTS)}/{len(RESULTS)} as expected")
    sys.exit(0 if all(RESULTS) else 1)


if __name__ == "__main__":
    main()
