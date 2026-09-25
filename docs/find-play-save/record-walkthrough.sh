#!/usr/bin/env bash
# Screen + stereo-audio walkthrough of the D01 browser in the installed release package.
# The app's audio goes to a silent PipeWire null sink and is recorded from its monitor.
#   pactl load-module module-null-sink sink_name=kabl_rec
#   packaging/linux/package.sh && tar -xzf target/dist/kabl-*.tar.gz -C /tmp/kabl-pkg
#   cargo build --release -p kabl-ui --example midi_player
#   Xvfb :97 -screen 0 1600x1000x24 &
#   DISPLAY=:97 KABL_BIN=/tmp/kabl-pkg/kabl-0.1.0-linux-x86_64/bin/kabl-ui \
#     docs/find-play-save/record-walkthrough.sh
# The controller is the fifo stand-in (KABL_MIDI_PIPE). FRAMES sets the buffer (default 256).
set -euo pipefail
OUT=${OUT:-target/find-play-save-walkthrough}
rm -rf "$OUT"
mkdir -p "$OUT"
export KABL_USER_DIR="$OUT/user"   # a fresh user library for the take
export KABL_MIDI_PIPE=${KABL_MIDI_PIPE:-/tmp/kabl-midi}
PIPEWIRE_NODE=kabl_rec KABL_PLAYER=1 KABL_DRIVE_LOG="$OUT/drive.log" KABL_STATS_FILE="$OUT/stats.txt" \
  KABL_APP_LOG="$OUT/app.log" KABL_ARGS="--midi kabl-pipe --rate 48000 --frames ${FRAMES:-256}" \
  python3 docs/rack-migration/drive.py \
  docs/find-play-save/scripts/walkthrough.txt 1440x900 "$OUT/shots" - &
DRIVE=$!
sleep 3   # drive.py starts driving after 3.5 s
# No rtkit in a container: give the audio thread FIFO priority by hand (RT=0 skips).
if [ "${RT:-1}" = 1 ]; then
  pid=$(pgrep -n -f 'bin/kabl-ui' || true)
  for t in /proc/$pid/task/*; do
    if [ "$(cat "$t/comm")" = cpal_alsa_out ]; then chrt -f -p 80 "$(basename "$t")" && echo "RT: $(basename "$t")" >> "$OUT/rt.txt"; fi
  done
fi
date +%s.%N > "$OUT/t0"   # video time 0, to place the drive.log lines on the timeline
ffmpeg -loglevel error -y -thread_queue_size 1024 -f x11grab -draw_mouse 1 -framerate 30 \
  -video_size 1440x900 -i "$DISPLAY+0,0" -thread_queue_size 1024 -f pulse -i kabl_rec.monitor \
  -c:v libx264 -preset veryfast -crf 26 -pix_fmt yuv420p \
  -c:a aac -b:a 160k "$OUT/walkthrough.mp4" &
REC=$!
trap 'kill -INT $REC 2>/dev/null' EXIT
wait $DRIVE
