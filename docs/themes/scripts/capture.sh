#!/bin/sh
# Capture a drive.py script under every built-in theme and mode, in the real release editor.
#   docs/themes/scripts/capture.sh SCRIPT SIZE PATCH OUTDIR [DISPLAY_NUMBER]
# Writes OUTDIR/<theme-id>-<light|dark>/<shot>.png. The theme comes from KABL_THEME, so no menu
# clicking is involved; pick any built-in (or user) theme id: THEMES="kabl.satin kabl.lacquer".
# KABL_CONFIG_DIR points at an empty folder so the run never reads or writes the user's settings.
# Needs target/release/kabl-ui, Xvfb, xdotool. Run from the repo root.
script=${1:?script}
size=${2:?size}
patch=${3:?patch}
out=${4:?outdir}
n=${5:-231}
themes=${THEMES:-"kabl.satin kabl.lacquer"}
cfg=$(mktemp -d)
Xvfb :$n -screen 0 1600x1000x24 >/dev/null 2>&1 &
xvfb=$!
sleep 2
for t in $themes; do
    for mode in light dark; do
        DISPLAY=:$n KABL_CONFIG_DIR=$cfg KABL_THEME=$t:$mode \
            python3 docs/rack-migration/drive.py "$script" "$size" "$out/$t-$mode" "$patch"
    done
done
kill $xvfb
rm -rf "$cfg"
