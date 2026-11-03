#!/usr/bin/env python3
"""Side by side: sheet.py OURS_DIR MOCK_DIR OUT_DIR scene:surf[,scene:surf...]

Ours (chrome_capture `<scene>__<dark|light>__WxH.png`) left, the g4 mockup (`<surf>__<graphite|paper>__WxH.png`
from shot_mockup.py, cropped to its app frame and scaled to ours) right."""
import os, sys
from PIL import Image
ours, mock, out, pairs = sys.argv[1:5]
os.makedirs(out, exist_ok=True)
for pair in pairs.split(','):
    scene, surf = pair.split(':')
    for dark, app in (('dark', 'graphite'), ('light', 'paper')):
        for w, h in ((1440, 900), (1180, 680)):
            a = Image.open(f'{ours}/{scene}__{dark}__{w}x{h}.png').convert('RGB')
            m = Image.open(f'{mock}/{surf}__{app}__{w}x{h}.png').convert('RGB')
            # the mockup's app frame: the border box inside the page margin
            px = m.load(); bg = px[2, 2]
            x0 = next(x for x in range(m.width) if px[x, m.height // 2] != bg)
            x1 = next(x for x in range(m.width - 1, 0, -1) if px[x, m.height // 2] != bg)
            y0 = next(y for y in range(m.height) if px[m.width // 2, y] != bg)
            y1 = next(y for y in range(m.height - 1, 0, -1) if px[m.width // 2, y] != bg)
            b = m.crop((x0, y0, x1 + 1, y1 + 1)).resize(a.size, Image.LANCZOS)
            c = Image.new('RGB', (a.width * 2 + 10, a.height), 'white')
            c.paste(a, (0, 0)); c.paste(b, (a.width + 10, 0))
            c.save(f'{out}/{scene}__{dark}__{w}x{h}__ours-left_mockup-right.png')
