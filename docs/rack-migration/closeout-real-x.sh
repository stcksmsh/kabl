#!/usr/bin/env bash
# Replays the modulation closeout scenario (single-source dot, Shift fine drags, Escape cancels,
# Hidden view, removal, undo/redo) on the real release kabl-ui rack with xdotool, saves the
# patch, and compares it with what the headless run of the same steps saved.
#   cargo test -p kabl-ui --test interaction closeout -- --ignored   # writes target/slice-closeout/
#   cargo build --release -p kabl-ui; Xvfb :97 -screen 0 1600x1000x24 &
#   DISPLAY=:97 docs/rack-migration/closeout-real-x.sh
set -euo pipefail
for SIZE in 1440x900 1280x800; do
  DIR=$PWD/target/slice-closeout/$SIZE
  rm -rf "$DIR/real" "$DIR/shots"
  python3 docs/rack-migration/drive.py "$DIR/script.txt" "$SIZE" "$DIR/shots"
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
done
