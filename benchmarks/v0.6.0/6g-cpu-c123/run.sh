#!/usr/bin/env bash
# ABAB native A/B of the server CPU fixes C1-C3 (SADD member move, cached
# latency histograms, inline ordering guards). Single worker, pipelined.
# Reports rps and server CPU-seconds per million requests (from ps cputime).
set -euo pipefail
A=${A:-../../../target/6g/bins/server-x4-prof}
B=${B:-../../../target/6g/bins/server-c123-prof}
N=${N:-2000000}
cpu() { ps -o time= -p "$1" | awk '{n=split($1,p,":"); s=0; for(i=1;i<=n;i++) s=s*60+p[i]; printf "%.2f", s}'; }
echo "round,build,test,rps,server_cpu_s,cpu_s_per_mreq,load1"
for round in 1 2 3; do
  for which in A B B A; do
    bin=${!which}
    RECACHED_PORT=7821 RECACHED_WS_PORT=7822 RECACHED_METRICS_PORT=0 RECACHED_SAVE_INTERVAL=0 \
      RECACHED_SAVE_PATH=/tmp/6g-c123.rdb RECACHED_WORKER_THREADS=1 "$bin" >/dev/null 2>&1 &
    pid=$!; sleep 1
    for t in sadd set hset lpush; do
      c0=$(cpu $pid)
      rps=$(redis-benchmark -p 7821 -t $t -n $N -P 16 -c 50 -r 100000 -q 2>/dev/null | tr '\r' '\n' | awk '/requests per second/{print $2}' | tail -1)
      c1=$(cpu $pid)
      load=$(sysctl -n vm.loadavg | awk '{print $2}')
      echo "$round,$which,$t,$rps,$(awk -v a=$c0 -v b=$c1 'BEGIN{printf "%.2f", b-a}'),$(awk -v a=$c0 -v b=$c1 -v n=$N 'BEGIN{printf "%.3f", (b-a)/(n/1e6)}'),$load"
    done
    kill $pid; wait $pid 2>/dev/null || true; sleep 0.5
  done
done
