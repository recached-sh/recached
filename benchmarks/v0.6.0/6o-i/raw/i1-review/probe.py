import os, sys, tempfile, shutil, importlib.util
spec = importlib.util.spec_from_file_location("tc", "/home/dennispaler/Documents/OpenSource/recached-6oi-design/benchmarks/v0.6.0/6o-i/tools/txn_compat.py")
tc = importlib.util.module_from_spec(spec); spec.loader.exec_module(tc)
BIN = "/home/dennispaler/recached-6oi-bench/bin/"
root = tempfile.mkdtemp(prefix="review-a-", dir="/home/dennispaler/recached-6oi-bench/tmp")
def run(binary, d, writes=(), keys=()):
    s = tc.Server(BIN + binary, d); up = s.start()
    out = None
    if up:
        for w in writes: tc.cmd(s.port, *w)
        out = [tc.cmd(s.port, "GET", k) for k in keys]
    s.kill9()
    return up, out, s.tail()
try:
    # P1: 6O-I crash with torn member; v0.4.0 takes over, writes; then a 0.5+/6O-H parser reads it.
    d = tc.fresh(root, "p1")
    body = tc.BASE + tc.ctl(tc.BEGIN, 2) + tc.TX[0]
    tc.write(d, body + tc.TX[1][:20])
    print("P1 v0.4.0 takeover:", run("rc-v0.4.0", d, [("SET","d","1")], ("a","b","d")))
    data = tc.read(d); print("P1 file tail:", data[len(body):])
    print("P1 6O-H on result:", run("rc-oh-6068337".replace("oh","6oh"), d, (), ("a","b","d")))
    # P4: A1 rationale: 6O-H on FORMAT(expirylog) + dangling group appends arrays
    d = tc.fresh(root, "p4")
    body = tc.FMT_H + tc.arr("SET","a","old") + tc.ctl(tc.BEGIN, 2) + tc.TX[0]
    tc.write(d, body)
    print("P4 6O-H appends:", run("rc-6oh-6068337", d, [("SET","d","1")], ("a",)), tc.read(d)[len(body):][:16])
    # P5: unknown ignorable kind 0x83 between members (future writer): old readers skip it
    d = tc.fresh(root, "p5")
    tc.write(d, tc.BASE + tc.ctl(tc.BEGIN, 2) + tc.TX[0] + tc.ctl(0x83, 7) + tc.TX[1] + tc.ctl(tc.COMMIT, 2))
    print("P5 6O-H with 0x83 inside group:", run("rc-6oh-6068337", d, (), ("a","b")))
    # P6: 6O-H started on a txn file then shutdown-save path: what FORMAT does SAVE write?
    d = tc.fresh(root, "p6")
    tc.write(d, tc.FMT_TXN + tc.arr("SET","a","1"))
    s = tc.Server(BIN+"rc-6oh-6068337", d); s.start(); tc.cmd(s.port,"SAVE"); s.kill9()
    print("P6 after 6O-H SAVE:", tc.read(d)[:64])
finally:
    shutil.rmtree(root, ignore_errors=True)
