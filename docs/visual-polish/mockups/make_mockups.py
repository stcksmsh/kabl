#!/usr/bin/env python3
"""Before/after boards for the visual polish proposal. The "after" faces are DRAWN (SVG), not rendered
by the app; the "before" faces are real release-editor captures (capture_faces.py output).

    python3 docs/visual-polish/mockups/make_mockups.py FACESDIR OUTDIR

Writes OUTDIR/board-<theme>-<mode>.png (three rows: current, option 1 Seated, option 2 Tinted) and
OUTDIR/closeup-<theme>-<mode>.png (the oscillator and the drum grid at 2x). Needs inkscape and Pillow.
Colours are the built-in theme roles from crates/ui/src/style/builtin.rs.
"""
import base64, io, subprocess, sys
from pathlib import Path
from PIL import Image

src, out = Path(sys.argv[1]), Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
FONT, MONO = "IBM Plex Sans", "IBM Plex Mono"

# ---------------------------------------------------------------- tokens (builtin.rs)
def sect(b, i, i2):
    return dict(base=b, ink=i, ink2=i2)

T = {
    ("kabl.satin", "light"): dict(
        rack="#d6cfbf", rail="#c3bcab", text="#2a2621", hole="#2a2826", disp_bg="#27241f", disp_grid="#3d382f",
        trace="#f1b25a", disp_text="#efe6d2", knob="#2c2a27", knob_hi="#5d5851", knob_ink="#f4efe6",
        accent="#b8501f", on_accent="#fff8ef", raised="#fbf9f3", inset="#dcd5c5", dark=False,
        osc=sect("#d79a82", "#2a1c14", "#3e291d"), filter=sect("#aebf9f", "#1f2a1b", "#3d4c37"),
        mod=sect("#bdb3d0", "#231d30", "#463d58"), amp=sect("#cabc92", "#2c2410", "#3f3620"),
        timing=sect("#a9bec1", "#16262a", "#35494d"), fx=sect("#d9cfbf", "#2a2318", "#4d4535")),
    ("kabl.satin", "dark"): dict(
        rack="#0f0e0d", rail="#3f3b35", text="#efe8da", hole="#050504", disp_bg="#0f0d0b", disp_grid="#2a2620",
        trace="#f4b862", disp_text="#f1e3c6", knob="#201e1b", knob_hi="#6b665d", knob_ink="#f4efe6",
        accent="#f0a04b", on_accent="#26180a", raised="#292520", inset="#110f0d", dark=True,
        osc=sect("#85594a", "#fff4e6", "#f8ece0"), filter=sect("#53664d", "#f4fbee", "#dbe7d3"),
        mod=sect("#62537a", "#f6f1ff", "#e0d8ee"), amp=sect("#6e5f40", "#fff8e4", "#ebe0c4"),
        timing=sect("#3f5a60", "#effbfd", "#d1e4e8"), fx=sect("#5d564c", "#f8f1e6", "#e0d8ca")),
    ("kabl.lacquer", "light"): dict(
        rack="#cfd4df", rail="#b4bbcb", text="#161922", hole="#2c303a", disp_bg="#12141a", disp_grid="#262a35",
        trace="#5ff0d6", disp_text="#d6f3f3", knob="#1c1f27", knob_hi="#5b6274", knob_ink="#f2f4fa",
        accent="#3b4fe0", on_accent="#ffffff", raised="#ffffff", inset="#dce1ea", dark=False,
        osc=sect("#f0a791", "#2a110c", "#4d2a22"), filter=sect("#aad3b8", "#0f2619", "#2c4a3a"),
        mod=sect("#c6baf0", "#1a1233", "#3d3262"), amp=sect("#ebd28e", "#2b2108", "#524416"),
        timing=sect("#a1d0e2", "#0c2530", "#26495a"), fx=sect("#d9d2e4", "#1e1829", "#413a54")),
    ("kabl.lacquer", "dark"): dict(
        rack="#08090c", rail="#2a2d36", text="#eceef4", hole="#030305", disp_bg="#07080b", disp_grid="#1d212a",
        trace="#5ff0d6", disp_text="#cfeff0", knob="#12141a", knob_hi="#4a5060", knob_ink="#f2f4fa",
        accent="#7f8bff", on_accent="#0b0d1f", raised="#1e2129", inset="#090a0d", dark=True,
        osc=sect("#8e4338", "#fff6ef", "#f1ddd2"), filter=sect("#35705a", "#f2fff8", "#d3eadf"),
        mod=sect("#5b4ca3", "#f6f3ff", "#ddd6f4"), amp=sect("#82672b", "#fff9e8", "#fcf3dc"),
        timing=sect("#2b6377", "#effcff", "#cde7ef"), fx=sect("#5d566b", "#f7f4fb", "#ded8e8")),
}


def rgb(h):
    h = h.lstrip("#")
    if len(h) == 3:
        h = "".join(c * 2 for c in h)
    return [int(h[i:i + 2], 16) for i in (0, 2, 4)]


def mix(a, b, t):
    A, B = rgb(a), rgb(b)
    return "#%02x%02x%02x" % tuple(round(A[i] + (B[i] - A[i]) * t) for i in range(3))


# ---------------------------------------------------------------- faces (real sizes and labels)
FACES = [
    dict(kind="osc.va", title="VA Oscillator", w=210, sec="osc", disp="wave", more=4,
         knobs=[("Frequency", "262 Hz", .5)], seg=["SIN", "TRI", "SAW", "SQR"], segsel=2,
         ins=["Pitch", "Sync"], outs=["Out"]),
    dict(kind="filter.svf", title="SVF Filter", w=210, sec="filter", disp="filter", more=1,
         knobs=[("Cutoff", "1.00 kHz", .55), ("Resonance", "0.30", .3)],
         ins=["In", "Cutoff CV", "Res CV"], outs=["LP", "BP", "HP", "Notch"]),
    dict(kind="env.adsr", title="ADSR Envelope", w=240, sec="mod", disp="adsr", more=0,
         knobs=[("Attack", "10 ms", .15), ("Decay", "100 ms", .3), ("Sustain", "0.70", .7), ("Release", "200 ms", .4)],
         seg=["CONT", "KEY"], segsel=0, ins=["Gate"], outs=["Out"]),
    dict(kind="drum.grid", title="Drum Grid", w=660, sec="timing", disp="grid", more=0, knobs=[],
         ins=["Clock", "Reset", "Pattern"],
         outs=["Kick", "Snare", "Clap", "Chat", "Ohat", "Tom", "Perc 1", "Perc 2"]),
    dict(kind="compressor", title="Compressor", w=300, sec="fx", disp="comp", more=1,
         knobs=[("Threshold", "-18 dB", .5), ("Ratio", "4.00", .35), ("Attack", "10 ms", .2),
                ("Release", "150 ms", .4), ("Makeup", "+0.0 dB", .5)],
         ins=["In L", "In R", "SC"], outs=["Left", "Right", "GR"]),
    dict(kind="mixer", title="Mixer", w=240, sec="amp", disp=None, more=0,
         knobs=[("Level 1", "1.00", .75), ("Level 2", "1.00", .75), ("Level 3", "1.00", .75), ("Level 4", "1.00", .75)],
         ins=["In 1", "In 2", "In 3", "In 4"], outs=["Out"]),
]
H = 340
BEFORE = {"osc.va", "filter.svf", "env.adsr", "drum.grid", "compressor", "mixer"}


class P:
    """Accumulates SVG fragments."""
    _n = 0

    def __init__(self):
        self.s, self.defs = [], []

    def add(self, x):
        self.s.append(x)

    def uid(self, p="g"):
        P._n += 1
        return f"{p}{P._n}"

    def grad(self, stops, x2=0, y2=1, radial=False, cx=.5, cy=.5, r=.5):
        i = self.uid("lg")
        st = "".join(f'<stop offset="{o}" stop-color="{c}" stop-opacity="{a}"/>' for o, c, a in stops)
        if radial:
            self.defs.append(f'<radialGradient id="{i}" cx="{cx}" cy="{cy}" r="{r}">{st}</radialGradient>')
        else:
            self.defs.append(f'<linearGradient id="{i}" x1="0" y1="0" x2="{x2}" y2="{y2}">{st}</linearGradient>')
        return f"url(#{i})"

    def blur(self, sd):
        i = self.uid("bl")
        self.defs.append(f'<filter id="{i}" x="-20%" y="-50%" width="140%" height="200%"><feGaussianBlur stdDeviation="{sd}"/></filter>')
        return f"url(#{i})"

    def clip(self, x, y, w, h, r):
        i = self.uid("cp")
        self.defs.append(f'<clipPath id="{i}"><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}"/></clipPath>')
        return f"url(#{i})"


def text(p, x, y, s, size, fill, anchor="middle", weight=500, mono=False, op=1, ls=0):
    fam = MONO if mono else FONT
    p.add(f'<text x="{x:.1f}" y="{y:.1f}" font-family="{fam}" font-size="{size}" font-weight="{weight}" '
          f'fill="{fill}" fill-opacity="{op}" text-anchor="{anchor}" letter-spacing="{ls}">{s}</text>')


def arc(cx, cy, r, n0, n1):
    import math
    a0, a1 = math.radians(135 + 270 * n0), math.radians(135 + 270 * n1)
    x0, y0, x1, y1 = cx + r * math.cos(a0), cy + r * math.sin(a0), cx + r * math.cos(a1), cy + r * math.sin(a1)
    large = 1 if (n1 - n0) * 270 > 180 else 0
    return f"M{x0:.2f},{y0:.2f} A{r},{r} 0 {large} 1 {x1:.2f},{y1:.2f}"


# ---------------------------------------------------------------- parts, both options
def face_bg(p, t, f, opt, x, y, w):
    s = t[f["sec"]]
    base = s["base"]
    if opt == 1:   # seated: faint satin gradient, hairline top light, soft shadow, nuts
        top, bot = mix(base, "#ffffff", .10), mix(base, "#000000", .10)
        p.add(f'<rect x="{x+1}" y="{y+3}" width="{w}" height="{H}" rx="4" fill="#000" fill-opacity=".28" filter="{p.blur(4)}"/>')
        p.add(f'<rect x="{x}" y="{y}" width="{w}" height="{H}" rx="3" fill="{p.grad([(0, top, 1), (1, bot, 1)])}"/>')
        p.add(f'<rect x="{x+.5}" y="{y+.5}" width="{w-1}" height="{H-1}" rx="3" fill="none" stroke="#000" stroke-opacity=".35"/>')
        p.add(f'<line x1="{x+2}" y1="{y+1.5}" x2="{x+w-2}" y2="{y+1.5}" stroke="#fff" stroke-opacity=".28"/>')
        for cx, cy in ((x + 8, y + 8), (x + w - 8, y + 8), (x + 8, y + H - 8), (x + w - 8, y + H - 8)):
            p.add(f'<circle cx="{cx}" cy="{cy}" r="3.2" fill="{t["hole"]}" fill-opacity=".85"/>'
                  f'<circle cx="{cx}" cy="{cy-.6}" r="3.2" fill="none" stroke="#fff" stroke-opacity=".22"/>')
    else:          # tinted: flat colour, one 4% light, hairline edge, no hardware
        top = mix(base, "#ffffff", .04)
        p.add(f'<rect x="{x}" y="{y}" width="{w}" height="{H}" rx="6" fill="{p.grad([(0, top, 1), (1, base, 1)])}"/>')
        p.add(f'<rect x="{x+.5}" y="{y+.5}" width="{w-1}" height="{H-1}" rx="6" fill="none" stroke="{mix(base, "#000", .28)}"/>')


def header(p, t, f, opt, x, y, w):
    s = t[f["sec"]]
    text(p, x + 14, y + 25, f["title"], 14, s["ink"], "start", 600)
    text(p, x + 14, y + 40, f"{f['kind']} · #1", 11, s["ink2"], "start", 400, mono=True, op=.85)
    if f["kind"] == "env.adsr":
        seg(p, t, f, opt, x + w - 14 - 84, y + 12, 84, f["seg"], f["segsel"])
    if f["more"]:
        cw = 28
        if t["dark"]:
            fill, op, col = "#000", .38, s["ink"]
        elif opt == 1:
            fill, op, col = s["ink"], .82, mix(s["base"], "#ffffff", .9)
        else:
            fill, op, col = s["ink"], .16, s["ink"]
        p.add(f'<rect x="{x+w-14-cw}" y="{y+13}" width="{cw}" height="18" rx="9" fill="{fill}" fill-opacity="{op}"/>')
        text(p, x + w - 14 - cw / 2, y + 26, f"+{f['more']}", 11, col, "middle", 600)


def well(p, t, f, opt, x, y, w, h):
    s = t[f["sec"]]
    if opt == 1:   # recessed glass: lip, bezel, inner shadow, glass sheen
        p.add(f'<rect x="{x-2}" y="{y-2}" width="{w+4}" height="{h+5}" rx="7" fill="none" stroke="#fff" stroke-opacity=".30" stroke-width="1" transform="translate(0,1)"/>')
        p.add(f'<rect x="{x-2}" y="{y-2}" width="{w+4}" height="{h+4}" rx="7" fill="#000" fill-opacity=".38"/>')
        p.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="5" fill="{t["disp_bg"]}"/>')
        c = p.clip(x, y, w, h, 5)
        p.add(f'<g clip-path="{c}"><rect x="{x}" y="{y}" width="{w}" height="14" fill="{p.grad([(0, "#000", .55), (1, "#000", 0)])}"/>'
              f'<rect x="{x}" y="{y}" width="8" height="{h}" fill="{p.grad([(0, "#000", .35), (1, "#000", 0)], 1, 0)}"/>'
              f'<rect x="{x+w-8}" y="{y}" width="8" height="{h}" fill="{p.grad([(0, "#000", 0), (1, "#000", .35)], 1, 0)}"/>'
              f'<polygon points="{x+w*.55},{y} {x+w},{y} {x+w},{y+h*.6}" fill="#fff" fill-opacity=".035"/></g>')
    else:          # tinted: same hue as the face, deeper, hairline, no bezel
        deep = mix(s["base"], "#000", .74 if not t["dark"] else .62)
        p.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="{deep}"/>')
        p.add(f'<rect x="{x+.5}" y="{y+.5}" width="{w-1}" height="{h-1}" rx="6" fill="none" stroke="{mix(deep, "#fff", .14)}"/>')
    return mix(s["base"], "#000", .74 if not t["dark"] else .62) if opt == 2 else t["disp_bg"]


def grid_lines(p, t, x, y, w, h, bg):
    g = mix(bg, "#ffffff", .09)
    for i in range(1, 4):
        p.add(f'<line x1="{x+w*i/4:.1f}" y1="{y+4}" x2="{x+w*i/4:.1f}" y2="{y+h-4}" stroke="{g}"/>')
    p.add(f'<line x1="{x+4}" y1="{y+h/2:.1f}" x2="{x+w-4}" y2="{y+h/2:.1f}" stroke="{g}"/>')


def trace(p, t, d, fill_d=None, glow=True, width=1.8, opt=1):
    c = t["trace"]
    if fill_d:
        p.add(f'<path d="{fill_d}" fill="{c}" fill-opacity=".14"/>')
    if glow:
        p.add(f'<path d="{d}" fill="none" stroke="{c}" stroke-width="{width+3}" stroke-opacity=".55" stroke-linejoin="round" filter="{p.blur(2.6)}"/>')
    p.add(f'<path d="{d}" fill="none" stroke="{c}" stroke-width="{width}" stroke-linejoin="round" stroke-linecap="round"/>')


def display(p, t, f, opt, x, y, w, h):
    bg = well(p, t, f, opt, x, y, w, h)
    grid_lines(p, t, x, y, w, h, bg)
    c = t["trace"]
    k = f["disp"]
    gl = opt == 1
    ix, iy, iw, ih = x + 8, y + 8, w - 16, h - 16
    if k == "wave":
        pts = []
        for cyc in range(2):
            x0 = ix + iw * cyc / 2
            pts += [(x0, iy + ih), (x0 + iw / 2, iy)] if cyc == 0 else [(x0, iy + ih), (x0 + iw / 2, iy)]
        d = "M" + " L".join(f"{a:.1f},{b:.1f}" for a, b in [(ix, iy + ih), (ix + iw / 2, iy), (ix + iw / 2, iy + ih), (ix + iw, iy)])
        trace(p, t, d, None, gl)
    elif k == "filter":
        yb, yt = iy + ih, iy + 6
        d = (f"M{ix},{yt+4} L{ix+iw*.50:.1f},{yt+4} C{ix+iw*.56:.1f},{yt+4} {ix+iw*.60:.1f},{yt-2} {ix+iw*.62:.1f},{yt-2} "
             f"C{ix+iw*.68:.1f},{yt-2} {ix+iw*.72:.1f},{yb-6} {ix+iw},{yb-4}")
        trace(p, t, d, d + f" L{ix+iw},{yb} L{ix},{yb} Z", gl)
        cx = ix + iw * .62
        p.add(f'<line x1="{cx}" y1="{iy}" x2="{cx}" y2="{iy+ih}" stroke="{c}" stroke-opacity=".35" stroke-dasharray="2 3"/>')
        p.add(f'<circle cx="{cx}" cy="{yt-2}" r="3.5" fill="#fff"/><circle cx="{cx}" cy="{yt-2}" r="6" fill="{c}" fill-opacity=".30"/>')
    elif k == "adsr":
        yb, yt = iy + ih, iy + 2
        d = f"M{ix},{yb} L{ix+iw*.18:.1f},{yt} L{ix+iw*.40:.1f},{yb-(ih)*.62:.1f} L{ix+iw*.74:.1f},{yb-ih*.62:.1f} L{ix+iw},{yb}"
        trace(p, t, d, d + " Z", gl)
        p.add(f'<circle cx="{ix+iw*.30:.1f}" cy="{yb-ih*.81:.1f}" r="3.5" fill="#fff"/><circle cx="{ix+iw*.30:.1f}" cy="{yb-ih*.81:.1f}" r="6" fill="{c}" fill-opacity=".30"/>')
    elif k == "comp":
        gw = iw * .46
        d = f"M{ix},{iy+ih} L{ix+gw*.45:.1f},{iy+ih*.55:.1f} Q{ix+gw*.6:.1f},{iy+ih*.30:.1f} {ix+gw:.1f},{iy+ih*.18:.1f}"
        trace(p, t, d, None, gl)
        p.add(f'<line x1="{ix}" y1="{iy+ih}" x2="{ix+gw:.1f}" y2="{iy}" stroke="{c}" stroke-opacity=".28" stroke-dasharray="3 3"/>')
        mx = ix + iw * .56
        text(p, mx, iy + 12, "GR -3.2 dB", 11, t["disp_text"], "start", 400, mono=True, op=.9)
        mw = iw * .44
        p.add(f'<rect x="{mx}" y="{iy+22}" width="{mw:.1f}" height="10" rx="3" fill="#fff" fill-opacity=".07"/>')
        p.add(f'<rect x="{mx}" y="{iy+22}" width="{mw*.32:.1f}" height="10" rx="3" fill="{c}" fill-opacity=".9"/>')


def knob(p, t, f, opt, cx, cy, r, label, value, n):
    s = t[f["sec"]]
    text(p, cx, cy - r - 10, label, 11, s["ink"], "middle", 500)
    if opt == 1:
        import math
        p.add(f'<circle cx="{cx+.8}" cy="{cy+2}" r="{r}" fill="#000" fill-opacity=".35" filter="{p.blur(1.6)}"/>')
        for i in range(11):   # scale ticks
            a = math.radians(135 + 27 * i)
            p.add(f'<line x1="{cx+(r+3)*math.cos(a):.1f}" y1="{cy+(r+3)*math.sin(a):.1f}" x2="{cx+(r+6)*math.cos(a):.1f}" y2="{cy+(r+6)*math.sin(a):.1f}" stroke="{s["ink2"]}" stroke-opacity=".6"/>')
        p.add(f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{p.grad([(0, t["knob_hi"], 1), (.45, t["knob"], 1), (1, mix(t["knob"], "#000", .4), 1)], radial=True, cx=.35, cy=.3, r=.9)}"/>')
        p.add(f'<circle cx="{cx}" cy="{cy}" r="{r-.5}" fill="none" stroke="#fff" stroke-opacity=".14"/>')
        p.add(f'<circle cx="{cx}" cy="{cy}" r="{r*.72}" fill="none" stroke="#000" stroke-opacity=".28"/>')
        a = math.radians(135 + 270 * n)
        p.add(f'<line x1="{cx+r*.30*math.cos(a):.1f}" y1="{cy+r*.30*math.sin(a):.1f}" x2="{cx+r*.82*math.cos(a):.1f}" y2="{cy+r*.82*math.sin(a):.1f}" stroke="{t["knob_ink"]}" stroke-width="2.2" stroke-linecap="round"/>')
    else:
        rr = r + 3
        p.add(f'<path d="{arc(cx, cy, rr, 0, 1)}" fill="none" stroke="{s["ink"]}" stroke-opacity=".18" stroke-width="3" stroke-linecap="round"/>')
        p.add(f'<path d="{arc(cx, cy, rr, 0, max(n, .02))}" fill="none" stroke="{s["ink"]}" stroke-width="3" stroke-linecap="round"/>')
        p.add(f'<circle cx="{cx}" cy="{cy}" r="{r-3}" fill="{t["knob"]}"/>')
        import math
        a = math.radians(135 + 270 * n)
        p.add(f'<circle cx="{cx+(r-9)*math.cos(a):.1f}" cy="{cy+(r-9)*math.sin(a):.1f}" r="2.1" fill="{t["knob_ink"]}"/>')
    text(p, cx, cy + r + 17, value, 11, s["ink2"], "middle", 400, mono=True)


def seg(p, t, f, opt, x, y, w, labels, sel):
    s = t[f["sec"]]
    h = 22
    n = len(labels)
    if opt == 1:
        p.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="5" fill="{t["disp_bg"]}"/>')
        p.add(f'<rect x="{x+.5}" y="{y+.5}" width="{w-1}" height="{h-1}" rx="5" fill="none" stroke="#000" stroke-opacity=".5"/>')
        p.add(f'<line x1="{x+3}" y1="{y+h+1}" x2="{x+w-3}" y2="{y+h+1}" stroke="#fff" stroke-opacity=".25"/>')
    else:
        p.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="{mix(s["base"], "#000", .74 if not t["dark"] else .62)}"/>')
    sw = w / n
    for i, lab in enumerate(labels):
        cx = x + sw * i + sw / 2
        if i == sel:
            if opt == 1:
                p.add(f'<rect x="{x+sw*i+2:.1f}" y="{y+2}" width="{sw-4:.1f}" height="{h-4}" rx="3.5" fill="{t["accent"]}"/>')
                col = t["on_accent"]
            else:
                p.add(f'<rect x="{x+sw*i+2:.1f}" y="{y+2}" width="{sw-4:.1f}" height="{h-4}" rx="4.5" fill="{t["trace"]}"/>')
                col = t["disp_bg"]
        else:
            col = t["disp_text"]
        text(p, cx, y + 15, lab, 11, col, "middle", 600 if i == sel else 500, mono=True)


def jack(p, t, f, opt, cx, cy, label):
    s = t[f["sec"]]
    if opt == 1:
        p.add(f'<circle cx="{cx}" cy="{cy+1.2}" r="11" fill="#000" fill-opacity=".30" filter="{p.blur(1.2)}"/>')
        p.add(f'<circle cx="{cx}" cy="{cy}" r="11" fill="{p.grad([(0, mix(s["base"], "#fff", .35), 1), (1, mix(s["base"], "#000", .35), 1)])}"/>')
        p.add(f'<circle cx="{cx}" cy="{cy}" r="7.4" fill="{t["hole"]}"/><circle cx="{cx}" cy="{cy}" r="7.4" fill="none" stroke="#000" stroke-opacity=".6"/>')
        p.add(f'<path d="M{cx-6},{cy+3.5} A7,7 0 0 0 {cx+6},{cy+3.5}" fill="none" stroke="#fff" stroke-opacity=".18"/>')
    else:
        p.add(f'<circle cx="{cx}" cy="{cy}" r="10" fill="none" stroke="{s["ink"]}" stroke-opacity=".55" stroke-width="2"/>')
        p.add(f'<circle cx="{cx}" cy="{cy}" r="7" fill="{t["hole"]}"/>')
    text(p, cx, cy + 25, label, 11, s["ink"], "middle", 500)


def tray(p, t, f, opt, x, y, w, h):
    s = t[f["sec"]]
    if opt == 1:   # a recess cut into the face
        p.add(f'<rect x="{x}" y="{y+1}" width="{w}" height="{h}" rx="6" fill="#fff" fill-opacity=".22"/>')
        p.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="{mix(s["base"], "#000", .22)}"/>')
        c = p.clip(x, y, w, h, 6)
        p.add(f'<g clip-path="{c}"><rect x="{x}" y="{y}" width="{w}" height="9" fill="{p.grad([(0, "#000", .30), (1, "#000", 0)])}"/></g>')
    else:
        p.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="#000" fill-opacity=".14"/>')
    # two rows: inputs above, outputs below, tags at the left edge
    rows = [("IN", f["ins"]), ("OUT", f["outs"])]
    wide = w >= 300
    x0 = x + (34 if wide else 6)
    span = w - (34 if wide else 6) - 6
    for ri, (tag, labs) in enumerate(rows):
        ry = y + 19 + ri * 40
        if wide:
            text(p, x + 8, ry + 4, tag, 11, s["ink2"], "start", 600, op=.8)
        for i, lab in enumerate(labs):
            cols = max(len(labs), 4) if len(labs) < 6 else len(labs)
            jack(p, t, f, opt, round(x0 + span * (i + .5) / cols, 1), ry, lab)


def drum_grid(p, t, f, opt, x, y, w):
    s = t[f["sec"]]
    # header controls: pattern, brush, chance in one right-aligned group
    gx = x + w - 14
    groups = [(["A", "B", "C", "D"], 0, 100), (["30", "55", "78", "100"], 3, 130), (["100%", "75%", "50%", "25%"], 0, 160)]
    cur = gx
    for labs, sel, gw in reversed(groups):
        cur -= gw
        seg(p, t, f, opt, cur, y + 12, gw, labs, sel)
        cur -= 10
    gy, gh = y + 50, 182
    gwid = w - 24
    bg = well(p, t, f, opt, x + 12, gy, gwid, gh)
    rows = [("kick", "#f2a58f"), ("snare", "#f1d27b"), ("clap", "#9fd7a2"), ("chat", "#8fe0d1"), ("ohat", "#8fc4ec"), ("tom", "#b7a6ee"), ("perc1", "#e5a6d6"), ("perc2", "#d6d6d6")]
    hits = {0: [0, 4, 8, 12], 1: [4, 12], 3: [2, 6, 10, 14], 4: [7, 15], 2: []}
    cw, ch, gap, rowgap = 32.8, 15, 2, 6
    x0 = x + 12 + 50
    for ri, (rn, col) in enumerate(rows):
        cy = gy + 8 + ri * (ch + rowgap)
        text(p, x + 12 + 10, cy + 11.5, rn, 11, t["disp_text"], "start", 400, mono=True, op=.75)
        for st in range(16):
            cx = x0 + st * (cw + gap) + (st // 4) * 6
            on = st in hits.get(ri, [])
            if opt == 1:
                if on:
                    p.add(f'<rect x="{cx-2}" y="{cy-2}" width="{cw+4}" height="{ch+4}" rx="4" fill="{col}" fill-opacity=".28" filter="{p.blur(2.5)}"/>')
                    p.add(f'<rect x="{cx}" y="{cy}" width="{cw}" height="{ch}" rx="2.5" fill="{p.grad([(0, mix(col, "#fff", .35), 1), (1, col, 1)])}"/>')
                else:
                    p.add(f'<rect x="{cx}" y="{cy}" width="{cw}" height="{ch}" rx="2.5" fill="{mix(bg, "#fff", .055)}"/>')
                    p.add(f'<rect x="{cx}" y="{cy}" width="{cw}" height="{ch}" rx="2.5" fill="none" stroke="#000" stroke-opacity=".5"/>')
            else:
                fill = mix(col, bg, .18) if on else mix(bg, "#fff", .075)
                p.add(f'<rect x="{cx}" y="{cy}" width="{cw}" height="{ch}" rx="4" fill="{fill}"/>')
    # playhead
    px = x0 + 9 * (cw + gap) + (9 // 4) * 6
    if opt == 1:
        p.add(f'<rect x="{px-1}" y="{gy+5}" width="{cw+2}" height="{gh-10}" rx="3" fill="#fff" fill-opacity=".10"/>')
    else:
        p.add(f'<rect x="{px}" y="{gy+4}" width="{cw}" height="3" rx="1.5" fill="{t["trace"]}"/>')
        p.add(f'<rect x="{px}" y="{gy+7}" width="{cw}" height="{gh-14}" fill="{t["trace"]}" fill-opacity=".10"/>')


def face_svg(f, t, opt, x, y):
    """One face (option 1 or 2) at (x, y); returns (svg-fragment, defs)."""
    p = P()
    w = f["w"]
    face_bg(p, t, f, opt, x, y, w)
    header(p, t, f, opt, x, y, w)
    ty = y + H - 12 - 88
    tray(p, t, f, opt, x + 12, ty, w - 24, 88)
    if f["disp"] == "grid":
        drum_grid(p, t, f, opt, x, y, w)
        return "".join(p.s), "".join(p.defs)
    top = y + 48
    if f["disp"]:
        dh = 60
        display(p, t, f, opt, x + 12, top, w - 24, dh)
        top += dh + 10
    avail = ty - top - 8
    n = len(f["knobs"])
    hseg = bool(f.get("seg")) and n < 4
    if f["kind"] == "mixer":   # no display: 2x2 larger knobs fill the zone
        r = 18
        for i, (lab, val, nn) in enumerate(f["knobs"]):
            cx = x + w * (.30 if i % 2 == 0 else .70)
            cy = top + 8 + 30 + r + (i // 2) * 86
            knob(p, t, f, opt, cx, cy, r, lab, val, nn)
    else:
        r = 18 if hseg else (24 if n <= 2 else (20 if n <= 3 else 17))
        bh = 2 * r + 46 + (30 if hseg else 0)
        start = top + (avail - bh) / 2
        cy = start + 19 + r
        for i, (lab, val, nn) in enumerate(f["knobs"]):
            cx = x + 12 + (w - 24) * (i + .5) / n
            knob(p, t, f, opt, cx, cy, r, lab, val, nn)
        if hseg:
            sw = 150 if len(f["seg"]) == 4 else 100
            seg(p, t, f, opt, x + (w - sw) / 2, cy + r + 26, sw, f["seg"], f["segsel"])
    return "".join(p.s), "".join(p.defs)


# ---------------------------------------------------------------- boards
def png_b64(path, box=None):
    im = Image.open(path).convert("RGB")
    if box:
        im = im.crop(box)
    b = io.BytesIO()
    im.save(b, "PNG")
    return base64.b64encode(b.getvalue()).decode(), im.size


def crop_before(path, w):
    """The captured face: 340 tall, bounded left by the first pixel column that differs from the rack."""
    im = Image.open(path).convert("RGB")
    bg = im.getpixel((900, 400))
    px = im.load()
    x0 = next(x for x in range(0, 1200) if any(max(abs(px[x, y][i] - bg[i]) for i in range(3)) > 24 for y in range(250, 560, 4)))
    return im.crop((x0, 238, x0 + w, 238 + H))


def board(theme, mode, scale=1, only=None, name="board"):
    t = T[(theme, mode)]
    faces = [f for f in FACES if not only or f["kind"] in only]
    gapx, m, lab = 22, 28, 30
    width = sum(f["w"] for f in faces) + gapx * (len(faces) - 1) + 2 * m
    rows = 3
    height = rows * (H + lab + 28) + 2 * m
    defs, body = [], []
    names = ["CURRENT (captured from the app)", "OPTION 1 · SEATED (drawn)", "OPTION 2 · TINTED (drawn)"]
    tcol = t["text"]
    for r in range(rows):
        ry = m + r * (H + lab + 28)
        body.append(f'<text x="{m}" y="{ry+16}" font-family="{FONT}" font-size="13" font-weight="600" fill="{tcol}" letter-spacing="1">{names[r]} — {theme.split(".")[1]} {mode}</text>')
        x = m
        for f in faces:
            y = ry + lab
            if r == 0:
                fp = src / f"{theme}-{mode}" / f["kind"] / "z100.png"
                im = crop_before(fp, f["w"])
                b = io.BytesIO()
                im.save(b, "PNG")
                body.append(f'<image x="{x}" y="{y}" width="{im.width}" height="{im.height}" xlink:href="data:image/png;base64,{base64.b64encode(b.getvalue()).decode()}"/>')
            else:
                frag, d = face_svg(f, t, r, x, y)
                body.append(frag)
                defs.append(d)
            x += f["w"] + gapx
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{width}" height="{height}" viewBox="0 0 {width} {height}">'
           f'<defs>{"".join(defs)}</defs><rect width="{width}" height="{height}" fill="{t["rack"]}"/>{"".join(body)}</svg>')
    f_svg = out / f"{name}-{theme}-{mode}.svg"
    f_svg.write_text(svg)
    f_png = out / f"{name}-{theme}-{mode}.png"
    subprocess.run(["inkscape", str(f_svg), "--export-type=png", f"--export-filename={f_png}", f"--export-dpi={96*scale}"],
                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    f_svg.unlink()


if __name__ == "__main__":
    for (theme, mode) in T:
        board(theme, mode)
        board(theme, mode, 2, ["osc.va", "env.adsr"], "closeup")
