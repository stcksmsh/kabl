#!/usr/bin/env python3
"""Build target | egui side-by-side sheets for G1b.

Usage (from the repo root, after `kabl_mock` wrote target/mock/{graphite,paper}.png):
    python3 docs/design/redesign/g1b/side_by_side.py

Left: mockups/r1-faces.html at 100/75/58 in headless Chrome, with its look patched from Satin to
line_solid (PLAN section 6 tokens, graphite/paper, amber, Condensed titles) so both columns use the
same look. Geometry is the unchanged r1-faces geometry; only colours, panel fill and title type
are replaced. Right: the egui render of the same faces. Output: docs/design/redesign/g1b/
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

# line_solid override, injected before the text-measure section of r1-faces.html.
OVERRIDE = r"""
const TK={dark:{surf:'#1A1B1E',raised:'#232529',inset:'#0F1012',line2:'#393C42',text:'#ECEAE6',text2:'#A29F99',edge:'rgba(236,234,230,.26)',skirt:'#141518',pointer:'#F2F0EB',nut:'#2B2D32',nutEdge:'#45484E',hole:'#050506',accent:'#F2A33A',onAcc:'#1C1206',audio:'#F59E3B',cv:'#38C6B2',gate:'#A68BFA',pitch:'#5BA8F7',track:'#0F1012',trackInk:'#D8D5CF',rack:'#0C0D0E',rail:'#1B1C1F',railHole:'#060607',soft:'rgba(242,163,58,.16)'},
 light:{surf:'#FBFAF7',raised:'#FFFFFF',inset:'#E4E1DA',line2:'#C8C4BB',text:'#1A1917',text2:'#5F5C56',edge:'rgba(26,25,23,.30)',skirt:'#141518',pointer:'#F2F0EB',nut:'#FFFFFF',nutEdge:'#C8C4BB',hole:'#24232A',accent:'#A97229',onAcc:'#1C1206',audio:'#D9781C',cv:'#12958A',gate:'#7651D6',pitch:'#2F7FD6',track:'#D6D2C8',trackInk:'#2A2824',rack:'#DCD9D2',rail:'#C9C5BC',railHole:'#A39E94',soft:'rgba(169,114,41,.22)'}};
for(const k of ['dark','light']){const T=TK[k];Object.assign(LOOKS[k],{rack:T.rack,rail:T.rail,railHole:T.railHole,skirt:T.skirt,pointer:T.pointer,nut:T.nut,nutEdge:T.nutEdge,hole:T.hole,segOn:T.accent,segOnInk:T.onAcc,trace:T.accent,traceSoft:T.soft,audio:T.audio,cv:T.cv,ucv:T.cv,gate:T.gate,pitch:T.pitch})}
const HUES={osc:'#E07A5F',filter:'#7FB59A',env:'#9D8DF1',timing:'#5FA8D3',amp:'#E3BE5C',fx:'#B9B2A6'};
function titleHue(f,dark){const T=dark?TK.dark:TK.light;let t=dark?.15:.35;for(;;){const c=mix(HUES[f],dark?'#FFFFFF':'#000000',t);if(contrast(c,T.surf)>=4.5||t>=.95)return c;t+=.05}}
famCol=function(L,f,dark){const T=dark?TK.dark:TK.light;return{base:T.surf,ink:T.text,ink2:T.text2,well:T.inset,wellInk:dark?'#C9C6C0':'#3A3833',tray:mix(T.surf,'#000000',.14),plate:T.raised,track:T.track,trackInk:T.trackInk,chip:mix(T.surf,'#000000',.1),card:T.surf,rule:T.line2,edge:T.edge,hi:'rgba(0,0,0,0)'}};
"""
PATCHES = [
    ("/* ---------- text measure ---------- */", OVERRIDE + "/* ---------- text measure ---------- */"),
    ('fill="${C.base}" stroke="${C.rule}"', 'fill="${C.base}" stroke="${C.edge}"'),
    ("fill=\"url(#topl-${dark?'d':'l'})\"", 'fill="none"'),
    ('stroke="${C.ink}" stroke-opacity=".85"', 'stroke="${L.trace}" stroke-opacity="1"'),
    ("text(title,M,Z.titleTop,tsize,{w:600,role:'title'})",
     "text(title,M,Z.titleTop,tsize,{w:600,role:'title',fill:titleHue(spec.fam,dark)})"),
    ("font-family='${mono?MONO:SANS}' font-size=\"${size.toFixed(2)}\"",
     "font-family='${role==='title'?'\"IBM Plex Sans Condensed\"':(mono?MONO:SANS)}' "
     "letter-spacing=\"${role==='title'?(size*.07).toFixed(2):0}\" font-size=\"${size.toFixed(2)}\""),
    ("${esc(str)}</text>", "${role==='title'?esc(str).toUpperCase():esc(str)}</text>"),
]


def patched(src):
    for old, new in PATCHES:
        assert old in src, old
        src = src.replace(old, new, 1)
    return src


def shot(look, z):
    (MOCK / "ref").mkdir(parents=True, exist_ok=True)
    html = MOCK / "ref" / f"r1-{look}-{z}.html"
    html.write_text(patched(SRC).replace(
        OLD, f"const S={{look:'{look}',zoom:{z},chk:false,tab:'faces'}};"))
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
        d.text((8, 8), f"TARGET (r1-faces geometry, line_solid) {int(z * 100)} %", fill=(200, 200, 200))
        d.text((r.width + 38, 8), f"EGUI ({theme}, line_solid) {int(z * 100)} %", fill=(200, 200, 200))
        rows.append(row)
    sheet = Image.new("RGB", (rows[0].width, sum(r.height for r in rows) + 10 * len(rows)), bg)
    y = 0
    for r in rows:
        sheet.paste(r, (0, y))
        y += r.height + 10
    sheet.save(OUT / f"side-by-side-{theme}.png")
    print("wrote", OUT / f"side-by-side-{theme}.png", sheet.size)
