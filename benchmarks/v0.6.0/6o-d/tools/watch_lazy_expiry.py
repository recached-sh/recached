import socket, sys, time
def conn(port):
    s=socket.create_connection(("127.0.0.1",port)); return s
def cmd(s,*a):
    s.sendall(("*%d\r\n"%len(a)+"".join("$%d\r\n%s\r\n"%(len(x),x) for x in a)).encode())
    time.sleep(0.05); return s.recv(65536)
for name,port in (("redis-7.2.5",int(sys.argv[1])),("recached-base",int(sys.argv[2]))):
    s=conn(port); o=conn(port)
    cmd(s,"FLUSHDB")
    cmd(s,"SET","wk","v","PX","150")
    print(name,"WATCH",cmd(s,"WATCH","wk"))
    time.sleep(0.4)  # wk expires; nobody touches it
    print(name,"MULTI",cmd(s,"MULTI"))
    print(name,"QUEUE SET",cmd(s,"SET","other","1"))
    print(name,"EXEC",cmd(s,"EXEC"))
    # control: no expiry
    cmd(s,"SET","wk2","v")
    cmd(s,"WATCH","wk2"); cmd(s,"MULTI"); cmd(s,"SET","other","2")
    print(name,"control EXEC",cmd(s,"EXEC"))
