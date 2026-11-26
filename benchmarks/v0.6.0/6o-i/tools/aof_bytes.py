#!/usr/bin/env python3
"""6O-I: AOF bytes and records per transaction, measured on the file.

For each binary and transaction shape: a fresh server (AOF everysec), N
transactions of the shape over one pipelined connection, then the AOF's size
and record count are read while the server still runs (a SIGTERM's shutdown
save would truncate it), minus what the file held before.

usage: aof_bytes.py NAME=BINARY ...
"""
import os, shutil, socket, sys, tempfile, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from txn_compat import Server, cmd, enc  # noqa: E402

N = int(os.environ.get("N", "2000"))
SHAPES = {
    "set-set": lambda i: [["SET", f"k:{i}:a", "v" * 32], ["SET", f"k:{i}:b", "v" * 32]],
    "single SET in EXEC": lambda i: [["SET", f"k:{i}", "v" * 32]],
    "wide20": lambda i: [["SET", f"k:{i}:{j}", "v" * 32] for j in range(20)],
    "setn100": lambda i: [["SET", f"k:{i}:{j}", "v" * 32] for j in range(100)],
}


def records(path):
    data = open(path, "rb").read()
    # Each record starts a line with `*` or `>` followed by a count; count
    # them by walking RESP headers.
    n, at = 0, 0
    while at < len(data):
        end = data.index(b"\r\n", at)
        count = int(data[at + 1:end])
        at = end + 2
        for _ in range(count):
            end = data.index(b"\r\n", at)
            ln = int(data[at + 1:end])
            at = end + 2 + ln + 2
        n += 1
    return len(data), n


def main():
    root = tempfile.mkdtemp(prefix="rc6oi-aofb-", dir=os.environ.get("TXN_TMP"))
    try:
        print(f"| shape | binary | bytes/tx | records/tx |")
        print("|---|---|---|---|")
        for shape, make in SHAPES.items():
            for name, binary in (a.split("=", 1) for a in sys.argv[1:]):
                d = os.path.join(root, f"{shape.replace(' ', '_')}-{name}")
                os.makedirs(d)
                s = Server(binary, d)
                assert s.start("everysec"), name
                path = os.path.join(d, "appendonly.aof")
                b0, r0 = records(path)
                with socket.create_connection(("127.0.0.1", s.port), timeout=30) as c:
                    for i in range(N):
                        c.sendall(enc(["MULTI"]) + b"".join(enc(w) for w in make(i)) + enc(["EXEC"]))
                    c.sendall(enc(["PING"]))
                    buf = b""
                    while not buf.endswith(b"+PONG\r\n"):
                        buf += c.recv(1 << 20)
                time.sleep(1.5)  # everysec: let the batch reach the file
                b1, r1 = records(path)
                s.kill9()
                print(f"| {shape} | {name} | {(b1 - b0) / N:.1f} | {(r1 - r0) / N:.2f} |", flush=True)
    finally:
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    main()
