#!/usr/bin/env bash
# 6O-I PR 6: B a fourth time. The third attempt picked ports inside the kernel's ephemeral range
# (32768-60999), where a client's outgoing socket took the primary's WebSocket port. Then the
# recovery-time run again with the binaries in reverse order (p3 first).
B=$HOME/recached-6oi-bench
E=RECACHED_AOF_SYNC
PORT_BASE=13600 LABELS="base i p3" REPS=5 SECS=20 MAX_LOAD=3.0 OUT=$B/results-e4-B \
  CELLS="1:set-set:unrelated:50:0::$E=everysec,REPLICA=1 1:setn:unrelated:20:0:TX_N=100:$E=everysec,REPLICA=1 1:set-set:unrelated:50:0::REPLICA=1 1:setn:unrelated:20:0:TX_N=100:REPLICA=1" \
  bash $B/i-tx.sh > $B/logs/e4-B.console 2>&1
echo "$(date -u +%FT%TZ) B done exit=$?" >> $B/logs/e-tx4.console
until awk '{exit !($1<3.0)}' /proc/loadavg; do sleep 10; done
{ echo "# replay_time.py $(date -u +%FT%TZ); reversed order: p3=6oi #200 976574a first, base=6oh 6068337; REPS=5"
  TXN_TMP=$B/tmp REPS=5 python3 $B/kit/tools/replay_time.py p3=$B/bin/rc-p3 base=$B/bin/rc-base
  echo "replay4 done $(date -u +%FT%TZ)"; } > $B/logs/i-replay4.txt 2>&1
echo "$(date -u +%FT%TZ) e-tx4 done" >> $B/logs/e-tx4.console
