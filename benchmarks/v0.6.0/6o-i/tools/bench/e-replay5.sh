#!/bin/bash
# 6O-I PR 6: recovery time, restarts interleaved between binaries (replay-1 and replay-4 ran each
# binary's reps back to back and moved by up to 50% between runs).
B=$HOME/recached-6oi-bench
until awk '{exit !($1<3.0)}' /proc/loadavg; do sleep 10; done
{ echo "# replay_time.py INTERLEAVE=1 $(date -u +%FT%TZ); base=6oh 6068337 p3=6oi #200 976574a; REPS=11"
  INTERLEAVE=1 TXN_TMP=$B/tmp REPS=11 python3 $B/kit/tools/replay_time.py base=$B/bin/rc-base p3=$B/bin/rc-p3
  echo "replay5 done $(date -u +%FT%TZ)"; } > $B/logs/i-replay5.txt 2>&1
