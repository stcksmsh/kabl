#!/bin/sh
# Capture module, control and Explain help in the real editor for each new module.
#   docs/sound-engines/scripts/capture_help.sh OUTDIR [DISPLAY_NUMBER]
# Run capture_faces.sh first (it writes OUTDIR/patches). Run from the repo root.
out=${1:?outdir}
n=${2:-97}
Xvfb :$n -screen 0 1600x1000x24 >/dev/null 2>&1 &
xvfb=$!
sleep 2
metacity --display=:$n --sm-disable >/dev/null 2>&1 &
wm=$!
sleep 2
python3 docs/sound-engines/scripts/face_patches.py "$out/patches"
# kind:param
for kp in osc.fm:ratio osc.fm6:feedback osc.fm6:algorithm osc.fm6:attack1 osc.wt:table quantizer:scale \
          sample.hold:mode slew:rise_ms attenuverter:amount logic:none comparator:threshold \
          crossfade:mix pan:pan random:length; do
    k=${kp%%:*}
    p=${kp##*:}
    sed "s/PARAM/$p/g" docs/sound-engines/scripts/help.txt > "$out/help-$k-$p.txt"
    DISPLAY=:$n KABL_CAPTURE_WINDOW=1 python3 docs/rack-migration/drive.py "$out/help-$k-$p.txt" 1280x800 "$out/help/$k-$p" "$out/patches/$k"
done
kill $wm $xvfb
