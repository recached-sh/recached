#!/usr/bin/env python3
"""6O-E2: WATCH against expiry and related changes, Redis vs Recached.

usage: watch_expiry_diff.py NAME=PORT [NAME=PORT ...]

Each case prepares a key, WATCHes it on connection `s`, lets something
happen (time passing, or another connection `o` changing the key), then
runs MULTI / SET <case>:other 1 / EXEC on `s`, and reports whether EXEC
committed or aborted. Sleeps are a few hundred ms, past PX deadlines of
50-100 ms; the one sweeper case waits 1.5 s so an active expiry cycle runs
on both servers.
"""
import socket, sys, time


class Conn:
    def __init__(self, port):
        self.s = socket.create_connection(("127.0.0.1", port))
        self.f = self.s.makefile("rb")

    def __call__(self, *a):
        self.s.sendall(("*%d\r\n" % len(a) + "".join("$%d\r\n%s\r\n" % (len(x), x) for x in a)).encode())
        return self.read()

    def read(self):
        l = self.f.readline(); t = l[:1]
        if t in b"+-:":
            return l[:-2].decode()
        if t == b"$":
            n = int(l[1:-2])
            return None if n < 0 else self.f.read(n + 2)[:-2].decode()
        if t == b"*":
            n = int(l[1:-2])
            return None if n < 0 else [self.read() for _ in range(n)]
        raise RuntimeError(repr(l))


def run_exec(s, k, extra=()):
    assert s("MULTI") == "+OK"
    for cmd in extra:
        s(*cmd)
    assert s("SET", k + ":other", "1") == "+QUEUED"
    r = s("EXEC")
    return "aborted" if r is None else ("committed" if isinstance(r, list) else repr(r))


def case_live_then_expires(s, o, k):
    s("SET", k, "v", "PX", "100"); s("WATCH", k); time.sleep(0.3)
    return run_exec(s, k)

def case_absent_stable(s, o, k):
    s("WATCH", k)
    return run_exec(s, k)

def case_expired_before_watch(s, o, k):
    s("SET", k, "v", "PX", "50"); time.sleep(0.15); s("WATCH", k)
    return run_exec(s, k)

def case_expired_before_watch_then_recreated(s, o, k):
    s("SET", k, "v", "PX", "50"); time.sleep(0.15); s("WATCH", k); o("SET", k, "v2")
    return run_exec(s, k)

def case_expired_before_watch_then_read(s, o, k):
    # Redis deletes the expired key on this read; an already-expired watched
    # key being deleted is "no logical change".
    s("SET", k, "v", "PX", "50"); time.sleep(0.15); s("WATCH", k); o("GET", k)
    return run_exec(s, k)

def case_expired_before_watch_then_swept(s, o, k):
    s("SET", k, "v", "PX", "50"); time.sleep(0.15); s("WATCH", k); time.sleep(1.5)
    return run_exec(s, k)

def case_live_then_expires_then_read(s, o, k):
    s("SET", k, "v", "PX", "100"); s("WATCH", k); time.sleep(0.3); o("GET", k)
    return run_exec(s, k)

def case_live_then_expires_then_swept(s, o, k):
    s("SET", k, "v", "PX", "100"); s("WATCH", k); time.sleep(1.5)
    return run_exec(s, k)

def case_live_then_expires_then_recreated(s, o, k):
    s("SET", k, "v", "PX", "100"); s("WATCH", k); time.sleep(0.3); o("SET", k, "v2")
    return run_exec(s, k)

def case_live_not_yet_expired(s, o, k):
    s("SET", k, "v", "PX", "5000"); s("WATCH", k)
    return run_exec(s, k)

def case_live_deleted(s, o, k):
    s("SET", k, "v"); s("WATCH", k); o("DEL", k)
    return run_exec(s, k)

def case_absent_deleted(s, o, k):
    s("WATCH", k); o("DEL", k)
    return run_exec(s, k)

def case_live_ttl_changed(s, o, k):
    s("SET", k, "v", "PX", "10000"); s("WATCH", k); o("PEXPIRE", k, "20000")
    return run_exec(s, k)

def case_live_persisted(s, o, k):
    s("SET", k, "v", "PX", "10000"); s("WATCH", k); o("PERSIST", k)
    return run_exec(s, k)

def case_live_expire_in_past(s, o, k):
    s("SET", k, "v"); s("WATCH", k); o("PEXPIREAT", k, "1")
    return run_exec(s, k)

def case_live_same_value(s, o, k):
    s("SET", k, "v"); s("WATCH", k); o("SET", k, "v")
    return run_exec(s, k)

def case_absent_created(s, o, k):
    s("WATCH", k); o("SET", k, "v")
    return run_exec(s, k)

def case_two_keys_one_expires(s, o, k):
    s("SET", k + ":1", "v", "PX", "100"); s("SET", k + ":2", "v")
    s("WATCH", k + ":1", k + ":2"); time.sleep(0.3)
    return run_exec(s, k)

def case_two_keys_none_change(s, o, k):
    s("SET", k + ":1", "v", "PX", "5000"); s("SET", k + ":2", "v")
    s("WATCH", k + ":1", k + ":2")
    return run_exec(s, k)

def case_unwatch_before_expiry(s, o, k):
    s("SET", k, "v", "PX", "100"); s("WATCH", k); s("UNWATCH"); time.sleep(0.3)
    return run_exec(s, k)

def case_discard_after_expiry(s, o, k):
    s("SET", k, "v", "PX", "100"); s("WATCH", k); time.sleep(0.3)
    s("MULTI"); s("DISCARD")
    return run_exec(s, k)

def case_expires_between_multi_and_exec(s, o, k):
    s("SET", k, "v", "PX", "100"); s("WATCH", k)
    assert s("MULTI") == "+OK"; s("GET", k); time.sleep(0.3)
    s("SET", k + ":other", "1")
    r = s("EXEC")
    return "aborted" if r is None else "committed"

def case_watch_cycles(s, o, k):
    # Repeated WATCH/EXEC on an expiring key: each cycle independent.
    out = []
    for i in range(3):
        s("SET", k, "v", "PX", "100"); s("WATCH", k); time.sleep(0.25)
        out.append(run_exec(s, k)[0])
        s("WATCH", k); out.append(run_exec(s, k)[0])  # now absent: commits
    return "".join(out)

def case_absent_flushdb(s, o, k):
    s("WATCH", k); o("FLUSHDB")
    return run_exec(s, k)

def case_live_flushdb(s, o, k):
    s("SET", k, "v"); s("WATCH", k); o("FLUSHDB")
    return run_exec(s, k)


# No-op writes: Redis signals a WATCH only when a command modifies the key.
def case_live_setnx_noop(s, o, k):
    s("SET", k, "v"); s("WATCH", k); o("SETNX", k, "x")
    return run_exec(s, k)

def case_live_persist_noop(s, o, k):
    s("SET", k, "v"); s("WATCH", k); o("PERSIST", k)
    return run_exec(s, k)

def case_live_hdel_missing_field(s, o, k):
    s("HSET", k, "f", "v"); s("WATCH", k); o("HDEL", k, "g")
    return run_exec(s, k)

def case_live_srem_missing_member(s, o, k):
    s("SADD", k, "m"); s("WATCH", k); o("SREM", k, "n")
    return run_exec(s, k)

def case_absent_lpushx_noop(s, o, k):
    s("WATCH", k); o("LPUSHX", k, "v")
    return run_exec(s, k)

def case_live_renamed_onto(s, o, k):
    s("SET", k, "v"); s("SET", k + ":src", "w"); s("WATCH", k); o("RENAME", k + ":src", k)
    return run_exec(s, k)

CASES = [(n[len("case_"):], f) for n, f in list(globals().items()) if n.startswith("case_")]


def main():
    targets = [(a.split("=")[0], int(a.split("=")[1])) for a in sys.argv[1:]]
    results = {}
    for name, port in targets:
        s, o = Conn(port), Conn(port)
        s("FLUSHDB")
        for case, f in CASES:
            k = "wx:" + case
            s("UNWATCH")
            results[(name, case)] = f(s, o, k)
    names = [n for n, _ in targets]
    print("| case | " + " | ".join(names) + " | match |")
    print("|---" * (len(names) + 2) + "|")
    for case, _ in CASES:
        row = [results[(n, case)] for n in names]
        print("| %s | %s | %s |" % (case, " | ".join(row), "yes" if len(set(row)) == 1 else "**NO**"))


main()
