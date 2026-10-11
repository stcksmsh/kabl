#!/bin/sh
# Every built-in module kind alone in a patch, at 100%, for the themes in THEMES (default both)
# and light mode only unless MODES="light dark". Used to look for a title colliding with face content.
#   docs/themes/scripts/capture_all_faces.sh OUTDIR [DISPLAY_NUMBER]
out=${1:?outdir}
n=${2:-232}
kinds="arp attenuverter bitcrusher chorus clock clock.div comparator compressor crossfade cues delay drive env.adsr eq filter.comb filter.formant filter.ladder filter.svf flanger gain lfo limiter logic macro midi.in mixer noise osc.fm osc.fm6 osc.va osc.wt out pan phaser quantizer random reverb ringmod sample.hold seq slew vca"
KINDS="$kinds" python3 docs/effects-filters/scripts/face_patches.py "$out/patches"
for k in $kinds; do
    MODES=${MODES:-light} docs/themes/scripts/capture.sh docs/themes/scripts/face.txt 1280x800 "$out/patches/$k" "$out/$k" "$n"
done
