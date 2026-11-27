#!/usr/bin/env bash
# 6O-I8: run queued campaigns one at a time (never two at once). Each line of
# queue.txt names a config in cfg/; a line is done once runs/<name>/DONE exists.
B=$HOME/recached-6oi8-bench
exec 9>$B/chain.lock
flock -n 9 || { echo "chain already running"; exit 1; }
while true; do
  next=""
  while read -r name; do
    [[ -z $name || $name == \#* ]] && continue
    [[ -e $B/runs/$name/DONE || -e $B/runs/$name/FAILED ]] && continue
    next=$name; break
  done < $B/queue.txt
  [[ -z $next ]] && { echo "$(date -u +%FT%TZ) queue empty" >> $B/logs/chain.txt; break; }
  echo "$(date -u +%FT%TZ) start $next" >> $B/logs/chain.txt
  python3 $B/i8run.py $B/cfg/$next.json > $B/logs/run-$next.console 2>&1
  rc=$?
  [[ $rc -ne 0 ]] && { mkdir -p $B/runs/$next; echo "exit $rc" > $B/runs/$next/FAILED; }
  echo "$(date -u +%FT%TZ) end $next exit=$rc" >> $B/logs/chain.txt
done
