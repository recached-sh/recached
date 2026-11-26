#!/bin/bash
# 6O-H PR6 natives: (1) the same legacy AOF replayed by base and final,
# alternating; (2) the lag experiment under disk pressure, final vs fixe.
set -u
B=${BENCH_DIR:-$HOME/recached-6oh-bench}
cd $B
OUT=$B/results-native3; mkdir -p $OUT
uptime > $OUT/host.txt
# 1. legacy replay A/B
python3 - <<PY > $OUT/legacy-replay.jsonl
import os, shutil, subprocess, tempfile, time, socket, json, sys
sys.path.insert(0, "$B")
import importlib.util
spec = importlib.util.spec_from_file_location("hrepl", "$B/h-repl.py")
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
legacy = "$B/results-repl/legacy-1.aof"
for rep in range(1, 6):
    order = [("base", "$B/bin/rc-base"), ("final", "$B/bin/rc-final")]
    if rep % 2 == 0: order.reverse()
    for name, binary in order:
        root = tempfile.mkdtemp(prefix="h-leg-", dir="$B/tmp")
        r, _ = m.exp_replay(name, binary, root, legacy_from=legacy)
        print(json.dumps({"exp": "replay-legacy-file", **r, "name": name, "rep": rep}), flush=True)
        shutil.rmtree(root, ignore_errors=True)
PY
# 2. lag under disk pressure
( while [ ! -f $OUT/stop ]; do dd if=/dev/zero of=$B/tmp/dd-pressure bs=1M count=512 conv=fdatasync status=none; done; rm -f $B/tmp/dd-pressure ) &
DD=$!
KEEP=1 EXPS=lag REPS=2 timeout 1500 python3 h-repl.py $OUT/lag-pressure final=$B/bin/rc-final fixe=$B/bin/rc-fixe > $OUT/lag-pressure.out 2>&1
touch $OUT/stop; wait $DD
echo done > $OUT/DONE
