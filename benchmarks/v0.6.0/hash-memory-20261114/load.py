import socket, sys
port, n, size, kind = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
val = b"x" * size
s = socket.create_connection(("127.0.0.1", port))
def enc(*parts):
    return b"*%d\r\n" % len(parts) + b"".join(b"$%d\r\n%s\r\n" % (len(p), p) for p in parts)
BATCH = 5000
for start in range(0, n, BATCH):
    out = []
    for i in range(start, min(n, start + BATCH)):
        k = b"mem:%010d" % i
        if kind == "string":
            out.append(enc(b"SET", k, val))
        elif kind == "hash":
            out.append(enc(b"HSET", k, b"f1", val, b"f2", val, b"f3", val))
        else:
            out.append(enc(b"SET", k, val, b"EX", b"3600"))
    s.sendall(b"".join(out))
    want, got, buf = len(out), 0, b""
    while got < want:
        chunk = s.recv(1 << 16)
        if not chunk:
            sys.exit("server closed the connection")
        buf += chunk
        got = buf.count(b"\r\n")
    if b"\r\n-" in b"\r\n" + buf:
        sys.exit("error reply while loading: " + buf.split(b"-", 1)[1][:200].decode(errors="replace"))
