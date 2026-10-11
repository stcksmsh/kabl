#!/usr/bin/env python3
"""Contact sheet of the effect faces for one theme and mode.

    python3 docs/themes/scripts/sheet.py FACESDIR THEME-MODE OUT.png
e.g. ... /tmp/faces kabl.lacquer-dark sheet.png. Crops the face row out of each capture."""
import sys
from pathlib import Path
from PIL import Image

root, key, out = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
H = int(sys.argv[4]) if len(sys.argv) > 4 else 364
tiles = []
for d in sorted(p for p in root.iterdir() if (p / key / "face.png").exists()):
    im = Image.open(d / key / "face.png").convert("RGB").crop((0, 228, 640, 228 + H))
    tiles.append(im)
cols = 3
rows = (len(tiles) + cols - 1) // cols
sheet = Image.new("RGB", (cols * 640, rows * H))
for i, t in enumerate(tiles):
    sheet.paste(t, ((i % cols) * 640, (i // cols) * H))
sheet.save(out)
