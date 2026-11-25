#!/usr/bin/env python3
"""6O-F F0: the expiring-counter timeline against Redis 7.2.5 and Recached
builds, end to end. Each server is started with an AOF and fsync always.
The counter is written with a deadline a little under a second ahead and
counted while live. The server is then SIGKILLed (no shutdown snapshot), and
restarted once the deadline has passed. A correct server has no key. Also
checks a long-deadline counter (exact deadline), a persistent counter, and
the INCR + EXPIRE idiom.

usage: counter_restart_diff.py NAME=BINARY ...   (NAME 'redis' is Redis)
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
    line, _, after = rest.partition(b"\r\n")
    if t in (b"+", b"-"):
        return line.decode(), after
    if t == b":":
        return int(line), after
    if t == b"$":
        n = int(line)
        if n < 0:
            return None, after
        return after[:n].decode(), after[n + 2:]
    raise ValueError(buf)


def cmd(port, *args):
    with socket.create_connection(("127.0.0.1", port), timeout=5) as s:
        s.sendall(enc(args))
        buf = b""
        while True:
            buf += s.recv(65536)
            try:
                v, _ = parse(buf)
                return v
            except (ValueError, IndexError):
                continue


def start(name, binary, d, port):
    if name == "redis":
        argv = [binary, "--port", str(port), "--bind", "127.0.0.1", "--dir", d,
                "--appendonly", "yes", "--appendfsync", "always", "--save", "",
                "--appenddirname", "aof"]
        env = None
    else:
        argv = [binary]
        env = {"RECACHED_BIND": "127.0.0.1", "RECACHED_PORT": str(port),
               "RECACHED_WS_PORT": str(free_port()), "RECACHED_METRICS_PORT": "0",
               "RECACHED_SAVE_PATH": os.path.join(d, "dump.rdb"),
               "RECACHED_AOF_PATH": os.path.join(d, "append.aof"),
               "RECACHED_AOF_SYNC": "always", "RECACHED_SAVE_INTERVAL": "0"}
    p = subprocess.Popen(argv, cwd=d, env=env, stdout=subprocess.DEVNULL,
                         stderr=subprocess.DEVNULL)
    for _ in range(400):
        try:
            if cmd(port, "PING") == "PONG":
                return p
        except OSError:
            time.sleep(0.025)
    raise RuntimeError(f"{name} did not start")


def run(name, binary):
    # NAME=WRITER,READER: the first binary writes, the second replays.
    writer, _, reader = binary.partition(",")
    reader = reader or writer
    d = tempfile.mkdtemp(prefix=f"6of-diff-{name}-")
    port = free_port()
    p = start(name, writer, d, port)
    now = lambda: int(time.time() * 1000)
    short = now() + 800
    cmd(port, "SET", "short", "5", "PXAT", str(short))
    a = [cmd(port, "INCR", "short"), cmd(port, "INCRBY", "short", "4")]
    long_ = now() + 3_600_000
    cmd(port, "SET", "long", "1", "PXAT", str(long_))
    a.append(cmd(port, "DECRBY", "long", "3"))
    a.append(cmd(port, "INCR", "plain"))
    a.append(cmd(port, "INCR", "window"))
    cmd(port, "EXPIRE", "window", "3600")
    a.append(cmd(port, "INCR", "window"))
    window = cmd(port, "PEXPIRETIME", "window")
    assert now() < short, "too slow"
    p.send_signal(signal.SIGKILL)
    p.wait()
    while now() <= short + 50:
        time.sleep(0.02)
    p = start(name, reader, d, port)
    out = {
        "live replies": a,
        "short GET/PTTL": (cmd(port, "GET", "short"), cmd(port, "PTTL", "short")),
        "long GET/exact deadline": (cmd(port, "GET", "long"),
                                    cmd(port, "PEXPIRETIME", "long") == long_),
        "plain GET/PTTL": (cmd(port, "GET", "plain"), cmd(port, "PTTL", "plain")),
        "window GET/exact deadline": (cmd(port, "GET", "window"),
                                      cmd(port, "PEXPIRETIME", "window") == window),
    }
    p.send_signal(signal.SIGKILL)
    p.wait()
    shutil.rmtree(d, ignore_errors=True)
    return out


if __name__ == "__main__":
    results = {}
    for spec in sys.argv[1:]:
        name, binary = spec.split("=", 1)
        results[name] = run(name, binary)
    keys = list(next(iter(results.values())).keys())
    print("| check | " + " | ".join(results) + " |")
    print("|---|" + "---|" * len(results))
    for k in keys:
        print(f"| {k} | " + " | ".join(str(results[n][k]) for n in results) + " |")
    ref = results.get("redis")
    if ref:
        for n, r in results.items():
            print(f"{n}: {'MATCHES' if r == ref else 'DIFFERS FROM'} redis")
