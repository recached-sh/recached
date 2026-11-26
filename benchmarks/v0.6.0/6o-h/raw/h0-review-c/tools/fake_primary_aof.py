#!/usr/bin/env python3
"""Fake replication primary: advertise a crafted run-id string to a real
replica binary and record what it sends back (requested run id string,
offset) on its first and second connection. Two frames are streamed so the
second connection can request a partial resync.

usage: fake_primary.py BINARY ADVERTISED_SUFFIX MODE(D|F) WORKDIR
"""
import os, socket, struct, subprocess, sys, time, random, signal

binary, suffix, mode, work = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
ARRAY = len(sys.argv) > 5 and sys.argv[5] == "array"
os.makedirs(work, exist_ok=True)


def resp_push(*parts):
    out = (b"*" if ARRAY else b">") + b"%d\r\n" % len(parts)
    for p in parts:
        p = p.encode() if isinstance(p, str) else p
        out += b"$%d\r\n%s\r\n" % (len(p), p)
    return out


def free_port():
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p


def recv_exact(c, n):
    b = b""
    while len(b) < n:
        chunk = c.recv(n - len(b))
        if not chunk:
            raise ConnectionError("closed after %d/%d" % (len(b), n))
        b += chunk
    return b


def resp_cmd(port, *args):
    out = b"*%d\r\n" % len(args)
    for a in args:
        a = a.encode()
        out += b"$%d\r\n%s\r\n" % (len(a), a)
    with socket.create_connection(("127.0.0.1", port), timeout=5) as s:
        s.sendall(out)
        time.sleep(0.2)
        return s.recv(65536)


run_id = "".join(random.choice("0123456789abcdef") for _ in range(40))
advertised = (run_id + suffix).encode()
ls = socket.socket(); ls.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
ls.bind(("127.0.0.1", 0)); ls.listen(4); ls.settimeout(20)
pport = ls.getsockname()[1]
rport, wport = free_port(), free_port()
env = dict(os.environ)
env.update(RECACHED_BIND="127.0.0.1", RECACHED_PORT=str(rport), RECACHED_WS_PORT=str(wport),
           RECACHED_METRICS_PORT="0", RECACHED_SAVE_INTERVAL="0",
           RECACHED_SAVE_PATH=os.path.join(work, "dump.rdb"), RECACHED_AOF_PATH=os.path.join(work, "appendonly.aof"), RECACHED_AOF_SYNC="always",
           RECACHED_REPLICAOF="127.0.0.1:%d" % pport)
log = open(os.path.join(work, "replica.log"), "ab")
p = subprocess.Popen([binary], cwd=work, env=env, stdout=log, stderr=log, stdin=subprocess.DEVNULL)
print("advertised len=%d: %s" % (len(advertised), advertised.decode()))
try:
    for attempt in (1, 2):
        c, _ = ls.accept()
        c.settimeout(10)
        c.sendall(b"RCP1" + struct.pack("<H", len(advertised)) + advertised)
        try:
            (n,) = struct.unpack("<H", recv_exact(c, 2))
        except ConnectionError as e:
            print("attempt %d: replica closed before requesting: %s" % (attempt, e))
            c.close(); break
        req = recv_exact(c, n).decode()
        (off,) = struct.unpack("<Q", recv_exact(c, 8))
        print("attempt %d: requested=%r offset=%d" % (attempt, req, off))
        if attempt == 1:
            c.sendall(mode.encode() + struct.pack("<Q", 0))
            c.sendall(struct.pack("<I", 1) + b"\x90")
            if mode == "D":
                c.sendall(struct.pack("<I", 1) + b"\x90")
            ack = recv_exact(c, 8)
            for i, (k, v) in enumerate((("a", "1"), ("b", "2")), start=1):
                fr = resp_push("SET", k, v)
                c.sendall(struct.pack("<Q", i) + struct.pack("<I", len(fr)) + fr)
            time.sleep(0.5)
            print("  GET a/b on replica:", resp_cmd(rport, "GET", "a"), resp_cmd(rport, "GET", "b"))
        c.close()
finally:
    p.send_signal(signal.SIGKILL); p.wait()
    with open(os.path.join(work, "replica.log"), errors="replace") as f:
        lines = [l.strip() for l in f if "Replica" in l or "ERROR" in l or "WARN" in l]
    print("  replica AOF:", open(os.path.join(work, "appendonly.aof"), "rb").read())
    for l in lines[-0:]:
        print("  log:", l[:220])
