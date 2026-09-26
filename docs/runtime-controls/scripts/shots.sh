#!/usr/bin/env bash
# D04 + D03-R2 screenshots at 1440x900 and 1280x800, A-light and A-dark (real app, scripted
# real X input, fifo MIDI stand-in).   DISPLAY=:97 docs/runtime-controls/scripts/shots.sh
set -euo pipefail
IMG=docs/runtime-controls/img
mkdir -p "$IMG"
export KABL_MIDI_PIPE=${KABL_MIDI_PIPE:-/tmp/kabl-midi}
for size in 1440x900 1280x800; do
  OUT=target/runtime-controls-shots/$size
  rm -rf "$OUT"; mkdir -p "$OUT"
  KABL_USER_DIR="$OUT/user" PIPEWIRE_NODE=kabl_rec KABL_PLAYER=1 KABL_APP_LOG="$OUT/app-a.log" \
    KABL_ARGS="--midi kabl-pipe --perform --rate 48000 --frames 256" \
    python3 docs/rack-migration/drive.py docs/runtime-controls/scripts/shots-comp.txt "$size" "$OUT/a" patches/composition
  KABL_USER_DIR="$OUT/user2" PIPEWIRE_NODE=kabl_rec KABL_PLAYER=1 KABL_APP_LOG="$OUT/app-b.log" \
    KABL_ARGS="--midi kabl-pipe --perform --rate 48000 --frames 256" \
    python3 docs/rack-migration/drive.py docs/runtime-controls/scripts/shots-r2.txt "$size" "$OUT/b" patches/init-keyboard
  for f in "$OUT"/a/*.png "$OUT"/b/*.png; do cp "$f" "$IMG/$size-$(basename "$f")"; done
done
