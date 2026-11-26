"""Minimal helpers: start a recached binary, send RESP commands."""
import os, signal, socket, subprocess, time


def free_port():
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p


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
    if t in (b"+", b"-"):
        return (line.decode() if t == b"+" else "ERR:" + line.decode()), after
    if t == b":":
        return int(line), after
    if t in (b"$",):
        n = int(line)
        if n < 0:
            return None, after
        if len(after) < n + 2:
            raise IndexError
        return after[:n].decode(errors="replace"), after[n + 2:]
    if t == b"_":
        return None, after
    if t in (b"*", b">", b"~"):
        n = int(line)
        items = []
        for _ in range(max(n, 0)):
            v, after = parse(after)
            items.append(v)
        return items, after
    raise ValueError(buf[:40])


class Conn:
    def __init__(self, port):
        self.s = socket.create_connection(("127.0.0.1", port), timeout=5)
        self.buf = b""

    def cmd(self, *args):
        self.s.sendall(enc(args))
        while True:
            try:
                v, rest = parse(self.buf)
                self.buf = rest
                return v
            except IndexError:
                chunk = self.s.recv(65536)
                if not chunk:
                    raise ConnectionError("closed")
                self.buf += chunk

    def close(self):
        self.s.close()


def cmd(port, *args):
    c = Conn(port)
    try:
        return c.cmd(*args)
    finally:
        c.close()


class Server:
    def __init__(self, binary, d, **extra):
        self.binary, self.d, self.extra = binary, d, extra
        os.makedirs(d, exist_ok=True)
        self.port, self.ws = free_port(), free_port()
        self.p = None

    def start(self, **more):
        env = dict(os.environ)
        env.update(RECACHED_BIND="127.0.0.1", RECACHED_PORT=str(self.port),
                   RECACHED_WS_PORT=str(self.ws), RECACHED_METRICS_PORT="0",
                   RECACHED_SAVE_INTERVAL="0",
                   RECACHED_SAVE_PATH=os.path.join(self.d, "dump.rdb"),
                   RECACHED_AOF_PATH=os.path.join(self.d, "appendonly.aof"),
                   RECACHED_AOF_SYNC="always")
        env.update(self.extra)
        env.update(more)
        env = {k: v for k, v in env.items() if v is not None}
        log = open(os.path.join(self.d, "server.log"), "ab")
        self.p = subprocess.Popen([self.binary], cwd=self.d, env=env, stdout=log, stderr=log,
                                  stdin=subprocess.DEVNULL)
        for _ in range(200):
            if self.p.poll() is not None:
                return False
            try:
                if cmd(self.port, "PING") == "PONG":
                    return True
            except OSError:
                pass
            time.sleep(0.05)
        return False

    def kill9(self):
        if self.p and self.p.poll() is None:
            self.p.send_signal(signal.SIGKILL); self.p.wait()

    def stop(self):
        if self.p and self.p.poll() is None:
            self.p.send_signal(signal.SIGTERM)
            try:
                self.p.wait(15)
            except subprocess.TimeoutExpired:
                self.kill9()

    def aof(self):
        with open(os.path.join(self.d, "appendonly.aof"), "rb") as f:
            return f.read()
