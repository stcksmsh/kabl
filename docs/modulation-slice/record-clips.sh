#!/usr/bin/env bash
# Screen recordings of the real kabl-ui (release build) driven by xdotool on Xvfb, for remote
# review. Same hit-target source as xdotool-walkthrough.sh. Video only: no audio is captured.
#   cargo test -p kabl-ui --test interaction -- --ignored && cargo build --release -p kabl-ui
#   Xvfb :97 -screen 0 1600x1000x24 &
#   DISPLAY=:97 docs/modulation-slice/record-clips.sh
set -euo pipefail
SIZE=1440x900
OUT=target/slice-clips
HITS=target/slice-shots/hits-$SIZE.txt
mkdir -p "$OUT"

at() { awk -v k="$1" '$1 == k { print $2, $3 }' "$HITS"; }
pause() { sleep "${1:-0.8}"; }
move() { xdotool mousemove "$1" "$2"; sleep 0.03; }
glide() { # glide from current pointer to X Y
  eval "$(xdotool getmouselocation --shell)"
  for i in $(seq 1 15); do move $((X + ($1 - X) * i / 15)) $((Y + ($2 - Y) * i / 15)); done
}
click() { read -r x y <<<"$(at "$1")"; glide "$x" "$y"; pause 0.3; xdotool click 1; pause; }
drag_to() { # KEY X Y
  read -r x y <<<"$(at "$1")"; glide "$x" "$y"; pause 0.3
  xdotool mousedown 1; sleep 0.1
  for i in $(seq 1 30); do move $((x + ($2 - x) * i / 30)) $((y + ($3 - y) * i / 30)); done
  pause 0.6; xdotool mouseup 1; pause
}
vdrag() { read -r x y <<<"$(at "$1")"; drag_to "$1" "$x" $((y - $2)); }
focus() { xdotool windowfocus --sync "$(xdotool search --name '^kabl$' | head -1)"; }

start() { # NAME: fresh app on the reference patch, start recording
  ./target/release/kabl-ui --patch "${PATCH:-patches/reference}" --size $SIZE >/dev/null 2>&1 &
  APP=$!; sleep 3; focus; xdotool mousemove 900 700
  ffmpeg -loglevel error -y -f x11grab -draw_mouse 1 -framerate 30 -video_size $SIZE -i "$DISPLAY+0,0" \
    -vf scale=1280:-2 -c:v libx264 -preset veryfast -crf 26 -pix_fmt yuv420p "$OUT/$1.mp4" &
  REC=$!; pause 1.5
}
stop() { pause 1.5; kill -INT $REC; wait $REC || true; kill $APP; wait $APP 2>/dev/null || true; echo "clip $1"; }

# 1: add a source by dragging, then ring (depth) vs body (base).
start 1-add-source
read -r kx ky <<<"$(at knob:4.decay_ms)"
drag_to out:8.out "$kx" "$ky"
vdrag ring:4.decay_ms 30      # single source: auto-selected, ring edits its depth
vdrag ring:4.decay_ms -60
read -r x y <<<"$(at knob:4.decay_ms)"
drag_to knob:4.decay_ms "$x" $((y - 25))   # body: base value only
stop 1-add-source

# 2: two sources on Attack: none selected, pick one, edit, invert, bypass, remove, undo.
start 2-two-sources
click knob:4.attack_ms
vdrag ring:4.attack_ms 30     # nothing selected: no change
click row:9
vdrag ring:4.attack_ms 30
click invert:9
click bypass:9
click bypass:9
click remove:9
vdrag ring:4.attack_ms 30     # selection cleared: no change
glide 900 700; xdotool key ctrl+z; pause 1.2
stop 2-two-sources

# 3: four sources on Cutoff, each picked and edited alone.
start 3-four-sources
click knob:3.cutoff_hz
for r in 10 11 12 13; do click row:$r; vdrag ring:3.cutoff_hz 20; done
stop 3-four-sources

# 4: cable views, envelope timing, undo/redo.
start 4-views-timing
click view:Focus; click knob:4.attack_ms; pause
click view:Hidden; pause 1.2
click view:All
click sel:4.timing.1; pause
glide 900 700; xdotool key ctrl+z; pause 1.2; xdotool key ctrl+shift+z; pause 1.2
stop 4-views-timing

# 5: per-source lanes on the knob: pick and edit each source without the drawer.
start 5-source-lanes
click knob:3.cutoff_hz; pause
for r in 12 10 13 11; do vdrag lane:$r 25; done
glide 300 820; xdotool click 1; pause   # empty canvas closes the lanes
click knob:4.attack_ms; pause
vdrag lane:9 -30
stop 5-source-lanes
