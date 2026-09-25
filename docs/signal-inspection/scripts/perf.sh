#!/usr/bin/env bash
# Inspector off / on / on+selection+edits in the real app on the dense piece (Composition),
# same host and settings, RUNS times each, interleaved. Keeps each run's KABL_STATS_FILE.
#   DISPLAY=:97 RUNS=3 docs/signal-inspection/scripts/perf.sh
set -uo pipefail
OUT=target/signal-inspection-perf
rm -rf "$OUT"; mkdir -p "$OUT"
for run in $(seq 1 "${RUNS:-3}"); do
  for mode in off on busy; do
    d="$OUT/$mode-$run"; mkdir -p "$d"
    KABL_USER_DIR="$d/user" PIPEWIRE_NODE=kabl_rec KABL_STATS_FILE="$d/stats.txt" \
      KABL_ARGS="--rate 48000 --frames 256" \
      python3 docs/rack-migration/drive.py "docs/signal-inspection/scripts/perf-$mode.txt" 1440x900 "$d/shots" patches/composition
  done
done
for f in "$OUT"/*/stats.txt; do echo "== $f"; cat "$f"; done > "$OUT/summary.txt"
