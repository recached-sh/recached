#!/usr/bin/env python3
"""Replica lag probe and convergence check for prod.sh (no dependencies).

    repl_check.py probe <primary-port> <replica-port> <count>
        Writes `SET lagprobe:<n> <ns>` to the primary 20 times a second and
        polls the replica (every 0.2 ms) until that value is readable there.
        One line per probe: "<n> <lag_ms>" or "<n> timeout".
    repl_check.py summarize <probe-file>
        "<p50_ms> <p99_ms> <max_ms>" over the probes that arrived (timeouts
        count as +inf for p99/max: printed as "timeout").
    repl_check.py converge <primary-port> <replica-port> <keys> <timeout-s>
        Waits until DBSIZE matches and 300 random keys of each prefix (k:, c:
        strings; h: hash field f1) hold the same value on both. Prints
        "<ms> ok" or "<ms> DIVERGED:<detail>".

The same probe is used for every engine, so lag is comparable even though
Recached's INFO has no replication offsets.
"""
import random
import socket
import sys
import time


class Conn:
    def __init__(self, port):
        self.s = socket.create_connection(("127.0.0.1", int(port)))
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.buf = b""

    def cmd(self, *args):
        out = [b"*%d\r\n" % len(args)]
        for a in args:
            a = a if isinstance(a, bytes) else str(a).encode()
            out.append(b"$%d\r\n%s\r\n" % (len(a), a))
        self.s.sendall(b"".join(out))
        return self.read()

    def _line(self):
        while b"\r\n" not in self.buf:
            chunk = self.s.recv(65536)
            if not chunk:
                raise ConnectionError("closed")
            self.buf += chunk
        line, self.buf = self.buf.split(b"\r\n", 1)
        return line

    def read(self):
        line = self._line()
        t, rest = line[:1], line[1:]
        if t in (b"+", b":"):
            return rest.decode()
        if t == b"-":
            return ("ERR", rest.decode())
        if t == b"$":
            n = int(rest)
            if n < 0:
                return None
            while len(self.buf) < n + 2:
                chunk = self.s.recv(65536)
                if not chunk:
                    raise ConnectionError("closed")
                self.buf += chunk
            v, self.buf = self.buf[:n], self.buf[n + 2:]
            return v
        if t == b"*":
            return [self.read() for _ in range(int(rest))]
        raise ValueError(f"unexpected {line!r}")


def probe(pport, rport, count):
    p, r = Conn(pport), Conn(rport)
    for n in range(int(count)):
        start = time.monotonic_ns()
        val = str(start).encode()
        key = f"lagprobe:{n}"
        p.cmd("SET", key, val)
        sent = time.monotonic_ns()
        deadline = sent + 10_000_000_000
        while True:
            if r.cmd("GET", key) == val:
                print(f"{n} {(time.monotonic_ns() - sent) / 1e6:.3f}", flush=True)
                break
            if time.monotonic_ns() > deadline:
                print(f"{n} timeout", flush=True)
                break
            time.sleep(0.0002)
        rest = 0.05 - (time.monotonic_ns() - start) / 1e9
        if rest > 0:
            time.sleep(rest)


def summarize(path):
    lags, timeouts = [], 0
    for line in open(path):
        parts = line.split()
        if len(parts) != 2:
            continue
        if parts[1] == "timeout":
            timeouts += 1
        else:
            lags.append(float(parts[1]))
    if not lags:
        print("timeout timeout timeout")
        return
    lags.sort()
    q = lambda f: lags[min(len(lags) - 1, int(f * len(lags)))]
    hi = "timeout" if timeouts else None
    print(f"{q(0.5):.3f} {hi or f'{q(0.99):.3f}'} {hi or f'{lags[-1]:.3f}'}")


def converge(pport, rport, keys, timeout):
    p, r = Conn(pport), Conn(rport)
    start = time.monotonic()
    rnd = random.Random(42)
    sample = [(pre, rnd.randint(1, int(keys))) for pre in ("k:", "c:", "h:") for _ in range(300)]
    detail = ""
    while time.monotonic() - start < float(timeout):
        pd, rd = p.cmd("DBSIZE"), r.cmd("DBSIZE")
        if pd == rd:
            bad = None
            for pre, i in sample:
                key = f"{pre}{i}"
                if pre == "h:":
                    a, b = p.cmd("HGET", key, "f1"), r.cmd("HGET", key, "f1")
                else:
                    a, b = p.cmd("GET", key), r.cmd("GET", key)
                if a != b:
                    bad = f"{key}"
                    break
            if bad is None:
                print(f"{(time.monotonic() - start) * 1000:.0f} ok")
                return
            detail = f"value:{bad}"
        else:
            detail = f"dbsize:{pd}/{rd}"
        time.sleep(0.01)
    print(f"{(time.monotonic() - start) * 1000:.0f} DIVERGED:{detail}")


if __name__ == "__main__":
    {"probe": probe, "summarize": summarize, "converge": converge}[sys.argv[1]](*sys.argv[2:])
