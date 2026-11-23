import socket, sys, threading, time
port=int(sys.argv[1]); mode=sys.argv[2]; secs=float(sys.argv[3])
def c():
    s=socket.create_connection(("127.0.0.1",port)); s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1); return s
def enc(*a): return ("*%d\r\n"%len(a)+"".join("$%d\r\n%s\r\n"%(len(x),x) for x in a)).encode()
def readn(f, n):
    out=[]
    for _ in range(n): out.append(readv(f))
    return out
def readv(f):
    l=f.readline()
    t=l[:1]
    if t in b"+-:": return l[1:-2]
    if t==b"$":
        n=int(l[1:-2]); 
        if n<0: return None
        d=f.read(n+2); return d[:-2]
    if t==b"*":
        n=int(l[1:-2]); return None if n<0 else [readv(f) for _ in range(n)]
w0=c(); w0.sendall(enc("MSET","iso:a","0","iso:b","0")); w0.makefile("rb").readline()
if mode=="watcher":
    wt=c(); wt.sendall(enc("WATCH","iso:unrelated")); wt.makefile("rb").readline()
stop=time.time()+secs; mixed=[0]; total=[0]; mmixed=[0]
def writer():
    s=c(); f=s.makefile("rb"); g=0
    while time.time()<stop:
        g+=1; s.sendall(enc("MSET","iso:a",str(g),"iso:b",str(g))); readv(f)
def reader():
    s=c(); f=s.makefile("rb")
    while time.time()<stop:
        s.sendall(enc("MULTI")+enc("GET","iso:a")+enc("GET","iso:b")+enc("EXEC")+enc("MGET","iso:a","iso:b"))
        r=readn(f,5); ex=r[3]; mg=r[4]
        total[0]+=1
        if ex[0]!=ex[1]: mixed[0]+=1
        if mg[0]!=mg[1]: mmixed[0]+=1
ts=[threading.Thread(target=writer) for _ in range(2)]+[threading.Thread(target=reader) for _ in range(2)]
[t.start() for t in ts]; [t.join() for t in ts]
print(f"mode={mode} rounds={total[0]} exec_mixed={mixed[0]} mget_mixed={mmixed[0]}")
