#!/bin/sh
# The side-by-side matrix: four scenes under every built-in theme, light and dark.
#   docs/themes/scripts/matrix.sh OUTDIR [SIZE] [DISPLAY_NUMBER]
# Writes OUTDIR/<scene>/<theme-id>-<light|dark>/shot.png. Run from the repo root.
out=${1:?outdir}
size=${2:-1440x900}
n=${3:-232}
d=docs/themes/scripts
$d/capture.sh $d/scene-simple.txt "$size" patches/reference "$out/simple" "$n"
$d/capture.sh $d/scene-dense.txt "$size" patches/composition "$out/dense" "$n"
$d/capture.sh $d/scene-closeup.txt "$size" patches/sound-engines/demos/iron-bell "$out/closeup" "$n"
$d/capture.sh $d/scene-perform.txt "$size" patches/composition "$out/perform" "$n"
