#!/usr/bin/env bash
# R3 real-app window-close runs, recorded as one video. Needs Xvfb, an EWMH window manager
# (openbox, undecorated at 0,0: scripts/openbox-rc.xml), wmctrl, xdotool, ffmpeg.
#   Xvfb :97 -screen 0 1600x1000x24 &  DISPLAY=:97 openbox --config-file docs/find-play-save/repair-1/scripts/openbox-rc.xml &
#   cargo build --release -p kabl-ui
#   DISPLAY=:97 docs/find-play-save/repair-1/record-close.sh
set -euo pipefail
OUT=${OUT:-target/d01-r1-close}
S=docs/find-play-save/repair-1/scripts
rm -rf "$OUT"; mkdir -p "$OUT"
export KABL_USER_DIR="$OUT/user"
ffmpeg -loglevel error -y -f x11grab -draw_mouse 1 -framerate 30 -video_size 1440x900 \
  -i "$DISPLAY+0,0" -c:v libx264 -preset veryfast -crf 26 -pix_fmt yuv420p "$OUT/close.mp4" &
REC=$!
trap 'kill -INT $REC 2>/dev/null; wait $REC 2>/dev/null' EXIT
run() { # part, extra env
  local part=$1; shift
  env "$@" PIPEWIRE_NODE=kabl_rec KABL_DRIVE_LOG="$OUT/$part.log" KABL_APP_LOG="$OUT/$part.app.log" \
    python3 docs/rack-migration/drive.py "$S/$part.txt" 1440x900 "$OUT/shots" -
  echo "$part: script finished" >> "$OUT/summary.txt"
}
run close-1-cancel-save
ls "$KABL_USER_DIR/sounds" >> "$OUT/summary.txt"
sha256sum "$KABL_USER_DIR"/sounds/closed-pad/* > "$OUT/closed-pad.before.sha256"
run close-2-failed-save KABL_SAVE_FAIL=replace:dir
sha256sum "$KABL_USER_DIR"/sounds/closed-pad/* > "$OUT/closed-pad.after.sha256"
cmp "$OUT/closed-pad.before.sha256" "$OUT/closed-pad.after.sha256" && echo "closed-pad unchanged by the failed save" >> "$OUT/summary.txt"
run close-3-dialog-open
run close-4-unedited
cat "$OUT/summary.txt"
