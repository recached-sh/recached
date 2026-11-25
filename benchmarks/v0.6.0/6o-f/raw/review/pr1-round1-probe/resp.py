import socket, sys
def enc(args):
    out=b'*%d\r\n'%len(args)
    for a in args:
        a=a.encode(); out+=b'$%d\r\n%s\r\n'%(len(a),a)
    return out
class R:
    def __init__(s,sock): s.f=sock.makefile('rb')
    def read(s):
        l=s.f.readline().rstrip(b'\r\n'); t=l[:1]; r=l[1:].decode()
        if t in (b'+',): return r
        if t==b'-': return 'ERR:'+r
        if t==b':': return int(r)
        if t==b'$':
            n=int(r)
            if n<0: return None
            d=s.f.read(n+2)[:-2]; return d.decode('latin1')
        if t in (b'*',b'>'):
            n=int(r)
            if n<0: return None
            return [s.read() for _ in range(n)]
        if t==b'_': return None
        return 'RAW:'+l.decode()
port=int(sys.argv[1]); 
s=socket.create_connection(('127.0.0.1',port)); r=R(s)
for line in open(sys.argv[2]):
    line=line.strip()
    if not line: continue
    s.sendall(enc(line.split()))
    v=r.read()
    if line.startswith('PEXPIRETIME') and isinstance(v,int) and v>1000:
        v='T(%d)'%v  # keep exact
    print(f'{line} -> {v!r}')
