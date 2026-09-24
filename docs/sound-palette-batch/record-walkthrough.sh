#!/usr/bin/env bash
# Screen + stereo-audio walkthrough of patches/sound-palette in the real release kabl-ui,
# played from the virtual MIDI controller. The app's audio goes to a silent PipeWire null
# sink and is recorded from its monitor; nothing plays on speakers.
#   pactl load-module module-null-sink sink_name=kabl_rec
#   cargo build --release -p kabl-ui --bin kabl-ui --example midi_player
#   Xvfb :97 -screen 0 1600x1000x24 &
#   DISPLAY=:97 docs/sound-palette-batch/record-walkthrough.sh
# Without an ALSA sequencer (a container) also set KABL_MIDI_PIPE=/tmp/kabl-midi; the
# controller then reaches kabl-ui through that fifo (MIDI=kabl-pipe below). FRAMES sets the
# buffer (default 256).
set -euo pipefail
OUT=${OUT:-target/sound-palette-walkthrough}
MIDI=${MIDI:-kabl-player}
mkdir -p "$OUT"
PIPEWIRE_NODE=kabl_rec KABL_PLAYER=1 KABL_DRIVE_LOG="$OUT/drive.log" KABL_STATS_FILE="$OUT/stats.txt" \
  KABL_ARGS="--midi $MIDI --perform --rate 48000 --frames ${FRAMES:-256}" \
  python3 docs/rack-migration/drive.py \
  docs/sound-palette-batch/scripts/walkthrough.txt 1440x900 "$OUT/shots" patches/sound-palette &
DRIVE=$!
sleep 3   # drive.py starts driving after 3.5 s (0.5 s for the controller)
date +%s.%N > "$OUT/t0"   # video time 0, to place the shots/w* markers on the timeline
ffmpeg -loglevel error -y -thread_queue_size 1024 -f x11grab -draw_mouse 1 -framerate 30 \
  -video_size 1440x900 -i "$DISPLAY+0,0" -thread_queue_size 1024 -f pulse -i kabl_rec.monitor \
  -vf scale=1280:-2 -c:v libx264 -preset veryfast -crf 26 -pix_fmt yuv420p \
  -c:a aac -b:a 160k "$OUT/walkthrough.mp4" &
REC=$!
trap 'kill -INT $REC 2>/dev/null' EXIT
wait $DRIVE
