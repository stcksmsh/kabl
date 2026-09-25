#!/usr/bin/env bash
# Screen + stereo-audio walkthrough of D03 (inspection, "Why no sound?", comparison, the three
# recipes) in the release build. The app's audio goes to a silent PipeWire null sink and is
# recorded from its monitor. Container setup (see README "Reproduce"):
#   pactl load-module module-null-sink sink_name=kabl_rec
#   cargo build --release -p kabl-ui --bin kabl-ui --example midi_player
#   Xvfb :97 -screen 0 1600x1000x24 &
#   DISPLAY=:97 docs/signal-inspection/record-walkthrough.sh
# The controller is the fifo stand-in (KABL_MIDI_PIPE), not hardware. FRAMES sets the buffer.
set -euo pipefail
OUT=${OUT:-target/signal-inspection-walkthrough}
rm -rf "$OUT"
mkdir -p "$OUT"
export KABL_USER_DIR="$OUT/user"
export KABL_MIDI_PIPE=${KABL_MIDI_PIPE:-/tmp/kabl-midi}
KABL_LOG=${KABL_LOG:-debug} PIPEWIRE_NODE=kabl_rec KABL_PLAYER=1 KABL_DRIVE_LOG="$OUT/drive.log" KABL_STATS_FILE="$OUT/stats.txt" \
  KABL_APP_LOG="$OUT/app.log" KABL_ARGS="--midi kabl-pipe --rate 48000 --frames ${FRAMES:-256}" \
  python3 docs/rack-migration/drive.py \
  docs/signal-inspection/scripts/walkthrough.txt 1440x900 "$OUT/shots" patches/init-keyboard &
DRIVE=$!
sleep 3
date +%s.%N > "$OUT/t0"
ffmpeg -loglevel error -y -thread_queue_size 1024 -f x11grab -draw_mouse 1 -framerate 30 \
  -video_size 1440x900 -i "$DISPLAY+0,0" -thread_queue_size 1024 -f pulse -i kabl_rec.monitor \
  -c:v libx264 -preset veryfast -crf 26 -pix_fmt yuv420p \
  -c:a aac -b:a 160k "$OUT/walkthrough.mp4" &
REC=$!
trap 'kill -INT $REC 2>/dev/null' EXIT
wait $DRIVE
