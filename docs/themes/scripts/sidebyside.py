#!/usr/bin/env python3
"""Join the A and B captures of each scene into one side-by-side WebP.

    python3 docs/themes/scripts/sidebyside.py CAPTUREDIR OUTDIR
CAPTUREDIR holds <scene>/<theme-id>-<light|dark>/<shot>.png as written by capture.sh and
matrix.sh. Writes OUTDIR/<scene>-<light|dark>.webp, Satin Studio on the left."""
import sys
from pathlib import Path
from PIL import Image

root, out = Path(sys.argv[1]), Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
for scene in sorted(p for p in root.iterdir() if p.is_dir() and p.name != "faces"):
    for mode in ("light", "dark"):
        pair = []
        for theme in ("kabl.satin", "kabl.lacquer"):
            shots = list((scene / f"{theme}-{mode}").glob("shot.png"))
            if shots:
                pair.append(Image.open(shots[0]).convert("RGB"))
        if len(pair) == 2:
            sheet = Image.new("RGB", (pair[0].width + pair[1].width + 8, pair[0].height), "white")
            sheet.paste(pair[0], (0, 0))
            sheet.paste(pair[1], (pair[0].width + 8, 0))
            sheet.save(out / f"{scene.name}-{mode}.webp", quality=88)
