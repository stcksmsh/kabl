#!/usr/bin/env python3
"""Build target | egui side-by-side sheets for G1b.

Usage (from the repo root, after `kabl_mock` wrote target/mock/{graphite,paper}.png):
    python3 docs/design/redesign/g1b/side_by_side.py

Left: mockups/r1-faces.html (Satin look, the layout reference) at 100/75/58 in headless Chrome.
Right: the egui render of the same faces (line_solid look). Output: docs/design/redesign/g1b/
side-by-side-{graphite,paper}.png.
"""
import pathlib
import subprocess

from PIL import Image, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parents[4]
MOCK = ROOT / "target" / "mock"
OUT = ROOT / "docs" / "design" / "redesign" / "g1b"
SRC = (ROOT / "docs/design/redesign/mockups/r1-faces.html").read_text()
OLD = "const S={look:prefersDark()?'dark':'light',zoom:1,chk:true,tab:'faces'};"
assert OLD in SRC
ZOOMS = [1, 0.75, 0.58]
FACE_H = 340


def shot(look, z):
    (MOCK / "ref").mkdir(parents=True, exist_ok=True)
    html = MOCK / "ref" / f"r1-{look}-{z}.html"
    html.write_text(SRC.replace(OLD, f"const S={{look:'{look}',zoom:{z},chk:false,tab:'faces'}};"))
    png = MOCK / "ref" / f"{look}-{z}.png"
    subprocess.run(
        ["google-chrome", "--headless=new", "--no-sandbox", "--disable-gpu", "--hide-scrollbars",
         "--window-size=1300,1900", f"--screenshot={png}", f"file://{html}"],
        check=True, capture_output=True)
    return Image.open(png).convert("RGB")


def html_crop(img, z):
    # rack frame starts at y 222; four rows of (face * z + rails 34) + padding
    h = int(4 * (FACE_H * z + 34) + 24)
    return img.crop((24, 222, 24 + int(948 * z + 36), 222 + h))


def egui_sections(img):
    out, y = [], 16
    for z in ZOOMS:
        h = 30 + 4 * (FACE_H * z + 14) + 10
        out.append(img.crop((0, int(y), 1048, int(y + h))))
        y += h
    return out


for look, theme in (("dark", "graphite"), ("light", "paper")):
    mine = egui_sections(Image.open(MOCK / f"{theme}.png").convert("RGB"))
    refs = [html_crop(shot(look, z), z) for z in ZOOMS]
    bg = mine[0].getpixel((2, 2))
    rows = []
    for z, r, m in zip(ZOOMS, refs, mine):
        h = max(r.height, m.height)
        row = Image.new("RGB", (r.width + m.width + 30, h + 28), bg)
        row.paste(r, (0, 28))
        row.paste(m, (r.width + 30, 28))
        d = ImageDraw.Draw(row)
        d.text((8, 8), f"TARGET (r1-faces.html, Satin look) {int(z * 100)} %", fill=(200, 200, 200))
        d.text((r.width + 38, 8), f"EGUI ({theme}, line_solid) {int(z * 100)} %", fill=(200, 200, 200))
        rows.append(row)
    sheet = Image.new("RGB", (rows[0].width, sum(r.height for r in rows) + 10 * len(rows)), bg)
    y = 0
    for r in rows:
        sheet.paste(r, (0, y))
        y += r.height + 10
    sheet.save(OUT / f"side-by-side-{theme}.png")
    print("wrote", OUT / f"side-by-side-{theme}.png", sheet.size)
