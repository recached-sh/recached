#!/usr/bin/env bash
# ABBA ×2: A = dev 38e2880 (v0.6 current), B = 6G X1 (Arc keys, boxed entries).
set -euo pipefail
A=${A:-../../../target/6g/tgt-dev/release/examples/hotpath}
B=${B:-../../../target/6g/tgt-opt/release/examples/hotpath}
sleep 60   # let the build load drain
for round in 1 2; do
  for which in A B B A; do
    bin=${!which}
    echo "### round=$round build=$which load=$(uptime | sed 's/.*averages*: //')"
    "$bin"
  done
done
