#!/usr/bin/env python3
"""Pixel diff of the app's kit parts sheet against the G1b mock's parts sheet.

    python3 docs/design/redesign/g1b/parts_diff.py graphite|paper [out_diff.png]

Needs target/mock/kit-<theme>.png (example kit_parts) and the committed sheet-<theme>.png.
Prints the share of pixels whose largest channel difference exceeds 32 (visible difference,
not anti-aliasing noise). The mock sheet's parts panel starts at y 3503 (see kit_parts.rs).
"""
import pathlib
import sys

from PIL import Image, ImageChops

ROOT = pathlib.Path(__file__).resolve().parents[4]
theme = sys.argv[1] if len(sys.argv) > 1 else "graphite"
app = Image.open(ROOT / "target" / "mock" / f"kit-{theme}.png").convert("RGB")
mock = Image.open(ROOT / "docs/design/redesign/g1b" / f"sheet-{theme}.png").convert("RGB")
mock = mock.crop((0, 3503, 1000, 3503 + 740))
app = app.crop((0, 0, 1000, 740))
diff = ImageChops.difference(app, mock)
px = diff.load()
bad = sum(1 for y in range(diff.height) for x in range(diff.width) if max(px[x, y]) > 32)
pct = 100.0 * bad / (diff.width * diff.height)
print(f"{theme}: {pct:.2f} % of pixels differ by more than 32/255")
if len(sys.argv) > 2:
    Image.merge("RGB", [diff.getchannel(0).point(lambda v: 255 if v > 32 else 0)] * 3).save(sys.argv[2])
