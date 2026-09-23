#!/usr/bin/env bash
# Real-X walkthrough of the modulation slice: drives the release kabl-ui binary with xdotool
# (real X pointer/keyboard events through winit), saves screenshots, saves the edited patch and
# reloads it. Target coordinates come from the UI's own hit rects:
#   cargo test -p kabl-ui --test interaction -- --ignored   # writes target/slice-shots/hits-*.txt
#   cargo build --release -p kabl-ui
#   Xvfb :97 -screen 0 1600x1000x24 &
#   DISPLAY=:97 docs/modulation-slice/xdotool-walkthrough.sh 1440x900
set -euo pipefail
SIZE=${1:-1440x900}
OUT=target/slice-shots/$SIZE
HITS=target/slice-shots/hits-$SIZE.txt
SAVE=$PWD/target/slice-shots/saved-$SIZE
rm -rf "$OUT" "$SAVE"
mkdir -p "$OUT"

at() { awk -v k="$1" '$1 == k { print $2, $3 }' "$HITS"; }
shot() { sleep 0.4; import -window root -crop "${SIZE}+0+0" "$OUT/$1.png"; echo "shot $1"; }
move() { xdotool mousemove $1 $2; sleep 0.15; }
click() { read -r x y <<<"$(at "$1")"; move "$x" "$y"; xdotool click 1; sleep 0.3; }
# drag KEY to X Y (absolute), optional screenshot name taken while still holding
drag_to() {
  read -r x y <<<"$(at "$1")"
  move "$x" "$y"; xdotool mousedown 1; sleep 0.15
  for i in 1 2 3 4 5 6; do
    move $((x + ($2 - x) * i / 6)) $((y + ($3 - y) * i / 6))
  done
  if [ -n "${4:-}" ]; then shot "$4"; fi
  xdotool mouseup 1; sleep 0.3
}
# Xvfb has no window manager, so give the window keyboard focus explicitly.
focus() { xdotool windowfocus --sync "$(xdotool search --name '^kabl$' | head -1)"; }
# ring drag straight up by DY pixels
ring_up() { read -r x y <<<"$(at "$1")"; drag_to "$1" "$x" $((y - $2)) "${3:-}"; }

./target/release/kabl-ui --patch patches/reference --size "$SIZE" >"$OUT/ui.log" 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null || true' EXIT
sleep 3
focus
shot 01-reference

# 1. LFO -> Decay by dragging the output jack onto the knob.
read -r kx ky <<<"$(at knob:4.decay_ms)"
drag_to out:8.out "$kx" "$ky" 02-dragging-lfo-to-decay
shot 03-decay-route-added

# 2. Two sources on Attack: nothing selected until a source is picked.
click knob:4.attack_ms
shot 04-attack-two-sources-none-selected
click row:9
ring_up ring:4.attack_ms 30 05-ring-drag-velocity
shot 06-velocity-amount-edited

# 3. Invert and bypass the selected route.
click invert:9
shot 07-inverted
click bypass:9
shot 08-bypassed

# 4. Cable views.
click view:Hidden
shot 09-hidden
click view:Focus
shot 10-focus
click view:All

# 5. Four sources on Cutoff: pick the envelope route and edit it.
click knob:3.cutoff_hz
click row:12
ring_up ring:3.cutoff_hz 15 11-four-sources-ring-drag
shot 12-four-sources-edited

# 6. Envelope timing: key-trigger, then undo and redo with the keyboard.
click sel:4.timing.1
shot 13-key-trigger
move 900 700
xdotool key ctrl+z; sleep 0.3
shot 14-undo-timing
xdotool key ctrl+shift+z; sleep 0.3
shot 15-redo-timing

# 7. Save to a scratch dir, then reload in a fresh process.
click patch-path
xdotool key ctrl+a; xdotool type --delay 5 "$SAVE"; xdotool key Return; sleep 0.2
click save
shot 16-saved
kill $PID; wait $PID 2>/dev/null || true
./target/release/kabl-ui --patch "$SAVE" --size "$SIZE" >"$OUT/ui-reload.log" 2>&1 &
PID=$!
sleep 3
focus
click knob:4.attack_ms
shot 17-reloaded-attack
echo "saved patch: $SAVE"
