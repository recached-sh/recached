#!/usr/bin/env python3
"""6L-G soak helpers. Python 3.9+, standard library only. Run with `python3 -I`.

    cmd <port> <arg>...                       one command; prints the reply (a redis-cli stand-in)
    ping <port> <timeout-s>                   waits for PONG; exit 1 on timeout
    preload <port> <bg> <stable>              bg:1..<bg> (64 B) and stable:0..<stable>-1 (16 B), no TTL
    load bg|ttl <port> <conns> <rate> <seconds> <out.json>
        Rate-limited background load, <rate> commands/s per connection.
        bg:  SET:GET 1:4 over bg:1..$BG_KEYS, 64 B values (keys exist: overwrites)
        ttl: SET ttl:<random> 32 B EX 10, pipelined 4 (creates; evictable)
    burst <port> <n> <size> <ttl-s>           n pipelined SET stall:<k> of <size> bytes EX <ttl>
    scan <port> <seconds> <bg> <stable> <out.json> [<metrics-url>]
        Full SCAN traversals in a loop, COUNT cycling 10/100/1000. Every key that
        exists for the whole traversal (bg:*, stable:*: never deleted, never
        evictable under volatile-lru) must come back exactly once, and every key
        must come back in strictly increasing byte order (the 6L-C contract).
        With a metrics URL, a traversal that overlapped a full resync is marked.
    compare <primary-port> <replica-port> <out.json>
        Exact keyspace comparison (quiesced): same key set except ttl:/stall:,
        the same value for every key, the same type where MGET says nil.
    info <port> <field>                       prints one INFO field
    metric <url> <name>                       sums a Prometheus metric family over its labels
    verdict <out-dir> <max-keys>              reads every summary; writes verdict.json; exit 1 on a FAIL

AUTH: every connection sends `AUTH $SOAK_PASSWORD` when it is set.
"""
import asyncio
import json
import os
import random
import socket
import sys
import time
import urllib.request

PW = os.environ.get("SOAK_PASSWORD")


def enc(*args):
    out = [b"*%d\r\n" % len(args)]
    for a in args:
        a = a if isinstance(a, bytes) else str(a).encode()
        out.append(b"$%d\r\n%s\r\n" % (len(a), a))
    return b"".join(out)


class Err(str):
    """An error reply."""


class Conn:
    def __init__(self, port, timeout=None):
        self.s = socket.create_connection(("127.0.0.1", int(port)), timeout=5)
        self.s.settimeout(timeout)
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.buf = b""
        if PW and self.cmd("AUTH", PW) != "OK":
            raise ConnectionError("AUTH refused")

    def send(self, *cmds):
        self.s.sendall(b"".join(enc(*c) for c in cmds))

    def cmd(self, *args):
        self.s.sendall(enc(*args))
        return self.read()

    def _fill(self):
        chunk = self.s.recv(1 << 16)
        if not chunk:
            raise ConnectionError("closed")
        self.buf += chunk

    def _line(self):
        while b"\r\n" not in self.buf:
            self._fill()
        line, self.buf = self.buf.split(b"\r\n", 1)
        return line

    def read(self):
        """Simple strings and integers as str, bulk strings as bytes, errors as Err."""
        line = self._line()
        t, rest = line[:1], line[1:]
        if t in (b"+", b":"):
            return rest.decode()
        if t == b"-":
            return Err(rest.decode(errors="replace"))
        if t == b"$":
            n = int(rest)
            if n < 0:
                return None
            while len(self.buf) < n + 2:
                self._fill()
            v, self.buf = self.buf[:n], self.buf[n + 2:]
            return v
        if t == b"*":
            n = int(rest)
            return None if n < 0 else [self.read() for _ in range(n)]
        raise ValueError(f"unexpected {line!r}")


def show(v):
    if isinstance(v, bytes):
        return v.decode(errors="replace")
    if isinstance(v, list):
        return "\n".join(show(x) for x in v)
    return "" if v is None else str(v)


def quantiles(xs):
    if not xs:
        return None
    xs.sort()
    q = lambda f: round(xs[min(len(xs) - 1, int(f * len(xs)))], 3)
    return {"n": len(xs), "p50": q(0.5), "p99": q(0.99), "p999": q(0.999), "max": round(xs[-1], 3)}


# ── cmd / ping / info / metric ────────────────────────────────────────────────
def cmd(port, *args):
    try:
        print(show(Conn(port, timeout=10).cmd(*args)))
    except (OSError, ConnectionError, ValueError) as e:
        print(f"(error) {e}", file=sys.stderr)
        sys.exit(1)


def ping(port, timeout):
    end = time.monotonic() + float(timeout)
    while time.monotonic() < end:
        try:
            if Conn(port, timeout=2).cmd("PING") == "PONG":
                return
        except (OSError, ConnectionError, ValueError):
            pass
        time.sleep(0.25)
    sys.exit(1)


def info_field(port, field):
    raw = Conn(port, timeout=10).cmd("INFO")
    for line in show(raw).splitlines():
        if line.startswith(field + ":"):
            return line.split(":", 1)[1].strip()
    return ""


def info(port, field):
    try:
        print(info_field(port, field))
    except (OSError, ConnectionError, ValueError):
        print("")


def metric_value(url, name):
    try:
        body = urllib.request.urlopen(url, timeout=5).read().decode()
    except OSError:
        return None
    total, seen = 0.0, False
    for line in body.splitlines():
        if line.startswith(name + " ") or line.startswith(name + "{"):
            total += float(line.rsplit(" ", 1)[1])
            seen = True
    return total if seen else 0.0


def metric(url, name):
    v = metric_value(url, name)
    print("" if v is None else f"{v:g}")


# ── preload / burst ───────────────────────────────────────────────────────────
def preload(port, bg, stable):
    c = Conn(port, timeout=60)
    bg, stable = int(bg), int(stable)
    val = b"v" * 64
    for kind, n, first, v in (("bg", bg, 1, val), ("stable", stable, 0, b"s" * 16)):
        for start in range(first, first + n, 1000):
            args = ["MSET"]
            for i in range(start, min(first + n, start + 1000)):
                args += [f"{kind}:{i}", v]
            r = c.cmd(*args)
            if r != "OK":
                print(f"preload failed at {kind}:{start}: {r}", file=sys.stderr)
                sys.exit(1)
    print(f"preloaded {bg} bg + {stable} stable keys")


def burst(port, n, size, ttl):
    c = Conn(port, timeout=60)
    n, size = int(n), int(size)
    val = os.urandom(size // 2).hex().encode()[:size]
    bad = 0
    for start in range(0, n, 50):
        k = min(n, start + 50) - start
        c.send(*[("SET", f"stall:{start + j}", val, "EX", ttl) for j in range(k)])
        bad += sum(1 for _ in range(k) if c.read() != "OK")
    print(f"burst {n} x {size} B, {bad} refused")


# ── load ──────────────────────────────────────────────────────────────────────
async def _reply(r):
    line = await r.readline()
    if not line:
        raise ConnectionError("closed")
    t, rest = line[:1], line[1:-2]
    if t in (b"+", b":"):
        return rest.decode()
    if t == b"-":
        return Err(rest.decode(errors="replace"))
    if t == b"$":
        n = int(rest)
        return None if n < 0 else (await r.readexactly(n + 2))[:-2]
    if t == b"*":
        n = int(rest)
        return None if n < 0 else [await _reply(r) for _ in range(n)]
    raise ValueError(f"unexpected {line!r}")


async def _load(kind, port, conns, rate, seconds, out):
    bg_keys = int(os.environ.get("BG_KEYS", "200000"))
    pipe = 4 if kind == "ttl" else 1
    loop = asyncio.get_running_loop()
    end = loop.time() + seconds
    st = {"kind": kind, "conns": conns, "rate_per_conn": rate, "ops": 0, "errors": {}, "error_samples": [],
          "reconnects": 0}
    lat = []

    async def one(cid):
        rnd = random.Random(cid * 7919 + os.getpid())
        val = (b"%064d" % cid) if kind == "bg" else (b"%032d" % cid)
        nxt = loop.time()
        while loop.time() < end:
            try:
                r, w = await asyncio.open_connection("127.0.0.1", port)
                if PW:
                    w.write(enc("AUTH", PW))
                    if await _reply(r) != "OK":
                        raise ConnectionError("AUTH refused")
                while loop.time() < end:
                    if kind == "bg":
                        k = f"bg:{rnd.randint(1, bg_keys)}"
                        batch = [("SET", k, val)] if rnd.random() < 0.2 else [("GET", k)]
                    else:
                        batch = [("SET", f"ttl:{rnd.randint(1, 5_000_000)}", val, "EX", 10) for _ in range(pipe)]
                    t0 = time.perf_counter_ns()
                    w.write(b"".join(enc(*c) for c in batch))
                    for _ in batch:
                        v = await _reply(r)
                        if isinstance(v, Err):
                            code = v.split(" ", 1)[0]
                            st["errors"][code] = st["errors"].get(code, 0) + 1
                            if len(st["error_samples"]) < 5:
                                st["error_samples"].append(str(v))
                    ms = (time.perf_counter_ns() - t0) / 1e6
                    if len(lat) < 500_000:
                        lat.append(ms)
                    elif rnd.random() < 0.01:
                        lat[rnd.randrange(len(lat))] = ms
                    st["ops"] += len(batch)
                    nxt += len(batch) / rate
                    wait = nxt - loop.time()
                    if wait > 0:
                        await asyncio.sleep(wait)
                    elif wait < -1:
                        nxt = loop.time()  # fell behind by more than 1 s: do not burst to catch up
                w.close()
            except (OSError, ConnectionError, ValueError, asyncio.IncompleteReadError):
                st["reconnects"] += 1
                await asyncio.sleep(0.5)

    await asyncio.gather(*(one(i) for i in range(conns)))
    st["latency_ms"] = quantiles(lat)
    st["ops_per_s"] = round(st["ops"] / seconds)
    json.dump(st, open(out, "w"))
    print(json.dumps({k: st[k] for k in ("kind", "ops", "ops_per_s", "errors", "reconnects")}))


def load(kind, port, conns, rate, seconds, out):
    asyncio.run(_load(kind, int(port), int(conns), float(rate), float(seconds), out))


# ── scan ──────────────────────────────────────────────────────────────────────
def scan(port, seconds, bg, stable, out, metrics_url=None):
    bg, stable = int(bg), int(stable)
    end = time.monotonic() + float(seconds)
    pause = float(os.environ.get("SCAN_PAUSE", "5"))
    counts = [10, 100, 1000]
    runs, interrupted, n = [], 0, 0
    c = None
    while time.monotonic() < end:
        count = counts[n % len(counts)]
        n += 1
        before = metric_value(metrics_url, "recached_replica_full_resync_total") if metrics_url else None
        seen_bg, seen_st = bytearray(bg + 1), bytearray(stable)
        others = set()
        prev, order_viol, dup_other, keys, calls = None, 0, 0, 0, 0
        t0 = time.monotonic()
        try:
            if c is None:
                c = Conn(port, timeout=5)
            cursor = "0"
            while True:
                rep = c.cmd("SCAN", cursor, "COUNT", count)
                if not isinstance(rep, list) or len(rep) != 2:
                    raise ConnectionError(f"SCAN answered {rep!r}"[:200])
                cursor = show(rep[0])
                calls += 1
                for k in rep[1]:
                    keys += 1
                    if prev is not None and k <= prev:
                        order_viol += 1
                    prev = k
                    if k.startswith(b"bg:") and k[3:].isdigit() and 0 < int(k[3:]) <= bg:
                        i = int(k[3:])
                        seen_bg[i] = min(255, seen_bg[i] + 1)
                    elif k.startswith(b"stable:") and k[7:].isdigit() and int(k[7:]) < stable:
                        i = int(k[7:])
                        seen_st[i] = min(255, seen_st[i] + 1)
                    elif k in others:
                        dup_other += 1
                    else:
                        others.add(k)
                if cursor == "0":
                    break
        except (OSError, ConnectionError, ValueError):
            interrupted += 1
            c = None
            time.sleep(1)
            continue
        after = metric_value(metrics_url, "recached_replica_full_resync_total") if metrics_url else None
        runs.append({
            "count": count, "secs": round(time.monotonic() - t0, 3), "calls": calls, "keys": keys,
            "missing_bg": sum(1 for x in seen_bg[1:] if x == 0), "dup_bg": sum(1 for x in seen_bg[1:] if x > 1),
            "missing_stable": seen_st.count(0), "dup_stable": sum(1 for x in seen_st if x > 1),
            "order_violations": order_viol, "dup_other": dup_other,
            "overlapped_full_resync": before is not None and after is not None and after != before,
        })
        time.sleep(pause)
    clean = [r for r in runs if not r["overlapped_full_resync"]]
    res = {
        "port": int(port), "traversals": len(runs), "interrupted": interrupted,
        "overlapped_full_resync": len(runs) - len(clean),
        "bad_traversals": sum(1 for r in clean if r["missing_bg"] or r["dup_bg"] or r["missing_stable"]
                              or r["dup_stable"] or r["order_violations"]),
        "order_violations": sum(r["order_violations"] for r in clean),
        "missing": sum(r["missing_bg"] + r["missing_stable"] for r in clean),
        "duplicates": sum(r["dup_bg"] + r["dup_stable"] for r in clean),
        "dup_other": sum(r["dup_other"] for r in runs),
        "secs_by_count": {str(k): quantiles([r["secs"] for r in clean if r["count"] == k]) for k in counts},
        "overlapped_samples": [r for r in runs if r["overlapped_full_resync"]][:5],
        "bad_samples": [r for r in clean if r["missing_bg"] or r["dup_bg"] or r["missing_stable"]
                        or r["dup_stable"] or r["order_violations"]][:5],
    }
    json.dump(res, open(out, "w"))
    print(json.dumps({k: res[k] for k in ("traversals", "interrupted", "bad_traversals", "order_violations",
                                          "missing", "duplicates")}))


# ── compare ───────────────────────────────────────────────────────────────────
VOLATILE = (b"ttl:", b"stall:")


def _all_keys(c):
    keys, cursor = set(), "0"
    while True:
        rep = c.cmd("SCAN", cursor, "COUNT", 1000)
        cursor = show(rep[0])
        keys.update(rep[1])
        if cursor == "0":
            return keys


def compare(pport, rport, out):
    p, r = Conn(pport, timeout=30), Conn(rport, timeout=30)
    pk, rk = _all_keys(p), _all_keys(r)
    vol = lambda k: k.startswith(VOLATILE)
    pks, rks = {k for k in pk if not vol(k)}, {k for k in rk if not vol(k)}
    only_p, only_r = sorted(pks - rks), sorted(rks - pks)
    both = sorted(pks & rks)
    mismatch = []
    for i in range(0, len(both), 500):
        chunk = both[i:i + 500]
        a, b = p.cmd("MGET", *chunk), r.cmd("MGET", *chunk)
        for k, x, y in zip(chunk, a, b):
            if x != y or (x is None and p.cmd("TYPE", k) != r.cmd("TYPE", k)):
                mismatch.append(k)
    res = {
        "primary_keys": len(pk), "replica_keys": len(rk), "compared": len(both),
        "volatile_primary": len(pk) - len(pks), "volatile_replica": len(rk) - len(rks),
        "only_primary": len(only_p), "only_replica": len(only_r), "value_mismatch": len(mismatch),
        "samples": {"only_primary": [show(k) for k in only_p[:5]], "only_replica": [show(k) for k in only_r[:5]],
                    "value_mismatch": [show(k) for k in mismatch[:5]]},
    }
    res["identical"] = not (only_p or only_r or mismatch)
    json.dump(res, open(out, "w"))
    print(json.dumps({k: res[k] for k in ("identical", "compared", "only_primary", "only_replica",
                                          "value_mismatch")}))
    sys.exit(0 if res["identical"] else 1)


# ── verdict ───────────────────────────────────────────────────────────────────
def verdict(outdir, max_keys):
    max_keys = int(max_keys)

    def j(name):
        try:
            return json.load(open(os.path.join(outdir, name)))
        except (OSError, ValueError):
            return None

    facts = {}
    try:
        for line in open(os.path.join(outdir, "facts.txt")):
            if "=" in line:
                k, v = line.rstrip("\n").split("=", 1)
                facts[k] = v
    except OSError:
        pass
    num = lambda k: float(facts.get(k) or 0)
    rows = []

    def check(name, ok, detail, info=False):
        rows.append({"check": name, "result": ("INFO" if info else "PASS" if ok else "FAIL"), "detail": detail})

    def need(name, data, ok, detail):
        if data is None:
            check(name, False, "summary missing (the step did not finish)")
        else:
            check(name, ok(data), detail(data))

    # Durability and replication.
    need("ledger after primary crash recovery", j("ledger-recovered.json"),
         lambda d: d["lost"] == 0 and d["dedup_mismatch"] == 0 and d["beyond_in_flight"] == 0,
         lambda d: f"{d['acknowledged']} acknowledged INCRs, lost {d['lost']}, dedup mismatch {d['dedup_mismatch']}")
    need("ledger on the replica", j("ledger-replica.json"),
         lambda d: d["lost"] == 0 and d["dedup_mismatch"] == 0 and d["beyond_in_flight"] == 0,
         lambda d: f"lost {d['lost']}, dedup mismatch {d['dedup_mismatch']}")
    need("WATCH/MULTI/EXEC compare-and-set", j("cas.json"), lambda d: d["exact"],
         lambda d: f"{d['committed']} committed, {d['aborted']} aborted, racer {d['racer_incrs']}, exact {d['exact']}")
    need("DEDUP retries (probe writer)", j("ws-writer.json"),
         lambda d: d["dedupWrong"] == 0 and d["errors"] == 0 and d["dedupApplied"] == d["dedupFirst"],
         lambda d: f"{d['dedupFirst']} firsts, {d['dedupDup']} DUP, wrong {d['dedupWrong']}, applied {d['dedupApplied']}")
    need("replica == primary (quiesced)", j("compare-quiesced.json"), lambda d: d["identical"],
         lambda d: f"{d['compared']} keys; only primary {d['only_primary']}, only replica {d['only_replica']}, "
                   f"value mismatch {d['value_mismatch']}")
    need("replica == primary (after the primary's crash recovery)", j("compare-recovered.json"),
         lambda d: d["identical"],
         lambda d: f"{d['compared']} keys; only primary {d['only_primary']}, only replica {d['only_replica']}, "
                   f"value mismatch {d['value_mismatch']}")

    # Caps (6L-A follower mode, 6L-B reservations).
    check("replica evicted nothing of its own (follower mode)", num("replica_evicted_keys") == 0,
          f"replica evicted_keys {facts.get('replica_evicted_keys', '?')}")
    check("primary's key cap was binding", num("primary_evicted_keys") > 0,
          f"primary evicted_keys {facts.get('primary_evicted_keys', '?')} (0 means the soak never exercised eviction)",
          info=num("primary_evicted_keys") == 0)
    tol = int(num("primary_workers")) + 1
    check("DBSIZE never above the key cap", num("max_primary_dbsize") <= max_keys + tol,
          f"max sampled {facts.get('max_primary_dbsize', '?')}, cap {max_keys} (+{tol} in-flight tolerance)")
    for f, label in (("load-bg.json", "background load"), ("load-ttl.json", "TTL churn")):
        need(f"{label}: no refused writes", j(f), lambda d: not d["errors"],
             lambda d: f"{d['ops']} ops at {d['ops_per_s']}/s, errors {d['errors']}, p99 "
                       f"{(d['latency_ms'] or {}).get('p99')} ms")

    # Full resync of a live replica (6L-A).
    stalls, resyncs = num("stalls"), num("replica_full_resyncs_during_stalls")
    check("each replica stall forced a full resync", stalls > 0 and resyncs >= stalls,
          f"{int(stalls)} stalls, {int(resyncs)} full resyncs of the live replica", info=stalls == 0)
    rws = j("replica-ws.json")
    need("replica WebSocket clients closed with 4001 on full resync", rws,
         lambda d: resyncs == 0 or d["closes"].get("4001", 0) > 0,
         lambda d: f"closes by code {d['closes']}, reconnects {d['reconnects']}, leaks {d['leaks']}")

    # WebSocket scoping.
    for f in ("ws-clients.json", "route-G.json", "route-H.json", "route-J.json", "narrow.json", "replica-ws.json"):
        need(f"no cross-scope push ({f[:-5]})", j(f), lambda d: d["leaks"] == 0,
             lambda d: f"{d['pushes']} pushes, leaks {d['leaks']}"
                       + (" (no writes target this group: it tests renewal, churn or backpressure)" if not d["pushes"] else ""))
    need("token renewal (route G)", j("route-G.json"), lambda d: d["renewFailures"] == 0 and d["renewals"] > 0,
         lambda d: f"{d['renewals']} renewals, {d['renewFailures']} failures")
    need("QSUB churn (route H)", j("route-H.json"), lambda d: d["churnFailures"] == 0 and d["churns"] > 0,
         lambda d: f"{d['churns']} QSUB/QUNSUB, {d['churnFailures']} failures")
    need("scope narrowing", j("narrow.json"),
         lambda d: d["narrowings"] > 0 and d["tokenFailures"] == 0 and d["revokeMissing"] == 0
         and d["unexpectedCloses"] == 0 and d["inScopeAfterNarrow"] > 0,
         lambda d: f"{d['narrowings']} narrowings, {d['revokeCloses']} 4003 closes for revoked QSUBs "
                   f"(missing {d['revokeMissing']}), unexpected closes {d['unexpectedCloses']}, "
                   f"in-scope pushes after narrowing {d['inScopeAfterNarrow']}, token failures {d['tokenFailures']}")

    # SCAN (6L-C).
    for f, label in (("scan-primary.json", "primary"), ("scan-replica.json", "replica")):
        need(f"SCAN exactly once, byte order ({label})", j(f),
             lambda d: d["traversals"] > 0 and d["bad_traversals"] == 0,
             lambda d: f"{d['traversals']} traversals ({d['interrupted']} interrupted, "
                       f"{d['overlapped_full_resync']} overlapped a full resync): missing {d['missing']}, "
                       f"duplicates {d['duplicates']}, order violations {d['order_violations']}")

    # Process health.
    check("no panics", num("panics") == 0, f"{facts.get('panics', '?')} 'panicked at' lines in server logs")
    check("primary RSS growth over the second half", True,
          f"{facts.get('rss_second_half_growth_pct', '?')}% (first-half end {facts.get('rss_mid_kb', '?')} kB, "
          f"end {facts.get('rss_end_kb', '?')} kB)", info=True)
    check("primary restart from snapshot + AOF", True, f"{facts.get('recovery_ms', '?')} ms", info=True)
    lag = facts.get("replica_lag", "")
    check("replica lag p50/p99/max ms (arrived probes)", True, f"{lag}; {facts.get('probe_timeouts', '?')} timeouts "
          f"(expected during stalls and restarts)", info=True)

    fails = [r for r in rows if r["result"] == "FAIL"]
    json.dump({"pass": not fails, "rows": rows, "facts": facts}, open(os.path.join(outdir, "verdict.json"), "w"),
              indent=1)
    w = max(len(r["check"]) for r in rows)
    for r in rows:
        print(f"{r['result']:4}  {r['check']:<{w}}  {r['detail']}")
    print(f"\n{'PASS' if not fails else 'FAIL'}: {len(rows) - len(fails)} of {len(rows)} checks pass or are informational")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    cmds = {"cmd": cmd, "ping": ping, "preload": preload, "load": load, "burst": burst, "scan": scan,
            "compare": compare, "info": info, "metric": metric, "verdict": verdict}
    if len(sys.argv) < 2 or sys.argv[1] not in cmds:
        print(__doc__, file=sys.stderr)
        sys.exit(64)
    cmds[sys.argv[1]](*sys.argv[2:])
