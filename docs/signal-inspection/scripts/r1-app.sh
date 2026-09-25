#!/usr/bin/env bash
# D03-R1 (review R2) real-app evidence on the release build: r1-app.txt (Init Keyboard, screen
# + audio recorded) and r1-default-vca.txt (a derived default-gain VCA patch). Container setup
# as in README "Reproduce".  DISPLAY=:97 docs/signal-inspection/scripts/r1-app.sh
set -euo pipefail
OUT=${OUT:-target/signal-inspection-r1}
rm -rf "$OUT"; mkdir -p "$OUT"
export KABL_MIDI_PIPE=${KABL_MIDI_PIPE:-/tmp/kabl-midi}
KABL_USER_DIR="$OUT/user" KABL_LOG=debug PIPEWIRE_NODE=kabl_rec KABL_PLAYER=1 \
  KABL_DRIVE_LOG="$OUT/drive.log" KABL_STATS_FILE="$OUT/stats.txt" KABL_APP_LOG="$OUT/app.log" \
  KABL_ARGS="--midi kabl-pipe --rate 48000 --frames 256" \
  python3 docs/rack-migration/drive.py docs/signal-inspection/scripts/r1-app.txt 1440x900 \
  "$OUT/shots" patches/init-keyboard &
DRIVE=$!
sleep 3
ffmpeg -loglevel error -y -thread_queue_size 1024 -f x11grab -draw_mouse 1 -framerate 30 \
  -video_size 1440x900 -i "$DISPLAY+0,0" -thread_queue_size 1024 -f pulse -i kabl_rec.monitor \
  -c:v libx264 -preset veryfast -crf 26 -pix_fmt yuv420p -c:a aac -b:a 160k "$OUT/r1-app.mp4" &
REC=$!
wait $DRIVE
kill -INT $REC; wait $REC || true
# Init Keyboard without VCA #4's gain setting and its cv cable: gain stays at the default.
P="$OUT/default-vca"; cp -r patches/init-keyboard "$P"
grep -v '"param":"gain"' patches/init-keyboard/log.jsonl | grep -v '"port":"cv"' > "$P/log.jsonl"
KABL_USER_DIR="$OUT/user2" PIPEWIRE_NODE=kabl_rec KABL_ARGS="--rate 48000 --frames 256" \
  python3 docs/rack-migration/drive.py docs/signal-inspection/scripts/r1-default-vca.txt 1440x900 \
  "$OUT/shots" "$P"
