#!/usr/bin/env python3
"""Revision-2 concept mockups: knob modulation, rich LFO, A-dark, illustrated skins.

Concept art only, not screenshots of kabl-ui. Extends ../render.py: same layout model, panel
grammar and drawing primitives. Revision 1 images are left untouched.

    python3 docs/design/revision-2/render2.py        # PNGs under docs/design/revision-2/img/
    python3 docs/design/revision-2/render2.py --qa   # measurements only (JSON)

Needs what ../render.py needs (google-chrome headless, Pillow, IBM Plex Sans/Mono).
"""
import json
import math
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep ../__pycache__ out of the tree
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import render as R  # noqa: E402
from render import (text, rect, circle, polar, arc, mix, exp_t, tw, k, sel, port,  # noqa: E402
                    UNIT, PANEL_H, TOP_H, FOOT_H, RACK_X, DRAWER_W)

OUT = HERE / "img"
BUILD = R.BUILD / "r2"


def viewport(w, h):
    """1440×900 is the proposed default; 1280×800 stays the supported minimum."""
    R.W, R.H = w, h
    # at 900 px the extra height becomes a 50 px cable channel between the rows
    R.ROW_Y = [62, 426] if h <= 800 else [70, 480]


# --------------------------------------------------------------------------- themes

A_LIGHT = dict(R.THEMES["a-warm"], key="a-light", title="A-light", concept="#c2412d")
A_DARK = dict(
    A_LIGHT, key="a-dark", title="A-dark",
    app="#141311", chrome="#1d1c1a", chrome_edge="#36332f", ctext="#efe8da", ctext2="#ada390",
    btn="#2c2a27", btn_on="#e9e0cf", btn_on_text="#1d1b18",
    rack="#0e0d0c", rail="#5f5b55", rail_hi="#8c877e", hole="#161513",
    panel="#2f2c28", panel_edge="#4d4943", ink="#efe8da", ink2="#b2a893",
    plate="#191816", plate_ink="#efe8da",
    # B's material: knurled light-metal caps on a dark skirt, restrained glow on active state
    knob="#8e8981", knob_hi="#ece7de", pointer="#1b1917", skirt="#1b1a18", tick="#8d8475",
    nut="#a29d94", nut_edge="#67635c", hole_c="#080807",
    display="#141311", display_ink="#f1cf94",
    seg_bg="#23211e", seg_on="#e9dcc0", seg_on_text="#1b1916",
    sel="#5a9bff", texture="brushed", glow=True, concept="#ff7a5c",
    sig={"audio": "#f0a640", "cv": "#35c2b1", "gate": "#a687ee", "pitch": "#35c2b1"})
SKIN_LIGHT = dict(A_LIGHT, key="skin-light", title="Illustrated skin · light", art="illustrated")
SKIN_DARK = dict(A_DARK, key="skin-dark", title="Illustrated skin · dark", art="illustrated")


# --------------------------------------------------------------------------- params and units

def fmt(scale, v):
    u = scale[3]
    if u == "ms":
        return f"{v:.3g} ms" if v < 1000 else f"{v / 1000:.2f} s"
    if u == "Hz":
        return f"{v / 1000:.2f} kHz" if v >= 1000 else f"{v:.0f} Hz" if v >= 100 else f"{v:.1f} Hz" if v >= 10 else f"{v:.2f} Hz"
    if u == "%":
        return f"{v:+.1f} %"
    if u == "°":
        return f"{v:.0f}°"
    if u == "±":
        return f"{v:+.2f}"
    return f"{v:.2f}"


def t_of(scale, v):
    return exp_t(v, scale[1], scale[2]) if scale[0] == "exp" else (v - scale[1]) / (scale[2] - scale[1])


def v_of(scale, t):
    t = min(1, max(0, t))
    return scale[1] * (scale[2] / scale[1]) ** t if scale[0] == "exp" else scale[1] + (scale[2] - scale[1]) * t


def kv(pid, label, v, scale, size, cx, cy):
    c = k(pid, label, fmt(scale, v), t_of(scale, v), size, cx, cy)
    c["scale"] = scale
    return c


CUT, RES, ATT = ("exp", 20, 20000, "Hz"), ("lin", 0, .95, ""), ("exp", .1, 1e4, "ms")
RATE, LIN1 = ("exp", .01, 100, "Hz"), ("lin", 0, 1, "")


# --------------------------------------------------------------------------- templates

def tpl_filter2(w, p):
    """Cutoff CV / Res CV jacks removed: the knobs themselves are modulation destinations."""
    return [kv("cutoff_hz", "Cutoff", p.get("cutoff_hz", 1200), CUT, "L", 66, 112),
            kv("resonance", "Resonance", p.get("resonance", .35), RES, "S", w - 56, 112)], [
        port("in", "In", "in", "audio", w / 2, 212),
        port("lp", "LP", "out", "audio", 40, 290), port("bp", "BP", "out", "audio", w / 2, 290),
        port("hp", "HP", "out", "audio", w - 40, 290)]


def tpl_env2(w, p):
    a, d, s, r = p.get("adsr", (8, 240, .6, 420))
    xs = [36 + i * (w - 72) / 3 for i in range(4)]
    return [dict(type="env", adsr=(a, d, s, r), x=16, y=54, w=w - 32, h=62),
            kv("attack_ms", "Attack", a, ATT, "S", xs[0], 168),
            kv("decay_ms", "Decay", d, ATT, "S", xs[1], 168),
            kv("sustain", "Sustain", s, LIN1, "S", xs[2], 168),
            kv("release_ms", "Release", r, ATT, "S", xs[3], 168)], [
        port("gate", "Gate", "in", "gate", 60, 272), port("out", "Out", "out", "cv", w - 60, 272)]


# Rich LFO. Rate + Waveform exist in the engine today; everything in LFO_CONCEPT does not.
LFO_DEF = {
    "rate_hz": ("Rate", RATE, .8), "waveform": ("Waveform", ["SIN", "TRI", "SAW", "SQR", "S&H"], 1),
    "fine": ("Fine", ("lin", -10, 10, "%"), 0), "phase": ("Phase", ("lin", 0, 360, "°"), 0),
    "fade_ms": ("Fade-in", ("exp", 1, 1e4, "ms"), 1200), "amp": ("Amplitude", LIN1, 1.0),
    "offset": ("Offset", ("lin", -1, 1, "±"), 0), "sync": ("Sync", ["FREE", "TEMPO"], 0),
    "polarity": ("Polarity", ["BI", "UNI"], 0), "trigger": ("Trigger", ["FREE", "NOTE", "ONCE"], 1),
    "scope": ("Scope", ["VOICE", "GLOBAL"], 0)}
LFO_CONCEPT = set(LFO_DEF) - {"rate_hz", "waveform"} | {"trig"}
LFO_PRIMARY = ["rate_hz", "waveform"]  # module-declared default; the user may change it
# advanced area, local to its left edge: knobs (cx, cy), selectors (x, y, w)
ADV = {"fine": (45, 112), "phase": (121, 112), "fade_ms": (197, 112), "amp": (273, 112),
       "offset": (349, 112), "sync": (14, 188, 116), "polarity": (142, 188, 96),
       "trigger": (250, 188, 126), "scope": (250, 270, 126)}
ADV_W = 13 * UNIT


def lfo_ctl(cid, p, *geo):
    label, spec, default = LFO_DEF[cid]
    if isinstance(spec, list):
        x, y, w = geo
        return sel(cid, label, spec, p.get(cid, default), x, y, w)
    size, cx, cy = geo
    return kv(cid, label, p.get(cid, default), spec, size, cx, cy)


def lfo_face(p):
    """Compact face = the primary controls. Width grows with the number of primary knobs."""
    prim = p.get("primary", LFO_PRIMARY)
    knobs = [c for c in prim if not isinstance(LFO_DEF[c][1], list)]
    sels = [c for c in prim if isinstance(LFO_DEF[c][1], list)]
    fw = UNIT * (7 if len(knobs) <= 2 else 8 if len(knobs) == 3 else 10)
    if len(knobs) == 1:
        ctl = [lfo_ctl(knobs[0], p, "L", fw / 2, 112)]
    else:
        ctl = [lfo_ctl(c, p, "S", fw * (i + .5) / len(knobs), 112) for i, c in enumerate(knobs)]
    n = sum(len(LFO_DEF[c][1]) for c in sels)
    x = 14
    for c in sels:
        w = (fw - 28 - 10 * (len(sels) - 1)) * len(LFO_DEF[c][1]) / n
        ctl.append(lfo_ctl(c, p, x, 188, w))
        x += w + 10
    return fw, ctl, [port("trig", "Trig", "in", "gate", 40, 272),
                     port("out", "Out", "out", "cv", fw - 44, 272)]


def lfo_wu(p):
    fw = lfo_face(p)[0]
    return fw // UNIT + (ADV_W // UNIT if p.get("state") == "expanded" else 0)


def tpl_lfo2(w, p):
    fw, ctl, ports = lfo_face(p)
    if p.get("state") == "expanded":
        prim = p.get("primary", LFO_PRIMARY)
        for cid, g in ADV.items():
            if cid not in prim:
                ctl.append(lfo_ctl(cid, p, *(("S", fw + g[0], g[1]) if len(g) == 2 else (fw + g[0], g[1], g[2]))))
        ctl.append(dict(type="lfo_out", x=fw + 14, y=238, w=222, h=70))
    return ctl, ports


def tpl_ensemble(w, p):
    """Conceptual user-made module with an illustrated skin (string-ensemble chorus)."""
    return [kv("rate", "Rate", .6, ("exp", .05, 10, "Hz"), "S", 40, 120),
            kv("depth", "Depth", .45, LIN1, "S", w / 2, 120),
            kv("mix", "Mix", .7, LIN1, "S", w - 40, 120),
            sel("mode", "Mode", ["I", "II", "I+II"], 2, 16, 198, w - 32)], [
        port("in", "In", "in", "audio", 38, 290),
        port("l", "L", "out", "audio", w - 88, 290), port("r", "R", "out", "audio", w - 38, 290)]


R.KINDS.update({"filter.svf": ("SVF Filter", 7, tpl_filter2, "voice"),
                "env.adsr": ("ADSR Envelope", 8, tpl_env2, "voice"),
                "lfo": ("LFO", 7, tpl_lfo2, "voice"),
                "user.ensemble": ("Night Ensemble", 7, tpl_ensemble, "voice")})
R.SHORT.update({"ens": "Ensemble", "lfoB": "LFO 2", "lfoA": "LFO 1"})


def build(rows, names=None, params=None, x0=None, y0=None):
    params = {k_: dict(v) for k_, v in (params or {}).items()}
    for row in rows:
        for mid, kind in row:
            if kind == "lfo":
                params.setdefault(mid, {})["wu"] = lfo_wu(params.get(mid, {}))
    mods, _ = R.build_patch(rows, [], names, params)
    for m in mods.values():
        m["p"] = params.get(m["id"], {})
        if m["kind"] == "lfo":
            m["head_w"] = lfo_face(m["p"])[0]
        if m["kind"] == "user.ensemble":
            m["tag"] = "user.ensemble · skin"
        if x0 is not None:
            m["x"] += x0 - RACK_X
        if y0 is not None:
            m["y"] = y0
    return mods


def route(rid, src, dst, amount=1.0, uni=False, bypass=False, typ=None, src_pt=None, src_name=None):
    return dict(id=rid, src=src, dst=dst, amount=amount, uni=uni, bypass=bypass, type=typ,
                src_pt=src_pt, src_name=src_name)


def resolve(mods, routes):
    for r in routes:
        if r["type"] is None:
            m, p = r["src"]
            r["type"] = mods[m]["ports"][p]["type"]
    return routes


def find_ctl(m, cid):
    return next((c for c in m["controls"] if c.get("id") == cid), None)


def is_param(mods, r):
    return r["dst"][1] not in mods[r["dst"][0]]["ports"]


def src_label(mods, r):
    if r["src_name"]:
        return r["src_name"]
    m, p = r["src"]
    return f"{R.SHORT.get(m, mods[m]['name'])} {R.PORT_NAMES.get(p, p)}"


def dst_label(mods, r, short=True):
    m, pid = r["dst"]
    mm = mods[m]
    name = R.SHORT.get(m, mm["name"]) if short else mm["name"]
    if pid in mm["ports"]:
        return f"{name} {R.PORT_NAMES.get(pid, pid)}"
    lab = LFO_DEF[pid][0] if mm["kind"] == "lfo" and pid in LFO_DEF else (find_ctl(mm, pid) or {}).get("label", pid)
    return f"{name} {lab}"


def mod_span(rts):
    """Combined contribution in knob travel (0..1 space): sum of every active route's range."""
    lo = hi = 0.0
    for r in rts:
        if r["bypass"]:
            continue
        a = r["amount"]
        lo += min(0, a) if r["uni"] else -abs(a)
        hi += max(0, a) if r["uni"] else abs(a)
    return lo, hi


# --------------------------------------------------------------------------- drawing

def socket(mods, r, idx=0, n=1):
    """Where a modulation cable docks on its destination (window/rack coordinates)."""
    m = mods[r["dst"][0]]
    pid = r["dst"][1]
    if pid in m["ports"]:
        return R.jack_pos(mods, r["dst"])
    c = find_ctl(m, pid)
    if c is None:  # parameter hidden by collapse: the cable docks on the "+N" button
        return m["x"] + m["head_w"] - 30, m["y"] + 27
    if c["type"] == "knob":  # 6 o'clock: the gap a 270° knob scale leaves free
        return m["x"] + c["cx"] + (idx - (n - 1) / 2) * 15, m["y"] + c["cy"] + c["r"] + 5
    return m["x"] + c["x"] + c["w"] - 12, m["y"] + c["y"] - 13


def draw_cable2(th, a, b, col, opacity=.96, selected=False, bypass=False, small_end=False):
    d, c1, c2 = R.cable_path(a, b)
    if bypass:
        col = mix(col, "#8f8a82", .65)
    edge, hi = mix(col, "#000000", .35), mix(col, "#ffffff", .45)
    dash = ' stroke-dasharray="11 8"' if bypass else ""
    # knob routes are "mod leads": thinner and slightly translucent, so the panel reads through
    we, wc = (5.5, 4) if small_end else (7.5, 5.5)
    o = []
    if opacity > .5 and not bypass:
        o.append(f'<path d="{d}" transform="translate(4,8)" fill="none" stroke="#000" stroke-width="8" opacity=".3" filter="url(#blur3)"/>')
    if selected:
        o.append(f'<path d="{d}" fill="none" stroke="{th["sel"]}" stroke-width="13" opacity=".85"/>')
    o.append(f'<path d="{d}" fill="none" stroke="{edge}" stroke-width="{we}" stroke-linecap="round"{dash}/>')
    o.append(f'<path d="{d}" fill="none" stroke="{col}" stroke-width="{wc}" stroke-linecap="round"{dash}/>')
    if not bypass:
        o.append(f'<path d="{d}" transform="translate(-0.9,-1.4)" fill="none" stroke="{hi}" stroke-width="1.4" opacity=".6"/>')
    for end, ((px, py), (qx, qy)) in enumerate(((a, c1), (b, c2))):
        small = small_end and end == 1  # knob plug: smaller, so the knob stays visible
        ln = math.hypot(qx - px, qy - py) or 1
        ux, uy = (qx - px) / ln, (qy - py) / ln
        L, wb = (12, 7) if small else (20, 10)
        o.append(f'<line x1="{px:.1f}" y1="{py:.1f}" x2="{px + ux * L:.1f}" y2="{py + uy * L:.1f}" stroke="{edge}" stroke-width="{wb}" stroke-linecap="round"/>')
        o.append(f'<line x1="{px:.1f}" y1="{py:.1f}" x2="{px + ux * L:.1f}" y2="{py + uy * L:.1f}" stroke="{mix(col, "#000000", .15)}" stroke-width="{wb - 2}" stroke-linecap="round"/>')
        r0 = 7 if small else 11.5
        o.append(circle(px, py, r0, fill="#1a1a1a"))
        o.append(circle(px, py, r0 * .78, fill=col))
        o.append(circle(px, py, r0 * .39, fill=edge))
        o.append(circle(px - r0 * .26, py - r0 * .3, r0 * .23, fill="#ffffff", opacity=.45))
    return f'<g opacity="{opacity}">{"".join(o)}</g>'


def pill(th, cx, y, s, col, size=13, mono=True, fill=None):
    w = tw(s, size, mono) + 14
    return (rect(cx - w / 2, y - 13.5, w, 19, fill=fill or th["panel"], rx=9.5, stroke=col, sw=1.6)
            + text(cx, y, s, size, th["ink"], "middle", 500, mono=mono))


def small_badge(th, cx, y, s, col):
    w = tw(s, 10.5) + 12
    return (rect(cx - w / 2, y, w, 14, fill=th["plate"], rx=7, stroke=col, sw=1.5)
            + text(cx, y + 10.5, s, 10.5, th["plate_ink"], "middle", 600))


def two_tone_arc(cx, cy, rr, t0, t1, col, width, opacity=1.0, dash=""):
    d = arc(cx, cy, rr, t0, t1)
    return (f'<path d="{d}" fill="none" stroke="{mix(col, "#000000", .45)}" stroke-width="{width + 2}" '
            f'stroke-linecap="round" opacity="{opacity}"{dash}/>'
            f'<path d="{d}" fill="none" stroke="{col}" stroke-width="{width}" stroke-linecap="round" '
            f'opacity="{opacity}"{dash}/>')


def knob_mod(th, m, c, rts, sc, hidden):
    """Base = pointer + value; depth = per-route arcs (inspect); range = outer ring, clamped."""
    cx, cy, r, base = m["x"] + c["cx"], m["y"] + c["cy"], c["r"], c["t"]
    col = th["sig"]["cv"]
    key = (m["id"], c["id"])
    strong = key == sc.get("inspect") or key == sc.get("ring_drag") or any(
        x["src"][0] in sc.get("selected", ()) for x in rts)
    rr, o = r + 11, []
    lo, hi = mod_span(rts)
    L, Hh = base + lo, base + hi
    act = [x for x in rts if not x["bypass"]]
    if act:
        cl, ch = max(0, L), min(1, Hh)
        for beyond, lim in ((L < 0, 0), (Hh > 1, 1)):
            if beyond:  # depth lost to the clamp: dashed, faint, stops at the param limit
                t0, t1 = (max(L, -.12), 0) if lim == 0 else (1, min(Hh, 1.12))
                o.append(f'<path d="{arc(cx, cy, rr, t0, t1)}" fill="none" stroke="{col}" stroke-width="2" '
                         f'stroke-dasharray="3 3" opacity=".7"/>')
                x0, y0 = polar(cx, cy, r + 5, lim)
                x1, y1 = polar(cx, cy, r + 17, lim)
                o.append(f'<line x1="{x0:.1f}" y1="{y0:.1f}" x2="{x1:.1f}" y2="{y1:.1f}" stroke="{mix(col, "#000000", .45)}" stroke-width="4.5"/>')
                o.append(f'<line x1="{x0:.1f}" y1="{y0:.1f}" x2="{x1:.1f}" y2="{y1:.1f}" stroke="{col}" stroke-width="2.5"/>')
        o.append(two_tone_arc(cx, cy, rr, cl, max(ch, cl + .004), col, 4 if strong else 3, 1 if strong else .75))
        if len(act) == 1:  # polarity: the dot marks where the source's positive peak lands
            px, py = polar(cx, cy, rr, min(1, max(0, base + act[0]["amount"])))
            o.append(circle(px, py, 4.2, fill=th["panel"], stroke=mix(col, "#000000", .45), sw=2.4))
            o.append(circle(px, py, 2.2, fill=col))
    else:  # every route bypassed: the range stays visible as a hollow grey outline
        blo, bhi = mod_span([dict(x, bypass=False) for x in rts])
        o.append(f'<path d="{arc(cx, cy, rr, max(0, base + blo), min(1, base + bhi))}" '
                 f'fill="none" stroke="{th["ink2"]}" stroke-width="2" stroke-dasharray="4 3" opacity=".9"/>')
    if key == sc.get("inspect") and len(rts) > 1:  # one lane per source, outside the result ring
        for i, x in enumerate(rts):
            a = x["amount"]
            t0, t1 = (base + min(0, a), base + max(0, a)) if x["uni"] else (base - abs(a), base + abs(a))
            o.append(two_tone_arc(cx, cy, rr + 7 + 6 * i, max(0, t0), min(1, t1), mix(col, "#ffffff", .25 * i), 2))
    if key == sc.get("ring_drag") and act:
        px, py = polar(cx, cy, rr, min(1, max(0, base + act[-1]["amount"])))
        o.append(circle(px, py, 7, fill="#ffffff", stroke=th["sel"], sw=2.5))
    pc = col if act else th["ink2"]
    o.append(pill(th, cx, cy + 43, c["value"], pc))
    if hidden:
        for i, x in enumerate(rts):
            sx, sy = socket({m["id"]: m}, x, i, len(rts))
            o.append(circle(sx, sy, 5, fill=mix(pc, "#000000", .45)) + circle(sx, sy, 3.4, fill=pc))
        s = f"← {R.SHORT.get(rts[0]['src'][0], '?') if not rts[0]['src_name'] else rts[0]['src_name']}" \
            if len(rts) == 1 else f"← {len(rts)} mods"
        # below the pill; if that hits another label, beside it (whichever side is clear)
        bw, pw = tw(s, 10.5) + 12, tw(c["value"], 13, True) + 14
        boxes = [b_ for b_ in R.text_boxes(m) if b_[4] not in (c["label"], c["value"])]
        for bx, by in ((cx, cy + 50), (cx + pw / 2 + 4 + bw / 2, cy + 36), (cx - pw / 2 - 4 - bw / 2, cy + 36)):
            box = (bx - bw / 2 - m["x"], by - m["y"], bx + bw / 2 - m["x"], by + 14 - m["y"])
            if box[0] >= 4 and box[2] <= m["w"] - 4 and not any(R.overlaps(box, b_, gap=2) for b_ in boxes):
                break
        o.append(small_badge(th, bx, by, s, pc))
    return "".join(o)


def select_mod(th, m, c, rts, sc, hidden):
    """Discrete destination: modulation moves in whole options; the bracket shows reachable ones."""
    x, y, w, n = m["x"] + c["x"], m["y"] + c["y"], c["w"], len(c["options"])
    col = th["sig"]["cv"]
    steps = sum(round(abs(r["amount"]) * (n - 1)) for r in rts if not r["bypass"])
    uni = all(r["uni"] for r in rts)
    i0 = max(0, c["idx"] - (0 if uni else steps))
    i1 = min(n - 1, c["idx"] + steps)
    sw = w / n
    bx0, bx1 = x + i0 * sw + 3, x + (i1 + 1) * sw - 3
    o = [f'<path d="M{bx0:.1f},{y + 31} L{bx0:.1f},{y + 36} L{bx1:.1f},{y + 36} L{bx1:.1f},{y + 31}" fill="none" '
         f'stroke="{mix(col, "#000000", .45)}" stroke-width="4.5" stroke-linejoin="round"/>',
         f'<path d="M{bx0:.1f},{y + 31} L{bx0:.1f},{y + 36} L{bx1:.1f},{y + 36} L{bx1:.1f},{y + 31}" fill="none" '
         f'stroke="{col}" stroke-width="2.5" stroke-linejoin="round"/>',
         rect(x - 2, y - 2, w + 4, 32, stroke=col, sw=1.6, rx=7)]
    if hidden:
        sx, sy = socket({m["id"]: m}, rts[0])
        o.append(circle(sx, sy, 5, fill=mix(col, "#000000", .45)) + circle(sx, sy, 3.4, fill=col))
    return "".join(o)


def hidden_param_mod(th, m, rts, sc, hidden):
    """A modulated advanced param on a collapsed module: the '+N' button carries the ring."""
    bx, by = m["x"] + m["head_w"] - 48, m["y"] + 13
    col = th["sig"]["cv"]
    o = [rect(bx - 4, by - 4, 44, 36, stroke=mix(col, "#000000", .45), sw=4.5, rx=10),
         rect(bx - 4, by - 4, 44, 36, stroke=col, sw=2.5, rx=10)]
    if hidden:
        o.append(small_badge(th, bx + 18, by + 34, f"◆ {len(rts)} hidden", col))
    return "".join(o)


def draw_lfo_out(th, m, c):
    """Output preview: waveform × amplitude + offset, with polarity, phase and fade-in applied."""
    p, (x, y, w, h) = m["p"], (c["x"], c["y"], c["w"], c["h"])
    wave = p.get("waveform", 1)
    amp, off = p.get("amp", 1.0), p.get("offset", 0.0)
    uni, ph = p.get("polarity", 0) == 1, p.get("phase", 0) / 360
    fade = min(.8, p.get("fade_ms", 1200) / 1000 * p.get("rate_hz", .8) / 2.5)
    y0, sy = y + h / 2, h / 2 - 7
    o = [rect(x, y, w, h, fill=th["display"], rx=5),
         f'<line x1="{x + 6}" y1="{y0}" x2="{x + w - 6}" y2="{y0}" stroke="{th["display_ink"]}" stroke-width="1" stroke-dasharray="3 4" opacity=".4"/>']
    pts, env = [], []
    for i in range(121):
        u = i / 120
        t = u * 2.5 + ph
        v = [math.sin(t * 2 * math.pi), 1 - 4 * abs((t % 1) - .5), 2 * (t % 1) - 1,
             1 if (t % 1) < .5 else -1, [.6, -.3, .9, -.7, .2][int(t * 2) % 5]][wave]
        g = min(1, u / fade) if fade > 0 else 1
        v = (v + 1) / 2 if uni else v
        v = max(-1, min(1, off + amp * g * v))
        pts.append((x + 8 + u * (w - 16), y0 - v * sy))
        env.append((x + 8 + u * (w - 16), y0 - max(-1, min(1, off + amp * g)) * sy))
    o.append(f'<path d="M{" L".join(f"{a:.1f},{b:.1f}" for a, b in env)}" fill="none" stroke="{th["display_ink"]}" stroke-width="1" stroke-dasharray="2 3" opacity=".55"/>')
    o.append(f'<path d="M{" L".join(f"{a:.1f},{b:.1f}" for a, b in pts)}" fill="none" stroke="{th["display_ink"]}" stroke-width="2"/>')
    return "".join(o)


def panel_button(th, x, y, w, h, label, primary=False):
    fill = th["seg_on"] if primary else th["seg_bg"]
    fg = th["seg_on_text"] if primary else th["ink"]
    return (rect(x, y, w, h, fill=fill, rx=6, stroke=th["panel_edge"])
            + text(x + w / 2, y + h / 2 + 4.5, label, 12.5, fg, "middle", 600))


def label_box(c):
    """(left, right, baseline) of a control's label, panel-local."""
    if c["type"] == "knob":
        lw = tw(c["label"], 13)
        return c["cx"] - lw / 2, c["cx"] + lw / 2, c["cy"] - 43
    lw = tw(c["label"], 13)
    return c["x"] + c["w"] / 2 - lw / 2, c["x"] + c["w"] / 2 + lw / 2, c["y"] - 8


def extras(th, m, sc):
    """Revision-2 panel additions: expand button, advanced area, pins, conceptual marks."""
    o, w = [], m["w"]
    if m["kind"] == "lfo":
        p, fw = m["p"], m["head_w"]
        prim = p.get("primary", LFO_PRIMARY)
        choosing = m["id"] in sc.get("choose", ())
        if p.get("state") == "expanded":
            o.append(f'<line x1="{fw}" y1="12" x2="{fw}" y2="{PANEL_H - 12}" stroke="#000" stroke-width="1" opacity=".25"/>'
                     f'<line x1="{fw + 1}" y1="12" x2="{fw + 1}" y2="{PANEL_H - 12}" stroke="#fff" stroke-width="1" opacity=".18"/>')
            cap = "CHOOSE PRIMARY CONTROLS" if choosing else "ADVANCED"
            o.append(text(fw + 16, 31, cap, 11, th["ink2"], "start", 600, extra='letter-spacing="0.8"'))
            if choosing:
                o.append(text(fw + 16, 46, f"{len(prim)} primary · face {lfo_face(p)[0] // UNIT}u", 11, th["ink2"], "start", 400, mono=True))
                o.append(panel_button(th, w - 72, 13, 56, 28, "Done", primary=True))
            else:
                o.append(panel_button(th, w - 66, 13, 50, 28, "Less"))
            for c in m["controls"]:
                if c["type"] == "lfo_out":
                    o.append(draw_lfo_out(th, m, c))
        else:
            o.append(panel_button(th, fw - 48, 13, 36, 28, f"+{len(LFO_DEF) - len(prim)}"))
        if choosing:
            for c in m["controls"]:
                if c.get("id") in LFO_DEF:
                    x0, _, yb = label_box(c)
                    on = c["id"] in prim
                    px, py = x0 - 11, yb - 4.5
                    if c["id"] in sc.get("just_pinned", ()):
                        o.append(circle(px, py, 10, fill=th["sel"], opacity=.25))
                    o.append(circle(px, py, 5.5, fill=th["sel"] if on else "none",
                                    stroke=th["sel"] if on else th["ink2"], sw=1.6))
                    if on:
                        o.append(circle(px, py, 2, fill="#ffffff"))
    if sc.get("concept_marks", True) and m["id"] not in sc.get("choose", ()):
        marks = LFO_CONCEPT if m["kind"] == "lfo" else set()
        for c in m["controls"]:
            if c.get("id") in marks:
                _, x1, yb = label_box(c)
                o.append(diamond(x1 + 7, yb - 4.5, th["concept"]))
        for p_ in m["ports"].values():
            if p_["id"] in marks:
                o.append(diamond(p_["cx"] + tw(p_["label"], 13) / 2 + 7, p_["cy"] - 23.5, th["concept"]))
    return f'<g transform="translate({m["x"]},{m["y"]})">{"".join(o)}</g>'


def diamond(x, y, col, s=4):
    return f'<polygon points="{x},{y - s} {x + s},{y} {x},{y + s} {x - s},{y}" fill="{col}"/>'


def rack_content(th, sc, mods, routes):
    hidden = sc["cables"] == "hidden"
    selected = set(sc.get("selected", ()))
    st = dict(mods={}, rings={}, badges={}, selected=tuple(selected), drag=None,
              hot_knob=sc.get("hot_knob"), focus_knob=sc.get("focus_knob"))
    if hidden:  # jack rings + badges naming the other end (revision 1 rule, now incl. knob routes)
        peers = {}
        for r in routes:
            if not r["src_pt"]:
                peers.setdefault(r["src"], []).append(("→", r["dst"], r))
            if not is_param(mods, r):
                peers.setdefault(r["dst"], []).append(("←", r["src"], r))
        for key, lst in peers.items():
            col = th["sig"][lst[0][2]["type"]]
            if key[0] in selected or any(o_[1][0] in selected for o_ in lst):
                col = th["sel"]
            st["rings"][key] = col
            if len(lst) == 1:
                arrow, other, r = lst[0]
                name = r["src_name"] if arrow == "←" and r["src_name"] else R.SHORT.get(other[0], mods[other[0]]["name"])
                s = f"{arrow} {name}"
            else:
                s = f"{lst[0][0]} {len(lst)} routes"
            st["badges"][key] = (s, col)
    clips = "".join(f'<clipPath id="clip-{m["id"]}"><rect width="{m["w"]}" height="{PANEL_H}" rx="2"/></clipPath>'
                    for m in mods.values())
    o = [f"<defs>{clips}</defs>"]
    for m in mods.values():
        o.append(R.draw_module(th, m, st))
        o.append(extras(th, m, sc))
    by_dst = {}
    for r in routes:
        if is_param(mods, r):
            by_dst.setdefault(r["dst"], []).append(r)
    if not hidden:
        for r in routes:
            a = r["src_pt"] or R.jack_pos(mods, r["src"])
            if is_param(mods, r):
                lst = by_dst[r["dst"]]
                b, small = socket(mods, r, lst.index(r), len(lst)), True
            else:
                b, small = R.jack_pos(mods, r["dst"]), False
            dim = sc["cables"] == "focus" and not ({r["src"][0], r["dst"][0]} & selected)
            related = {r["src"][0], r["dst"][0]} & selected or r["dst"] == sc.get("inspect")
            op = (.13 if dim else .25 if r["id"] in sc.get("xray", ()) else
                  .72 if is_param(mods, r) and not related else .96)
            o.append(draw_cable2(th, a, b, th["sig"][r["type"]], op, selected=r["id"] == sc.get("selected_cable"),
                                 bypass=r["bypass"], small_end=small))
    for (mid, pid), rts in by_dst.items():
        m = mods[mid]
        c = find_ctl(m, pid)
        if c is None:
            o.append(hidden_param_mod(th, m, rts, sc, hidden))
        elif c["type"] == "knob":
            o.append(knob_mod(th, m, c, rts, sc, hidden))
        else:
            o.append(select_mod(th, m, c, rts, sc, hidden))
    d = sc.get("drag")
    if d:  # every knob is a destination; compatible input jacks too
        for m in mods.values():
            for c in m["controls"]:
                if c["type"] == "knob":
                    hot = (m["id"], c["id"]) == d.get("hover")
                    cx, cy = m["x"] + c["cx"], m["y"] + c["cy"]
                    o.append(circle(cx, cy, c["r"] + 6, stroke=th["sel"], sw=3 if hot else 1.6,
                                    fill=th["sel"] if hot else "none", opacity=1 if hot else .75,
                                    extra='fill-opacity=".18"' + ('' if hot else ' stroke-dasharray="4 3"')))
            for p_ in m["ports"].values():
                if p_["dir"] == "in" and p_["type"] in ("cv",) :
                    o.append(circle(m["x"] + p_["cx"], m["y"] + p_["cy"], 17, stroke=th["sel"], sw=2.5,
                                    fill=th["sel"], extra='fill-opacity=".15"'))
        o.append(draw_cable2(th, R.jack_pos(mods, d["from"]), d["to"], th["sig"]["cv"], .8, small_end=True))
    for n in sc.get("notes", ()):
        o.append(note(th, *n))
    return "".join(o)


def note(th, x, y, lines, w=None):
    """Design annotation (not UI): concept-red callout."""
    w = w or max(tw(s, 12) for s in lines) + 18
    h = 10 + 17 * len(lines)
    o = [rect(x, y, w, h, fill=th["concept"], rx=5, opacity=.95)]
    for i, s in enumerate(lines):
        o.append(text(x + 9, y + 19 + i * 17, s, 12, "#ffffff", "start", 600 if i == 0 else 400))
    return "".join(o)


def ghost_box(th, x, y, w, h, label):
    return (rect(x, y, w, h, stroke=th["concept"], sw=2, rx=4, extra='stroke-dasharray="7 5"')
            + text(x + w / 2, y + h / 2, label, 12, th["concept"], "middle", 600))


# --------------------------------------------------------------------------- drawer

def route_values(mods, r, rts=None):
    """(base str, lo str, hi str, clamped?) of a knob destination for this route (or all routes)."""
    m = mods[r["dst"][0]]
    c = find_ctl(m, r["dst"][1])
    if c is None or c["type"] != "knob" or "scale" not in c:
        return None
    lo, hi = mod_span(rts if rts is not None else [dict(r, bypass=False)])
    sc = c["scale"]
    t0, t1 = c["t"] + lo, c["t"] + hi
    return c["value"], fmt(sc, v_of(sc, t0)), fmt(sc, v_of(sc, t1)), t0 < 0 or t1 > 1


def drawer2(th, sc, mods, routes):
    x0, y0, y1 = R.W - DRAWER_W, TOP_H, R.H - FOOT_H
    x = x0 + 16
    o = [rect(x0, y0, DRAWER_W, y1 - y0, fill=th["chrome"]),
         f'<line x1="{x0 + .5}" y1="{y0}" x2="{x0 + .5}" y2="{y1}" stroke="{th["chrome_edge"]}"/>']
    mode = sc["drawer"]
    rid = sc.get("focus_route")
    fr = next((r for r in routes if r["id"] == rid), None)

    def card(y, r):
        col = th["sig"][r["type"]] if not r["bypass"] else th["ctext2"]
        h = 178
        o.append(rect(x - 4, y, DRAWER_W - 24, h, fill=mix(th["chrome"], th["app"], .5), rx=8, stroke=th["sel"], sw=1.5))
        o.append(rect(x - 4, y, 5, h, fill=col, rx=2))
        o.append(text(x + 10, y + 22, f"{src_label(mods, r)} → {dst_label(mods, r)}", 14, th["ctext"], "start", 600))
        sub = f"{'unipolar' if r['uni'] else 'bipolar'} source · per voice · {r['id']}"
        o.append(text(x + 10, y + 40, sub, 12, th["ctext2"], "start", 400))
        a = r["amount"]
        o.append(text(x + 10, y + 70, "Amount", 13, th["ctext"], "start", 500))
        sx, sw_ = x + 70, 130
        o.append(rect(sx, y + 62, sw_, 6, fill=th["chrome_edge"], rx=3))
        o.append(f'<line x1="{sx + sw_ / 2}" y1="{y + 56}" x2="{sx + sw_ / 2}" y2="{y + 74}" stroke="{th["ctext2"]}" stroke-width="1.5"/>')
        fx = sx + sw_ / 2 + a * sw_ / 2  # ±100 % of knob travel
        o.append(rect(min(sx + sw_ / 2, fx), y + 62, abs(fx - sx - sw_ / 2), 6, fill=col, rx=3))
        o.append(circle(fx, y + 65, 8, fill=th["ctext"], stroke=col, sw=2))
        o.append(rect(x + 214, y + 53, 80, 24, fill=th["chrome"], rx=5, stroke=th["chrome_edge"]))
        o.append(text(x + 254, y + 70, f"{a * 100:+.0f} %", 13, th["ctext"], "middle", 500, mono=True))
        rv = route_values(mods, r)
        if r["bypass"]:
            line = "Bypassed: no effect, amount kept"
        elif rv:
            lab = dst_label(mods, r).split(" ", 1)[1]
            line = f"{lab} {rv[0]} · swings {rv[1]} – {rv[2]}{' (clamped)' if rv[3] else ''}"
        else:
            line = "Moves the selector by whole options"
        o.append(text(x + 10, y + 98, line, 12, th["ctext2"], "start", 400))
        if not r["bypass"]:
            o.append(text(x + 10, y + 115, "Amount = % of knob travel · ● = where + peak lands", 11, th["ctext2"], "start", 400, opacity=.85))
        bx = x + 10
        for lab, on in (("Invert", a < 0), ("Bypass", r["bypass"]), ("Remove", False), ("Locate", False)):
            b, bw = R.button(th, bx, y + 132, lab, on=on, h=30)
            o.append(b)
            bx += bw + 8
        return y + h + 14

    def row(y, r, hl=False):
        col = th["sig"][r["type"]]
        if hl:
            o.append(rect(x - 6, y, DRAWER_W - 20, 32, fill=th["btn"], rx=6))
        o.append(rect(x, y + 7, 4, 18, fill=col, rx=2))
        s = f"{src_label(mods, r)} → {dst_label(mods, r)}"
        o.append(text(x + 14, y + 21, R.ellipsize(s, 13, 226), 13, th["ctext"] if not r["bypass"] else th["ctext2"], "start", 500))
        kind = "knob" if is_param(mods, r) else R.TYPE_LABEL[r["type"]]
        o.append(text(R.W - 22, y + 21, ("bypassed" if r["bypass"] else kind), 12, th["ctext2"], "end", 400))
        return y + 34

    if mode == "routing":
        o.append(text(x, y0 + 30, "Routing", 16, th["ctext"], "start", 600))
        b, _ = R.button(th, R.W - 16 - 62, y0 + 11, "Close", w=62, h=28)
        o.append(b)
        y = y0 + 50
        chips = [("All", len(routes)), ("Audio", sum(r["type"] == "audio" for r in routes)),
                 ("Mod", sum(is_param(mods, r) or r["type"] == "cv" for r in routes)),
                 ("Gate", sum(r["type"] == "gate" for r in routes))]
        cx = x
        for i, (lab, n) in enumerate(chips):
            b, bw = R.button(th, cx, y, f"{lab} {n}", on=i == 0, h=28, size=12.5)
            o.append(b)
            cx += bw + 6
        y += 46
        if fr:
            same = [r for r in routes if r["src"] == fr["src"]]
            o.append(text(x, y + 4, f"SELECTED SOURCE · {src_label(mods, fr)} · {len(same)} routes", 11,
                          th["ctext2"], "start", 600, extra='letter-spacing="0.8"'))
            y = card(y + 14, fr)
            for r in same:
                if r is not fr:
                    y = row(y, r)
            y += 10
        o.append(text(x, y + 4, f"ALL CONNECTIONS · {len(routes)}", 11, th["ctext2"], "start", 600, extra='letter-spacing="0.8"'))
        y += 14
        for r in routes:
            if y > y1 - 90:
                o.append(text(x, y + 16, f"… {len(routes) - routes.index(r)} more (scroll)", 12, th["ctext2"]))
                break
            y = row(y, r, hl=fr is not None and r["src"] == fr["src"])
    else:  # destination inspector
        dst = sc["dest"]
        rts = [r for r in routes if r["dst"] == dst]
        m = mods[dst[0]]
        c = find_ctl(m, dst[1])
        o.append(text(x, y0 + 30, f"{R.SHORT.get(m['id'], m['name'])} · {c['label']}", 16, th["ctext"], "start", 600))
        b, _ = R.button(th, R.W - 16 - 62, y0 + 11, "Close", w=62, h=28)
        o.append(b)
        y = y0 + 52
        o.append(text(x, y + 17, "Base value", 13, th["ctext2"], "start", 500))
        o.append(rect(x + 90, y, 90, 26, fill=th["btn"], rx=5, stroke=th["chrome_edge"]))
        o.append(text(x + 135, y + 18, c["value"], 13, th["ctext"], "middle", 500, mono=True))
        b, _ = R.button(th, x + 190, y, "Reset", h=26)
        o.append(b)
        y += 50
        # range bar: base (▲), depth per source (thin brackets), resulting range (thick), clamp
        o.append(text(x, y, "RESULTING RANGE", 11, th["ctext2"], "start", 600, extra='letter-spacing="0.8"'))
        tx, tw_ = x + 4, DRAWER_W - 44
        ty = y + 40
        col = th["sig"]["cv"]
        o.append(rect(tx, ty, tw_, 8, fill=th["chrome_edge"], rx=4))
        lo, hi = mod_span(rts)
        base = c["t"]
        for i, r in enumerate(rts):
            a = r["amount"]
            t0, t1 = (base + min(0, a), base + max(0, a)) if r["uni"] else (base - abs(a), base + abs(a))
            yy = ty - 9 - 6 * i
            ccol = th["ctext2"] if r["bypass"] else mix(col, "#ffffff", .25 * i)
            o.append(f'<path d="M{tx + max(0, t0) * tw_:.1f},{yy + 3} L{tx + max(0, t0) * tw_:.1f},{yy} L{tx + min(1, t1) * tw_:.1f},{yy} L{tx + min(1, t1) * tw_:.1f},{yy + 3}" fill="none" stroke="{ccol}" stroke-width="2"'
                     f'{" stroke-dasharray=" + chr(34) + "3 3" + chr(34) if r["bypass"] else ""}/>')
        if any(not r["bypass"] for r in rts):
            o.append(rect(tx + max(0, base + lo) * tw_, ty, (min(1, base + hi) - max(0, base + lo)) * tw_, 8, fill=col, rx=4))
        bxp = tx + base * tw_
        o.append(f'<polygon points="{bxp - 6},{ty + 20} {bxp + 6},{ty + 20} {bxp},{ty + 11}" fill="{th["ctext"]}"/>')
        s0, s1 = fmt(c["scale"], c["scale"][1]), fmt(c["scale"], c["scale"][2])
        o.append(text(tx, ty + 34, s0, 11, th["ctext2"], "start", 400, mono=True))
        o.append(text(tx + tw_, ty + 34, s1, 11, th["ctext2"], "end", 400, mono=True))
        o.append(text(min(max(bxp, tx + 50), tx + tw_ - 50), ty + 50, f"base {c['value']}", 11, th["ctext"], "middle", 500, mono=True))
        ty += 16
        rv = route_values(mods, rts[0], rts)
        res = "no active source" if not any(not r["bypass"] for r in rts) else f"{rv[1]} – {rv[2]}{' (clamped)' if rv[3] else ''}"
        o.append(text(x, ty + 56, f"Result {res}", 13, th["ctext"], "start", 500))
        o.append(text(x, ty + 74, "Thin bracket = depth of one source; bar = sum,", 11.5, th["ctext2"]))
        o.append(text(x, ty + 89, "clamped to the parameter range.", 11.5, th["ctext2"]))
        y = ty + 116
        o.append(text(x, y, f"MODULATION SOURCES · {len(rts)}", 11, th["ctext2"], "start", 600, extra='letter-spacing="0.8"'))
        y += 10
        for r in rts:
            y = card(y, r)
        b, _ = R.button(th, x, y, "+ Add source…", w=DRAWER_W - 32, h=32)
        o.append(b)
    b, _ = R.button(th, x, y1 - 48, "+ New route…", w=DRAWER_W - 32, h=34, primary=True)
    o.append(b)
    return "".join(o)


# --------------------------------------------------------------------------- scenes

def base(**kw):
    sc = dict(cables="all", drawer=None, selected=())
    sc.update(kw)
    return sc


def scene_svg(th, sc, mods, routes):
    W, H = R.W, R.H
    rack_w = W - (DRAWER_W if sc.get("drawer") else 0)
    pan = sc.get("pan", 0)
    total = max(m["x"] + m["w"] for m in mods.values()) + RACK_X
    body = R.rack_bg(th, mods, max(W, total) + pan) + rack_content(th, sc, mods, routes)
    over = [R.tooltip(th, *t) for t in sc.get("tooltips", ())]
    if sc.get("toast"):
        over.append(R.toast(th, *sc["toast"]))
    if sc.get("cursor"):
        over.append(R.cursor(*sc["cursor"]))
    if sc.get("scrollbar"):
        over.append(R.scrollbar(th, mods, pan, rack_w))
    for n in sc.get("screen_notes", ()):
        over.append(note(th, *n))
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">',
             R.defs(th), rect(0, 0, W, H, fill=th["rack"]),
             f'<svg x="0" y="0" width="{rack_w}" height="{H}" overflow="hidden">'
             f'<g transform="translate({-pan},0)">{body}</g>{"".join(over)}</svg>',
             R.topbar(th, sc)]
    if sc.get("drawer"):
        parts.append(drawer2(th, sc, mods, routes))
    parts.append(R.footer(th, sc))
    parts.append("</svg>")
    return "".join(parts)


def emit(name, svg, w=None, h=None):
    BUILD.mkdir(parents=True, exist_ok=True)
    OUT.mkdir(parents=True, exist_ok=True)
    sp = BUILD / f"{name}.svg"
    sp.write_text(svg)
    out = OUT / f"{name}.png"
    R.chrome_render(sp, out, w or R.W, h or R.H)
    return out


# reference patch, revision 2: LFO → Cutoff and LFO → Attack land on knobs, not CV jacks
REF_ROWS = [[("osc", "osc.va"), ("lfo", "lfo"), ("filter", "filter.svf")],
            [("midi", "midi.in"), ("env", "env.adsr"), ("vca", "vca"), ("out", "out")]]


def ref_routes(attack=.25, bypass=False, with_attack=True):
    rs = [route("C1", ("midi", "gate"), ("env", "gate")), route("C2", ("midi", "pitch"), ("osc", "pitch")),
          route("C3", ("osc", "out"), ("filter", "in")), route("C4", ("filter", "lp"), ("vca", "in")),
          route("C5", ("env", "out"), ("vca", "cv")),
          route("C6", ("lfo", "out"), ("filter", "cutoff_hz"), amount=.10),
          route("C7", ("vca", "out"), ("out", "left")), route("C8", ("vca", "out"), ("out", "right"))]
    if with_attack:
        rs.append(route("C9", ("lfo", "out"), ("env", "attack_ms"), amount=attack, bypass=bypass))
    return rs


def ref(lfo=None, extra_rows=None):
    return build(extra_rows or REF_ROWS, params={"lfo": lfo or {}})


def cap(s):
    return f"{s} · not an app screenshot · ◆ = conceptual control"


def story_frames(th):
    """LFO → ADSR Attack: drop on knob, amount, invert, bypass, hide, inspect, undo."""
    mods = ref()
    env = mods["env"]
    att = find_ctl(env, "attack_ms")
    ax, ay = env["x"] + att["cx"], env["y"] + att["cy"]
    hint = "Drop a cable on any knob to modulate it · Esc cancels"
    f = []
    f.append(("1  Drag from LFO Out. Every knob is a destination; hovering Attack shows the default.",
              base(selected=("lfo",), caption=cap("storyboard 1/8"), hint=hint,
                   drag={"from": ("lfo", "out"), "to": (ax + 4, ay + 26), "hover": ("env", "attack_ms")},
                   tooltips=[(ax + 34, ay - 6, ["ADSR · Attack", "Release to modulate (+25 %)", "Alt-release: +0 %"])],
                   cursor=(ax + 2, ay + 22)),
              ref_routes(with_attack=False)))
    f.append(("2  Dropped: small plug at 6 o'clock, range ring, value pill. Tooltip gives units.",
              base(selected=("lfo",), caption=cap("storyboard 2/8"), hint=hint,
                   tooltips=[(ax + 34, ay - 30, ["LFO → Attack  +25 %", "Base 8 ms · swings 0.45 – 142 ms"])]),
              ref_routes(.25)))
    f.append(("3  Drag the ring (not the knob) to set depth: +40 %. Knob body still sets the base.",
              base(selected=("lfo",), ring_drag=("env", "attack_ms"), caption=cap("storyboard 3/8"),
                   hint="Ring = depth of the selected source · knob = base value · Shift = fine",
                   tooltips=[(ax + 34, ay - 30, ["LFO → Attack  +40 %", "Base 8 ms · swings 0.16 – 400 ms"])],
                   cursor=(ax + 20, ay - 22)),
              ref_routes(.40)))
    f.append(("4  Route card: Invert flips the sign (−40 %); the peak dot moves to the low end.",
              base(selected=("lfo",), drawer="routing", focus_route="C9", caption=cap("storyboard 4/8"),
                   inspect=("env", "attack_ms"), hint="Amount, polarity and bypass belong to the route"),
              ref_routes(-.40)))
    f.append(("5  Bypass: cable dashed and grey, ring hollow, amount remembered.",
              base(selected=("lfo",), drawer="routing", focus_route="C9", caption=cap("storyboard 5/8"),
                   hint="Bypass keeps the route and its amount; Remove deletes it"),
              ref_routes(-.40, bypass=True)))
    f.append(("6  Cables hidden. Same layout; Attack keeps ring + pill + '← LFO' badge.",
              base(cables="hidden", selected=("lfo",), caption=cap("storyboard 6/8"),
                   hint="Hidden: badges and rings show the same routes"),
              ref_routes(-.40, bypass=True)))
    f.append(("7  Click Attack's ring: inspector shows base, the (bypassed, dashed) depth, and no result.",
              base(cables="hidden", drawer="dest", dest=("env", "attack_ms"), selected=("env",),
                   inspect=("env", "attack_ms"), caption=cap("storyboard 7/8"),
                   hint="Inspector: base ▲, depth brackets, result bar"),
              ref_routes(-.40, bypass=True)))
    f.append(("8  Undo reverts Bypass (hide/inspect are views, not undo steps). Result bar returns.",
              base(cables="hidden", drawer="dest", dest=("env", "attack_ms"), selected=("env",),
                   inspect=("env", "attack_ms"), pressed="Undo", caption=cap("storyboard 8/8"),
                   toast=("Undid: Bypass LFO → Attack", "Redo")),
              ref_routes(-.40)))
    return [(c, s, mods, resolve(mods, r)) for c, s, r in f]


# ---- sheets

def sheet_svg(th, title, subtitle, body, w=None, h=None):
    w, h = w or R.W, h or R.H
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">'
            + R.defs(th) + rect(0, 0, w, h, fill=th["rack"])
            + text(24, 34, title, 20, th["ctext"], "start", 600)
            + text(24, 56, subtitle, 13, th["ctext2"])
            + body + "</svg>")


def cell_head(th, x, y, title, lines):
    o = [text(x, y + 18, title, 15, th["ctext"], "start", 600)]
    for i, s in enumerate(lines):
        o.append(text(x, y + 37 + i * 16, s, 12.5, th["ctext2"]))
    return "".join(o)


def lfo_sheet(th):
    o = []
    y = 130
    specs = [("Compact · default", "Rate + Waveform are declared primary", {}),
             ("Compact · user-chosen", "Amplitude + Offset pinned: face grows 7u → 8u",
              dict(primary=["rate_hz", "amp", "offset", "waveform"])),
             ("Expanded in place", "Face unchanged at left; advanced area appended",
              dict(state="expanded", fade_ms=1200, phase=90))]
    x = 24
    for title, sub, p in specs:
        mods = build([[("lfo", "lfo")]], params={"lfo": p}, x0=x, y0=y)
        m = mods["lfo"]
        o.append(cell_head(th, x, y - 64, title, [sub]))
        o.append(rack_content(th, base(), mods, []))
        x += m["w"] + 40
    y2 = y + PANEL_H + 80
    mods = build([[("lfo", "lfo")]], params={"lfo": dict(state="expanded", primary=["rate_hz", "amp", "offset", "waveform"], phase=90)},
                 x0=24, y0=y2)
    o.append(cell_head(th, 24, y2 - 64, "Choosing primary controls",
                       ["Context menu › Choose primary… Filled pin = on the face. Done collapses to the new face."]))
    o.append(rack_content(th, base(choose=("lfo",), just_pinned=("amp", "offset")), mods, []))
    nx = 24 + mods["lfo"]["w"] + 40
    lines = ["RULES (proposal)",
             "• Face = primary controls, in the module's declared order.",
             "• Expanding never moves a face control; advanced controls",
             "  append to the right. Name stays centred over the face.",
             "• Jacks are always on the face: a cable never disappears.",
             "• Face width grows with primary knobs: ≤2 → 7u, 3 → 8u, 4 → 10u.",
             "• '+9' = number of controls not on the face.",
             "• Primary choice is saved per module instance, in the patch.",
             "  'Reset to module default' in the context menu.",
             "",
             "◆ CONCEPTUAL (engine today: Rate, Waveform only)",
             "Fine ±10 % · Phase · Fade-in · Amplitude · Offset · Sync",
             "(needs a clock) · Polarity · Trigger · Scope · Trig input.",
             "Rate and Waveform exist; their modulation needs engine work."]
    for i, s in enumerate(lines):
        bold = s.isupper() or s.startswith("◆")
        o.append(text(nx, y2 + 6 + i * 21, s, 13, th["ctext"] if bold else th["ctext2"], "start", 600 if bold else 400))
    return sheet_svg(th, "Rich LFO — compact, expanded, primary controls",
                     "Concept at 100 % scale. Controls marked ◆ do not exist in kabl's LFO; they are proposals.",
                     "".join(o))


def ensemble_art(m, dark):
    """Procedural PLACEHOLDER for an illustrated skin (no image generator was available)."""
    w, h = m["w"], PANEL_H
    if dark:
        sky = ('<linearGradient id="skyD" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#1b1840"/>'
               '<stop offset=".6" stop-color="#2d2a5e"/><stop offset="1" stop-color="#3b2f4f"/></linearGradient>')
        o = [f"<defs>{sky}</defs>", rect(0, 0, w, h, fill="url(#skyD)")]
        for i in range(26):
            o.append(circle((i * 7919 % 211) * w / 211, 14 + (i * 104729 % 199), 1.1 + (i % 3) * .5,
                            fill="#fff6d8", opacity=.8))
        o.append(f'<path d="M-10,150 C60,110 120,190 230,120" fill="none" stroke="#5de0c4" stroke-width="18" opacity=".22"/>')
        o.append(circle(w - 48, 64, 22, fill="#f3ead0"))
        o.append(circle(w - 38, 58, 20, fill="#26235a"))
        hills = ["#221f3d", "#18162c", "#100f1e"]
    else:
        sky = ('<linearGradient id="skyL" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#f6c89b"/>'
               '<stop offset=".6" stop-color="#eea08e"/><stop offset="1" stop-color="#c9829b"/></linearGradient>')
        o = [f"<defs>{sky}</defs>", rect(0, 0, w, h, fill="url(#skyL)")]
        o.append(circle(w / 2 + 30, 150, 42, fill="#fbe7b5", opacity=.9))
        for i in range(3):
            o.append(f'<path d="M-10,{70 + i * 22} C60,{55 + i * 22} 120,{85 + i * 22} 230,{62 + i * 22}" fill="none" stroke="#fff3dc" stroke-width="3" opacity=".45"/>')
        hills = ["#b76e86", "#8e5a7c", "#5f4a6e"]
    for i, col in enumerate(hills):
        yy = 190 + i * 45
        d = f"M0,{h} L0,{yy} C{w * .3},{yy - 40} {w * .6},{yy + 20} {w},{yy - 25} L{w},{h} Z"
        o.append(f'<path d="{d}" fill="{col}"/>')
    return "".join(o)


_ORIG_BG = R.c_background
_SKIN_DARK = [False]


def c_background2(m):
    return ensemble_art(m, _SKIN_DARK[0]) if m["kind"] == "user.ensemble" else _ORIG_BG(m)


R.c_background = c_background2


def skin_sheet():
    """Same user module in light and dark, beside core modules, plus its art layer alone."""
    o = []
    th0 = A_LIGHT
    for row, (core, skin, label) in enumerate(((A_LIGHT, SKIN_LIGHT, "Light theme"),
                                               (A_DARK, SKIN_DARK, "Dark theme"))):
        y = 130 + row * (PANEL_H + 84)
        _SKIN_DARK[0] = row == 1
        mods = build([[("filter", "filter.svf"), ("ens", "user.ensemble")]], x0=24, y0=y)
        o.append(f'<g>{rect(0, y - 70, 1440, PANEL_H + 84, fill=core["rack"])}</g>')
        o.append(cell_head(core, 24, y - 64, f"{label}: core module (A) + illustrated user module",
                           ["UI draws every control, label and value on theme-coloured plates over the art."]))
        o.append('<defs><clipPath id="clip-ens"><rect width="210" height="340" rx="2"/></clipPath></defs>')
        o.append(f'<g>{R.draw_module(core, mods["filter"], dict(mods={}, rings={}, badges={}, selected=()))}</g>')
        o.append(R.draw_module(skin, mods["ens"], dict(mods={}, rings={}, badges={}, selected=())))
        # the art layer alone (what a skin author supplies)
        ax = 24 + 420 + 60
        o.append(cell_head(core, ax, y - 64, "Art layer only", ["Skin file: no text, no controls."]))
        o.append(f'<g transform="translate({ax},{y})"><clipPath id="clipart{row}"><rect width="210" height="{PANEL_H}" rx="2"/></clipPath>'
                 f'<g clip-path="url(#clipart{row})">{ensemble_art(dict(w=210), row == 1)}</g></g>')
        # rejected: labels straight on art
        bx = ax + 210 + 60
        o.append(cell_head(core, bx, y - 64, "Rejected: labels straight on art", ["Contrast depends on the picture."]))
        bare = dict(skin, panel="#00000000")
        mm = dict(mods["ens"], x=bx)
        o.append(R.draw_module(dict(bare, art="illustrated"), mm, dict(mods={}, rings={}, badges={}, selected=()))
                 .replace(f'fill="{bare["panel"]}" rx="8" opacity="0.96"', 'fill="none" rx="8" opacity="0"'))
        tx = bx + 210 + 50
        notes = ["Manifest (proposal): light.png + dark.png,",
                 "accent colour, optional 'busy zones'.",
                 "Plates follow the theme, not the art.",
                 "Missing dark.png: light art dimmed 35 %",
                 "under dark plates.",
                 "",
                 "PLACEHOLDER ART: procedural vectors.",
                 "No image generator was available; not",
                 "finished art. Prompts: IMAGEGEN_PROMPTS."]
        for i, s in enumerate(notes):
            o.append(text(tx, y + 20 + i * 20, s, 13, core["ctext"] if s.isupper() or s.startswith("PLACE") else core["ctext2"],
                          "start", 600 if s.startswith("PLACE") else 400))
    _SKIN_DARK[0] = False
    return sheet_svg(th0, "Illustrated user module — light and dark variants",
                     "Concept. 'Night Ensemble' is a hypothetical user module. Art is a placeholder, not finished artwork.",
                     "".join(o))


def cases_sheet(th):
    """The difficult cases, each at 100 %."""
    o = []
    cw, ch = 464, 412
    cells = []
    # 1 multiple sources on one knob
    mods = build([[("filter", "filter.svf")]], x0=0, y0=0)
    rs = [route("M1", ("lfo", "out"), ("filter", "cutoff_hz"), .12, typ="cv", src_pt=(-30, 150), src_name="LFO"),
          route("M2", ("env", "out"), ("filter", "cutoff_hz"), .22, uni=True, typ="cv", src_pt=(-30, 185), src_name="ADSR")]
    rv = route_values(mods, rs[0], rs)
    cells.append(("1  Several sources on one knob",
                  ["Sum in knob-travel space, then clamp. Outer ring = result;", "inspecting shows one lane per source (LFO ±12 %, ADSR +22 %)."],
                  mods, rs, dict(inspect=("filter", "cutoff_hz")),
                  ["Plugs fan out at 6 o'clock", "(max 3, then one '+N' plug).", "", "Cutoff 1.20 kHz", "LFO ±12 %  ADSR +22 %", f"result {rv[1]} – {rv[2]}",
                   "", "Drawer lists each source", "with its own amount,", "Invert and Bypass."]))
    # 2 discrete
    mods = build([[("osc", "osc.va")]], x0=0, y0=0)
    rs = [route("D1", ("lfo", "out"), ("osc", "waveform"), .34, typ="cv", src_pt=(230, 110), src_name="S&H LFO")]
    cells.append(("2  Modulating a discrete control",
                  ["Amount moves whole options (±1 step here); result snaps to the", "nearest option, with hysteresis so it never chatters."],
                  mods, rs, {},
                  ["Bracket = reachable options", "(TRI · SAW · SQR).", "Selected fill = base (SAW).", "", "Bypass/Invert as for knobs.",
                   "On/off switches: threshold", "at half travel.", "", "Recommendation: change", "applies at next block."]))
    # 3 hidden advanced param
    mods = build([[("lfo", "lfo")]], x0=0, y0=0)
    rs = [route("H1", ("env", "out"), ("lfo", "amp"), .6, uni=True, typ="cv", src_pt=(230, 70), src_name="ADSR")]
    cells.append(("3  Modulated control hidden by collapse",
                  ["The cable docks on the '+9' button, which gets the route ring.", "Clicking it expands the module and flashes the target."],
                  mods, rs, dict(tooltips_local=[(14, 56, ["Amplitude ← ADSR  +60 %", "Hidden: click to expand"])]),
                  ["Hidden mode: '◆ 1 hidden'", "badge under the button.", "", "Never silently drop the", "route: the cable, ring and",
                   "drawer row all still exist.", "", "Delayed vibrato: ADSR", "opens LFO amplitude."]))
    # 4 modulating the LFO itself
    mods = build([[("lfoB", "lfo"), ("lfoA", "lfo")]], names={"lfoA": "LFO 1", "lfoB": "LFO 2"},
                 params={"lfoB": dict(rate_hz=.07), "lfoA": dict(rate_hz=2.0)}, x0=0, y0=0)
    rs = [route("L1", ("lfoB", "out"), ("lfoA", "rate_hz"), .12, typ="cv")]
    cells.append(("4  Modulating an LFO with another LFO",
                  ["LFO 2 (0.07 Hz) sweeps LFO 1's Rate ±12 % of travel = ×0.33…×3.", "Self-modulation (own Out → own Rate) is allowed: 1-block delay."],
                  mods, rs, dict(selected=("lfoB",)), None))
    # 5 cable over a control
    mods = build([[("vca", "vca")]], x0=0, y0=0)
    rs = [route("X1", ("osc", "out"), ("vca", "in"), typ="audio", src_pt=(250, 40), src_name="Osc"),
          route("X2", ("env", "out"), ("vca", "cv"), typ="cv", src_pt=(-40, 80), src_name="ADSR")]
    cells.append(("5  A cable crossing a control",
                  ["The control wins hit-testing. Hovering a covered control fades", "the crossing cables to 25 % (x-ray) until the pointer leaves."],
                  mods, rs, dict(xray=("X1",), hover_knob=("vca", "gain")),
                  ["Cable selectable at its", "plugs and over bare rack.", "", "Focus mode fades all", "unrelated cables to 13 %.",
                   "", "Hidden mode removes the", "problem entirely."]))
    # 6 skin behind labels
    _SKIN_DARK[0] = True
    mods = build([[("ens", "user.ensemble")]], x0=0, y0=0)
    cells.append(("6  Illustrated skin behind labels",
                  ["Art never carries text. Every label/value sits on a theme plate;", f"worst case ink/plate over art: {min(skin_plate_contrast().values())}:1 (measured)."],
                  mods, [], dict(skin=SKIN_DARK),
                  ["Plate colour comes from", "the theme (dark here).", "", "Art may be busy; text", "contrast never depends", "on it.",
                   "", "Art is a placeholder."]))
    for i, (title, lines, mods, rs, extra, side) in enumerate(cells):
        cx, cy = 24 + (i % 3) * (cw + 8), 70 + (i // 3) * ch
        o.append(cell_head(th, cx, cy, title, lines))
        sub = base(**{k_: v for k_, v in extra.items() if k_ not in ("skin", "tooltips_local", "hover_knob")})
        body = rack_content(extra.get("skin", th), sub, mods, resolve(mods, rs))
        if "hover_knob" in extra:
            m = mods["vca"]
            c = find_ctl(m, "gain")
            body += circle(m["x"] + c["cx"], m["y"] + c["cy"], c["r"] + 2, stroke=th["sel"], sw=3)
            body += R.cursor(m["x"] + c["cx"] + 6, m["y"] + c["cy"] + 4)
        for tt in extra.get("tooltips_local", ()):
            body += R.tooltip(th, *tt)
        clip = f'<clipPath id="cell{i}"><rect x="-24" y="-8" width="{cw}" height="{PANEL_H + 16}"/></clipPath>'
        o.append(f'<g transform="translate({cx + 24},{cy + 64})">{clip}<g clip-path="url(#cell{i})">{body}</g></g>')
        if side:
            for j, s in enumerate(side):
                o.append(text(cx + 24 + 210 + 34, cy + 90 + j * 19, s, 12.5, th["ctext2"]))
    _SKIN_DARK[0] = False
    return sheet_svg(th, "Difficult cases — proposed rules",
                     "Concept at 100 %. Recommendations, not owner decisions. Stub cables enter from off-cell sources.",
                     "".join(o))


def crowded():
    rows = [[("midi", "midi.in"), ("osc1", "osc.va"), ("osc2", "osc.va"), ("ring", "ringmod"),
             ("mix", "mixer"), ("filter", "filter.svf"), ("filt2", "filter.svf")],
            [("lfo1", "lfo"), ("lfo2", "lfo"), ("env1", "env.adsr"), ("env2", "env.adsr"),
             ("vca1", "vca"), ("vca2", "vca"), ("out", "out")]]
    names = {"osc1": "VA Oscillator 1", "osc2": "VA Oscillator 2", "filt2": "SVF Filter 2",
             "lfo1": "LFO 1", "lfo2": "LFO 2", "env1": "ADSR Amp", "env2": "ADSR Filter",
             "vca1": "VCA 1", "vca2": "VCA 2"}
    params = {"osc2": dict(waveform=3, base_hz=262.9, base_hz_s="262.9 Hz"),
              "lfo2": dict(waveform=0, rate_hz=.23), "env2": dict(adsr=(120, 900, .3, 1500))}
    mods = build(rows, names, params)
    rs = [route("K1", ("midi", "pitch"), ("osc1", "pitch")), route("K2", ("midi", "pitch"), ("osc2", "pitch")),
          route("K3", ("midi", "gate"), ("env1", "gate")), route("K4", ("midi", "gate"), ("env2", "gate")),
          route("K5", ("osc1", "out"), ("mix", "in1")), route("K6", ("osc2", "out"), ("mix", "in2")),
          route("K7", ("osc1", "out"), ("ring", "a")), route("K8", ("osc2", "out"), ("ring", "b")),
          route("K9", ("ring", "out"), ("mix", "in3")), route("K10", ("mix", "out"), ("filter", "in")),
          route("K11", ("mix", "out"), ("filt2", "in")),
          route("K12", ("lfo1", "out"), ("filter", "cutoff_hz"), .12),
          route("K13", ("lfo2", "out"), ("filt2", "cutoff_hz"), .2),
          route("K14", ("env2", "out"), ("filter", "cutoff_hz"), .22, uni=True),
          route("K15", ("filter", "lp"), ("vca1", "in")), route("K16", ("filt2", "bp"), ("vca2", "in")),
          route("K17", ("env1", "out"), ("vca1", "cv")), route("K18", ("env1", "out"), ("vca2", "cv")),
          route("K19", ("vca1", "out"), ("out", "left")), route("K20", ("vca2", "out"), ("out", "right")),
          route("K21", ("lfo1", "out"), ("filt2", "resonance"), .15),
          route("K22", ("lfo2", "out"), ("lfo1", "rate_hz"), .1)]
    return mods, resolve(mods, rs)


# --------------------------------------------------------------------------- QA

def text_boxes2(m):
    out = R.text_boxes(m)
    for c in m["controls"]:
        if c["type"] == "lfo_out":
            out.append((c["x"], c["y"], c["x"] + c["w"], c["y"] + c["h"], "output preview"))
    if m["kind"] == "lfo":
        fw = m["head_w"]
        out.append((fw - 48, 13, fw - 12, 41, "+N button") if m["p"].get("state") != "expanded"
                   else (m["w"] - 66, 13, m["w"] - 16, 41, "Less button"))
    return out


def hit_regions2(mods, choose=()):
    regs = R.hit_regions(mods)
    for m in mods.values():
        if m["kind"] == "lfo":
            fw = m["head_w"]
            if m["p"].get("state") == "expanded":
                regs.append((f"{m['id']}.less", m["x"] + m["w"] - 66, m["y"] + 13, 50, 28))
            else:
                regs.append((f"{m['id']}.more", m["x"] + fw - 48, m["y"] + 13, 36, 28))
            if m["id"] in choose:
                for c in m["controls"]:
                    if c.get("id") in LFO_DEF:
                        x0, _, yb = label_box(c)
                        dy = 2 if c["type"] == "knob" else -2  # clear of header / segments
                        regs.append((f"{m['id']}.pin.{c['id']}", m["x"] + x0 - 25, m["y"] + yb - 18.5 + dy, 28, 28))
    return regs


def bez(a, b, n=80):
    d, c1, c2 = R.cable_path(a, b)
    pts = []
    for i in range(n + 1):
        t = i / n
        u = 1 - t
        pts.append(tuple(u ** 3 * a[j] + 3 * u * u * t * c1[j] + 3 * u * t * t * c2[j] + t ** 3 * b[j] for j in (0, 1)))
    return pts


def cable_obstruction(mods, routes):
    """Cable centre-lines (half-width 3.75 px) crossing essential text or a knob, All mode."""
    hits = []
    by_dst = {}
    for r in routes:
        if is_param(mods, r):
            by_dst.setdefault(r["dst"], []).append(r)
    for r in routes:
        a = r["src_pt"] or R.jack_pos(mods, r["src"])
        if is_param(mods, r):
            lst = by_dst[r["dst"]]
            b = socket(mods, r, lst.index(r), len(lst))
        else:
            b = R.jack_pos(mods, r["dst"])
        pts = bez(a, b)[4:-4]  # skip plugs/boots at the ends
        for m in mods.values():
            for x0, y0, x1, y1, lab in R.text_boxes(m):
                X0, Y0, X1, Y1 = m["x"] + x0 - 3.75, m["y"] + y0 - 3.75, m["x"] + x1 + 3.75, m["y"] + y1 + 3.75
                if lab.endswith(" options"):
                    continue
                own = find_ctl(m, r["dst"][1]) if m["id"] == r["dst"][0] else None
                if own and lab in (own.get("label"), own.get("value")):
                    continue  # the value pill is drawn above its own mod lead
                if any(X0 <= px <= X1 and Y0 <= py <= Y1 for px, py in pts):
                    hits.append(f"{r['id']} crosses {m['id']} label '{lab}'")
            for c in m["controls"]:
                if c["type"] == "knob":
                    cx, cy = m["x"] + c["cx"], m["y"] + c["cy"]
                    if (m["id"], c["id"]) == r["dst"]:
                        continue
                    if any(math.hypot(px - cx, py - cy) < c["r"] + 3.75 for px, py in pts):
                        hits.append(f"{r['id']} crosses {m['id']} knob '{c['label']}'")
    return sorted(set(hits))


def contrast_table(th):
    C = R.contrast
    t = {"panel label": C(th["ink"], th["panel"]), "kind line": C(th["ink2"], th["panel"]),
         "plate label": C(th["plate_ink"], th["plate"]), "selector on": C(th["seg_on_text"], th["seg_on"]),
         "selector off": C(th["ink"], th["seg_bg"]), "chrome text": C(th["ctext"], th["chrome"]),
         "chrome secondary": C(th["ctext2"], th["chrome"]), "button": C(th["ctext"], th["btn"]),
         "active button": C(th["btn_on_text"], th["btn_on"]), "display trace (non-text)": C(th["display_ink"], th["display"]),
         "selection vs panel (non-text)": C(th["sel"], th["panel"]), "selection vs rack (non-text)": C(th["sel"], th["rack"]),
         "concept mark vs panel (annotation)": C(th["concept"], th["panel"])}
    for s in ("audio", "cv", "gate"):
        col, edge = th["sig"][s], mix(th["sig"][s], "#000000", .45)
        t[f"{s} two-tone best vs panel (non-text)"] = max(C(col, th["panel"]), C(edge, th["panel"]))
        t[f"{s} two-tone best vs plate (non-text)"] = max(C(col, th["plate"]), C(edge, th["plate"]))
    cv = th["sig"]["cv"]
    t["mod ring two-tone best vs panel (non-text)"] = max(C(cv, th["panel"]), C(mix(cv, "#000000", .45), th["panel"]))
    t["value pill text"] = C(th["ink"], th["panel"])
    return {k_: round(v, 2) for k_, v in t.items()}


def skin_plate_contrast():
    """Plate at 96 % opacity over the darkest/lightest placeholder-art colour: worst case."""
    arts = {"skin-light": ["#f6c89b", "#5f4a6e", "#fbe7b5", "#b76e86"],
            "skin-dark": ["#1b1840", "#fff6d8", "#5de0c4", "#100f1e"]}
    res = {}
    for key, th in (("skin-light", SKIN_LIGHT), ("skin-dark", SKIN_DARK)):
        worst = min(R.contrast(th["ink"], mix(a, th["panel"], .96)) for a in arts[key])
        res[key] = round(worst, 2)
    return res


def qa2():
    viewport(1440, 900)
    res = {"contrast": {k_: contrast_table(t) for k_, t in (("a-light", A_LIGHT), ("a-dark", A_DARK))},
           "skin_plate_worst_contrast": skin_plate_contrast()}
    checks = {
        "reference": (ref(), ()),
        "lfo-expanded": (ref(dict(state="expanded")), ()),
        "lfo-choose": (ref(dict(state="expanded", primary=["rate_hz", "amp", "offset", "waveform"])), ("lfo",)),
        "lfo-custom": (ref(dict(primary=["rate_hz", "amp", "offset", "waveform"])), ()),
        "crowded": (crowded()[0], ()),
    }
    for name, (mods, choose) in checks.items():
        regs = hit_regions2(mods, choose)
        boxes = [(r[1], r[2], r[1] + r[3], r[2] + r[4], r[0]) for r in regs]
        text_issues = []
        for m in mods.values():
            tb = text_boxes2(m)
            for b in tb:
                if b[0] < 4 or b[2] > m["w"] - 4:
                    text_issues.append(f"{m['id']}: '{b[4]}' exceeds panel edge")
            for i, a in enumerate(tb):
                for b in tb[i + 1:]:
                    if R.overlaps(a, b, gap=4):
                        text_issues.append(f"{m['id']}: '{a[4]}' overlaps '{b[4]}'")
        res[name] = {"hit_regions": len(regs), "min_w_h": [min(r[3] for r in regs), min(r[4] for r in regs)],
                     # the +N/Less button sits inside the header drag zone and wins hit-testing there
                     "overlapping_hits": [(a[4], b[4]) for i, a in enumerate(boxes) for b in boxes[i + 1:]
                                          if R.overlaps(a, b, gap=-.01)
                                          and {a[4].split(".")[1], b[4].split(".")[1]} not in ({"header", "more"}, {"header", "less"})],
                     "text": text_issues}
    res["cable_obstruction"] = {"reference": cable_obstruction(ref(), resolve(ref(), ref_routes())),
                                "crowded": cable_obstruction(*crowded())}
    viewport(1280, 800)
    mods = ref(dict(state="expanded"))
    view_w = 1280 - DRAWER_W
    total = max(m["x"] + m["w"] for m in mods.values()) + RACK_X
    lfo = mods["lfo"]
    res["min_window"] = {"rack_view_width_with_drawer": view_w, "row_width_lfo_expanded": total,
                         "pan_needed_to_show_whole_lfo": max(0, lfo["x"] + lfo["w"] + 12 - view_w),
                         "lfo_expanded_width": lfo["w"], "fits_view": lfo["w"] + 24 <= view_w}
    return res


# --------------------------------------------------------------------------- main

def main():
    if sys.argv[1:] == ["--qa"]:
        print(json.dumps(qa2(), indent=1))
        return
    only = set(sys.argv[1:])

    def want(n):
        return not only or n in only

    viewport(1440, 900)
    imgs = {}
    for th in (A_LIGHT, A_DARK):
        mods = ref()
        rs = resolve(mods, ref_routes())
        tag = th["key"]
        if want("ref"):
            imgs[f"ref-{tag}-all"] = emit(f"ref-{tag}-all", scene_svg(th, base(
                caption=cap("reference patch, 1440×900")), mods, rs))
            imgs[f"ref-{tag}-hidden"] = emit(f"ref-{tag}-hidden", scene_svg(th, base(
                cables="hidden", drawer="routing", selected=("lfo",), focus_route="C9",
                caption=cap("reference patch, cables hidden"),
                hint="Hidden: rings, pills and badges show the same routes as the cables"), mods, rs))
    th = A_LIGHT
    if want("story"):
        frames = []
        for i, (c, sc, mods, rs) in enumerate(story_frames(th)):
            frames.append((emit(f"story-{i + 1}", scene_svg(th, sc, mods, rs)), c))
        R.W, R.H = 1440, 900
        board(frames, "Storyboard — LFO → ADSR Attack by dropping a cable on the knob (A-light, 1440×900)",
              "Concept frames. Knob modulation, per-route amount/invert/bypass and op grouping are not built.",
              OUT / "storyboard.png")
    if want("lfo"):
        emit("lfo-sheet", lfo_sheet(th))
        mods = ref(dict(state="expanded", phase=90))
        rs = resolve(mods, ref_routes())
        f = mods["filter"]
        emit("lfo-expanded-rack", scene_svg(th, base(
            selected=("lfo",), caption=cap("LFO expanded in place, 1440×900"),
            hint="Expanding pushes row neighbours right; cables follow their jacks",
            screen_notes=[(f["x"] + 16, f["y"] + PANEL_H + 16, ["Filter moved +13u (390 px)", "Row 2 unchanged; cables re-route"])]),
            mods, rs))
        emit("lfo-expanded-focus", scene_svg(th, base(
            cables="focus", selected=("lfo",), caption=cap("LFO expanded, Focus mode"),
            hint="Mitigation: Focus fades cables that only pass over the expanded module"), mods, rs))
    if want("cases"):
        emit("cases-sheet", cases_sheet(th))
    if want("skin"):
        emit("skin-sheet", skin_sheet())
    if want("crowded"):
        mods, rs = crowded()
        emit("crowded-all", scene_svg(th, base(selected=("filter",), patch_name="Dual-voice sketch",
                                                caption=cap("crowded: 14 modules, 22 routes, All"),
                                                hint="All mode at density: Focus or Hidden recommended"), mods, rs))
        emit("crowded-hidden", scene_svg(A_DARK, base(cables="hidden", selected=("filter",), inspect=("filter", "cutoff_hz"),
                                                       drawer="dest", dest=("filter", "cutoff_hz"),
                                                       patch_name="Dual-voice sketch",
                                                       caption=cap("crowded, hidden, Cutoff inspected")), mods, rs))
    if want("min"):
        viewport(1280, 800)
        mods = ref()
        rs = resolve(mods, ref_routes())
        emit("min-1280-hidden", scene_svg(th, base(cables="hidden", drawer="routing", selected=("lfo",), focus_route="C9",
                                                   caption=cap("minimum window 1280×800, drawer open")), mods, rs))
        mods = ref(dict(state="expanded", phase=90))
        rs = resolve(mods, ref_routes())
        lfo = mods["lfo"]
        pan = max(0, lfo["x"] + lfo["w"] + 12 - (1280 - DRAWER_W))
        emit("min-1280-expanded", scene_svg(th, base(selected=("lfo",), drawer="routing", focus_route="C9", pan=pan,
                                                     scrollbar=True, caption=cap("1280×800, LFO expanded, drawer open, panned"),
                                                     hint="Rack pans to keep the expanded module whole"), mods, rs))
        viewport(1440, 900)
    (BUILD / "qa2.json").write_text(json.dumps(qa2(), indent=1))


def board(frames, title, subtitle, out, cols=2, cell_w=700):
    R.board([p for p, _ in frames], cols, cell_w, [c for _, c in frames], title, out, subtitle)


if __name__ == "__main__":
    main()
