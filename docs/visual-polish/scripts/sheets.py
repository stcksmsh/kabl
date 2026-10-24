#!/usr/bin/env python3
"""Contact sheets from capture_faces.py and capture_scenes.py output.

    python3 docs/visual-polish/scripts/sheets.py faces FACESDIR OUTDIR     # one sheet per theme-mode and zoom
    python3 docs/visual-polish/scripts/sheets.py scenes SCENESDIR OUTDIR   # one sheet per scene shot and size

Face sheets crop each capture to the face (the pixels that differ from the empty rack), label it with the
module kind, and tile them 8 across. Scene sheets put theme A light, A dark, B light, B dark in a 2x2 grid.
"""
import sys
from pathlib import Path
from PIL import Image, ImageDraw

mode, src, dst = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
dst.mkdir(parents=True, exist_ok=True)
order = ['kabl.satin-light', 'kabl.satin-dark', 'kabl.lacquer-light', 'kabl.lacquer-dark']


def crop_face(im, y0=240, y1=578):
    """Bounding box of the module: pixels in the rack band that differ from the rack colour."""
    bg = im.getpixel((900, 400))
    px = im.load()
    xs, ys = [], []
    for y in range(y0, y1, 2):
        for x in range(0, 1230, 2):
            p = px[x, y]
            if max(abs(p[i] - bg[i]) for i in range(3)) > 24:
                xs.append(x)
                ys.append(y)
    if not xs:
        return im.crop((0, 240, 300, 580))
    return im.crop((max(min(xs) - 10, 0), max(min(ys) - 10, y0 - 8), min(max(xs) + 10, 1280), min(max(ys) + 10, y1 + 8)))


def faces():
    for t in order:
        kinds = sorted(p.name for p in (src / t).iterdir() if p.is_dir())
        for z in ('z100', 'z50'):
            tiles = []
            for k in kinds:
                f = src / t / k / f'{z}.png'
                if f.exists():
                    tiles.append((k, crop_face(Image.open(f).convert('RGB'), *((240, 578) if z == 'z100' else (312, 504)))))
            if not tiles:
                continue
            # shelf packing: tiles keep their own size, rows wrap at W
            W, x, y, rh, pos = 2600, 0, 0, 0, []
            for k, im in tiles:
                tw, th = im.width + 8, im.height + 22
                if x + tw > W:
                    x, y, rh = 0, y + rh, 0
                pos.append((k, im, x, y))
                x, rh = x + tw, max(rh, th)
            sheet = Image.new('RGB', (W, y + rh), (128, 128, 128))
            d = ImageDraw.Draw(sheet)
            for k, im, x, y in pos:
                d.text((x + 4, y + 4), k, fill=(255, 255, 255))
                sheet.paste(im, (x + 4, y + 20))
            sheet.save(dst / f'faces-{t}-{z}.png')


def scenes():
    for size in sorted(p.name for p in src.iterdir()):
        shots = sorted({f.relative_to(src / size / order[0]).as_posix()
                        for f in (src / size / order[0]).rglob('*.png')})
        for s in shots:
            ims = [Image.open(src / size / t / s).convert('RGB') for t in order if (src / size / t / s).exists()]
            if len(ims) < 4:
                continue
            w, h = ims[0].size
            sheet = Image.new('RGB', (2 * w, 2 * h))
            for n, im in enumerate(ims):
                sheet.paste(im, ((n % 2) * w, (n // 2) * h))
            sheet.save(dst / f'{size}-{s.replace("/", "-")}')


faces() if mode == 'faces' else scenes()
