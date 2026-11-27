#!/usr/bin/env bash
# 6O-I8 gate3: the diag v2 builds (after reviews A and B), and a check that px-diag2 compiles.
B=$HOME/recached-6oi8-bench
for l in h p px; do PROF_ENV="CARGO_PROFILE_RELEASE_STRIP=none CARGO_PROFILE_RELEASE_DEBUG=line-tables-only" $B/build.sh $l-diag2 $B/src-$l-diag2 "-C force-frame-pointers=yes" || echo "BUILD FAILED $l-diag2" >> $B/logs/build-diag2.txt; done
echo "diag2 done $(date -u +%FT%TZ)" >> $B/logs/build-diag2.txt
