#!/bin/sh
# Capture each new module's face, light and dark, at 100% zoom, in the real release editor.
#   docs/effects-filters/scripts/capture_faces.sh OUTDIR [DISPLAY_NUMBER]
# Needs metacity, target/release/kabl-ui, Xvfb, xdotool. Run from the repo root.
out=${1:?outdir}
n=${2:-97}
Xvfb :$n -screen 0 1600x1000x24 >/dev/null 2>&1 &
xvfb=$!
sleep 2
metacity --display=:$n --sm-disable >/dev/null 2>&1 &
wm=$!
sleep 2
python3 docs/effects-filters/scripts/face_patches.py "$out/patches"
for d in "$out"/patches/*/; do
    k=$(basename "$d")
    DISPLAY=:$n python3 docs/rack-migration/drive.py docs/sound-engines/scripts/face.txt 1280x800 "$out/$k" "$d"
done
kill $wm $xvfb
