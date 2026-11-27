#!/usr/bin/env python3
"""6O-I8 campaign driver (native host, never the Flatpak sandbox).

    i8run.py CONFIG.json

Runs every cell of CONFIG for every rep and label (label order rotated
each rep), each on fresh servers, and appends one JSON record per run to
OUT/runs.jsonl. Per run, OUT/<rep>-<cell>-<label>/ keeps the logs, the
1 Hz series (series.jsonl), the load generator's output, perf output and
the final INFO of each server.

Process safety (brief §3.4):
  * every child is started in its own session/process group and recorded;
    an exit, an exception or SIGTERM/SIGINT/SIGHUP kills those groups and
    nothing else;
  * before a run: no compiler, load below MAX_LOAD, no recached server not
    started by this driver, every port of the run's block free (bound on
    0.0.0.0 to check). A collision marks the attempt INVALID and the run
    is retried once on the next block;
  * after a server answers PONG, INFO server's process_id must be the
    child's pid: otherwise a stale server owns the port and the run is
    INVALID.
Ports stay below 32768 (the kernel's ephemeral range starts there).
"""
import json
import os
import signal
import socket
import subprocess
import sys
import threading
import time

CFG = json.load(open(sys.argv[1]))
B = os.path.expanduser(CFG.get("bench", "~/recached-6oi8-bench"))
OUT = os.path.expanduser(CFG["out"])
os.makedirs(OUT, exist_ok=True)
BIN = f"{B}/bin"
RB = os.path.expanduser("~/recached-6l-bench/bin/redis-benchmark")
HOSTPERF = os.path.expanduser("~/recached-6oi-bench/hostperf.sh")
MAX_LOAD = float(CFG.get("max_load", 3.0))
CHILDREN = {}  # pgid -> description
LOCK = threading.Lock()


def log(msg):
    line = f"{time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())} {msg}"
    print(line, flush=True)
    with open(f"{OUT}/driver.txt", "a") as f:
        f.write(line + "\n")


def kill_group(pgid, sig=signal.SIGKILL):
    try:
        os.killpg(pgid, sig)
    except ProcessLookupError:
        pass


def kill_all(*_):
    with LOCK:
        for pgid in list(CHILDREN):
            kill_group(pgid)
        CHILDREN.clear()


def on_signal(signum, _frame):
    log(f"signal {signum}: stopping every child group")
    kill_all()
    with open(f"{OUT}/ABORTED", "w") as f:
        f.write(f"signal {signum} at {time.time()}\n")
    sys.exit(128 + signum)


for s in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
    signal.signal(s, on_signal)


def spawn(args, env=None, out=None, err=None, desc=""):
    p = subprocess.Popen(args, env=env, stdout=out or subprocess.DEVNULL,
                         stderr=err or subprocess.DEVNULL, stdin=subprocess.DEVNULL,
                         start_new_session=True)
    with LOCK:
        CHILDREN[p.pid] = desc
    return p


def reap(p, timeout=15):
    """Stop a child's whole group: SIGTERM, then SIGKILL."""
    if p is None:
        return
    if p.poll() is None:
        kill_group(p.pid, signal.SIGTERM)
        try:
            p.wait(timeout)
        except subprocess.TimeoutExpired:
            pass
    kill_group(p.pid, signal.SIGKILL)
    try:
        p.wait(5)
    except subprocess.TimeoutExpired:
        pass
    with LOCK:
        CHILDREN.pop(p.pid, None)


# ── RESP ────────────────────────────────────────────────────────────────

class Resp:
    def __init__(self, port, timeout=3.0):
        self.s = socket.create_connection(("127.0.0.1", port), timeout=timeout)
        self.f = self.s.makefile("rb")

    def cmd(self, *args):
        out = f"*{len(args)}\r\n".encode()
        for a in args:
            a = a if isinstance(a, bytes) else str(a).encode()
            out += f"${len(a)}\r\n".encode() + a + b"\r\n"
        self.s.sendall(out)
        return self.read()

    def read(self):
        line = self.f.readline()
        if not line:
            raise ConnectionError("closed")
        t, rest = line[:1], line[1:-2]
        if t in (b"+", b"-"):
            return rest.decode()
        if t == b":":
            return int(rest)
        if t == b"$":
            n = int(rest)
            if n < 0:
                return None
            data = self.f.read(n + 2)
            return data[:-2].decode(errors="replace")
        if t == b"*":
            n = int(rest)
            return None if n < 0 else [self.read() for _ in range(n)]
        raise ValueError(line)

    def info(self, section):
        text = self.cmd("INFO", section) or ""
        d = {}
        for line in text.split("\r\n"):
            if ":" in line and not line.startswith("#"):
                k, v = line.split(":", 1)
                try:
                    d[k] = int(v)
                except ValueError:
                    d[k] = v
        return d

    def close(self):
        try:
            self.s.close()
        except OSError:
            pass


def ping(port):
    try:
        r = Resp(port, 1.0)
        ok = r.cmd("PING") == "PONG"
        r.close()
        return ok
    except OSError:
        return False


def info_once(port, section):
    try:
        r = Resp(port, 3.0)
        d = r.info(section)
        r.close()
        return d
    except (OSError, ValueError, ConnectionError):
        return {}


# ── host ────────────────────────────────────────────────────────────────

def read(path, default=""):
    try:
        with open(path) as f:
            return f.read()
    except OSError:
        return default


def status(pid):
    d = {}
    for line in read(f"/proc/{pid}/status").splitlines():
        k, _, v = line.partition(":")
        if k in ("VmRSS", "VmHWM", "RssAnon", "RssFile", "RssShmem", "VmSwap", "Threads",
                 "voluntary_ctxt_switches", "nonvoluntary_ctxt_switches"):
            d[k] = int(v.split()[0])
    # Context switches are per thread in /proc/<pid>/status: sum the tasks.
    vol = inv = 0
    try:
        for task in os.listdir(f"/proc/{pid}/task"):
            for line in read(f"/proc/{pid}/task/{task}/status").splitlines():
                if line.startswith("voluntary_ctxt_switches:"):
                    vol += int(line.split()[1])
                elif line.startswith("nonvoluntary_ctxt_switches:"):
                    inv += int(line.split()[1])
        d["voluntary_ctxt_switches"], d["nonvoluntary_ctxt_switches"] = vol, inv
    except OSError:
        pass
    return d


def sched_ns(pid):
    total = 0
    try:
        for t in os.listdir(f"/proc/{pid}/task"):
            v = read(f"/proc/{pid}/task/{t}/schedstat").split()
            if v:
                total += int(v[0])
    except OSError:
        return None
    return total


def proc_stat(pid):
    """utime and stime (clock ticks) and minor/major faults from /proc/<pid>/stat."""
    s = read(f"/proc/{pid}/stat")
    if not s:
        return None
    f = s[s.rfind(")") + 2:].split()
    # fields after the comm: state(3) ... minflt(10) majflt(12) utime(14) stime(15), 1-based in proc(5)
    return {"minflt": int(f[7]), "majflt": int(f[9]), "utime": int(f[11]), "stime": int(f[12])}


def cpu_busy_ns():
    f = read("/proc/stat").splitlines()[0].split()[1:]
    v = list(map(int, f))
    hz = os.sysconf("SC_CLK_TCK")
    return (v[0] + v[1] + v[2] + v[5] + v[6]) * 1e9 / hz


def disk(dev="sda"):
    for line in read("/proc/diskstats").splitlines():
        p = line.split()
        if len(p) > 13 and p[2] == dev:
            return {"r_ios": int(p[3]), "r_sect": int(p[5]), "r_ms": int(p[6]),
                    "w_ios": int(p[7]), "w_sect": int(p[9]), "w_ms": int(p[10]),
                    "inflight": int(p[11]), "io_ms": int(p[12]), "wq_ms": int(p[13])}
    return {}


def pressure(kind):
    for line in read(f"/proc/pressure/{kind}").splitlines():
        if line.startswith("some"):
            return float(line.split()[1].split("=")[1])
    return None


def meminfo():
    d = {}
    for line in read("/proc/meminfo").splitlines():
        k, _, v = line.partition(":")
        if k in ("MemAvailable", "SwapFree", "SwapTotal", "Dirty", "Writeback"):
            d[k] = int(v.split()[0])
    return d


def host_snapshot():
    govs = {}
    freqs = []
    for c in range(os.cpu_count()):
        g = read(f"/sys/devices/system/cpu/cpu{c}/cpufreq/scaling_governor").strip()
        govs[g] = govs.get(g, 0) + 1
        f = read(f"/sys/devices/system/cpu/cpu{c}/cpufreq/scaling_cur_freq").strip()
        freqs.append(int(f) // 1000 if f else None)
    return {"loadavg": read("/proc/loadavg").split()[:3], "governors": govs, "mhz": freqs,
            "mem": meminfo(), "psi_cpu": pressure("cpu"), "psi_io": pressure("io"),
            "psi_mem": pressure("memory"), "disk": disk()}


def foreign_servers(ours):
    """Recached servers (any rc-* or recached-server binary) not started here."""
    found = []
    for pid in os.listdir("/proc"):
        if not pid.isdigit() or int(pid) in ours:
            continue
        try:
            exe = os.readlink(f"/proc/{pid}/exe")
        except OSError:
            continue
        base = os.path.basename(exe)
        if base.startswith("rc-") or base == "recached-server" or base in ("txbench", "i8bench"):
            found.append(f"{pid}:{exe}")
    return found


def compilers():
    n = 0
    for pid in os.listdir("/proc"):
        if pid.isdigit() and read(f"/proc/{pid}/comm").strip() in ("cc1", "rustc", "cargo", "ld", "cc1plus", "lld"):
            n += 1
    return n


def ports_free(ports):
    busy = []
    for port in ports:
        for addr in ("0.0.0.0", "127.0.0.1"):
            s = socket.socket()
            try:
                s.bind((addr, port))
            except OSError:
                busy.append(port)
                break
            finally:
                s.close()
    return busy


def wait_quiet(max_wait=1800):
    waited = 0
    if CFG.get("smoke"):
        # A smoke test of the driver, never a measurement.
        return 0, float(read("/proc/loadavg").split()[0]), None
    while True:
        load = float(read("/proc/loadavg").split()[0])
        comp = compilers()
        foreign = foreign_servers(set())
        if load < MAX_LOAD and comp == 0 and not foreign:
            return waited, load, None
        if waited >= max_wait:
            return waited, load, f"host not quiet: load={load} compilers={comp} foreign={foreign}"
        time.sleep(5)
        waited += 5


# ── sampler ─────────────────────────────────────────────────────────────

class Sampler(threading.Thread):
    def __init__(self, path, procs, ports, aof_ports):
        super().__init__(daemon=True)
        self.path, self.procs, self.ports, self.aof_ports = path, procs, ports, aof_ports
        self.stop_evt = threading.Event()
        self.conns = {}
        self.last = {}

    def conn(self, name):
        if name not in self.conns:
            self.conns[name] = Resp(self.ports[name], 2.0)
        return self.conns[name]

    def run(self):
        with open(self.path, "a") as f:
            while not self.stop_evt.is_set():
                t0 = time.time()
                row = {"t": round(t0, 3), "busy_ns": cpu_busy_ns(), "disk": disk(),
                       "psi_io": pressure("io"), "psi_cpu": pressure("cpu"), "mem": meminfo()}
                for name, pid in list(self.procs.items()):
                    if pid is None:
                        continue
                    row[name] = {"st": status(pid), "sched_ns": sched_ns(pid), "ps": proc_stat(pid)}
                for name in list(self.ports):
                    try:
                        c = self.conn(name)
                        r = c.info("replication")
                        if name in self.aof_ports:
                            r.update({k: v for k, v in c.info("persistence").items() if k.startswith("recached_aof_")})
                        row.setdefault(name, {})["info"] = r
                    except (OSError, ValueError, ConnectionError) as e:
                        row.setdefault(name, {})["info_err"] = str(e)[:80]
                        self.conns.pop(name, None)
                row["dt"] = round(time.time() - t0, 3)
                f.write(json.dumps(row) + "\n")
                f.flush()
                self.stop_evt.wait(max(0.0, 1.0 - (time.time() - t0)))
        for c in self.conns.values():
            c.close()

    def stop(self):
        self.stop_evt.set()
        self.join(10)


# ── a run ───────────────────────────────────────────────────────────────

def server_env(port, data, workers, extra, aof=None, repl_port=None, replicaof=None, aof_dir=None, run_dir=None):
    env = {"PATH": "/usr/bin:/bin", "HOME": os.environ.get("HOME", "/tmp"),
           "RECACHED_BIND": "127.0.0.1", "RECACHED_PORT": str(port),
           "RECACHED_WS_PORT": str(port + 1), "RECACHED_METRICS_PORT": "0",
           "RECACHED_SAVE_INTERVAL": "0", "RECACHED_SAVE_PATH": f"{data}/dump-{port}.rdb",
           "RECACHED_WORKER_THREADS": str(workers)}
    if aof_dir:
        # Every file of this server (AOF and snapshot) in that directory.
        env["RECACHED_SAVE_PATH"] = f"{aof_dir}/dump-{port}.rdb"
    if aof:
        env["RECACHED_AOF_PATH"] = f"{aof_dir or data}/append-{port}.aof"
        env["RECACHED_AOF_SYNC"] = aof
    if repl_port:
        env["RECACHED_REPL_ENABLE"] = "1"
        env["RECACHED_REPL_PORT"] = str(repl_port)
    if replicaof:
        env["RECACHED_REPLICAOF"] = replicaof
    env.update({k: str(v).replace("{run_dir}", run_dir or data) for k, v in (extra or {}).items()})
    return env


def start_server(binary, cpus, env, d, name, wait_s=60):
    out = open(f"{d}/{name}.stdout.txt", "w")
    err = open(f"{d}/{name}.log.txt", "w")
    p = spawn(["taskset", "-c", cpus, binary], env=env, out=out, err=err, desc=name)
    port = int(env["RECACHED_PORT"])
    for _ in range(int(wait_s * 4)):
        if p.poll() is not None:
            return p, f"{name} exited at start ({p.returncode})"
        if ping(port):
            pid = info_once(port, "server").get("process_id")
            if pid != p.pid:
                return p, f"{name}: port {port} answered by pid {pid}, not ours ({p.pid})"
            return p, None
        time.sleep(0.25)
    return p, f"{name}: no PONG"


def wait_until(pred, timeout, step=0.5):
    t0 = time.time()
    while time.time() - t0 < timeout:
        if pred():
            return round(time.time() - t0, 2)
        time.sleep(step)
    return None


def repl_state(port):
    i = info_once(port, "replication")
    return i


def converged(port, want=1):
    i = repl_state(port)
    return (i.get("connected_slaves") == want and i.get("recached_replication_lag_frames") == 0
            and i.get("recached_replication_queue_depth") == 0)


def kv_line(text):
    d = {}
    for tok in text.split():
        if "=" in tok:
            k, v = tok.split("=", 1)
            try:
                d[k] = float(v) if "." in v else int(v)
            except ValueError:
                d[k] = v
    return d


def run_once(rep, cell, label, port):
    name = f"{rep}-{cell['name']}-{label}"
    d = f"{OUT}/{name}"
    os.makedirs(d, exist_ok=True)
    data = f"{d}/data"
    os.makedirs(data, exist_ok=True)
    rec = {"rep": rep, "cell": cell["name"], "label": label, "port": port, "valid": True, "why": []}
    suffix = cell.get("bin_suffix", "")
    binary = f"{BIN}/rc-{label}{suffix}"
    rec["binary"] = binary
    pc, rc = cell["primary"], cell.get("replica")
    ports = list(range(port, port + 8))
    waited, load, why = wait_quiet()
    rec.update(waited_s=waited, load_at_start=load)
    if why:
        rec["valid"], rec["why"] = False, [why]
        return rec
    busy = ports_free(ports)
    if busy:
        rec["valid"], rec["why"] = False, [f"port collision {busy}"]
        rec["collision"] = True
        return rec
    rec["host_before"] = host_snapshot()
    prim = repl = gen = None
    sampler = None
    perf_procs = []
    try:
        penv = server_env(port, data, pc["workers"], pc.get("env"), aof=pc.get("aof"),
                          repl_port=port + 2 if rc else None, aof_dir=pc.get("aof_dir"), run_dir=d)
        prim, err = start_server(binary, pc["cpus"], penv, d, "primary")
        if err:
            raise RuntimeError(err)
        procs = {"primary": prim.pid}
        rec["primary_pid"] = prim.pid
        sports = {"primary": port}
        aof_ports = {"primary"} if pc.get("aof") else set()
        if rc:
            raof_dir = rc.get("aof_dir")
            if raof_dir:
                raof_dir = os.path.expanduser(raof_dir) + f"/{name}"
                os.makedirs(raof_dir, exist_ok=True)
            renv = server_env(port + 3, data, rc["workers"], rc.get("env"), aof=rc.get("aof"),
                              replicaof=f"127.0.0.1:{port + 2}", aof_dir=raof_dir, run_dir=d)
            # A replica may run another variant of the same label (e.g. the
            # production build beside a diag primary).
            rbinary = f"{BIN}/rc-{label}{cell.get('replica_bin_suffix', suffix)}"
            rec["replica_binary"] = rbinary
            repl, err = start_server(rbinary, rc["cpus"], renv, d, "replica")
            if err:
                raise RuntimeError(err)
            procs["replica"] = repl.pid
            rec["replica_pid"] = repl.pid
            sports["replica"] = port + 3
            if rc.get("aof"):
                aof_ports.add("replica")
            if wait_until(lambda: converged(port), 60) is None:
                raise RuntimeError("replica never attached")
        load_cfg = cell["load"]
        gcpus = cell.get("gen_cpus", "3-5")
        if load_cfg["kind"] == "contend":
            subprocess.run(["taskset", "-c", gcpus, RB, "-p", str(port), "-n", "300000", "-c", "20",
                            "-P", "64", "-r", "100000", "-d", "32", "-t", "set", "-q"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=300, check=True)
        else:
            subprocess.run(["taskset", "-c", gcpus, f"{BIN}/i8bench", "prefill", str(port),
                            str(load_cfg["keys"])], stdout=subprocess.DEVNULL, timeout=300, check=True)
        if rc and wait_until(lambda: converged(port), 120) is None:
            raise RuntimeError("replica not in sync after prefill")
        rec["prefill_info"] = repl_state(port)
        time.sleep(1)
        sampler = Sampler(f"{d}/series.jsonl", procs, sports, aof_ports)
        sampler.start()
        time.sleep(2)  # two samples before the load
        warm, measure = cell["warm"], cell["measure"]
        gout = open(f"{d}/load.txt", "w")
        if load_cfg["kind"] == "contend":
            gargs = [f"{BIN}/i8bench", "contend", str(port), str(load_cfg["tx_n"]), str(load_cfg["clients"]),
                     str(load_cfg["writers"]), str(warm), str(measure), f"{d}/load-series.tsv"]
        else:
            gargs = [f"{BIN}/i8bench", "paced", str(port), str(load_cfg["rate"]), str(load_cfg["conns"]),
                     str(load_cfg["tx_n"]), str(load_cfg["keys"]), str(warm), str(measure), f"{d}/load"]
        rec["load_start"] = time.time()
        gen = spawn(["taskset", "-c", gcpus] + gargs, out=gout, err=open(f"{d}/load.err.txt", "w"), desc="load")
        procs["gen"] = gen.pid
        # Counting perf over the measured window, and optional sampling.
        time.sleep(warm)
        if cell.get("perf_stat"):
            for who, pid in (("primary", prim.pid), ("replica", repl.pid if repl else None)):
                if pid:
                    perf_procs.append(spawn([HOSTPERF, "stat", "-x,", "-o", f"{d}/perf-stat-{who}.csv", "-e",
                                             "instructions:u,cycles:u,cache-misses:u,branch-misses:u",
                                             "-p", str(pid), "--", "sleep", str(measure)], desc="perf"))
        if cell.get("perf_record"):
            for who, pid in (("primary", prim.pid), ("replica", repl.pid if repl else None)):
                if pid and who in cell["perf_record"]:
                    perf_procs.append(spawn([HOSTPERF, "record", "-e", "cycles:u", "-g", "--user-callchains",
                                             "-F", "499", "-p", str(pid), "-o", f"{d}/perf-{who}.data", "--",
                                             "sleep", str(min(measure, 30))], desc="perf"))
        # Forced resynchronizations under load (review C F4): at each time
        # (seconds after the load's start) the replica is killed and started
        # again on its own files; it replays its AOF, reconnects, and resyncs
        # while the load runs.
        bounces = []
        for at in cell.get("replica_bounce", []):
            delay = rec["load_start"] + at - time.time()
            if delay > 0:
                time.sleep(delay)
            if gen.poll() is not None:
                break
            t_kill = time.time()
            kill_group(repl.pid, signal.SIGKILL)
            repl.wait(10)
            with LOCK:
                CHILDREN.pop(repl.pid, None)
            repl, err = start_server(rec.get("replica_binary", binary), rc["cpus"], renv, d, f"replica-b{len(bounces) + 1}", 900)
            procs["replica"] = repl.pid
            bounces.append({"at": at, "killed": t_kill, "pong_s": round(time.time() - t_kill, 2), "err": err})
            if err:
                raise RuntimeError(f"bounce restart: {err}")
        rec["bounces"] = bounces
        try:
            gen.wait(warm + measure + 600)
        except subprocess.TimeoutExpired:
            raise RuntimeError("load generator did not finish")
        rec["load_end"] = time.time()
        procs.pop("gen", None)
        gout.close()
        for p in perf_procs:
            try:
                p.wait(60)
            except subprocess.TimeoutExpired:
                pass
        rec["load"] = kv_line(read(f"{d}/load.txt"))
        if gen.returncode != 0 or not rec["load"]:
            raise RuntimeError(f"load generator failed ({gen.returncode}): {read(d + '/load.err.txt')[-300:]}")
        # Drain: until the replica holds everything, then the post window.
        if rc:
            t = wait_until(lambda: converged(port), cell.get("drain_max", 120), 0.25)
            rec["catchup_s"] = t
            rec["converged"] = t is not None
            if t is None:
                rec["why"].append("replica not converged within drain window")
        time.sleep(cell.get("post", 30))
        sampler.stop()
        sampler = None
        rec["final"] = {"primary": {s: info_once(port, s) for s in ("replication", "persistence", "memory", "stats")}}
        if repl:
            rec["final"]["replica"] = {s: info_once(port + 3, s) for s in ("replication", "persistence", "memory", "stats")}
        rec["primary_status"] = status(prim.pid)
        if repl:
            rec["replica_status"] = status(repl.pid)
        # Verification: exact values.
        keys = load_cfg.get("keys", 100000)
        exp = f"{d}/load.expected" if load_cfg["kind"] == "paced" else "none"
        vp = kv_line(subprocess.run([f"{BIN}/i8bench", "verify", str(port), str(keys), exp],
                                    capture_output=True, text=True, timeout=300).stdout)
        rec["verify_primary"] = vp
        if repl:
            vr = kv_line(subprocess.run([f"{BIN}/i8bench", "verify", str(port + 3), str(keys), exp],
                                        capture_output=True, text=True, timeout=300).stdout)
            rec["verify_replica"] = vr
            rec["digest_equal"] = vp.get("digest") == vr.get("digest")
            if not rec["digest_equal"]:
                rec["why"].append("replica digest differs from primary")
            for k in ("dbsize", "ttl_set", "missing"):
                if vp.get(k) != vr.get(k):
                    rec["why"].append(f"replica {k} {vr.get(k)} differs from primary {vp.get(k)}")
        if exp != "none" and vp.get("ok") != "true":
            rec["why"].append("primary does not match the expected keyspace")
        # The replica restarted from its own AOF, standalone.
        if repl and rc.get("aof") and cell.get("restart_check", True):
            time.sleep(2)
            xd = (os.path.expanduser(rc["aof_dir"]) + f"/{name}") if rc.get("aof_dir") else data
            rec["replica_files_before_restart"] = {f: os.path.getsize(f"{xd}/{f}") for f in os.listdir(xd) if f"-{port + 3}." in f}
            kill_group(repl.pid, signal.SIGKILL)
            repl.wait(10)
            with LOCK:
                CHILDREN.pop(repl.pid, None)
            renv = server_env(port + 3, data, rc["workers"], rc.get("env"), aof=rc.get("aof"), aof_dir=raof_dir, run_dir=d)
            t0 = time.time()
            repl, err = start_server(rec.get("replica_binary", binary), rc["cpus"], renv, d, "replica-restart", cell.get("restart_wait", 900))
            rec["restart_s"] = round(time.time() - t0, 2)
            if err:
                rec["why"].append(f"restart: {err}")
            else:
                vr2 = kv_line(subprocess.run([f"{BIN}/i8bench", "verify", str(port + 3), str(keys), exp],
                                             capture_output=True, text=True, timeout=300).stdout)
                rec["verify_restart"] = vr2
                if exp != "none" and vr2.get("ok") != "true":
                    rec["why"].append("restarted replica does not match the expected keyspace")
                if exp == "none" and vr2.get("digest") != vp.get("digest"):
                    rec["why"].append("restarted replica digest differs")
        rec["alive"] = prim.poll() is None
    except Exception as e:  # noqa: BLE001 - every failure invalidates the run
        rec["valid"] = False
        rec["why"].append(f"error: {e}")
    finally:
        if sampler:
            sampler.stop()
        for p in perf_procs:
            reap(p, 5)
        reap(gen, 5)
        reap(repl)
        reap(prim)
        # Keep logs; drop the data files (AOFs and snapshots are large).
        for f in os.listdir(data):
            try:
                rec.setdefault("data_bytes", {})[f] = os.path.getsize(f"{data}/{f}")
                os.remove(f"{data}/{f}")
            except OSError:
                pass
        if rc and rc.get("aof_dir"):
            xd = os.path.expanduser(rc["aof_dir"]) + f"/{name}"
            for f in os.listdir(xd) if os.path.isdir(xd) else []:
                try:
                    rec.setdefault("data_bytes", {})["aofdir/" + f] = os.path.getsize(f"{xd}/{f}")
                    os.remove(f"{xd}/{f}")
                except OSError:
                    pass
        rec["host_after"] = host_snapshot()
    return rec


def main():
    if CFG.get("wait_for"):
        # A gate in the queue: hold the next campaigns until the file exists
        # (a build runs meanwhile, with no campaign on the host).
        path = os.path.expanduser(CFG["wait_for"])
        log(f"gate {CFG.get('name')}: waiting for {path}")
        open(f"{OUT}/WAITING", "w").write(path + "\n")
        while not os.path.exists(path):
            time.sleep(10)
        log(f"gate {CFG.get('name')}: open")
        open(f"{OUT}/DONE", "w").write(f"{time.time()}\n")
        return
    labels = CFG["labels"]
    reps = CFG.get("reps", 5)
    port = CFG.get("port_base", 21000)
    with open(f"{OUT}/config.json", "w") as f:
        json.dump(CFG, f, indent=1)
    with open(f"{OUT}/meta.txt", "w") as f:
        f.write(f"host {os.uname().release} {os.cpu_count()} cpus\n")
        for l in labels:
            for cell in CFG["cells"]:
                for sfx in {cell.get("bin_suffix", ""), cell.get("replica_bin_suffix", cell.get("bin_suffix", ""))}:
                    b = f"{BIN}/rc-{l}{sfx}"
                    sha = subprocess.run(["sha256sum", b], capture_output=True, text=True).stdout.split()[:1]
                    f.write(f"{l}{sfx} {sha} {os.path.realpath(b)}\n")
        sha = subprocess.run(["sha256sum", f"{BIN}/i8bench"], capture_output=True, text=True).stdout.split()[:1]
        f.write(f"i8bench {sha}\n")
        sha = subprocess.run(["sha256sum", os.path.abspath(__file__)], capture_output=True, text=True).stdout.split()[:1]
        f.write(f"i8run.py {sha}\n")
    log(f"campaign {CFG.get('name')} start: {reps} reps x {len(CFG['cells'])} cells x {labels}")
    for rep in range(1, reps + 1):
        s = (rep - 1) % len(labels)
        order = labels[s:] + labels[:s] if CFG.get("rotate", True) else labels
        for cell in CFG["cells"]:
            for label in order:
                for attempt in range(2):
                    port += 10
                    if port + 10 >= 32000:
                        port = CFG.get("port_base", 21000)
                    if 27490 <= port <= 27510:
                        port = 27520
                    log(f"run rep={rep} cell={cell['name']} label={label} port={port} attempt={attempt}")
                    rec = run_once(rep, cell, label, port)
                    rec["attempt"] = attempt
                    with open(f"{OUT}/runs.jsonl", "a") as f:
                        f.write(json.dumps(rec) + "\n")
                    log(f"  valid={rec['valid']} why={rec['why']} load={ {k: rec.get('load', {}).get(k) for k in ('tx_per_s', 'accepted_per_s', 'writer_ops_per_s')} }")
                    if not rec.get("collision"):
                        break
    log("campaign done")
    with open(f"{OUT}/DONE", "w") as f:
        f.write(f"{time.time()}\n")


if __name__ == "__main__":
    if os.path.exists("/.flatpak-info") and not CFG.get("smoke"):
        sys.exit("refusing to run in the sandbox (smoke tests only)")
    try:
        main()
    finally:
        kill_all()
