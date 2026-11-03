#!/usr/bin/env bash
# Regenerates the hicolor PNGs (needs inkscape). 32 px and up come from
# hicolor/scalable/apps/kabl.svg; 16 and 24 px from kabl-icon-small.svg, whose plugs are
# simplified because plug detail turns to mush below 32 px.
# The PNGs are committed so the package build needs no SVG tool.
set -euo pipefail
cd "$(dirname "$0")/hicolor"
for s in 16 24 32 48 64 128 256 512; do
  src=scalable/apps/kabl.svg
  [ "$s" -lt 32 ] && src=../kabl-icon-small.svg
  mkdir -p "${s}x${s}/apps"
  inkscape "$src" -o "${s}x${s}/apps/kabl.png" -w "$s" -h "$s"
done
