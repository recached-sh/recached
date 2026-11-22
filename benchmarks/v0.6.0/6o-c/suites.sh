#!/usr/bin/env bash
# suites.sh <worktree> <outdir>: CI's commands, summaries to <outdir>.
set -uo pipefail
export PATH=$HOME/.cargo/bin:$PATH
wt=$1 out=$2; mkdir -p "$out"; cd "$wt"
sum() { grep -E "^(test result|     Running|error|warning: unused)|FAILED|panicked" "$1" > "${1%.log}.summary.txt"; }
echo "head $(git rev-parse HEAD)" > "$out/head.txt"
cargo fmt --all -- --check > "$out/fmt.log" 2>&1; echo "fmt exit $?" >> "$out/status.txt"
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings > "$out/clippy.log" 2>&1; echo "clippy exit $?" >> "$out/status.txt"
cargo test --locked --workspace --all-features > "$out/workspace.log" 2>&1; echo "workspace exit $?" >> "$out/status.txt"; sum "$out/workspace.log"
for m in inline coalesced; do RECACHED_AOF_FANOUT=$m cargo test --locked -p recached --all-features --bins > "$out/bins-$m.log" 2>&1; echo "bins-$m exit $?" >> "$out/status.txt"; sum "$out/bins-$m.log"; done
cargo test -p recached -- --include-ignored --skip dedup_write_throughput_over_websocket --skip sync_paths --skip new_commands_match_redis --skip request_path_bench --skip hot_key_scaling_bench --skip bench_replicated_set_cpu --skip layered_cost_bench > "$out/load-chaos.log" 2>&1; echo "load-chaos exit $?" >> "$out/status.txt"; sum "$out/load-chaos.log"
port=$((7700 + RANDOM % 200)); $HOME/recached-6l-bench/bin/redis-server --port $port --save '' --appendonly no --daemonize no > "$out/redis.log" 2>&1 & rp=$!; sleep 1
RECACHED_DIFF_REDIS=127.0.0.1:$port cargo test --locked -p recached --test redis_diff -- --ignored --nocapture > "$out/redis-diff.log" 2>&1; echo "redis-diff exit $?" >> "$out/status.txt"; sum "$out/redis-diff.log"; kill $rp
cat "$out/status.txt"
