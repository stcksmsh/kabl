#!/usr/bin/env bash
# One command: rebuild the patch, render the audio, drive the real app, record the video.
#   docs/promo-demo/record.sh
# Needs: cargo, Xvfb, xdotool, ffmpeg, ImageMagick (import), PipeWire/PulseAudio (pactl), python3 + numpy.
# The app plays into a silent null sink (kabl_rec); the recording takes its monitor. The window is
# a 1920x1080 Xvfb screen driven by xdotool (real X input), not the machine's own display.
set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=${OUT:-target/promo-demo}
XDISP=${XDISP:-:98}
SIZE=1920x1080
PATCH=docs/promo-demo/patch/morph-suite
rm -rf "$OUT"
mkdir -p "$OUT"

cargo build --release -p kabl-ui --bin kabl-ui --example promo_demo
DEMO=target/release/examples/promo_demo
$DEMO write $PATCH docs/promo-demo/timeline.json
$DEMO render "$OUT/render.wav" > "$OUT/render-levels.txt"
head -1 "$OUT/render-levels.txt"
ffmpeg -loglevel error -y -i "$OUT/render.wav" -b:a 160k docs/promo-demo/morph-suite-audio.mp3
python3 -I docs/promo-demo/scripts/make-drive-script.py docs/promo-demo/timeline.json docs/promo-demo/targets.json "$OUT/drive.txt"

pactl list short sinks | grep -q kabl_rec || pactl load-module module-null-sink sink_name=kabl_rec
Xvfb "$XDISP" -screen 0 ${SIZE}x24 >/dev/null 2>&1 &
XVFB=$!
trap 'kill $XVFB 2>/dev/null || true' EXIT
sleep 2

export DISPLAY=$XDISP
FF_START=$(date +%s.%N)
echo "$FF_START" > "$OUT/ffmpeg-start.txt"
ffmpeg -loglevel error -y -thread_queue_size 1024 -f x11grab -draw_mouse 1 -framerate 30 \
  -video_size $SIZE -i "$DISPLAY+0,0" -thread_queue_size 1024 -f pulse -i kabl_rec.monitor \
  -c:v libx264 -preset veryfast -crf 20 -pix_fmt yuv420p -c:a aac -b:a 160k "$OUT/recording.mp4" &
REC=$!
KABL_USER_DIR="$OUT/user" PIPEWIRE_NODE=kabl_rec KABL_DRIVE_LOG="$OUT/drive.log" \
  KABL_APP_LOG="$OUT/app.log" KABL_STATS_FILE="$OUT/stats.txt" \
  KABL_ARGS="--rate 48000 --frames 256" \
  python3 docs/rack-migration/drive.py "$OUT/drive.txt" $SIZE "$OUT/shots" $PATCH
kill -INT $REC
wait $REC || true

python3 -I docs/promo-demo/scripts/post.py docs/promo-demo/timeline.json "$OUT/render.wav" \
  "$OUT/recording.mp4" "$OUT/drive.log" "$FF_START" docs/promo-demo/morph-suite-promo.mp4 "$OUT"
