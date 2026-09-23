#!/usr/bin/env bash
# Screen + sound walkthrough of patches/echo in the real release kabl-ui. The app's
# audio goes to a silent PipeWire null sink and is recorded from its monitor; the rack is
# driven by docs/rack-migration/drive.py (scripts/walkthrough.txt). Nothing plays on speakers.
#   pactl load-module module-null-sink sink_name=kabl_rec
#   cargo build --release -p kabl-ui; Xvfb :97 -screen 0 1600x1000x24 &
#   DISPLAY=:97 docs/echo/record-walkthrough.sh
set -euo pipefail
OUT=${OUT:-target/echo-walkthrough}
mkdir -p "$OUT"
PIPEWIRE_NODE=kabl_rec python3 docs/rack-migration/drive.py \
  docs/echo/scripts/walkthrough.txt 1440x900 "$OUT/shots" patches/echo &
DRIVE=$!
sleep 2.5   # drive.py starts driving after 3 s
date +%s.%N > "$OUT/t0"   # video time 0, to place the shots/m* markers on the timeline
ffmpeg -loglevel error -y -thread_queue_size 1024 -f x11grab -draw_mouse 1 -framerate 30 \
  -video_size 1440x900 -i "$DISPLAY+0,0" -thread_queue_size 1024 -f pulse -i kabl_rec.monitor \
  -vf scale=1280:-2 -c:v libx264 -preset veryfast -crf 26 -pix_fmt yuv420p \
  -c:a aac -b:a 160k "$OUT/walkthrough.mp4" &
REC=$!
trap 'kill -INT $REC 2>/dev/null' EXIT
wait $DRIVE
