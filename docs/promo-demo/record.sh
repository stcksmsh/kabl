#!/usr/bin/env bash
# One command: rebuild the patch, render the audio, drive the real app, record the video.
#   docs/promo-demo/record.sh
# Needs: cargo, Xvfb, xdotool, ffmpeg, ImageMagick (import), PipeWire/PulseAudio (pactl), python3 + numpy.
# The app plays into a silent null sink (default kabl_agents; the recording takes its monitor). The
# window is a 1920x1080 Xvfb screen driven by xdotool (real X input), not the machine's own display.
#
# Options (environment):
#   FPS=60          capture frame rate
#   SINK=kabl_agents  audio sink; set SINK=kabl_rec for the old one (created if missing)
#   CRF=18          final quality (post.py)
#   TAKES="patch:drive[:timeline:render] ..."  several takes instead of the built-in morph-suite one.
#                   patch = patch folder, drive = drive script (make-drive-script.py output), timeline and
#                   render (promo_demo timeline.json, reference wav) default to the morph-suite ones.
#                   Each take is recorded and finished into $OUT/take-N/take.mp4.
#   CUTS=S1,S2,...  with TAKES: join the takes at these seconds (see post.py --join); output $FINAL
#   KABL_ANCHOR     bar-0 click key if not targets.json "anchor"
#   FINAL           final mp4 (default docs/promo-demo/morph-suite-promo.mp4)
set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=${OUT:-target/promo-demo}
XDISP=${XDISP:-:98}
FPS=${FPS:-60}
SINK=${SINK:-kabl_agents}
FINAL=${FINAL:-docs/promo-demo/morph-suite-promo.mp4}
SIZE=1920x1080
PATCH=docs/promo-demo/patch/morph-suite
TIMELINE=docs/promo-demo/timeline.json
rm -rf "$OUT"
mkdir -p "$OUT"

TARGET=${CARGO_TARGET_DIR:-target}
cargo build --release -p kabl-ui --bin kabl-ui
export KABL_BIN=${KABL_BIN:-$TARGET/release/kabl-ui}
if [ -z "${TAKES:-}" ]; then
  cargo build --release -p kabl-ui --example promo_demo
  DEMO=$TARGET/release/examples/promo_demo
  $DEMO write "$PATCH" $TIMELINE
  $DEMO render "$OUT/render.wav" > "$OUT/render-levels.txt"
  head -1 "$OUT/render-levels.txt"
  ffmpeg -loglevel error -y -i "$OUT/render.wav" -b:a 160k docs/promo-demo/morph-suite-audio.mp3
  python3 -I docs/promo-demo/scripts/make-drive-script.py $TIMELINE docs/promo-demo/targets.json "$OUT/drive.txt"
  TAKES="$PATCH:$OUT/drive.txt:$TIMELINE:$OUT/render.wav"
fi

pactl list short sinks | grep -q "$SINK" || pactl load-module module-null-sink sink_name="$SINK" \
  sink_properties=device.description=kabl-agents-silent
Xvfb "$XDISP" -screen 0 ${SIZE}x24 >/dev/null 2>&1 &
XVFB=$!
trap 'kill $XVFB 2>/dev/null || true' EXIT
sleep 2
export DISPLAY=$XDISP

N=0
DONE=()
for TAKE in $TAKES; do
  N=$((N + 1))
  IFS=: read -r T_PATCH T_DRIVE T_TIMELINE T_RENDER <<< "$TAKE"
  T_TIMELINE=${T_TIMELINE:-$TIMELINE}
  T_RENDER=${T_RENDER:-$OUT/render.wav}
  TO="$OUT/take-$N"
  mkdir -p "$TO"
  FF_START=$(date +%s.%N)
  echo "$FF_START" > "$TO/ffmpeg-start.txt"
  # Lossless capture so no frame drops; post.py re-encodes.
  ffmpeg -loglevel error -y -thread_queue_size 1024 -f x11grab -draw_mouse 1 -framerate "$FPS" \
    -video_size $SIZE -i "$DISPLAY+0,0" -thread_queue_size 1024 -f pulse -i "$SINK.monitor" \
    -c:v libx264rgb -preset ultrafast -qp 0 -c:a aac -b:a 160k "$TO/recording.mp4" &
  REC=$!
  KABL_USER_DIR="$TO/user" PIPEWIRE_NODE="$SINK" PULSE_SINK="$SINK" KABL_DRIVE_LOG="$TO/drive.log" \
    KABL_APP_LOG="$TO/app.log" KABL_STATS_FILE="$TO/stats.txt" \
    KABL_ARGS="--rate 48000 --frames 256" \
    python3 docs/rack-migration/drive.py "$T_DRIVE" $SIZE "$TO/shots" "$T_PATCH"
  kill -INT $REC
  wait $REC || true
  python3 -I docs/promo-demo/scripts/post.py "$T_TIMELINE" "$T_RENDER" \
    "$TO/recording.mp4" "$TO/drive.log" "$FF_START" "$TO/take.mp4" "$TO"
  DONE+=("$TO/take.mp4")
done

if [ "$N" -eq 1 ]; then
  cp "${DONE[0]}" "$FINAL"
elif [ -n "${CUTS:-}" ]; then
  python3 -I docs/promo-demo/scripts/post.py --join "$FINAL" "$CUTS" "${DONE[@]}"
else
  echo "takes are in $OUT/take-*/take.mp4; set CUTS to join them"
fi
