#!/usr/bin/env bash
# Screen + sound walkthrough of the real release kabl-ui: chords played in through ALSA's Midi
# Through port, the app's real audio sent to a silent PipeWire null sink and recorded from its
# monitor, the rack driven by drive.py (scripts/walkthrough.txt). Nothing plays on the speakers.
#   pactl load-module module-null-sink sink_name=kabl_rec
#   cargo build --release -p kabl-ui; Xvfb :97 -screen 0 1600x1000x24 &
#   DISPLAY=:97 docs/rack-migration/record-walkthrough.sh     # chords: target/slice-av/chords.mid
set -euo pipefail
OUT=target/rack-walkthrough
mkdir -p "$OUT"
PIPEWIRE_NODE=kabl_rec python3 docs/rack-migration/drive.py \
  docs/rack-migration/scripts/walkthrough.txt 1440x900 "$OUT/shots" &
DRIVE=$!
sleep 2.5   # drive.py starts driving after 3 s
ffmpeg -loglevel error -y -thread_queue_size 1024 -f x11grab -draw_mouse 1 -framerate 30 \
  -video_size 1440x900 -i "$DISPLAY+0,0" -thread_queue_size 1024 -f pulse -i kabl_rec.monitor \
  -vf scale=1280:-2 -c:v libx264 -preset veryfast -crf 26 -pix_fmt yuv420p \
  -c:a aac -b:a 160k "$OUT/walkthrough.mp4" &
REC=$!
( while kill -0 $DRIVE 2>/dev/null; do aplaymidi -p 14:0 target/slice-av/chords.mid; done ) &
MIDI=$!
trap 'kill -INT $REC 2>/dev/null; kill $MIDI 2>/dev/null; pkill -x aplaymidi' EXIT
wait $DRIVE
