#!/usr/bin/env bash
# 6O-I8: once a second, the replication sockets' buffers during r5 (primary
# replication ports 27012..27062: base + 10k + 2). Output: runs/r5-ss.txt.
OUT=$HOME/recached-6oi8-bench/runs/r5-ss.txt
while [ ! -e $HOME/recached-6oi8-bench/runs/r5/DONE ]; do
  echo "T $(date +%s.%N)"
  ss -tmin state established '( sport >= :27002 and sport <= :27099 ) or ( dport >= :27002 and dport <= :27099 )' 2>/dev/null | grep -A2 -E ":270[0-9][2]\b" | grep -vE "^--"
  sleep 1
done >> $OUT
