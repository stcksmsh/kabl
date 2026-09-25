#!/usr/bin/env bash
# Screenshots of the dialogs D01-R1 changed, at 1440×900 and 1280×800 in A-light and A-dark.
# Same setup as record-close.sh (Xvfb + openbox + wmctrl). Output: $OUT/<size>/*.png
set -euo pipefail
OUT=${OUT:-target/d01-r1-dialogs}
S=docs/find-play-save/repair-1/scripts
rm -rf "$OUT"; mkdir -p "$OUT"
for size in 1440x900 1280x800; do
  for theme in light dark; do
    for part in dialogs failed; do
      sed "s/THEME/$theme/g" "$S/$part.txt.in" > "$OUT/$part.$theme.txt"
      env $([ $part = failed ] && echo KABL_SAVE_FAIL=staged) KABL_USER_DIR="$OUT/user-$size-$theme-$part" \
        PIPEWIRE_NODE=kabl_rec python3 docs/rack-migration/drive.py "$OUT/$part.$theme.txt" "$size" "$OUT/$size" -
    done
  done
done
ls "$OUT"/*/*.png
