#!/usr/bin/env bash
# 6O-I8: copy a campaign's evidence from the bench directory into raw/<campaign>/.
#   pack_raw.sh <campaign> [<campaign> ...]
# Kept: the campaign's config, meta, driver log, runs.jsonl (every run's record),
# summary.md/.json and extra reports; per run: the load generator's output and
# series, the servers' logs (stdout = tracing; *.log.txt = stderr, the diag
# lines), perf stat CSVs, perf report text, jemalloc heap profiles. The 1 Hz
# series and the stderr logs are gzipped. Not kept: data directories, the
# expected-keyspace files (3 MB each) and perf.data recordings; each is listed
# with its size and SHA-256 in MANIFEST-not-copied.txt.
set -euo pipefail
SRC=$HOME/recached-6oi8-bench/runs
DST=$(cd "$(dirname "$0")/.." && pwd)/raw
for c in "$@"; do
  s=$SRC/$c d=$DST/$c
  rm -rf "$d"; mkdir -p "$d"
  for f in config.json meta.txt driver.txt runs.jsonl summary.md summary.json cycles.md perf-summary.md callers-h.txt callers-p.txt bounces.md stalls.md; do
    [[ -e $s/$f ]] && cp "$s/$f" "$d/"
  done
  for f in "$s"/jeprof-*.txt; do [[ -e $f ]] && cp "$f" "$d/"; done
  : > "$d/MANIFEST-not-copied.txt"
  for r in "$s"/*/; do
    r=${r%/}; n=$(basename "$r")
    [[ $n == data ]] && continue
    mkdir -p "$d/$n"
    for f in "$r"/*; do
      b=$(basename "$f")
      case $b in
        data) continue ;;
        load.expected|perf-*.data|*.heap)
          if [[ $b == *.heap ]]; then gzip -c "$f" > "$d/$n/$b.gz"; continue; fi
          echo "$n/$b $(stat -c %s "$f") $(sha256sum "$f" | cut -d' ' -f1)" >> "$d/MANIFEST-not-copied.txt" ;;
        series.jsonl|*.log.txt) gzip -c "$f" > "$d/$n/$b.gz" ;;
        *.stdout.txt)
          # Server logs: gzipped when large (the tmpfs-quota runs log one error per write).
          if [[ $(stat -c %s "$f") -gt 1000000 ]]; then gzip -9 -c "$f" > "$d/$n/$b.gz"; else cp "$f" "$d/$n/"; fi ;;
        *) cp "$f" "$d/$n/" ;;
      esac
    done
  done
  echo "$c: $(du -sh "$d" | cut -f1)"
done
