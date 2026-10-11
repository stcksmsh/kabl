#!/bin/sh
# Every effect module face under every built-in theme and mode (needs capture.sh's tools).
#   docs/themes/scripts/capture_faces.sh OUTDIR [DISPLAY_NUMBER]
# Writes OUTDIR/<kind>/<theme-id>-<light|dark>/face.png. Run from the repo root.
out=${1:?outdir}
n=${2:-232}
python3 docs/effects-filters/scripts/face_patches.py "$out/patches"
for d in "$out"/patches/*/; do
    k=$(basename "$d")
    docs/themes/scripts/capture.sh docs/themes/scripts/face.txt 1280x800 "$d" "$out/$k" "$n"
done
