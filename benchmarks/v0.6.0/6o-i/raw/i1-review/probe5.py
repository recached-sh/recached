import os, tempfile, shutil, importlib.util
spec = importlib.util.spec_from_file_location("tc", "/home/dennispaler/Documents/OpenSource/recached-6oi-design/benchmarks/v0.6.0/6o-i/tools/txn_compat.py")
tc = importlib.util.module_from_spec(spec); spec.loader.exec_module(tc)
OI = "/home/dennispaler/Documents/OpenSource/recached-6oi-aof/target/debug/recached-server"
BIN = "/home/dennispaler/recached-6oi-bench/bin/"
CMDS = [("SET","s","v","PX","100000"),("APPEND","s","x"),("INCR","n"),("HSET","h","f","v","g","w"),("HDEL","h","g"),
        ("LPUSH","l","a","b"),("RPOP","l"),("SADD","st","a","b"),("SREM","st","a"),("ZADD","z","1","a","2","b"),("ZINCRBY","z","3","a"),
        ("MSET","m1","1","m2","2"),("EXPIRE","m1","1000"),("PERSIST","m1"),("RENAME","m2","m3"),("DEL","n"),("GETSET","s2","v"),
        ("SETEX","e","100","v"),("LSET","l","0","z"),("INCRBYFLOAT","f","1.5"),("HINCRBY","h","c","2"),("FLUSHDB",),("SET","after","1")]
root = tempfile.mkdtemp(prefix="review-a5-", dir="/home/dennispaler/recached-6oi-bench/tmp")
try:
    for b in ("rc-v0.4.0","rc-v0.5.2","rc-6og-f9a364d"):
        d = tc.fresh(root, b)
        s = tc.Server(BIN+b, d); up = s.start(); errs=[]
        for c in CMDS:
            r = tc.cmd(s.port, *c)
            if isinstance(r,str) and r.startswith("ERR"): errs.append((c[0],r))
        s.kill9()
        data = tc.read(d)
        s2 = tc.Server(OI, d); up2 = s2.start(); got = tc.cmd(s2.port,"GET","after") if up2 else None; s2.kill9()
        print(b, "bytes", len(data), "errs", errs, "-> 6O-I started", up2, "after=", got, "" if up2 else s2.tail(1)[:160])
finally:
    shutil.rmtree(root, ignore_errors=True)
