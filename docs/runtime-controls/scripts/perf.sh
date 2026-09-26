#!/usr/bin/env bash
# D04 baseline-versus-final performance in the real app (release builds, same host, settings and
# workload), Composition at 48 kHz / 256 frames: steady play, a rack knob kept turning, two
# mapped CCs swept, topology swaps (undo/redo of a cable). RUNS runs each, builds interleaved.
# BASE is 6404b21 plus the measurement-only patch evidence/baseline-measurement.patch.
#   source container setup (README "Reproduce"); DISPLAY=:97 RUNS=3 docs/runtime-controls/scripts/perf.sh
set -uo pipefail
OUT=${OUT:-target/runtime-controls-perf}
BASE=${BASE:-target/base-bin/kabl-ui-measure}
FINAL=${FINAL:-target/release/kabl-ui}
export KABL_MIDI_PIPE=${KABL_MIDI_PIPE:-/tmp/kabl-midi}
rm -rf "$OUT"; mkdir -p "$OUT"
for run in $(seq 1 "${RUNS:-3}"); do
  for work in steady knob cc swaps; do
    for build in base final; do
      d="$OUT/$work-$build-$run"; mkdir -p "$d"
      bin=$BASE; [ "$build" = final ] && bin=$FINAL
      player=""; midi=""
      if [ "$work" = cc ]; then player=1; midi="--midi kabl-pipe"; fi
      echo "== $d $(date +%T)"
      KABL_BIN="$bin" KABL_USER_DIR="$d/user" KABL_LOG=debug KABL_LOG_DIR="$d/logs" \
        PIPEWIRE_NODE=kabl_rec KABL_STATS_FILE="$d/stats.txt" KABL_LATENCY_FILE="$d/latency.txt" \
        KABL_PLAYER=$player KABL_ARGS="$midi --rate 48000 --frames 256" KABL_DRIVE_LOG="$d/drive.log" \
        timeout 120 python3 docs/rack-migration/drive.py "docs/runtime-controls/scripts/perf-$work.txt" \
        1440x900 "$d/shots" patches/composition
      rm -rf "$d/shots" "$d/user"
    done
  done
done
{
  for d in "$OUT"/*/; do
    echo "== $d"
    cat "$d/stats.txt" 2>/dev/null
    cat "$d/latency.txt" 2>/dev/null
    echo "compiles logged: $(grep -c 'compiled generation' "$d"/logs/*.log 2>/dev/null)"
  done
} > "$OUT/summary.txt"
