#!/usr/bin/env python3
"""6O-G memory bound (design N1): 10,000 APPENDs in one EXEC to an 8 MiB (MEM_MIB) string,
with a TCP WATCHer and a WebSocket QSUB subscriber (full values) present.
Usage: g-mem.py <server-binary> <label> <rep>. Prints one line with VmHWM
before and after, per transaction, and whether the subscriber stayed connected.
Stdlib only; native host."""
import base64, os, socket, subprocess, sys, tempfile, threading, time

binary, label, rep = sys.argv[1], sys.argv[2], sys.argv[3]
port = 27600 + int(rep) * 4 + (0 if label == "base" else 2)
d = tempfile.mkdtemp(dir=os.path.expanduser("~/recached-6og-bench"))
env = dict(os.environ, RECACHED_BIND="127.0.0.1", RECACHED_PORT=str(port), RECACHED_WS_PORT=str(port + 1),
           RECACHED_METRICS_PORT="0", RECACHED_SAVE_INTERVAL="0", RECACHED_SAVE_PATH=d + "/dump.rdb",
           RECACHED_WORKER_THREADS="3")
srv = subprocess.Popen(["taskset", "-c", "0-2", binary], env=env, stdout=open(d + "/server.log", "w"), stderr=subprocess.STDOUT)

def enc(*args):
    out = b"*%d\r\n" % len(args)
    for a in args:
        a = a if isinstance(a, bytes) else a.encode()
        out += b"$%d\r\n%s\r\n" % (len(a), a)
    return out

def conn():
    for _ in range(100):
        try:
            return socket.create_connection(("127.0.0.1", port))
        except OSError:
            time.sleep(0.05)
    raise SystemExit("no server")

def read_reply(f):
    line = f.readline()
    t = line[:1]
    if t in (b"+", b"-", b":"):
        return line
    if t == b"$":
        n = int(line[1:])
        return f.read(n + 2) if n >= 0 else None
    if t == b"*":
        n = int(line[1:])
        return [read_reply(f) for _ in range(max(n, 0))]
    raise RuntimeError(line)

def hwm():
    for l in open(f"/proc/{srv.pid}/status"):
        if l.startswith("VmHWM"):
            return int(l.split()[1])

c = conn(); f = c.makefile("rb")
KEY = "mem:big"
MIB = int(os.environ.get("MEM_MIB", "8"))  # 8: past the 8 MiB subscriber queue bound, so the subscriber is dropped either way
c.sendall(enc("SET", KEY, b"v" * (MIB << 20))); read_reply(f)
# A TCP WATCHer (CAS: notified, never sent the value).
w = conn(); wf = w.makefile("rb"); w.sendall(enc("WATCH", KEY)); read_reply(wf)
# A WebSocket live query (full values), drained in the background.
ws = socket.create_connection(("127.0.0.1", port + 1))
key = base64.b64encode(os.urandom(16)).decode()
ws.sendall(f"GET / HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n".encode())
resp = b""
while b"\r\n\r\n" not in resp:
    resp += ws.recv(4096)
def ws_send(payload):
    mask = os.urandom(4); n = len(payload)
    hdr = bytes([0x81]) + (bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + n.to_bytes(2, "big"))
    ws.sendall(hdr + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(payload)))
ws_send(enc("QSUB", "mem:*"))
received = [0, True]
def drain():
    try:
        while True:
            chunk = ws.recv(1 << 20)
            if not chunk:
                received[1] = False; return
            received[0] += len(chunk)
    except OSError:
        received[1] = False
threading.Thread(target=drain, daemon=True).start()
time.sleep(0.5)
print("setup done", file=sys.stderr, flush=True)
before = hwm()
results = []
for i in range(3):
    t0 = time.time()
    c.sendall(enc("MULTI")); read_reply(f)
    batch = b"".join(enc("APPEND", KEY, "x") for _ in range(10000))
    sender = threading.Thread(target=c.sendall, args=(batch,))
    sender.start()
    for _ in range(10000):
        read_reply(f)
    sender.join()
    c.sendall(enc("EXEC")); r = read_reply(f)
    results.append((time.time() - t0, len(r) if isinstance(r, list) else r, hwm()))
time.sleep(1.0)
after = hwm()
print(f"label={label} rep={rep} mib={MIB} hwm_before_kb={before} hwm_after_kb={after} growth_kb={after - before} "
      f"per_tx=[{' '.join(f'{s*1000:.1f}ms/{n}/{h}kB' for s, n, h in results)}] ws_bytes={received[0]} ws_connected={received[1]}")
srv.terminate(); srv.wait()
import shutil; shutil.rmtree(d, ignore_errors=True)
