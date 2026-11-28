#!/usr/bin/env bash
B=$HOME/recached-6oi8-bench
for l in h i p; do $B/build.sh $l $B/src-$l || echo "BUILD FAILED $l" >> $B/logs/build-all.txt; done
# Profiling variants: frame pointers + symbols, for perf attribution only (never for throughput).
for l in h i p; do PROF_ENV="CARGO_PROFILE_RELEASE_STRIP=none CARGO_PROFILE_RELEASE_DEBUG=line-tables-only" $B/build.sh $l-fp $B/src-$l "-C force-frame-pointers=yes" || echo "BUILD FAILED $l-fp" >> $B/logs/build-all.txt; done
echo "all done $(date -u +%FT%TZ)" >> $B/logs/build-all.txt
