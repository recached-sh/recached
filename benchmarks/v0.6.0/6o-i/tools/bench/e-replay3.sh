#!/bin/bash
# 6O-I PR 6: recovery time again (the first run's exec2 writer deadlocked: fixed), after e-tx2.
B=$HOME/recached-6oi-bench
until grep -q "e-tx2 done" $B/logs/e-tx2.console 2>/dev/null; do sleep 20; done
until awk '{exit !($1<3.0)}' /proc/loadavg; do sleep 10; done
{ echo "# replay_time.py $(date -u +%FT%TZ); base=6oh 6068337 p3=6oi #200 976574a; REPS=5"
  TXN_TMP=$B/tmp REPS=5 python3 $B/kit/tools/replay_time.py base=$B/bin/rc-base p3=$B/bin/rc-p3
  echo "replay3 done $(date -u +%FT%TZ)"; } > $B/logs/i-replay3.txt 2>&1
