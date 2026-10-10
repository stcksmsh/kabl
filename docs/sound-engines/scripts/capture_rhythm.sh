#!/bin/sh
# Capture the faces the rhythm batch adds or changes (arp, clock, seq and seq's more-controls
# drawer), light and dark, at 100% zoom, in the real release editor.
#   docs/sound-engines/scripts/capture_rhythm.sh OUTDIR [DISPLAY_NUMBER]
# Needs a window manager (metacity), target/release/kabl-ui, Xvfb, xdotool. Run from the repo root.
out=${1:?outdir}
n=${2:-97}
Xvfb :$n -screen 0 1600x1000x24 >/dev/null 2>&1 &
xvfb=$!
sleep 2
metacity --display=:$n --sm-disable >/dev/null 2>&1 &
wm=$!
sleep 2
python3 docs/sound-engines/scripts/face_patches.py "$out/patches"
for k in arp clock seq; do
    DISPLAY=:$n KABL_CAPTURE_WINDOW=1 python3 docs/rack-migration/drive.py docs/sound-engines/scripts/face.txt 1280x800 "$out/$k" "$out/patches/$k"
done
DISPLAY=:$n KABL_CAPTURE_WINDOW=1 python3 docs/rack-migration/drive.py docs/sound-engines/scripts/face-drawer.txt 1500x860 "$out/seq-drawer" "$out/patches/seq"
kill $wm $xvfb
