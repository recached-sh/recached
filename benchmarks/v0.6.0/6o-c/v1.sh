#!/bin/bash
# 6O-C validation run 1: C1 (6e57aad) re-measured, then C2 (40f4e2b) and the C3 prototype, each
# against the pinned 6O-B binary (rc-base, d5563d7). ~25-30 s cells, 3 interleaved reps (5 for the
# 3-worker P=16 tail), Redis 7.2.5 alongside. Diagnostic counters in their own cells, never mixed.
set -u
B=$HOME/recached-6oc-bench; KIT=$B/kit
BASE=d5563d78034f3ccccc91975051de2cb6099b76ba
C2=40f4e2b; C3=$(git -C $B/src-c3 rev-parse HEAD)
C1=$(git -C $B/src-c1 rev-parse HEAD); C2=$(git -C $B/src-c2 rev-parse HEAD)
RUSTC="rustc 1.98.0 (88d9e12ae 2026-08-18)"
log() { echo "$(date -u +%FT%TZ) $*" >> $B/driver.log; }
quiet() { until awk '{exit !($1<1.0)}' /proc/loadavg && ! pgrep -x 'cc1|rustc|updatedb|cargo' >/dev/null; do sleep 20; done; }
run() { local name=$1; shift; quiet; log "$name start: load $(cut -d' ' -f1 /proc/loadavg)"; env "$@" RC_RUSTC="$RUSTC" UPROF=$B/bin/uprof MAX_LOAD=1.0 OUT=$B/results-$name bash $KIT/run-6oc.sh > $B/$name.console 2>&1; log "$name exit $?"; }

# C1, one worker
c=""; for t in set-c50-P1 set-c50-P4 set-c50-P16 mset-c50-P16; do c+="base/rc-noaof-$t new/rc-noaof-$t off/rc-noaof-$t "; done; c+="redis-noaof-set-c50-P1"
run c1v2-1w RC_BINS="base=$B/bin/rc-base new=$B/bin/rc-c1 off=$B/bin/rc-c1" RC_SHAS="base=$BASE new=$C1 off=$C1" \
  REQ_C50_P1=5000000 REQ_C50_P16=30000000 REQ_SCALE_mset=0.2 LABEL_ENV_off="RECACHED_REPLY_COALESCE=off" REPS=3 ALTERNATE=1 CELLS="$c"
# C1, three workers: one client (CPU), and the P=16 tail with 5 reps
c=""; for t in set-c1-P1 set-c50-P16; do c+="base/rc-noaof-$t new/rc-noaof-$t off/rc-noaof-$t "; done
run c1v2-3w RC_BINS="base=$B/bin/rc-base new=$B/bin/rc-c1 off=$B/bin/rc-c1" RC_SHAS="base=$BASE new=$C1 off=$C1" \
  REQ_C1_P1=900000 REQ_C50_P16=70000000 SERVER_PIN=0-2 BENCH_THREADS=3 \
  LABEL_ENV_base="RECACHED_WORKER_THREADS=3" LABEL_ENV_new="RECACHED_WORKER_THREADS=3" LABEL_ENV_off="RECACHED_WORKER_THREADS=3 RECACHED_REPLY_COALESCE=off" \
  REPS=5 ALTERNATE=1 CELLS="$c"
# C1 diagnostic counters (yields, migrations, back-offs), 1 rep
c=""; for t in set-c1-P1 set-c50-P1 set-c50-P16; do c+="bdiag/rc-noaof-$t cdiag/rc-noaof-$t "; done
run c1v2-diag-3w RC_BINS="bdiag=$B/bin/rc-basediag cdiag=$B/bin/rc-c1diag" RC_SHAS="bdiag=$BASE+diag cdiag=$C1+diag" \
  REQ_C1_P1=400000 REQ_C50_P1=3000000 REQ_C50_P16=30000000 SERVER_PIN=0-2 BENCH_THREADS=3 \
  LABEL_ENV_bdiag="RECACHED_WORKER_THREADS=3" LABEL_ENV_cdiag="RECACHED_WORKER_THREADS=3" REPS=1 CELLS="$c"
c=""; for t in set-c50-P1 set-c50-P16 mset-c50-P16; do c+="bdiag/rc-noaof-$t cdiag/rc-noaof-$t "; done
run c1v2-diag-1w RC_BINS="bdiag=$B/bin/rc-basediag cdiag=$B/bin/rc-c1diag" RC_SHAS="bdiag=$BASE+diag cdiag=$C1+diag" \
  REQ_C50_P1=2000000 REQ_C50_P16=12000000 REQ_SCALE_mset=0.2 REPS=1 CELLS="$c"

# C2: LPUSH, RPUSH, SADD
c=""; for t in lpush rpush sadd; do c+="base/rc-noaof-$t-c50-P1 c2/rc-noaof-$t-c50-P1 redis-noaof-$t-c50-P1 "; done
for t in lpush rpush sadd; do c+="base/rc-noaof-$t-c50-P16 c2/rc-noaof-$t-c50-P16 "; done; c+="redis-noaof-lpush-c50-P16 redis-noaof-sadd-c50-P16 "
for t in lpush sadd; do c+="base/rc-noaof-$t-c1-P1 c2/rc-noaof-$t-c1-P1 base/rc-everysec-$t-c50-P1 c2/rc-everysec-$t-c50-P1 "; done
run c2 RC_BINS="base=$B/bin/rc-base c2=$B/bin/rc-c2" RC_SHAS="base=$BASE c2=$C2" \
  REQ_C1_P1=900000 REQ_C50_P1=5000000 REQ_C50_P16=25000000 REPS=3 ALTERNATE=1 CELLS="$c"

# C3: the into_split prototype
c=""; for t in get-c50-P1 set-c50-P1 lpush-c50-P1 sadd-c50-P1 mset-c50-P1 set-c50-P16; do c+="base/rc-noaof-$t c3/rc-noaof-$t "; done
run c3-1w RC_BINS="base=$B/bin/rc-base c3=$B/bin/rc-c3" RC_SHAS="base=$BASE c3=$C3" \
  REQ_C50_P1=5000000 REQ_C50_P16=25000000 REQ_SCALE_mset=0.5 REPS=3 ALTERNATE=1 CELLS="$c"
c="base/rc-noaof-set-c50-P1 c3/rc-noaof-set-c50-P1 base/rc-noaof-set-c1-P1 c3/rc-noaof-set-c1-P1"
run c3-3w RC_BINS="base=$B/bin/rc-base c3=$B/bin/rc-c3" RC_SHAS="base=$BASE c3=$C3" \
  REQ_C1_P1=900000 REQ_C50_P1=7500000 SERVER_PIN=0-2 BENCH_THREADS=3 LABEL_ENV_base="RECACHED_WORKER_THREADS=3" LABEL_ENV_c3="RECACHED_WORKER_THREADS=3" \
  REPS=3 ALTERNATE=1 CELLS="$c"
log "v1 driver done"
