#!/bin/bash
# 6O-I PR 6: review E's measurements, in sequence on the host.
B=$HOME/recached-6oi-bench
bash $B/e-tx.sh
bash $B/i-ord.sh
bash $B/i-stall2.sh
until awk '{exit !($1<3.0)}' /proc/loadavg; do sleep 10; done
{ echo "# replay_time.py $(date -u +%FT%TZ); base=6oh 6068337 p3=6oi #200 976574a; REPS=5"
  TXN_TMP=$B/tmp REPS=5 python3 $B/kit/tools/replay_time.py base=$B/bin/rc-base p3=$B/bin/rc-p3
  echo "replay done $(date -u +%FT%TZ)"; } > $B/logs/i-replay2.txt 2>&1
echo "chain done $(date -u +%FT%TZ)" >> $B/logs/e-chain.console
