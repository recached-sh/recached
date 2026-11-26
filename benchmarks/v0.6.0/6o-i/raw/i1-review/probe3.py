import os, sys, tempfile, shutil, importlib.util, stat
spec = importlib.util.spec_from_file_location("tc", "/home/dennispaler/Documents/OpenSource/recached-6oi-design/benchmarks/v0.6.0/6o-i/tools/txn_compat.py")
tc = importlib.util.module_from_spec(spec); spec.loader.exec_module(tc)
OI = "/home/dennispaler/Documents/OpenSource/recached-6oi-aof/target/debug/recached-server"
OH = "/home/dennispaler/recached-6oi-bench/bin/rc-6oh-6068337"
root = tempfile.mkdtemp(prefix="review-a3-", dir="/home/dennispaler/recached-6oi-bench/tmp")
def boot(binary, d, keys, writes=()):
    s = tc.Server(binary, d); up = s.start(); got=None
    if up:
        for w in writes: tc.cmd(s.port, *w)
        got = [tc.cmd(s.port, "GET", k) for k in keys]
    s.kill9(); return up, got, s.tail(2)
def mk(name, data, aofdir_ro=False):
    d = tc.fresh(root, name)
    # keep the AOF in a subdir we can make read-only while the snapshot path stays writable
    tc.write(d, data); return d
try:
    torn = tc.BASE + tc.arr("SET","p","1") + tc.TX[0][:9]           # a plain torn record (no group)
    grp  = tc.BASE + tc.ctl(tc.BEGIN,2) + tc.TX[0]                    # an incomplete group
    for name, data in (("torn", torn), ("group", grp)):
        for label, binary in (("6O-H", OH), ("6O-I", OI)):
            # Q1: AOF directory not writable (cut copy cannot be created); the AOF file itself is writable.
            d = tc.fresh(root, f"q1-{name}-{label}")
            sub = os.path.join(d, "aof"); os.makedirs(sub)
            p = os.path.join(sub, "appendonly.aof"); open(p,"wb").write(data)
            os.chmod(sub, 0o555)
            env_aof = p
            s = tc.Server(binary, d)
            # point AOF into the read-only dir
            import subprocess
            env = dict(os.environ, RECACHED_BIND="127.0.0.1", RECACHED_PORT=str(s.port), RECACHED_WS_PORT=str(s.ws),
                       RECACHED_METRICS_PORT="0", RECACHED_SAVE_INTERVAL="0", RECACHED_SAVE_PATH=os.path.join(d,"dump.rdb"),
                       RECACHED_AOF_PATH=p, RECACHED_AOF_SYNC="always")
            log = open(s.log,"ab")
            s.p = subprocess.Popen([binary], cwd=d, env=env, stdout=log, stderr=log, stdin=subprocess.DEVNULL)
            import time
            up=False
            for _ in range(100):
                if s.p.poll() is not None: break
                try:
                    if tc.cmd(s.port,"PING")=="PONG": up=True; break
                except OSError: pass
                time.sleep(0.05)
            s.kill9(); os.chmod(sub, 0o755)
            print(f"Q1 {name:5s} {label}: started={up} size {len(data)}->{os.path.getsize(p)} | {s.tail(1)[:150]}")
    # Q2: P1 file (v0.4.0 torn member + appended push) on 6O-I
    d = tc.fresh(root, "q2"); tc.write(d, tc.BASE + tc.ctl(tc.BEGIN,2) + tc.TX[0] + tc.TX[1][:20] + tc.resp_push("SET","d","1"))
    print("Q2 P1 on 6O-I:", boot(OI, d, ("a","b","d")))
    # Q3: older server's DEDUP push after a dangling group (row 1 precedence)
    d = tc.fresh(root, "q3"); tc.write(d, tc.BASE + tc.ctl(tc.BEGIN,2) + tc.TX[0] + tc.resp_push("DEDUP","c","7","SET","k","v"))
    print("Q3 DEDUP push abandons:", boot(OI, d, ("a","b","k")))
    # Q4: later writer's ignorable kind inside a group
    d = tc.fresh(root, "q4"); tc.write(d, tc.BASE + tc.ctl(tc.BEGIN,2) + tc.TX[0] + tc.ctl(0x83,7) + tc.TX[1] + tc.ctl(tc.COMMIT,2))
    print("Q4 0x83 inside group:", boot(OI, d, ("a","b")))
    # Q5: push checkpoint marker (old binary's SAVE crash) inside a dangling group, then a 6O-I write: is it grouped after?
    d = tc.fresh(root, "q5"); body = tc.BASE + tc.ctl(tc.BEGIN,3) + tc.TX[0] + tc.resp_push("RECACHED-CHECKPOINT","9"); tc.write(d, body)
    s = tc.Server(OI, d); up = s.start()
    if up:
        tc.cmd(s.port,"MULTI"); tc.cmd(s.port,"SET","x","1"); tc.cmd(s.port,"SET","y","1"); r=tc.cmd(s.port,"EXEC")
    s.kill9(); tail = tc.read(d)[len(body):]
    print("Q5 after abandoned-by-marker, 6O-I appends:", up, tail[:70])
    print("Q5 restart:", boot(OI, d, ("a","b","x","y")))
finally:
    for dp,dn,fn in os.walk(root):
        for x in dn: os.chmod(os.path.join(dp,x),0o755)
    shutil.rmtree(root, ignore_errors=True)
