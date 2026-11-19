#!/usr/bin/env bash
set -u
KIT=/home/dennispaler/Documents/OpenSource/recached-6nc4-bench-pr/benchmarks/v0.6.0/6n-c4/run-m1.sh
B=~/recached-6nc4-bench/bin/rc-diag-fullfix
for _ in $(seq 360); do l=$(cut -d' ' -f1 /proc/loadavg); awk -v l=$l 'BEGIN{exit !(l<1.0)}' && break; sleep 10; done
stamp=$(date -u +%Y%m%dT%H%M%SZ)
for t in 2 3; do
  OUT=~/recached-6nc4-bench/ceiling-$stamp/threads$t RC_BIN=$B RC_SHA=41403f2f24a79fe3dbaa0a23a16e803751d55476 RC_SHA256=$(sha256sum $B | cut -d' ' -f1) \
    REPS=1 BENCH_THREADS=$t CELLS="rc-standalone-P16 redis-P16 rc-coalesced-P16" MAX_LOAD=3.5 bash $KIT >~/recached-6nc4-bench/ceiling-$stamp-t$t.log 2>&1
  echo "threads $t exit $?"
done
echo ~/recached-6nc4-bench/ceiling-$stamp
