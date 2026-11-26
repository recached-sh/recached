import os, sys, tempfile, shutil, importlib.util
spec = importlib.util.spec_from_file_location("tc", "/home/dennispaler/Documents/OpenSource/recached-6oi-design/benchmarks/v0.6.0/6o-i/tools/txn_compat.py")
tc = importlib.util.module_from_spec(spec); spec.loader.exec_module(tc)
BIN = "/home/dennispaler/recached-6oi-bench/bin/"
root = tempfile.mkdtemp(prefix="review-a2-", dir="/home/dennispaler/recached-6oi-bench/tmp")
try:
    # P7: mid-file length corruption (one digit) parses as an incomplete tail: later records cut away.
    d = tc.fresh(root, "p7")
    bad = b"*3\r\n$3\r\nSET\r\n$1\r\nb\r\n$9999\r\nxx\r\n"   # '$2' corrupted to '$9999'
    body = tc.FMT_H + tc.arr("SET","a","1") + bad + b"".join(tc.arr("SET","c%d"%i,"1") for i in range(50))
    tc.write(d, body)
    before = len(body)
    s = tc.Server(BIN+"rc-6oh-6068337", d); up = s.start()
    got = [tc.cmd(s.port,"GET",k) for k in ("a","b","c0","c49")] if up else None
    s.kill9()
    print("P7 6O-H mid-file length corruption: up", up, "got", got, "file", before, "->", len(tc.read(d)))
finally:
    shutil.rmtree(root, ignore_errors=True)
