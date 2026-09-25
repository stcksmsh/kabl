#!/usr/bin/env bash
# All D03 screenshots at 1440x900 and 1280x800 (real app, scripted real X input).
#   DISPLAY=:97 docs/signal-inspection/scripts/shots.sh
set -euo pipefail
IMG=docs/signal-inspection/img
mkdir -p "$IMG"
for size in 1440x900 1280x800; do
  OUT=target/signal-inspection-shots/$size
  rm -rf "$OUT"; mkdir -p "$OUT"
  KABL_USER_DIR="$OUT/user" KABL_MIDI_PIPE=${KABL_MIDI_PIPE:-/tmp/kabl-midi} PIPEWIRE_NODE=kabl_rec \
    KABL_PLAYER=1 KABL_APP_LOG="$OUT/app.log" KABL_ARGS="--midi kabl-pipe --perform --rate 48000 --frames 256" \
    python3 docs/rack-migration/drive.py docs/signal-inspection/scripts/shots.txt "$size" "$OUT/a" patches/init-keyboard
  KABL_USER_DIR="$OUT/user2" PIPEWIRE_NODE=kabl_rec KABL_ARGS="--perform --rate 48000 --frames 256" \
    python3 docs/rack-migration/drive.py docs/signal-inspection/scripts/lead-glide.txt "$size" "$OUT/b" patches/palette/lead
  for f in "$OUT"/a/*.png "$OUT"/b/*.png; do cp "$f" "$IMG/$size-$(basename "$f")"; done
done
