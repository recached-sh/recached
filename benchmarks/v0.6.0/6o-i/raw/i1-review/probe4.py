import os, socket, tempfile, shutil, importlib.util, time
spec = importlib.util.spec_from_file_location("tc", "/home/dennispaler/Documents/OpenSource/recached-6oi-design/benchmarks/v0.6.0/6o-i/tools/txn_compat.py")
tc = importlib.util.module_from_spec(spec); spec.loader.exec_module(tc)
OI = "/home/dennispaler/Documents/OpenSource/recached-6oi-aof/target/debug/recached-server"
root = tempfile.mkdtemp(prefix="review-a4-", dir="/home/dennispaler/recached-6oi-bench/tmp")
def exec_tx(port, *cmds):
    with socket.create_connection(("127.0.0.1", port), timeout=5) as s:
        s.sendall(b"".join(tc.enc(c) for c in (("MULTI",),)+cmds+(("EXEC",),)))
        time.sleep(0.3); return s.recv(65536)
def boot(d, keys):
    s = tc.Server(OI, d); up = s.start(); got = [tc.cmd(s.port,"GET",k) for k in keys] if up else None
    s.kill9(); return up, got, s.tail(1)[:160]
try:
    for name, body in (
        ("marker-abandoned", tc.BASE + tc.ctl(tc.BEGIN,3) + tc.TX[0] + tc.resp_push("RECACHED-CHECKPOINT","9")),
        ("clean", tc.BASE),
    ):
        d = tc.fresh(root, name); tc.write(d, body)
        s = tc.Server(OI, d); up = s.start(); r = exec_tx(s.port, ("SET","x","1"), ("SET","y","1")) if up else None
        s.kill9(); tail = tc.read(d)[len(body):]
        print(name, "exec:", r, "| appended starts:", tail[:60])
        # cut the appended group in the middle and restart: all or nothing?
        whole = tc.read(d)
        cut = len(body) + 57 + 10   # inside the first member
        open(tc.path(d),"wb").write(whole[:cut])
        print(name, "restart after cutting inside the new group:", boot(d, ("a","b","x","y")))
finally:
    shutil.rmtree(root, ignore_errors=True)
