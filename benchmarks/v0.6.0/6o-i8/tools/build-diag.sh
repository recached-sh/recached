#!/usr/bin/env bash
B=$HOME/recached-6oi8-bench
until grep -q "all done" $B/logs/build-all.txt 2>/dev/null; do sleep 10; done
for l in h i p; do PROF_ENV="CARGO_PROFILE_RELEASE_STRIP=none CARGO_PROFILE_RELEASE_DEBUG=line-tables-only" $B/build.sh $l-diag $B/src-$l-diag "-C force-frame-pointers=yes" || echo "BUILD FAILED $l-diag" >> $B/logs/build-diag.txt; done
echo "diag done $(date -u +%FT%TZ)" >> $B/logs/build-diag.txt
