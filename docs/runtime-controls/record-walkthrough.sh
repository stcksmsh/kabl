#!/usr/bin/env bash
# Screen + stereo-audio walkthrough of D04 and D03-R2 in the release build (scripts/walkthrough.txt,
# Composition), plus the D03-R2 run on Init Keyboard (scripts/d03r2.txt, screenshots only).
# The app's audio goes to a silent PipeWire null sink and is recorded from its monitor; the
# controller is the fifo stand-in (KABL_MIDI_PIPE), not hardware.
#   pactl load-module module-null-sink sink_name=kabl_rec
#   cargo build --release -p kabl-ui --bin kabl-ui --example midi_player
#   Xvfb :97 -screen 0 1600x1000x24 &
#   DISPLAY=:97 docs/runtime-controls/record-walkthrough.sh
set -euo pipefail
OUT=${OUT:-target/runtime-controls-walkthrough}
rm -rf "$OUT"
mkdir -p "$OUT"
export KABL_MIDI_PIPE=${KABL_MIDI_PIPE:-/tmp/kabl-midi}
KABL_USER_DIR="$OUT/user" KABL_LOG=debug KABL_LOG_DIR="$OUT/logs" PIPEWIRE_NODE=kabl_rec \
  KABL_PLAYER=1 KABL_DRIVE_LOG="$OUT/drive.log" KABL_STATS_FILE="$OUT/stats.txt" \
  KABL_LATENCY_FILE="$OUT/latency.txt" KABL_APP_LOG="$OUT/app.log" \
  KABL_ARGS="--midi kabl-pipe --perform --rate 48000 --frames 256" \
  python3 docs/rack-migration/drive.py docs/runtime-controls/scripts/walkthrough.txt 1440x900 \
  "$OUT/shots" patches/composition &
DRIVE=$!
sleep 3
date +%s.%N > "$OUT/t0"
ffmpeg -loglevel error -y -thread_queue_size 1024 -f x11grab -draw_mouse 1 -framerate 30 \
  -video_size 1440x900 -i "$DISPLAY+0,0" -thread_queue_size 1024 -f pulse -i kabl_rec.monitor \
  -c:v libx264 -preset veryfast -crf 26 -pix_fmt yuv420p \
  -c:a aac -b:a 160k "$OUT/walkthrough.mp4" &
REC=$!
wait $DRIVE
kill -INT $REC; wait $REC || true
R2="$OUT/d03r2"; mkdir -p "$R2"
KABL_USER_DIR="$R2/user" KABL_LOG=debug KABL_LOG_DIR="$R2/logs" PIPEWIRE_NODE=kabl_rec \
  KABL_PLAYER=1 KABL_DRIVE_LOG="$R2/drive.log" KABL_STATS_FILE="$R2/stats.txt" \
  KABL_ARGS="--midi kabl-pipe --perform --rate 48000 --frames 256" \
  python3 docs/rack-migration/drive.py docs/runtime-controls/scripts/d03r2.txt 1440x900 \
  "$R2/shots" patches/init-keyboard
