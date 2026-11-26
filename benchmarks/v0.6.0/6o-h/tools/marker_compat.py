#!/usr/bin/env python3
"""6O-H0: do released servers treat the candidate AOF control records as
no-ops? Runs each binary as a real process on crafted AOF and snapshot files.

Candidate A is `RECACHED-CHECKPOINT <id>` with `id` in a reserved range
(top 16 bits 0xEC00). Candidate B is `DEL <reserved key>`. Each case prints
one line: `ok`/`FAIL`, the binary, the case, and what was observed.

usage: marker_compat.py NAME=BINARY ...
"""
import os, shutil, signal, socket, subprocess, sys, tempfile, time

TAG = 0xEC00
FORMAT, RESUME, SEAL, LEGACY = 1, 2, 3, 4


def control_id(kind, payload=0, version=1):
    return (TAG << 48) | (kind << 40) | (version << 32) | payload


def resp_push(*parts):
    out = b">%d\r\n" % len(parts)
    for p in parts:
        p = p.encode() if isinstance(p, str) else p
        out += b"$%d\r\n%s\r\n" % (len(p), p)
    return out


def resp_array(*parts):
    """The same record as a RESP2 array: how a 6O-H server writes the
    expiry-log format (authorship, design rev 2 §2.2)."""
    return b"*" + resp_push(*parts)[1:]


def marker(kind, payload=0, version=1):
    return resp_push("RECACHED-CHECKPOINT", str(control_id(kind, payload, version)))


FMT = marker(FORMAT, 1)
RES = marker(RESUME)
SEA = marker(SEAL)
LEG = marker(LEGACY)


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
    line, sep, after = rest.partition(b"\r\n")
    if not sep:
        raise IndexError
    if t in (b"+", b"-"):
        return (line.decode() if t == b"+" else "ERR:" + line.decode()), after
    if t == b":":
        return int(line), after
    if t == b"$":
        n = int(line)
        if n < 0:
            return None, after
        if len(after) < n + 2:
            raise IndexError
        return after[:n].decode(), after[n + 2:]
    if t == b"*":
        n = int(line)
        items = []
        for _ in range(max(n, 0)):
            v, after = parse(after)
            items.append(v)
        return items, after
    raise ValueError(buf)


def cmd(port, *args):
    with socket.create_connection(("127.0.0.1", port), timeout=5) as s:
        s.sendall(enc(args))
        buf = b""
        while True:
            chunk = s.recv(65536)
            if not chunk:
                raise ConnectionError("closed")
            buf += chunk
            try:
                v, _ = parse(buf)
                return v
            except IndexError:
                continue


class Server:
    def __init__(self, binary, d):
        self.binary, self.d = binary, d
        self.port, self.ws = free_port(), free_port()
        self.log = os.path.join(d, "server.log")
        self.p = None

    def start(self, sync="always"):
        env = dict(os.environ)
        env.update(
            RECACHED_BIND="127.0.0.1",
            RECACHED_PORT=str(self.port),
            RECACHED_WS_PORT=str(self.ws),
            RECACHED_METRICS_PORT="0",
            RECACHED_SAVE_INTERVAL="0",
            RECACHED_SAVE_PATH=os.path.join(self.d, "dump.rdb"),
            RECACHED_AOF_PATH=os.path.join(self.d, "appendonly.aof"),
            RECACHED_AOF_SYNC=sync,
        )
        with open(self.log, "ab") as log:
            self.p = subprocess.Popen([self.binary], cwd=self.d, env=env,
                                      stdout=log, stderr=log, stdin=subprocess.DEVNULL)
        for _ in range(100):
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
            self.p.send_signal(signal.SIGKILL)
            self.p.wait()

    def stop(self):
        if self.p and self.p.poll() is None:
            self.p.send_signal(signal.SIGTERM)
            try:
                self.p.wait(10)
            except subprocess.TimeoutExpired:
                self.kill9()

    def tail(self, n=3):
        try:
            with open(self.log, errors="replace") as f:
                return " | ".join(l.strip() for l in f.readlines()[-n:])
        except OSError:
            return ""


def snapshot_checkpoint(path):
    """`SnapshotFile { version, aof_checkpoint, entries }`, msgpack array."""
    with open(path, "rb") as f:
        b = f.read(16)
    assert b[0] == 0x93 and b[1] == 0x01, b[:4]
    t = b[2]
    if t <= 0x7F:
        return t
    width = {0xCC: 1, 0xCD: 2, 0xCE: 4, 0xCF: 8}[t]
    return int.from_bytes(b[3:3 + width], "big")


RESULTS = []


def report(ok, name, case, detail):
    line = f"{'ok  ' if ok else 'FAIL'}  {name:10s} {case:48s} {detail}"
    print(line, flush=True)
    RESULTS.append(ok)


def fresh(root, case):
    d = os.path.join(root, case)
    os.makedirs(d)
    return d


def write(d, data, name="appendonly.aof"):
    with open(os.path.join(d, name), "wb") as f:
        f.write(data)


def size(d, name="appendonly.aof"):
    return os.path.getsize(os.path.join(d, name))


def run_cases(name, binary, root):
    far = str(int(time.time() * 1000) + 3_600_000)

    # A1: every control kind is accepted and changes nothing.
    d = fresh(root, f"{name}-a1")
    write(d, FMT + resp_push("SET", "a", "1") + RES +
          resp_push("SET", "b", "2", "PXAT", far) + SEA + LEG +
          resp_push("INCRBY", "n", "5") + marker(9, 0xFFFF, 7))
    s = Server(binary, d)
    up = s.start()
    got = [cmd(s.port, "GET", k) for k in ("a", "b", "n")] if up else None
    pttl = cmd(s.port, "PTTL", "b") if up else None
    dbsize = cmd(s.port, "DBSIZE") if up else None
    keys = sorted(cmd(s.port, "KEYS", "*")) if up else None
    report(up and got == ["1", "2", "5"] and dbsize == 3 and keys == ["a", "b", "n"]
           and isinstance(pttl, int) and pttl > 3_000_000,
           name, "A1 every control kind replays as a no-op",
           f"up={up} got={got} dbsize={dbsize} keys={keys} pttl={pttl} {'' if up else s.tail()}")
    # A1b: it keeps appending after them, and replays its own records after
    # a SIGKILL (the markers do not break a later replay).
    if up:
        cmd(s.port, "SET", "c", "3")
        s.kill9()
        up2 = s.start()
        got2 = [cmd(s.port, "GET", k) for k in ("a", "b", "c", "n")] if up2 else None
        report(up2 and got2 == ["1", "2", "3", "5"], name,
               "A1b appends after markers; replays after SIGKILL", f"got={got2}")
        # A1c: its SAVE checkpoints with id 1 (reserved ids never adopted)
        # and truncates the markers away.
        if up2:
            saved = cmd(s.port, "SAVE")
            ck = snapshot_checkpoint(os.path.join(d, "dump.rdb"))
            with open(os.path.join(d, "appendonly.aof"), "rb") as f:
                left = f.read()
            report(saved == "OK" and ck == 1 and b"RECACHED-CHECKPOINT" not in left, name,
                   "A1c SAVE: checkpoint id 1, markers truncated", f"save={saved} ck={ck} aof_bytes={len(left)}")
            s.stop()
    else:
        s.kill9()

    # A2: markers around a real checkpoint marker do not move the replay
    # start. Snapshot written by the binary itself (checkpoint 1); then an
    # AOF where records before `CHECKPOINT 1` are in the snapshot and must
    # not replay, and records after it must.
    d = fresh(root, f"{name}-a2")
    s = Server(binary, d)
    if s.start():
        cmd(s.port, "SET", "snap", "s")
        cmd(s.port, "SAVE")
        s.kill9()
        ck = snapshot_checkpoint(os.path.join(d, "dump.rdb"))
        write(d, FMT + resp_push("SET", "pre", "x") + marker(RESUME) +
              resp_push("RECACHED-CHECKPOINT", str(ck)) + RES +
              resp_push("SET", "post", "y") + SEA)
        up = s.start()
        got = [cmd(s.port, "GET", k) for k in ("snap", "pre", "post")] if up else None
        report(up and ck == 1 and got == ["s", None, "y"], name,
               "A2 replay starts after the snapshot's own marker",
               f"ck={ck} got={got} {'' if up else s.tail()}")
        s.stop()
    else:
        report(False, name, "A2 replay starts after the snapshot's own marker", s.tail())

    # A3: a torn marker at the tail is handled exactly like a torn data
    # record (control A3c): cut by servers that cut torn tails (0.5 and
    # later), left in place by those that do not (0.4).
    def torn(case, body, tail):
        d = fresh(root, f"{name}-{case}")
        write(d, body + tail)
        s = Server(binary, d)
        up = s.start()
        got = cmd(s.port, "GET", "a") if up else None
        db = cmd(s.port, "DBSIZE") if up else None
        s.kill9()  # not SIGTERM: its shutdown SAVE would truncate the file
        return up, got, db, size(d) == len(body)

    body = FMT + resp_push("SET", "a", "1")
    data_tail = resp_push("SET", "z", "torn")[:11]
    ctl = torn("a3c", body, data_tail)
    mk = torn("a3", body, SEA[:11])
    report(mk[0] and mk[1] == "1" and mk[3] == ctl[3], name,
           "A3 torn marker tail handled as a torn data record",
           f"marker: up={mk[0]} got={mk[1]} cut={mk[3]}; control data record: cut={ctl[3]}")
    ctl = torn("a3d", b"", data_tail)
    mk = torn("a3b", b"", FMT[:20])
    report(mk[0] and mk[2] == 0 and mk[3] == ctl[3], name,
           "A3b torn head marker handled as a torn data record",
           f"marker: up={mk[0]} dbsize={mk[2]} cut={mk[3]}; control: cut={ctl[3]}")

    # A4: the largest reserved id and u64::MAX are still accepted.
    d = fresh(root, f"{name}-a4")
    write(d, resp_push("RECACHED-CHECKPOINT", str(2**64 - 1)) +
          resp_push("RECACHED-CHECKPOINT", str(control_id(0xFF, 0xFFFFFFFF, 0xFF))) +
          resp_push("SET", "a", "1"))
    s = Server(binary, d)
    up = s.start()
    got = cmd(s.port, "GET", "a") if up else None
    s.stop()
    report(up and got == "1", name, "A4 ids up to u64::MAX accepted", f"up={up} got={got}")

    # A5 (boundary, expected refusal): a non-u64 id is a command, not a
    # marker, so the encoding must stay a bare u64.
    for label, ident in (("A5 id past u64::MAX is refused", str(2**64)),
                         ("A5b non-numeric id is refused", "expirylog-v1"),
                         ("A5c three-part record is refused", None)):
        d = fresh(root, f"{name}-{label.split()[0].lower()}")
        rec = (resp_push("RECACHED-CHECKPOINT", str(control_id(FORMAT, 1)), "x") if ident is None
               else resp_push("RECACHED-CHECKPOINT", ident))
        write(d, rec + resp_push("SET", "a", "1"))
        s = Server(binary, d)
        up = s.start()
        s.kill9()
        report(not up, name, label + " (boundary)", f"started={up} {s.tail(1)[:120]}")

    # A6: array records after FORMAT replay, and the binary appends its own
    # records as pushes after them (what tells a 6O-H reader where they end).
    d = fresh(root, f"{name}-a6")
    body = (b"*" + FMT[1:]) + resp_array("SET", "a", "1") + resp_array("APPEND", "a", "x") + \
        resp_array("SET", "b", "2", "PXAT", far)
    write(d, body)
    s = Server(binary, d)
    up = s.start()
    got = [cmd(s.port, "GET", k) for k in ("a", "b")] if up else None
    if up:
        cmd(s.port, "SET", "c", "3")
    s.kill9()
    with open(os.path.join(d, "appendonly.aof"), "rb") as f:
        tail = f.read()[len(body):]
    report(up and got == ["1x", "2"] and tail.startswith(b">3\r\n$3\r\nSET"), name,
           "A6 array records replay; its own appends are pushes",
           f"up={up} got={got} appended={tail[:12]!r}")

    # B1: candidate B, `DEL <reserved key>`, deletes a user key of that name.
    d = fresh(root, f"{name}-b1")
    reserved = "\x00recached:expirylog:v1"
    write(d, resp_push("SET", reserved, "user-data") + resp_push("DEL", reserved) +
          resp_push("SET", "a", "1"))
    s = Server(binary, d)
    up = s.start()
    got = cmd(s.port, "GET", reserved) if up else "n/a"
    s.stop()
    report(up and got is None, name,
           "B1 reserved-key DEL removes a user key of that name",
           f"up={up} got={got!r} (None = user data lost: candidate B unsafe)")


def main():
    pairs = [a.split("=", 1) for a in sys.argv[1:]]
    root = tempfile.mkdtemp(prefix="rc6oh-marker-", dir=os.environ.get("MARKER_TMP"))
    try:
        for name, binary in pairs:
            run_cases(name, binary, root)
    finally:
        shutil.rmtree(root, ignore_errors=True)
    print(f"{sum(RESULTS)}/{len(RESULTS)} checks as expected")
    sys.exit(0 if all(RESULTS) else 1)


if __name__ == "__main__":
    main()
