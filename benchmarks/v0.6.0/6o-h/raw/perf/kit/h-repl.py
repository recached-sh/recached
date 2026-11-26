#!/usr/bin/env python3
"""6O-H native measurements the request-path kit does not cover. Each
experiment runs for every binary given, REPS times, alternating order.

  replay   restart time after 1,000,000 SETs in the AOF (everysec), and for
           the 6O-H binary also on the baseline's (legacy) file
  sweep    1,000,000 keys with PX 1000 expiring under a GET load: sweeper
           drain time, AOF bytes the sweep added, GET p50/p99 meanwhile
  repl     primary + replica, SET PX 100 at c50 for 30 s: primary ops/s, the
           replica's DBSIZE and RSS against the primary's every second,
           replica bytes read (/proc io), after the load the time for the
           replica's DBSIZE to fall to the primary's
  lag      as repl, the replica SIGSTOPped for 10 s from t=10 s: peak replica
           DBSIZE/RSS and the catch-up time

usage: h-repl.py OUTDIR NAME=BINARY ...   env EXPS=replay,sweep,repl,lag REPS=3
"""
import json, os, shutil, signal, socket, subprocess, sys, tempfile, time

BIN = os.path.expanduser("~/recached-6l-bench/bin")
BENCH = os.path.join(BIN, "redis-benchmark")
SERVER_CPUS = os.environ.get("SERVER_CPUS", "0")
REPLICA_CPUS = os.environ.get("REPLICA_CPUS", "1")
BENCH_CPUS = os.environ.get("BENCH_CPUS", "3-5")
N_REPLAY = int(os.environ.get("N_REPLAY", "1000000"))
N_SWEEP = int(os.environ.get("N_SWEEP", "1000000"))
REPL_SECS = float(os.environ.get("REPL_SECS", "30"))


def free_port():
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p


def enc(args):
    out = b"*%d\r\n" % len(args)
    for a in args:
        a = a.encode() if isinstance(a, str) else a
        out += b"$%d\r\n%s\r\n" % (len(a), a)
    return out


def cmd(port, *args, timeout=10):
    with socket.create_connection(("127.0.0.1", port), timeout=timeout) as s:
        s.sendall(enc(args))
        buf = b""
        while b"\r\n" not in buf:
            chunk = s.recv(65536)
            if not chunk:
                raise ConnectionError
            buf += chunk
        if buf[:1] == b"$":
            n = int(buf[1:buf.index(b"\r\n")])
            while n >= 0 and len(buf) < buf.index(b"\r\n") + 2 + n + 2:
                buf += s.recv(65536)
        return buf


def dbsize(port, timeout=10):
    try:
        return int(cmd(port, "DBSIZE", timeout=timeout)[1:].split(b"\r\n")[0])
    except (OSError, ValueError):
        return None


def total_commands(port):
    info = cmd(port, "INFO", "stats", timeout=30)
    return int(info.split(b"total_commands_processed:")[1].split(b"\r\n")[0])


def rss_kb(pid):
    with open(f"/proc/{pid}/status") as f:
        for line in f:
            if line.startswith("VmRSS:"):
                return int(line.split()[1])
    return 0


def read_bytes(pid):
    with open(f"/proc/{pid}/io") as f:
        for line in f:
            if line.startswith("rchar:"):
                return int(line.split()[1])
    return 0


class Node:
    def __init__(self, binary, d, aof=None, primary_repl=None):
        self.binary, self.d = binary, d
        self.port, self.repl = free_port(), free_port()
        self.aof, self.primary_repl = aof, primary_repl
        os.makedirs(d, exist_ok=True)

    def start(self, cpus):
        env = {"RECACHED_BIND": "127.0.0.1", "RECACHED_PORT": str(self.port),
               "RECACHED_WS_PORT": str(free_port()), "RECACHED_METRICS_PORT": "0",
               "RECACHED_SAVE_PATH": os.path.join(self.d, "dump.rdb"), "RECACHED_SAVE_INTERVAL": "0",
               "RECACHED_WORKER_THREADS": "1", "RECACHED_REPL_ENABLE": "1",
               "RECACHED_REPL_PORT": str(self.repl), "PATH": "/usr/bin:/bin"}
        if self.aof:
            env.update(RECACHED_AOF_PATH=os.path.join(self.d, "append.aof"), RECACHED_AOF_SYNC=self.aof)
        if self.primary_repl:
            env["RECACHED_REPLICAOF"] = f"127.0.0.1:{self.primary_repl}"
        log = open(os.path.join(self.d, "server.log"), "ab")
        t0 = time.monotonic()
        self.p = subprocess.Popen(["taskset", "-c", cpus, self.binary], cwd=self.d, env=env,
                                  stdout=log, stderr=log)
        while True:
            try:
                if b"PONG" in cmd(self.port, "PING", timeout=2):
                    return time.monotonic() - t0
            except OSError:
                if self.p.poll() is not None:
                    raise RuntimeError(f"server exited: {self.d}/server.log")
                time.sleep(0.01)

    def kill(self):
        if self.p.poll() is None:
            self.p.send_signal(signal.SIGCONT)
            self.p.send_signal(signal.SIGKILL)
            self.p.wait()


def bench(port, n, *args, clients=50, pipe=1, rand=100000, wait=True):
    argv = ["taskset", "-c", BENCH_CPUS, BENCH, "-p", str(port), "-n", str(n), "-c", str(clients),
            "-P", str(pipe), "-r", str(rand), "--threads", "2", "--csv", *args]
    if not wait:
        return subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    out = subprocess.run(argv, capture_output=True, text=True).stdout
    return parse_csv(out)


def parse_csv(out):
    rows = [l for l in out.strip().splitlines() if l.startswith('"') and not l.startswith('"test"')]
    if not rows:
        return {}
    f = [x.strip('"') for x in rows[-1].split(",")]
    return {"rps": float(f[1]), "avg": float(f[2]), "p50": float(f[4]), "p95": float(f[5]),
            "p99": float(f[6]), "max": float(f[7])}


def exp_replay(name, binary, root, legacy_from=None):
    d = os.path.join(root, "replay")
    n = Node(binary, d, aof="everysec")
    if legacy_from:
        os.makedirs(d, exist_ok=True)
        shutil.copy(legacy_from, os.path.join(d, "append.aof"))
    else:
        n.start(SERVER_CPUS)
        bench(n.port, N_REPLAY, "-t", "set", clients=20, pipe=64, rand=N_REPLAY)
        time.sleep(1.5)  # everysec
        n.kill()
    size = os.path.getsize(os.path.join(d, "append.aof"))
    secs = n.start(SERVER_CPUS)
    keys = dbsize(n.port)
    n.kill()
    return {"restart_s": round(secs, 3), "aof_bytes": size, "keys": keys}, os.path.join(d, "append.aof")


def exp_sweep(name, binary, root):
    n = Node(binary, os.path.join(root, "sweep"), aof="everysec")
    n.start(SERVER_CPUS)
    bench(n.port, N_SWEEP, "SET", "key:__rand_int__", "v" * 32, "PX", "1000", clients=20, pipe=64,
          rand=N_SWEEP)
    keys = dbsize(n.port)
    aof0 = os.path.getsize(os.path.join(n.d, "append.aof"))
    g = bench(n.port, 3_000_000, "GET", "key:__rand_int__", rand=N_SWEEP, wait=False)
    t0 = time.monotonic()
    while dbsize(n.port) > 0 and time.monotonic() - t0 < 120:
        time.sleep(0.1)
    drained = time.monotonic() - t0
    get = parse_csv(g.communicate()[0])
    time.sleep(1.5)
    aof1 = os.path.getsize(os.path.join(n.d, "append.aof"))
    n.kill()
    return {"keys": keys, "drain_s": round(drained, 2), "sweep_aof_bytes": aof1 - aof0,
            "sweep_aof_bytes_per_key": round((aof1 - aof0) / max(keys, 1), 1), "get_under_sweep": get}


def exp_repl(name, binary, root, stop=None):
    p = Node(binary, os.path.join(root, "p"))
    p.start(SERVER_CPUS)
    # The replica's AOF (fsync no) measures the replication stream: it logs
    # exactly the frames it applies.
    r = Node(binary, os.path.join(root, "r"), aof="no", primary_repl=p.repl)
    r.start(REPLICA_CPUS)
    cmd(p.port, "SET", "ready", "1")
    while (dbsize(r.port) or 0) < 1:
        time.sleep(0.05)
    raof = os.path.join(r.d, "append.aof")
    rb0 = os.path.getsize(raof) if os.path.exists(raof) else 0
    ops0, t_load = total_commands(p.port), time.monotonic()
    load = bench(p.port, 100_000_000, "SET", "key:__rand_int__", "v" * 32, "PX", "100", rand=100_000,
                 wait=False)
    samples, t0 = [], time.monotonic()
    stopped_at, resumed = None, False
    while time.monotonic() - t0 < REPL_SECS:
        t = time.monotonic() - t0
        if stop and stopped_at is None and not resumed and t >= stop[0]:
            r.p.send_signal(signal.SIGSTOP)
            stopped_at = time.monotonic()
        if stopped_at is not None and time.monotonic() - stopped_at >= stop[1]:
            r.p.send_signal(signal.SIGCONT)
            stopped_at, resumed = None, True
        rd = None
        if stopped_at is None:
            try:
                rd = dbsize(r.port)
            except OSError:
                pass
        samples.append({"t": round(t, 1), "primary_keys": dbsize(p.port, timeout=30), "replica_keys": rd,
                        "replica_rss_kb": rss_kb(r.p.pid), "primary_rss_kb": rss_kb(p.p.pid)})
        time.sleep(1)
    ops1, t_end = total_commands(p.port), time.monotonic()
    load.send_signal(signal.SIGINT)
    load.communicate()
    rb1 = os.path.getsize(raof) if os.path.exists(raof) else 0
    # Catch-up: the replica's DBSIZE down to the primary's (keys gone).
    t1 = time.monotonic()
    while time.monotonic() - t1 < 60:
        rk, pk = dbsize(r.port, timeout=30), dbsize(p.port, timeout=30)
        if rk is not None and pk is not None and abs(rk - pk) <= 10 and pk < 1000:
            break
        time.sleep(0.2)
    catchup = time.monotonic() - t1
    total_ops = ops1 - ops0
    p.kill(); r.kill()
    live = [s for s in samples if s["replica_keys"] is not None]
    return {"samples": samples, "replica_stream_bytes": rb1 - rb0,
            "peak_replica_keys": max((s["replica_keys"] for s in live if s["replica_keys"] is not None), default=None),
            "peak_primary_keys": max((s["primary_keys"] for s in samples if s["primary_keys"] is not None), default=None),
            "peak_replica_rss_kb": max(s["replica_rss_kb"] for s in samples),
            "peak_primary_rss_kb": max(s["primary_rss_kb"] for s in samples),
            "drain_after_load_s": round(catchup, 2),
            "primary_ops_per_s": round(total_ops / (t_end - t_load), 0),
            "replica_stream_bytes_per_op": round((rb1 - rb0) / max(total_ops, 1), 1)}


if __name__ == "__main__":
    out_dir = sys.argv[1]
    os.makedirs(out_dir, exist_ok=True)
    pairs = [a.split("=", 1) for a in sys.argv[2:]]
    exps = os.environ.get("EXPS", "replay,sweep,repl,lag").split(",")
    reps = int(os.environ.get("REPS", "3"))
    results = []
    for rep in range(1, reps + 1):
        order = pairs if rep % 2 else list(reversed(pairs))
        legacy = {}
        for name, binary in order:
            for exp in exps:
                root = tempfile.mkdtemp(prefix=f"h-{exp}-{name}-", dir=os.environ.get("TMPDIR_BENCH"))
                try:
                    if exp == "replay":
                        r, path = exp_replay(name, binary, root)
                        res = {"exp": "replay", **r}
                        if name == "base":
                            keep = os.path.join(out_dir, f"legacy-{rep}.aof")
                            shutil.copy(path, keep); legacy[rep] = keep
                    elif exp == "sweep":
                        res = {"exp": "sweep", **exp_sweep(name, binary, root)}
                    elif exp == "repl":
                        res = {"exp": "repl", **exp_repl(name, binary, root)}
                    else:
                        res = {"exp": "lag", **exp_repl(name, binary, root, stop=(REPL_SECS / 3, REPL_SECS / 3))}
                except Exception as e:  # noqa: BLE001
                    res = {"exp": exp, "error": repr(e)}
                finally:
                    if os.environ.get("KEEP"):
                        for d, _, fs in os.walk(root):
                            for f in fs:
                                if f == "server.log":
                                    keep = os.path.join(out_dir, f"{exp}-{name}-r{rep}-{os.path.basename(d)}.log")
                                    shutil.copy(os.path.join(d, f), keep)
                    shutil.rmtree(root, ignore_errors=True)
                res.update(name=name, rep=rep, binary_sha=subprocess.run(
                    ["sha256sum", binary], capture_output=True, text=True).stdout.split()[0])
                results.append(res)
                print(json.dumps({k: v for k, v in res.items() if k != "samples"}), flush=True)
        # The 6O-H binary on the baseline's legacy file.
        for name, binary in pairs:
            if name != "base" and rep in legacy and "replay" in exps:
                root = tempfile.mkdtemp(prefix="h-legacy-", dir=os.environ.get("TMPDIR_BENCH"))
                try:
                    r, _ = exp_replay(name, binary, root, legacy_from=legacy[rep])
                    res = {"exp": "replay-legacy-file", **r, "name": name, "rep": rep}
                finally:
                    shutil.rmtree(root, ignore_errors=True)
                results.append(res)
                print(json.dumps(res), flush=True)
    with open(os.path.join(out_dir, "results.json"), "w") as f:
        json.dump(results, f, indent=1)
