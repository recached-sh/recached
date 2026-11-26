#!/usr/bin/env python3
"""6O-I: AOF recovery time, per binary, on files each binary wrote itself.

Workloads (AOF everysec, pinned like the kit: server CPU 0, client 3-5):
  sets     1,000,000 SETs (redis-benchmark), no transactions
  exec2    250,000 MULTI/SET/SET/EXEC over one pipelined connection
  tail     exec2's file with an incomplete group appended (TXN-BEGIN 2 and
           one member), the recovery path that discards and cuts it
Each: write, SIGKILL, then REPS restarts on a copy of the file, timed from
spawn to the first PONG; the keyspace size is checked after each.

usage: replay_time.py NAME=BINARY ...   env REPS=3, INTERLEAVE=1 (restarts alternate between binaries)
"""
import os, shutil, signal, socket, subprocess, sys, tempfile, threading, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from txn_compat import cmd, enc, free_port  # noqa: E402

BENCH = os.path.expanduser("~/recached-6l-bench/bin/redis-benchmark")
REPS = int(os.environ.get("REPS", "3"))
TAIL = (b"*2\r\n$19\r\nRECACHED-CHECKPOINT\r\n$20\r\n%d\r\n" % ((0xEC00 << 48) | (0x81 << 40) | (1 << 32) | 2)
        + b"*3\r\n$3\r\nSET\r\n$4\r\ntail\r\n$1\r\nx\r\n")


def start(binary, d):
    port = free_port()
    env = dict(os.environ, RECACHED_BIND="127.0.0.1", RECACHED_PORT=str(port), RECACHED_WS_PORT=str(free_port()),
               RECACHED_METRICS_PORT="0", RECACHED_SAVE_INTERVAL="0",
               RECACHED_SAVE_PATH=os.path.join(d, "dump.rdb"),
               RECACHED_AOF_PATH=os.path.join(d, "appendonly.aof"), RECACHED_AOF_SYNC="everysec")
    t0 = time.monotonic()
    p = subprocess.Popen(["taskset", "-c", "0", binary], cwd=d, env=env, stdout=subprocess.DEVNULL,
                         stderr=open(os.path.join(d, "server.log"), "ab"))
    while True:
        try:
            if cmd(port, "PING") == "PONG":
                return p, port, time.monotonic() - t0
        except OSError:
            if p.poll() is not None:
                raise RuntimeError(f"exited: {d}")
            time.sleep(0.005)


def kill9(p):
    p.send_signal(signal.SIGKILL)
    p.wait()


def write_sets(port):
    subprocess.run(["taskset", "-c", "3-5", BENCH, "-p", str(port), "-n", "1000000", "-c", "50", "-P", "16",
                    "-r", "1000000", "-d", "32", "-t", "set", "-q"], capture_output=True)


def write_exec2(port):
    # Replies are drained on a second thread while requests are sent: a client that only writes
    # fills the server's send buffer, the server stops reading, and both sides block.
    with socket.create_connection(("127.0.0.1", port), timeout=120) as c:
        done = threading.Event()

        def drain():
            buf = b""
            while not buf.endswith(b"+PONG\r\n"):
                data = c.recv(1 << 20)
                if not data:
                    break
                buf = (buf + data)[-16:]
            done.set()

        t = threading.Thread(target=drain, daemon=True)
        t.start()
        chunk = []
        for i in range(250_000):
            chunk.append(enc(["MULTI"]) + enc(["SET", f"a:{i}", "v" * 32]) + enc(["SET", f"b:{i}", "v" * 32]) + enc(["EXEC"]))
            if len(chunk) == 2000:
                c.sendall(b"".join(chunk))
                chunk = []
        c.sendall(b"".join(chunk) + enc(["PING"]))
        t.join()
        if not done.is_set():
            raise RuntimeError("connection closed before PONG")


def interleaved():
    """INTERLEAVE=1: write every binary's file first, then restart them in turn, the order
    alternating every rep, so that a change in the host's state moves every binary alike."""
    root = tempfile.mkdtemp(prefix="rc6oi-replay-", dir=os.environ.get("TXN_TMP"))
    bins = [a.split("=", 1) for a in sys.argv[1:]]
    print("| workload | binary | AOF MiB | restart s (each rep, interleaved) | median s | keys | median ratio to the first, paired |")
    print("|---|---|---|---|---|---|---|")
    try:
        for workload in ("sets", "exec2", "tail"):
            srcs, sizes = {}, {}
            for name, binary in bins:
                src = os.path.join(root, f"{name}-{workload}-src")
                os.makedirs(src)
                p, port, _ = start(binary, src)
                (write_sets if workload == "sets" else write_exec2)(port)
                time.sleep(1.5)
                kill9(p)
                aof = os.path.join(src, "appendonly.aof")
                if workload == "tail":
                    with open(aof, "ab") as f:
                        f.write(TAIL)
                srcs[name], sizes[name] = src, os.path.getsize(aof) / 2**20
            times = {name: [] for name, _ in bins}
            keys = {}
            for rep in range(REPS):
                order = bins if rep % 2 == 0 else bins[::-1]
                for name, binary in order:
                    d = srcs[name] + f"-run{rep}"
                    shutil.copytree(srcs[name], d, ignore=shutil.ignore_patterns("server.log"))
                    p, port, t = start(binary, d)
                    keys[name] = cmd(port, "DBSIZE")
                    kill9(p)
                    shutil.rmtree(d, ignore_errors=True)
                    times[name].append(t)
            first = bins[0][0]
            for name, _ in bins:
                ts = times[name]
                ratios = sorted(b / a for a, b in zip(times[first], ts))
                print(f"| {workload} | {name} | {sizes[name]:.1f} | {' '.join(f'{t:.2f}' for t in ts)} | "
                      f"{sorted(ts)[len(ts) // 2]:.2f} | {keys[name]} | {ratios[len(ratios) // 2]:.3f} |", flush=True)
            for src in srcs.values():
                shutil.rmtree(src, ignore_errors=True)
    finally:
        shutil.rmtree(root, ignore_errors=True)


def main():
    if os.environ.get("INTERLEAVE") == "1":
        return interleaved()
    root = tempfile.mkdtemp(prefix="rc6oi-replay-", dir=os.environ.get("TXN_TMP"))
    print("| workload | binary | AOF MiB | restart s (each rep) | median s | keys |")
    print("|---|---|---|---|---|---|")
    try:
        for workload in ("sets", "exec2", "tail"):
            for name, binary in (a.split("=", 1) for a in sys.argv[1:]):
                src = os.path.join(root, f"{name}-{workload}-src")
                os.makedirs(src)
                p, port, _ = start(binary, src)
                (write_sets if workload == "sets" else write_exec2)(port)
                time.sleep(1.5)
                kill9(p)
                aof = os.path.join(src, "appendonly.aof")
                if workload == "tail":
                    with open(aof, "ab") as f:
                        f.write(TAIL)
                size = os.path.getsize(aof) / 2**20
                times, keys = [], None
                for _ in range(REPS):
                    d = src + f"-run{len(times)}"
                    shutil.copytree(src, d, ignore=shutil.ignore_patterns("server.log"))
                    p, port, t = start(binary, d)
                    keys = cmd(port, "DBSIZE")
                    kill9(p)
                    shutil.rmtree(d, ignore_errors=True)
                    times.append(t)
                med = sorted(times)[len(times) // 2]
                print(f"| {workload} | {name} | {size:.1f} | {' '.join(f'{t:.2f}' for t in times)} | {med:.2f} | {keys} |",
                      flush=True)
                shutil.rmtree(src, ignore_errors=True)
    finally:
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    main()
