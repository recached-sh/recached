#!/usr/bin/env python3
"""6O-I1: what do existing servers do with the candidate transaction framing?

Runs each binary as a real process on crafted AOF files (`appendfsync
always`) and prints one line per case: `ok`/`FAIL`, the binary, the case,
and what was observed. "ok" means the binary behaved as the design says it
does (which, for an older binary, is often "replays the group's frames one by
one": that is recorded, not endorsed).

Encodings (design `V0.6O-I-PROTOCOL-DESIGN.md` §2):
- `FORMAT` with features `expirylog` (bit 0) and `txn` (bit 16, must-understand);
- `TXN-BEGIN n` / `TXN-COMMIT n`: control records of ignorable kinds 0x81 and
  0x82, payload the group's member count;
- every record of a grouped file written as a RESP2 array (`*N`).
Rejected candidates, for the evidence: option B (one composite record) and
option C (Redis-style `MULTI`/`EXEC` records).

usage: txn_compat.py [--files-out DIR] NAME=BINARY ...
"""
import os, shutil, signal, socket, subprocess, sys, tempfile, time

TAG = 0xEC00
FORMAT, BEGIN, COMMIT = 1, 0x81, 0x82
EXPIRYLOG, TXN = 1, 1 << 16


def control_id(kind, payload=0, layout=1):
    return (TAG << 48) | (kind << 40) | (layout << 32) | payload


def resp_push(*parts):
    out = b">%d\r\n" % len(parts)
    for p in parts:
        p = p.encode() if isinstance(p, str) else p
        out += b"$%d\r\n%s\r\n" % (len(p), p)
    return out


def arr(*parts):
    return b"*" + resp_push(*parts)[1:]


def ctl(kind, payload):
    return arr("RECACHED-CHECKPOINT", str(control_id(kind, payload)))


FMT_TXN = ctl(FORMAT, EXPIRYLOG | TXN)
FMT_H = ctl(FORMAT, EXPIRYLOG)


def group(*records):
    return ctl(BEGIN, len(records)) + b"".join(records) + ctl(COMMIT, len(records))


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

    def tail(self, n=1):
        """The log's last error line (else its last line), without colour."""
        import re
        try:
            with open(self.log, errors="replace") as f:
                lines = [re.sub(r"\x1b\[[0-9;]*m", "", l).strip() for l in f]
        except OSError:
            return ""
        errors = [l for l in lines if "Error" in l or "ERROR" in l]
        text = " | ".join((errors or lines)[-n:])
        # The reason, not the temporary path in front of it.
        for start in ("AOF command failed", "AOF corrupted", "invalid AOF"):
            if start in text:
                text = text[text.index(start):]
                break
        return text[:160]


RESULTS = []


def report(ok, name, case, detail):
    line = f"{'ok  ' if ok else 'FAIL'}  {name:14s} {case:58s} {detail}"
    print(line, flush=True)
    RESULTS.append(ok)


def fresh(root, case):
    d = os.path.join(root, case)
    os.makedirs(d)
    return d


def path(d, name="appendonly.aof"):
    return os.path.join(d, name)


def write(d, data):
    with open(path(d), "wb") as f:
        f.write(data)


def read(d):
    with open(path(d), "rb") as f:
        return f.read()


def boot(binary, d, keys):
    """Start, read `keys`, SIGKILL (a SIGTERM's shutdown SAVE would rewrite
    the files). Returns (started, values, dbsize, log tail)."""
    s = Server(binary, d)
    up = s.start()
    got = [cmd(s.port, "GET", k) for k in keys] if up else None
    db = cmd(s.port, "DBSIZE") if up else None
    s.kill9()
    return up, got, db, s.tail()


BASE = FMT_TXN + arr("SET", "a", "old") + arr("SET", "b", "old")
TX = (arr("SET", "a", "new"), arr("SET", "b", "new"))


def run_cases(name, binary, root, files_out):
    # The 6O-I binary itself (a NAME starting "6oi"): it discards incomplete
    # groups, cuts the dangling one before appending, and understands `txn`.
    new = name.startswith("6oi")
    def keep(case, data):
        if files_out:
            with open(os.path.join(files_out, f"{case}.resp"), "wb") as f:
                f.write(data)

    # T1: a complete group replays; the control records change nothing else.
    d = fresh(root, f"{name}-t1")
    body = BASE + group(*TX) + arr("SET", "c", "1")
    write(d, body)
    up, got, db, tail = boot(binary, d, ("a", "b", "c"))
    report(up and got == ["new", "new", "1"] and db == 3, name,
           "T1 complete group replays; markers are no-ops",
           f"up={up} got={got} dbsize={db} {'' if up else tail}")

    # T2: an incomplete group at the tail (crash after its first member).
    # An older binary replays the members it finds one by one: the
    # transaction's prefix. Recorded; only 6O-I discards it.
    d = fresh(root, f"{name}-t2")
    write(d, BASE + ctl(BEGIN, 2) + TX[0])
    up, got, db, tail = boot(binary, d, ("a", "b"))
    report(up and (got == ["old", "old"] if new else True), name, "T2 incomplete tail group: what it replays",
           f"up={up} got={got} (['new','old'] = the prefix applied) {'' if up else tail}")

    # T2b: the same with the second member torn part-way.
    d = fresh(root, f"{name}-t2b")
    body = BASE + ctl(BEGIN, 2) + TX[0]
    write(d, body + TX[1][:9])
    up, got, db, tail = boot(binary, d, ("a", "b"))
    cut = len(read(d))
    report(up and (got == ["old", "old"] if new else True), name, "T2b incomplete group, member torn: replay and cut",
           f"up={up} got={got} file={cut} (torn record cut: {cut == len(body)})")

    # T3: downgrade by the documented procedure: SAVE, then the older
    # binary writes, crashes, restarts. The markers are truncated away.
    d = fresh(root, f"{name}-t3")
    write(d, BASE + group(*TX))
    s = Server(binary, d)
    if s.start():
        saved = cmd(s.port, "SAVE")
        left = read(d)
        cmd(s.port, "SET", "d", "1")
        s.kill9()
        up2 = s.start()
        got = [cmd(s.port, "GET", k) for k in ("a", "b", "d")] if up2 else None
        s.kill9()
        groups_left = str(control_id(BEGIN, 2)).encode() in left
        report(saved == "OK" and not groups_left and up2 and got == ["new", "new", "1"], name,
               "T3 SAVE, write, SIGKILL, restart (downgrade procedure)",
               f"save={saved} got={got} group_markers_after_save={groups_left}")
    else:
        report(False, name, "T3 SAVE, write, SIGKILL, restart (downgrade procedure)", s.tail())

    # T4: an older binary recovers a crashed grouped file directly (no
    # SAVE) and appends after the dangling group. Its appended record's
    # encoding is what lets 6O-I tell the group was abandoned (§5.3).
    d = fresh(root, f"{name}-t4")
    body = BASE + ctl(BEGIN, 2) + TX[0]
    write(d, body)
    s = Server(binary, d)
    up = s.start()
    if up:
        cmd(s.port, "SET", "d", "1")
    s.kill9()
    data = read(d)
    appended = data[len(body):]
    keep(f"t4-{name}", data)
    # 6O-I cuts the dangling group first and continues in arrays.
    t4_ok = (data.startswith(BASE) and b"\r\nnew\r\n" not in data
             and data[len(BASE):len(BASE) + 1] == b"*") if new else appended[:1] == b">"
    report(up and t4_ok, name,
           "T4 appends after a dangling group as a push" if not new else
           "T4 (6O-I) cuts the dangling group, then appends",
           f"up={up} appended={appended[:12]!r}")

    # T5: the must-understand `txn` bit. Timeless replay keeps an entry
    # whose deadline passed until a logged DEL; legacy replay evaluates it.
    # `APPEND` then shows which. Legacy: the entry is gone when `APPEND`
    # runs, which creates a permanent 'x'. Timeless: the expired entry is
    # kept, `APPEND` extends it and keeps its past deadline, and the read
    # masks it (nil).
    past = str(int(time.time() * 1000) - 60_000)
    for case, head, label in (("t5", FMT_H, "expirylog only"),
                              ("t5b", FMT_TXN, "expirylog + txn")):
        d = fresh(root, f"{name}-{case}")
        write(d, head + arr("SET", "k", "v", "PXAT", past) + arr("APPEND", "k", "x"))
        up, got, db, tail = boot(binary, d, ("k",))
        mode = ("timeless" if got == [None] else "legacy" if got == ["x"] else f"? {got}")
        report(up and (mode == "timeless" if new else True), name, f"T{case[1:]} FORMAT {label}: replay mode",
               f"up={up} k={got} -> {mode} {'' if up else tail}")
    # ... and what it appends to a txn-bit file: pushes if it does not
    # understand the bit (or the format at all).
    d = fresh(root, f"{name}-t5c")
    body = FMT_TXN + arr("SET", "a", "1")
    write(d, body)
    s = Server(binary, d)
    up = s.start()
    if up:
        cmd(s.port, "SET", "z", "1")
    s.kill9()
    appended = read(d)[len(body):]
    report(up and (appended[:1] == b"*" if new else True), name, "T5c appends to a txn-bit file as",
           f"up={up} first_byte={appended[:1]!r} ('>' = push, '*' = array)")

    # T6: the group's markers around the snapshot's own checkpoint marker
    # do not move the replay start.
    d = fresh(root, f"{name}-t6")
    s = Server(binary, d)
    if s.start():
        cmd(s.port, "SET", "snap", "s")
        cmd(s.port, "SAVE")
        s.kill9()
        write(d, FMT_TXN + group(arr("SET", "pre", "x"), arr("SET", "pre2", "x")) +
              resp_push("RECACHED-CHECKPOINT", "1") +
              group(arr("SET", "post", "y"), arr("SET", "post2", "y")))
        up, got, db, tail = boot(binary, d, ("snap", "pre", "post", "post2"))
        report(up and got == ["s", None, "y", "y"], name,
               "T6 replay starts after the snapshot's marker, not a group's",
               f"got={got} {'' if up else tail}")
    else:
        report(False, name, "T6 replay starts after the snapshot's marker", s.tail())

    # T7 (rejected option C): Redis-style MULTI/EXEC records.
    d = fresh(root, f"{name}-t7")
    write(d, BASE + arr("MULTI") + TX[0] + TX[1] + arr("EXEC"))
    up, got, db, tail = boot(binary, d, ("a", "b"))
    report(True, name, "T7 option C, MULTI/EXEC records (rejected)",
           f"started={up} got={got} {'' if up else tail}")

    # T8 (rejected option B): one composite record.
    d = fresh(root, f"{name}-t8")
    write(d, BASE + arr("RECACHED-TXN", "2", "SET", "a", "new", "SET", "b", "new"))
    up, got, db, tail = boot(binary, d, ("a", "b"))
    report(True, name, "T8 option B, one composite record (rejected)",
           f"started={up} got={got} {'' if up else tail}")

    # T9: control ids at the edges of the new kinds' payloads.
    d = fresh(root, f"{name}-t9")
    write(d, FMT_TXN + ctl(BEGIN, 0xFFFFFFFF) + ctl(COMMIT, 0xFFFFFFFF) +
          ctl(BEGIN, 2) + ctl(COMMIT, 2) + arr("SET", "a", "1"))
    up, got, db, tail = boot(binary, d, ("a",))
    report(True, name, "T9 malformed groups (empty, huge count)",
           f"started={up} got={got} {'' if up else tail}")


def main():
    args = sys.argv[1:]
    files_out = None
    if args and args[0] == "--files-out":
        files_out = args[1]
        os.makedirs(files_out, exist_ok=True)
        args = args[2:]
    pairs = [a.split("=", 1) for a in args]
    root = tempfile.mkdtemp(prefix="rc6oi-txn-", dir=os.environ.get("TXN_TMP"))
    try:
        for name, binary in pairs:
            run_cases(name, binary, root, files_out)
    finally:
        shutil.rmtree(root, ignore_errors=True)
    print(f"{sum(RESULTS)}/{len(RESULTS)} checks as expected")
    sys.exit(0 if all(RESULTS) else 1)


if __name__ == "__main__":
    main()
