#!/usr/bin/env bash
# Screen + application-audio walkthrough of D06 in the release app on DISPLAY (an Xvfb of at
# least 1440x900). The controller is scripted virtual MIDI (play_events.py | midi_player). The
# sound track is the app's own recording of its output (Perform > Record), placed at the
# moment Record was clicked; it is not dubbed offline audio. Audio also plays on the default
# output.
#   Xvfb :98 -screen 0 1440x900x24 &
#   DISPLAY=:98 OUT=/tmp/d06-walkthrough docs/midi-timing/scripts/record-walkthrough.sh
set -euo pipefail
OUT=${OUT:?}
rm -rf "$OUT"
mkdir -p "$OUT"
python3 docs/midi-timing/scripts/play_events.py docs/midi-timing/scripts/walkthrough.events 6 \
  | ./target/release/examples/midi_player 2> "$OUT/player.log" &
trap 'pkill -f play_events.py 2>/dev/null; pkill -x midi_player 2>/dev/null; true' EXIT
sleep 0.7
KABL_USER_DIR="$OUT/user" KABL_STATS_FILE="$OUT/stats.txt" KABL_APP_LOG="$OUT/app.log" \
  KABL_DRIVE_LOG="$OUT/drive.log" \
  KABL_ARGS="--midi kabl-player --rate 48000 --frames 256 --perform --record-dir $OUT/rec" \
  python3 docs/rack-migration/drive.py docs/midi-timing/scripts/walkthrough-ui.txt 1440x900 \
  "$OUT/shots" patches/expressive/split &
DRIVE=$!
sleep 1.5
date +%s.%N > "$OUT/t0"
ffmpeg -loglevel error -y -f x11grab -draw_mouse 1 -framerate 30 -video_size 1440x900 \
  -i "$DISPLAY+0,0" -c:v libx264 -preset veryfast -crf 26 -pix_fmt yuv420p "$OUT/video.mp4" &
REC=$!
wait $DRIVE
kill -INT $REC
wait $REC || true
take=$(ls "$OUT"/rec/*.wav)
start=$(awk '/click rec-start/ {print $1; exit}' "$OUT/drive.log")
delay=$(python3 -c "print(max(0.0, $start - $(cat "$OUT/t0")))")
ffmpeg -loglevel error -y -i "$OUT/video.mp4" -itsoffset "$delay" -i "$take" -map 0:v -map 1:a \
  -c:v copy -c:a aac -b:a 192k "$OUT/walkthrough.mp4"
echo "walkthrough: $OUT/walkthrough.mp4 (take placed at $delay s)"
