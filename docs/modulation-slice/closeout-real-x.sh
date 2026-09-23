#!/usr/bin/env bash
# Replays the closeout scenario (single-source dot, Shift fine drags, Escape cancels, Hidden
# view, removal, undo/redo) on the real release kabl-ui with xdotool, saves the patch, and
# compares it with the state the headless run of the same steps saved.
#   cargo test -p kabl-ui --test interaction -- --ignored     # writes target/slice-closeout/
#   cargo build --release -p kabl-ui; Xvfb :97 -screen 0 1600x1000x24 &
#   DISPLAY=:97 docs/modulation-slice/closeout-real-x.sh 1440x900
set -euo pipefail
SIZE=${1:-1440x900}
DIR=$PWD/target/slice-closeout/$SIZE
rm -rf "$DIR/real" "$DIR"/*.png
move() { xdotool mousemove "$1" "$2"; sleep 0.03; }
glide() {
  eval "$(xdotool getmouselocation --shell)"
  for i in $(seq 1 10); do move $((X + ($1 - X) * i / 10)) $((Y + ($2 - Y) * i / 10)); done
  sleep 0.2
}
press_to() { glide "$1" "$2"; xdotool mousedown 1; sleep 0.1
  for i in $(seq 1 12); do move $(($1 + ($3 - $1) * i / 12)) $(($2 + ($4 - $2) * i / 12)); done; sleep 0.2; }

./target/release/kabl-ui --patch patches/reference --size "$SIZE" >/dev/null 2>&1 &
APP=$!; trap 'kill $APP 2>/dev/null || true' EXIT
sleep 3
xdotool windowfocus --sync "$(xdotool search --name '^kabl$' | head -1)"
while read -r cmd a b c d; do
  case $cmd in
    drag) press_to "$a" "$b" "$c" "$d"; xdotool mouseup 1; sleep 0.3 ;;
    hold) press_to "$a" "$b" "$c" "$d" ;;
    move) glide "$a" "$b" ;;
    release) xdotool mouseup 1; sleep 0.3 ;;
    click) glide "$a" "$b"; xdotool click 1; sleep 0.3 ;;
    key) xdotool key "$a"; sleep 0.3 ;;
    shift) xdotool key"$a" shift ;;
    shot) sleep 0.3; import -window root -crop "${SIZE}+0+0" "$DIR/$a.png" ;;
    save) glide 210 32; xdotool click 1; xdotool key ctrl+a; xdotool type --delay 5 "$DIR/real"
          xdotool key Return; glide 376 32; xdotool click 1; sleep 0.5 ;;
  esac
done < "$DIR/script.txt"
python3 - "$DIR" <<'PY'
import json, sys
d = sys.argv[1]
real = json.load(open(f"{d}/real/checkpoint.json"))
exp = json.load(open(f"{d}/expected/checkpoint.json"))
def same(a, b):  # equal, numbers within 1e-5 (the real pointer path sums steps differently)
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(same(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return abs(a - b) < 1e-5
    return a == b
print("MATCH" if same(real, exp) else "DIFFER", d)
PY
