#!/usr/bin/env python3
"""Acknowledged-write ledger for the 6I-G soak (no dependencies).

    ledger.py write <port> <conns> <seconds> <out.json>
        <conns> connections, each pipelining INCR (8 deep) on its own
        counters `ledger:<conn>:<k>` (k < 16) and, every 50th round, a DEDUP
        INCR on `ledger:dedup:<conn>` sent twice with one id. Records the
        highest acknowledged value of every counter, the DEDUP firsts, and
        errors. Every acknowledgement it records was answered after the
        server's AOF write (that is the guarantee under test).
    ledger.py verify <port> <ledger.json>
        After a crash and restart: every counter must be at least its
        highest acknowledged value (an INCR in flight at the crash may also
        have landed, so +pipeline is allowed), and each DEDUP counter must
        equal its count of acknowledged firsts (+1 in flight). Prints a JSON
        verdict; exit status 1 on any lost or duplicated write.
    ledger.py cas <port> <seconds> <out.json>
        WATCH/MULTI/INCR/EXEC compare-and-set loop on `cas:counter` racing
        the load: committed transactions must equal the counter's growth.
"""
import json
import socket
import sys
import time


class Conn:
    def __init__(self, port):
        self.s = socket.create_connection(("127.0.0.1", int(port)))
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.buf = b""

    def send(self, *cmds):
        out = []
        for args in cmds:
            out.append(b"*%d\r\n" % len(args))
            for a in args:
                a = a if isinstance(a, bytes) else str(a).encode()
                out.append(b"$%d\r\n%s\r\n" % (len(a), a))
        self.s.sendall(b"".join(out))

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
            return v.decode(errors="replace")
        if t == b"*":
            n = int(rest)
            return None if n < 0 else [self.read() for _ in range(n)]
        raise ValueError(f"unexpected {line!r}")

    def cmd(self, *args):
        self.send(args)
        return self.read()


def write(port, conns, seconds, out):
    import threading
    conns, seconds = int(conns), float(seconds)
    acked, firsts, errors = {}, {}, {"count": 0, "samples": []}
    lock = threading.Lock()

    def run(c):
        conn = Conn(port)
        end = time.time() + seconds
        rnd = 0
        while time.time() < end:
            keys = [f"ledger:{c}:{(rnd * 8 + j) % 16}" for j in range(8)]
            conn.send(*[("INCR", k) for k in keys])
            replies = [conn.read() for _ in keys]
            with lock:
                for k, r in zip(keys, replies):
                    if isinstance(r, str) and r.lstrip("-").isdigit():
                        acked[k] = max(acked.get(k, 0), int(r))
                    else:
                        errors["count"] += 1
                        if len(errors["samples"]) < 5:
                            errors["samples"].append(repr(r))
            if rnd % 50 == 0:
                key, ident = f"ledger:dedup:{c}", str(rnd + 1)
                a = conn.cmd("DEDUP", f"ledger-{c}", ident, "INCR", key)
                b = conn.cmd("DEDUP", f"ledger-{c}", ident, "INCR", key)
                with lock:
                    if isinstance(a, str) and a.isdigit():
                        firsts[key] = firsts.get(key, 0) + 1
                    if b != "DUP":
                        errors["count"] += 1
                        errors["samples"].append(f"retry answered {b!r}")
            rnd += 1

    threads = [threading.Thread(target=run, args=(c,)) for c in range(conns)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    json.dump({"acked": acked, "dedup_firsts": firsts, "errors": errors}, open(out, "w"))
    print(f"ledger: {len(acked)} counters, {sum(acked.values())} acknowledged INCRs, "
          f"{sum(firsts.values())} DEDUP firsts, {errors['count']} errors")


def verify(port, path):
    led = json.load(open(path))
    conn = Conn(port)
    lost, extra, dedup_bad = [], [], []
    for k, v in led["acked"].items():
        got = int(conn.cmd("GET", k) or 0)
        if got < v:
            lost.append((k, v, got))
        elif got > v + 8:
            extra.append((k, v, got))
    for k, n in led["dedup_firsts"].items():
        got = int(conn.cmd("GET", k) or 0)
        if not (n <= got <= n + 1):
            dedup_bad.append((k, n, got))
    verdict = {"counters": len(led["acked"]), "acknowledged": sum(led["acked"].values()),
               "lost": len(lost), "lost_samples": lost[:5], "beyond_in_flight": len(extra),
               "dedup_mismatch": len(dedup_bad), "dedup_samples": dedup_bad[:5]}
    print(json.dumps(verdict))
    sys.exit(1 if lost or dedup_bad or extra else 0)


def cas(port, seconds, out):
    import threading
    conn = Conn(port)
    start = int(conn.cmd("GET", "cas:counter") or 0)
    committed = aborted = 0
    end = time.time() + float(seconds)
    racer = {"acked": 0}

    def race():
        # Plain INCRs on the watched key: each must abort a transaction
        # that read the value before it, or be lost by one that did not.
        rc = Conn(port)
        while time.time() < end:
            if isinstance(rc.cmd("INCR", "cas:counter"), str):
                racer["acked"] += 1
            time.sleep(0.001)

    t = threading.Thread(target=race)
    t.start()
    while time.time() < end:
        conn.cmd("WATCH", "cas:counter")
        cur = int(conn.cmd("GET", "cas:counter") or 0)
        conn.cmd("MULTI")
        conn.cmd("SET", "cas:counter", str(cur + 1))
        r = conn.cmd("EXEC")
        if r is None:
            aborted += 1
        else:
            committed += 1
    t.join()
    final = int(conn.cmd("GET", "cas:counter") or 0)
    res = {"committed": committed, "aborted": aborted, "racer_incrs": racer["acked"],
           "growth": final - start, "exact": final - start == committed + racer["acked"]}
    json.dump(res, open(out, "w"))
    print(json.dumps(res))


if __name__ == "__main__":
    {"write": write, "verify": verify, "cas": cas}[sys.argv[1]](*sys.argv[2:])
