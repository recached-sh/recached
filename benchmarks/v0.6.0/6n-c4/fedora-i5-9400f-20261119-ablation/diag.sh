#!/usr/bin/env bash
# 6N-C4 root-cause ablation (diagnostic, not M1): old 6N-C binary vs gain-fix-only vs full fix.
set -u
KIT=/home/dennispaler/Documents/OpenSource/recached-6nc4-fix/benchmarks/v0.6.0/6n-c/run-m1.sh

SHA_FIX=$(git -C /home/dennispaler/Documents/OpenSource/recached-6nc4-fix rev-parse 41403f2)
for _ in $(seq 360); do l=$(cut -d' ' -f1 /proc/loadavg); awk -v l=$l 'BEGIN{exit !(l<0.8)}' && break; sleep 10; done
echo "start $(date -u +%FT%TZ) load $(cat /proc/loadavg)"
stamp=$(date -u +%Y%m%dT%H%M%SZ)
run() { # name bin sha cells
  OUT=~/recached-6nc4-bench/diag-$stamp/$1 RC_BIN=$2 RC_SHA=$3 RC_SHA256=$(sha256sum "$2" | cut -d' ' -f1) \
    REPS=2 CELLS="$4" MAX_LOAD=3.5 bash "$KIT" >~/recached-6nc4-bench/diag-$stamp-$1.log 2>&1
  echo "$1 exit $?"
}
mkdir -p ~/recached-6nc4-bench/diag-$stamp
run old ~/recached-6nc-bench/target-m1/release/recached-server 0b48332d840811d35ee95b46b909384f211a122f "rc-coalesced-P1 rc-coalesced-P16"
run gainonly ~/recached-6nc4-bench/bin/rc-diag-gainonly $SHA_FIX "rc-coalesced-P1 rc-coalesced-P16"
run fullfix ~/recached-6nc4-bench/bin/rc-diag-fullfix $SHA_FIX "rc-inline-P16 rc-coalesced-P1 rc-coalesced-P16"
echo "done $(date -u +%FT%TZ)"
