#!/usr/bin/env bash
# 6O-I8 I8-D: correctness runs, in the sandbox, while gate2 holds the host's
# campaigns. Output: ~/recached-6oi8-bench/raw-tests/
export PATH=$HOME/.cargo/bin:$PATH
B=$HOME/recached-6oi8-bench
O=$B/raw-tests
mkdir -p "$O"
SKIPS="--skip dedup_write_throughput_over_websocket --skip sync_paths --skip new_commands_match_redis --skip request_path_bench --skip hot_key_scaling_bench --skip bench_replicated_set_cpu --skip layered_cost_bench"
FLAKY=tests::replica_expirylog_6oh::a_replica_reads_ahead_while_its_full_sync_checkpoint_runs
note() { echo "$(date -u +%FT%TZ) $*" | tee -a "$O/index.txt"; }

suite() { # label tree
  local l=$1 t=$2 td=$B/target-test-$1
  note "== $l $(git -C "$t" rev-parse HEAD) dirty=$(git -C "$t" status --porcelain | wc -l)"
  (cd "$t" && cargo test --locked -p recached --all-features --bins --target-dir "$td" > "$O/$l-inline.txt" 2>&1); note "$l inline exit=$? $(grep 'test result' "$O/$l-inline.txt" | tail -1)"
  (cd "$t" && RECACHED_AOF_FANOUT=coalesced cargo test --locked -p recached --all-features --bins --target-dir "$td" > "$O/$l-coalesced.txt" 2>&1); note "$l coalesced exit=$? $(grep 'test result' "$O/$l-coalesced.txt" | tail -1)"
  (cd "$t" && cargo test --locked -p recached --all-features --bins --target-dir "$td" -- --include-ignored $SKIPS > "$O/$l-chaos.txt" 2>&1); note "$l load-and-chaos exit=$? $(grep 'test result' "$O/$l-chaos.txt" | tail -1)"
}

flaky() { # label tree
  local l=$1 t=$2 td=$B/target-test-$1
  local bin
  bin=$(cd "$t" && cargo test --locked -p recached --all-features --bins --target-dir "$td" --no-run 2>&1 | grep -o '/[^ ]*recached_server-[0-9a-f]*' | tail -1)
  note "$l flaky test binary $bin"
  for i in $(seq 1 10); do
    s=$(date +%s.%N); "$bin" --include-ignored --exact "$FLAKY" > "$O/$l-flaky-alone-$i.txt" 2>&1; rc=$?
    note "$l flaky alone $i exit=$rc secs=$(awk -v a="$s" -v b="$(date +%s.%N)" "BEGIN{printf \"%.1f\", b-a}")"
  done
  # Starved: the test on 2 CPUs with 4 busy loops on the same 2 CPUs.
  for i in $(seq 1 10); do
    pids=(); for _ in 1 2 3 4; do taskset -c 0,1 sh -c 'while :; do :; done' & pids+=($!); done
    s=$(date +%s.%N); taskset -c 0,1 "$bin" --include-ignored --exact "$FLAKY" > "$O/$l-flaky-starved-$i.txt" 2>&1; rc=$?
    note "$l flaky starved $i exit=$rc secs=$(awk -v a="$s" -v b="$(date +%s.%N)" "BEGIN{printf \"%.1f\", b-a}")"
    kill "${pids[@]}" 2>/dev/null; wait "${pids[@]}" 2>/dev/null
  done
  # The whole load-and-chaos suite on 2 CPUs, as a starved runner would run it.
  for i in 1 2 3; do
    s=$(date +%s.%N); taskset -c 0,1 "$bin" --include-ignored $SKIPS > "$O/$l-chaos-2cpu-$i.txt" 2>&1; rc=$?
    note "$l chaos on 2 CPUs $i exit=$rc secs=$(awk -v a="$s" -v b="$(date +%s.%N)" "BEGIN{printf \"%.1f\", b-a}") $(grep -E '^test result' "$O/$l-chaos-2cpu-$i.txt" | tail -1) failed: $(grep -E ' \.\.\. FAILED' "$O/$l-chaos-2cpu-$i.txt" | awk '{print $2}' | tr '\n' ' ')"
  done
}

note "gate2 start"
suite p "$B/src-p"
suite px "$B/src-px"
flaky p "$B/src-p"
flaky h "$B/src-h"
note "gate2 tests done"
