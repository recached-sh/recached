#!/usr/bin/env bash
# ABAB native A/B of C4 (no registry locks on writes before any ESET/EADD),
# 4 workers, distinct keys (-r 100000), pipelined.
# Reports rps and server CPU-seconds per million requests (from ps cputime).
set -uo pipefail
A=${A:-../../../target/6g/bins/server-x5}
B=${B:-../../../target/6g/bins/server-c4}
N=${N:-2000000}
cpu() { ps -o time= -p "$1" | awk '{n=split($1,p,":"); s=0; for(i=1;i<=n;i++) s=s*60+p[i]; printf "%.2f", s}'; }
echo "round,build,test,rps,server_cpu_s,cpu_s_per_mreq,load1"
for round in 1 2 3; do
  for which in A B B A; do
    bin=${!which}
    RECACHED_PORT=7821 RECACHED_WS_PORT=7822 RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
      RECACHED_SAVE_PATH=/tmp/6g-c123.rdb RECACHED_WORKER_THREADS=${WORKERS:-4} "$bin" >/dev/null 2>&1 &
    pid=$!
    for _ in $(seq 120); do redis-cli -p 7821 ping 2>/dev/null | grep -q PONG && break; sleep 0.25; done
    for t in set incr hset sadd; do
      c0=$(cpu $pid)
      rps=$(redis-benchmark -p 7821 -t $t -n $N -P 16 -c 50 -r 100000 -q 2>/dev/null | tr '\r' '\n' | awk '/requests per second/{print $2}' | tail -1)
      c1=$(cpu $pid)
      load=$(sysctl -n vm.loadavg | awk '{print $2}')
      echo "$round,$which,$t,$rps,$(awk -v a=$c0 -v b=$c1 'BEGIN{printf "%.2f", b-a}'),$(awk -v a=$c0 -v b=$c1 -v n=$N 'BEGIN{printf "%.3f", (b-a)/(n/1e6)}'),$load"
    done
    kill $pid; wait $pid 2>/dev/null || true; sleep 0.5
  done
done
