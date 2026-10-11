#!/bin/sh
# Opens each wet demo patch in the standalone editor (kabl-ui, private display), lets it play for
# 10 s, saves a screenshot and the app's stderr to OUTDIR/<name>/. Needs scratch/effects-demos
# (make_demos.py), target/release/kabl-ui, Xvfb, metacity. Run from the repo root.
#   docs/effects-filters/scripts/standalone.sh OUTDIR
out=${1:?outdir}
n=143
mkdir -p "$out"
printf 'sleep 10\nshot running\n' > "$out/play.txt"
Xvfb :$n -screen 0 1600x1000x24 >/dev/null 2>&1 &
xvfb=$!
sleep 2
metacity --display=:$n --sm-disable >/dev/null 2>&1 &
wm=$!
sleep 2
for d in scratch/effects-demos/*-wet/; do
    k=$(basename "$d")
    DISPLAY=:$n KABL_APP_LOG="$out/$k.log" python3 docs/rack-migration/drive.py "$out/play.txt" 1280x800 "$out/$k" "$d"
done
kill $wm $xvfb
