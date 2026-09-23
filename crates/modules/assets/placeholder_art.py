"""Regenerates the osc.va demo skin art: osc_va_light.png and osc_va_dark.png.

PLACEHOLDER ART. Procedural (gradient sky, moon, hills), drawn from the revision-2 prototype's
approved placeholder ("Night Ensemble"), because no image generator was available. Art only:
no controls; the only text is the placeholder notice. 2x the 7u x 340 panel (210 x 340 points).

    python3 crates/modules/assets/placeholder_art.py
"""
import math
import os
import random

from PIL import Image, ImageDraw, ImageFont

W, H = 420, 680
HERE = os.path.dirname(os.path.abspath(__file__))


def hexrgb(s):
    return tuple(int(s[i:i + 2], 16) for i in (1, 3, 5))


def lerp(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def render(dark):
    sky = [hexrgb(c) for c in (("#0d1030", "#2a1f58", "#51307a") if dark
                               else ("#f6c9a6", "#d99bb8", "#8c7fc4"))]
    img = Image.new("RGB", (W, H))
    d = ImageDraw.Draw(img)
    for y in range(H):
        f = y / H
        c = lerp(sky[0], sky[1], f * 2) if f < 0.5 else lerp(sky[1], sky[2], (f - 0.5) * 2)
        d.line([(0, y), (W, y)], fill=c)
    rnd = random.Random(7)
    if dark:
        for _ in range(70):
            x, y = rnd.uniform(0, W), rnd.uniform(0, H * 0.6)
            r = rnd.choice((1, 1, 1.5, 2))
            d.ellipse([x - r, y - r, x + r, y + r], fill=(255, 244, 220))
    mx, my, mr = W * 0.74, H * 0.2, W * 0.09
    d.ellipse([mx - mr, my - mr, mx + mr, my + mr], fill=hexrgb("#f3e7c1" if dark else "#fff6e2"))
    for layer, (base, amp, col) in enumerate(((0.70, 0.05, "#2a1f58" if dark else "#8c6fae"),
                                              (0.78, 0.05, "#1b1540" if dark else "#6d5c9e"))):
        pts = [(x, H * (base + amp * math.sin(x / W * (7 + 2 * layer) + layer))) for x in range(0, W + 1, 6)]
        d.polygon(pts + [(W, H), (0, H)], fill=hexrgb(col))
    font = None
    for path in ("/usr/share/fonts/truetype/ibm-plex/IBMPlexMono-Regular.ttf",
                 "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"):
        if os.path.exists(path):
            font = ImageFont.truetype(path, 15)
            break
    d.text((W / 2, H - 18), "PLACEHOLDER ART", fill=(255, 255, 255), font=font, anchor="mm")
    img.save(os.path.join(HERE, f"osc_va_{'dark' if dark else 'light'}.png"), optimize=True)


for dark in (False, True):
    render(dark)
