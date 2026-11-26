import subprocess,collections,bisect,re,sys
label,mode,func=sys.argv[1],sys.argv[2],sys.argv[3]
binp=f"../bin/rc-{label}-prof"
syms=[]
for l in subprocess.run(["nm","-n","-C",binp],capture_output=True,text=True).stdout.splitlines():
    p=l.split(None,2)
    if len(p)==3 and p[1] in "tTwW": syms.append((int(p[0],16),p[2]))
addrs=[a for a,_ in syms]
def sym(a):
    i=bisect.bisect_right(addrs,a)-1; return syms[i][1] if i>=0 else "?"
cnt=collections.Counter(); total=0
for r in (1,2,3):
    d=f"{label}-{mode}-r{r}"; base=None
    for l in open(d+"/samples.txt.maps"):
        pa=l.split()
        if len(pa)>=6 and pa[5].endswith(f"rc-{label}-prof") and int(pa[2],16)==0:
            base=int(pa[0].split("-")[0],16)
    for l in open(d+"/samples.txt"):
        if not l.startswith("S "): continue
        ip=int(l.split()[2],16); total+=1
        va=ip-base
        if func in sym(va): cnt[va]+=1
print("samples", total, "in", func, sum(cnt.values()))
top=[a for a,_ in cnt.most_common(600)]
out=subprocess.run(["addr2line","-a","-i","-e",binp]+[hex(a) for a in top],capture_output=True,text=True).stdout
blocks=re.split(r'\n(?=0x)',out.strip())
byline=collections.Counter()
for b in blocks:
    ls=b.split("\n"); a=int(ls[0],16)
    outer=[x.split("/")[-1] for x in ls[1:]]
    byline[" <- ".join(outer[:int(sys.argv[4]) if len(sys.argv)>4 else 3])]+=cnt[a]
for k,v in byline.most_common(int(sys.argv[5]) if len(sys.argv)>5 else 30): print(v, k)
