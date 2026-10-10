#!/usr/bin/env bash
# Regenerates the hicolor PNGs from hicolor/scalable/apps/kabl.svg (needs inkscape).
# The PNGs are committed so the package build needs no SVG tool.
set -euo pipefail
cd "$(dirname "$0")/hicolor"
for s in 16 24 32 48 64 128 256 512; do
  mkdir -p "${s}x${s}/apps"
  inkscape scalable/apps/kabl.svg -o "${s}x${s}/apps/kabl.png" -w "$s" -h "$s"
done
