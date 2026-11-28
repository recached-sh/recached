#!/usr/bin/env bash
# 6O-I8: once a second, the replication sockets' buffers during r5 (primary
# replication ports 27012..27062: base + 10k + 2). Output: runs/r6-ss.txt.
OUT=$HOME/recached-6oi8-bench/runs/r6-ss.txt
while [ ! -e $HOME/recached-6oi8-bench/runs/r6/DONE ]; do
  echo "T $(date +%s.%N)"
  ss -tmin state established '( sport >= :31702 and sport <= :31799 ) or ( dport >= :31702 and dport <= :31799 )' 2>/dev/null | grep -A2 -E ":317[0-9][2]\b" | grep -vE "^--"
  sleep 1
done >> $OUT
