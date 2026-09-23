#!/usr/bin/env bash
# Screen + sound recordings of the real kabl-ui release binary: MIDI notes played into it through
# ALSA's Midi Through port (aplaymidi), its real audio output sent to a silent PipeWire null sink
# and recorded from that sink's monitor, knobs driven by xdotool, all muxed by one ffmpeg.
# Nothing plays on the speakers.
#   pactl load-module module-null-sink sink_name=kabl_rec
#   cargo test -p kabl-ui --test interaction -- --ignored && cargo build --release -p kabl-ui
#   Xvfb :97 -screen 0 1600x1000x24 &
#   DISPLAY=:97 docs/modulation-slice/record-av.sh     # MIDI files: see target/slice-av/*.mid
set -euo pipefail
SIZE=1440x900
OUT=target/slice-av
HITS=target/slice-shots/hits-$SIZE.txt

at() { awk -v k="$1" '$1 == k { print $2, $3 }' "$HITS"; }
pause() { sleep "${1:-0.8}"; }
move() { xdotool mousemove "$1" "$2"; sleep 0.03; }
glide() {
  eval "$(xdotool getmouselocation --shell)"
  for i in $(seq 1 15); do move $((X + ($1 - X) * i / 15)) $((Y + ($2 - Y) * i / 15)); done
}
click() { read -r x y <<<"$(at "$1")"; glide "$x" "$y"; pause 0.3; xdotool click 1; pause; }
# slow drag: KEY DX DY SECONDS
sdrag() {
  read -r x y <<<"$(at "$1")"; glide "$x" "$y"; pause 0.3
  xdotool mousedown 1; local n=$(( ${4%.*} * 25 + 10 ))
  for i in $(seq 1 $n); do xdotool mousemove $((x + $2 * i / n)) $((y + $3 * i / n)); sleep 0.04; done
  pause 0.4; xdotool mouseup 1; pause 0.5
}
focus() { xdotool windowfocus --sync "$(xdotool search --name '^kabl$' | head -1)"; }

start() {
  PIPEWIRE_NODE=kabl_rec ./target/release/kabl-ui --patch patches/reference --size $SIZE >/dev/null 2>&1 &
  APP=$!; sleep 3; focus; xdotool mousemove 900 700
  ffmpeg -loglevel error -y -thread_queue_size 1024 -f x11grab -draw_mouse 1 -framerate 30 \
    -video_size $SIZE -i "$DISPLAY+0,0" -thread_queue_size 1024 -f pulse -i kabl_rec.monitor \
    -vf scale=1280:-2 -c:v libx264 -preset veryfast -crf 26 -pix_fmt yuv420p \
    -c:a aac -b:a 160k -shortest "$OUT/$1.mp4" &
  REC=$!; pause 1
}
stop() { pause 2; kill -INT $REC; wait $REC || true; kill $APP; wait $APP 2>/dev/null || true; echo "clip $1"; }

# A: play chords while editing: Cutoff base swept, envelope->Cutoff depth on its lane, a new
# LFO -> Decay route, Hidden cables. Listen for clicks, bumps or hanging notes.
start A-play-and-edit
aplaymidi -p 14:0 "$OUT/chords.mid" & MIDI=$!
pause 2
sdrag knob:3.cutoff_hz 0 60 3      # base down
sdrag knob:3.cutoff_hz 0 -110 4    # base up
click knob:3.cutoff_hz
sdrag lane:12 0 -45 3              # envelope -> Cutoff depth up
read -r kx ky <<<"$(at knob:4.decay_ms)"
read -r ox oy <<<"$(at out:8.out)"
sdrag out:8.out $((kx - ox)) $((ky - oy)) 2   # LFO #8 onto Decay
click view:Hidden; pause 2; click view:All
wait $MIDI || true
stop A-play-and-edit

# B: envelope timing by ear. Attack base 300 ms, fast LFO #8 dropped on Attack, then single
# notes: first half CONT (attack follows the LFO), second half KEY (captured at note-on).
start B-cont-vs-key
click knob:4.attack_ms
click base-entry:4.attack_ms; xdotool key ctrl+a; xdotool type --delay 20 "300 ms"; xdotool key Return; pause
read -r kx ky <<<"$(at knob:4.attack_ms)"
read -r ox oy <<<"$(at out:8.out)"
sdrag out:8.out $((kx - ox)) $((ky - oy)) 1
glide 700 700
aplaymidi -p 14:0 "$OUT/notes.mid" & MIDI=$!
pause 8.5
click sel:4.timing.1               # KEY
wait $MIDI || true
stop B-cont-vs-key
