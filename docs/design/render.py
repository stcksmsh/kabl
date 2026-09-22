#!/usr/bin/env python3
"""Render kabl's physical-rack concept mockups (see BRIEF.md, INTERACTIONS.md).

Concept art only: nothing here is a screenshot of kabl-ui. Every direction is drawn from the same
layout model below, so module positions, values, and route identities cannot drift between
directions or cable presentations.

    python3 docs/design/render.py        # writes PNGs under docs/design/

Needs google-chrome (headless, SVG -> PNG), Pillow, and IBM Plex Sans/Mono installed.
Bespoke raster art is optional: if docs/design/art/c-illustrated/<kind>.png exists it replaces
direction C's procedural placeholder for that module (see IMAGEGEN_PROMPTS.md).
"""
import json
import math
import subprocess
import sys
from html import escape
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
BUILD = HERE / "build"  # intermediate SVGs, not committed
W, H = 1280, 800
UNIT, PANEL_H = 30, 340
TOP_H, FOOT_H = 48, 24
ROW_Y = [62, 426]  # panel tops; rails sit in the gaps
RACK_X = 24
DRAWER_W = 336
SANS, MONO = "IBM Plex Sans", "IBM Plex Mono"


# --------------------------------------------------------------------------- helpers

def tw(s, size, mono=False):
    """Estimated text width. Plex Sans averages ~0.55em; Plex Mono is 0.6em fixed."""
    return len(s) * size * (0.6 if mono else 0.55)


def ellipsize(s, size, max_w, mono=False):
    if tw(s, size, mono) <= max_w:
        return s
    while s and tw(s + "…", size, mono) > max_w:
        s = s[:-1]
    return s.rstrip() + "…"


def wrap2(s, size, max_w):
    """Header names: up to two lines, then ellipsis (long-name rule, INTERACTIONS.md)."""
    if tw(s, size) <= max_w:
        return [s]
    words, line1 = s.split(), ""
    while words and tw((line1 + " " + words[0]).strip(), size) <= max_w:
        line1 = (line1 + " " + words.pop(0)).strip()
    if not line1:  # a single word longer than the line
        return [ellipsize(s, size, max_w)]
    return [line1, ellipsize(" ".join(words), size, max_w)]


def text(x, y, s, size=13, fill="#000", anchor="start", weight=400, mono=False, opacity=1.0,
         extra=""):
    return (f'<text x="{x:.1f}" y="{y:.1f}" font-family="{MONO if mono else SANS}" '
            f'font-size="{size}" font-weight="{weight}" fill="{fill}" text-anchor="{anchor}" '
            f'opacity="{opacity}" {extra}>{escape(s)}</text>')


def rect(x, y, w, h, fill="none", rx=0, stroke="none", sw=1, opacity=1.0, extra=""):
    return (f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}" rx="{rx}" '
            f'fill="{fill}" stroke="{stroke}" stroke-width="{sw}" opacity="{opacity}" {extra}/>')


def circle(cx, cy, r, fill="none", stroke="none", sw=1, opacity=1.0, extra=""):
    return (f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r:.1f}" fill="{fill}" stroke="{stroke}" '
            f'stroke-width="{sw}" opacity="{opacity}" {extra}/>')


def polar(cx, cy, r, t):
    """Knob position t in [0,1] maps to -135..+135 degrees from 12 o'clock."""
    a = math.radians(-135 + 270 * t)
    return cx + r * math.sin(a), cy - r * math.cos(a)


def arc(cx, cy, r, t0, t1):
    x0, y0 = polar(cx, cy, r, t0)
    x1, y1 = polar(cx, cy, r, t1)
    large = 1 if (t1 - t0) * 270 > 180 else 0
    return f"M{x0:.1f},{y0:.1f} A{r},{r} 0 {large} 1 {x1:.1f},{y1:.1f}"


def mix(c1, c2, t):
    a = [int(c1[i:i + 2], 16) for i in (1, 3, 5)]
    b = [int(c2[i:i + 2], 16) for i in (1, 3, 5)]
    return "#" + "".join(f"{round(x + (y - x) * t):02x}" for x, y in zip(a, b))


def exp_t(v, lo, hi):
    return math.log(v / lo) / math.log(hi / lo)


# --------------------------------------------------------------------------- themes

SIG = {"audio": "#e0962b", "cv": "#23a597", "gate": "#8d67d6", "pitch": "#23a597"}
TYPE_LABEL = {"audio": "audio", "cv": "CV", "gate": "gate", "pitch": "pitch"}

THEMES = {
    "a-warm": dict(
        title="A — Warm studio hardware",
        app="#1f1e1c", chrome="#2a2826", chrome_edge="#3d3a36", ctext="#f1ece2", ctext2="#b9b1a4",
        btn="#3a3733", btn_on="#efe7d8", btn_on_text="#23201c",
        rack="#1b1a19", rail="#8f8b84", rail_hi="#bdb8ae", hole="#2a2826",
        panel="#e8e2d4", panel_edge="#b9b2a2", ink="#2a2520", ink2="#5d564c",
        plate="#2f2b27", plate_ink="#f1ece2",
        knob="#2a2826", knob_hi="#57524b", pointer="#f4efe6", skirt="#cfc7b6", tick="#6b6357",
        nut="#c3bfb6", nut_edge="#8b877f", hole_c="#121110",
        display="#23211e", display_ink="#efe6d2",
        seg_bg="#d8d1c1", seg_on="#2a2520", seg_on_text="#f4efe6",
        sel="#2a6ae0", texture="brushed", art=None,
        sig=SIG),
    "b-dark": dict(
        title="B — Dark precision instrument",
        app="#0b0c0d", chrome="#131517", chrome_edge="#25282c", ctext="#eceff2", ctext2="#a3a9b0",
        btn="#22262a", btn_on="#e9edf1", btn_on_text="#101214",
        rack="#08090a", rail="#3b3f45", rail_hi="#5a6068", hole="#0b0c0d",
        panel="#1b1d20", panel_edge="#3a3e44", ink="#eef1f4", ink2="#aab1b9",
        plate="#e6e9ec", plate_ink="#15171a",
        knob="#c9cdd2", knob_hi="#f4f6f8", pointer="#15171a", skirt="#2a2d31", tick="#8b939c",
        nut="#8e949b", nut_edge="#5a5f66", hole_c="#050505",
        display="#0d0f11", display_ink="#dff3ff",
        seg_bg="#262a2e", seg_on="#dff3ff", seg_on_text="#0b0d0f",
        sel="#5aa2ff", texture="matte", art=None, glow=True,
        sig={"audio": "#f0a640", "cv": "#35c2b1", "gate": "#a687ee", "pitch": "#35c2b1"}),
    "c-illustrated": dict(
        title="C — Illustrated pedals",
        app="#1d1b1f", chrome="#29262b", chrome_edge="#3d3940", ctext="#f5efe6", ctext2="#bdb3a8",
        btn="#3a3640", btn_on="#f5efe6", btn_on_text="#231f26",
        rack="#171518", rail="#8a857f", rail_hi="#b8b2aa", hole="#29262b",
        panel="#f4eee2", panel_edge="#a79e8e", ink="#2a2420", ink2="#5f574d",
        plate="#2c2622", plate_ink="#f7f1e7",
        knob="#2b2724", knob_hi="#5a534c", pointer="#f7f1e7", skirt="#e6dccb", tick="#6b6357",
        nut="#c3bfb6", nut_edge="#8b877f", hole_c="#121110",
        display="#2b2622", display_ink="#f7f1e7",
        seg_bg="#e3d9c7", seg_on="#2a2420", seg_on_text="#f7f1e7",
        sel="#2a6ae0", texture="paper", art="illustrated",
        sig=SIG),
}

# Procedural placeholder art for direction C (replaced by art/<dir>/<kind>.png when present).
ART_COLORS = {"midi.in": ("#e5b547", "#c9922a"), "osc.va": ("#e7775b", "#c95a41"),
              "filter.svf": ("#3f80a8", "#2c5f82"), "lfo": ("#40356b", "#6d5fa6"),
              "env.adsr": ("#dcb98a", "#b88e5a"), "vca": ("#7fa37a", "#5b7f57"),
              "out": ("#4d5d6d", "#33404d"), "mixer": ("#b0739b", "#8b5277"),
              "ringmod": ("#d98f4e", "#b06c30")}


# --------------------------------------------------------------------------- module templates
# Each returns (controls, ports) in panel-local coordinates. Only controls and ports that the
# real ModuleInfo declares are drawn (see BRIEF.md reference table).

def k(pid, label, value, t, size, cx, cy):
    return dict(type="knob", id=pid, label=label, value=value, t=t, r=22 if size == "L" else 17,
                cx=cx, cy=cy)


def sel(pid, label, options, idx, x, y, w):
    return dict(type="select", id=pid, label=label, options=options, idx=idx, x=x, y=y, w=w)


def port(pid, label, d, typ, cx, cy, lab="below"):
    return dict(id=pid, label=label, dir=d, type=typ, cx=cx, cy=cy, lab=lab)


def tpl_midi(w, p):
    return [dict(type="art", x=12, y=56, w=w - 24, h=112)], [
        port("pitch", "Pitch", "out", "pitch", 40, 206, "right"),
        port("velocity", "Velocity", "out", "cv", 40, 254, "right"),
        port("gate", "Gate", "out", "gate", 40, 302, "right")]


def tpl_osc(w, p):
    return [k("base_hz", "Frequency", p.get("base_hz_s", "261.6 Hz"),
              exp_t(p.get("base_hz", 261.63), 20, 20000), "L", w / 2, 112),
            sel("waveform", "Waveform", ["SIN", "TRI", "SAW", "SQR"], p.get("waveform", 2), 16,
                188, w - 32)], [
        port("pitch", "Pitch", "in", "pitch", 40, 272),
        port("sync", "Sync", "in", "gate", w / 2, 272),
        port("out", "Out", "out", "audio", w - 40, 272)]


def tpl_filter(w, p):
    return [k("cutoff_hz", "Cutoff", p.get("cutoff_s", "1.20 kHz"),
              exp_t(p.get("cutoff_hz", 1200), 20, 20000), "L", 66, 112),
            k("resonance", "Resonance", p.get("res_s", "0.35"), p.get("resonance", .35) / .95,
              "S", w - 56, 112)], [
        port("in", "In", "in", "audio", 40, 200),
        port("cutoff_cv", "Cutoff CV", "in", "cv", w / 2, 200),
        port("resonance_cv", p.get("res_cv_label", "Res CV"), "in", "cv", w - 40, 200),
        port("lp", "LP", "out", "audio", 40, 290),
        port("bp", "BP", "out", "audio", w / 2, 290),
        port("hp", "HP", "out", "audio", w - 40, 290)]


def tpl_lfo(w, p):
    return [k("rate_hz", "Rate", p.get("rate_s", "0.80 Hz"), exp_t(p.get("rate_hz", .8), .01, 100),
              "L", w / 2, 112),
            sel("waveform", "Waveform", ["SIN", "TRI", "SAW", "SQR", "S&H"], p.get("waveform", 1),
                14, 188, w - 28),
            dict(type="wave", wave=p.get("waveform", 1), x=16, y=234, w=94, h=64)], [
        port("out", "Out", "out", "cv", w - 44, 272)]


def tpl_env(w, p):
    a, d, s, r = p.get("adsr", (8, 240, .6, 420))
    xs = [36 + i * (w - 72) / 3 for i in range(4)]
    return [dict(type="env", adsr=(a, d, s, r), x=16, y=54, w=w - 32, h=62),
            k("attack_ms", "Attack", f"{a:g} ms", exp_t(a, .1, 1e4), "S", xs[0], 168),
            k("decay_ms", "Decay", f"{d:g} ms", exp_t(d, .1, 1e4), "S", xs[1], 168),
            k("sustain", "Sustain", f"{s:.2f}", s, "S", xs[2], 168),
            k("release_ms", "Release", f"{r:g} ms", exp_t(r, .1, 1e4), "S", xs[3], 168)], [
        port("gate", "Gate", "in", "gate", 60, 272),
        port("out", "Out", "out", "cv", w - 60, 272)]


def tpl_vca(w, p):
    # controls sit right of centre: the left edge is the lane where a patched "In" cable drops
    return [k("gain", "Gain", f"{p.get('gain', .85):.2f}", p.get("gain", .85), "L", w / 2 + 20, 112),
            sel("exponential", "Response", ["LIN", "EXP"], p.get("exp", 0), 62, 188, w - 80)], [
        port("in", "In", "in", "audio", 34, 272),
        port("cv", "CV", "in", "cv", w / 2, 272),
        port("out", "Out", "out", "audio", w - 34, 272)]


def tpl_out(w, p):
    return [dict(type="art", x=12, y=56, w=w - 24, h=112)], [
        port("left", "Left", "in", "audio", 40, 230, "right"),
        port("right", "Right", "in", "audio", 40, 278, "right")]


def tpl_mixer(w, p):
    xs = [30 + i * (w - 60) / 3 for i in range(4)]
    return [k("level", "Level", f"{p.get('level', .7):.2f}", p.get("level", .7), "L", w / 2, 112)], [
        port(f"in{i + 1}", f"In {i + 1}", "in", "audio", xs[i], 200) for i in range(4)] + [
        port("out", "Out", "out", "audio", w - 40, 290)]


def tpl_ringmod(w, p):
    return [dict(type="art", x=12, y=56, w=w - 24, h=100)], [
        port("a", "A", "in", "audio", 40, 200), port("b", "B", "in", "audio", w - 40, 200),
        port("out", "Out", "out", "audio", w / 2, 290)]


# ---- conceptual future modules (musical north star, PLAN.md). None of these exist in kabl.

def btn_ctl(pid, label, on, x, y, w):
    return dict(type="button", id=pid, label=label, on=on, x=x, y=y, w=w)


def tpl_clock(w, p):
    return [k("bpm", "Tempo", "112 BPM", .45, "L", w / 2, 112),
            btn_ctl("run", "Run", p.get("running", True), 16, 190, 70),
            btn_ctl("reset", "Reset", False, w - 86, 190, 70)], [
        port("clock", "Clock", "out", "gate", 50, 272), port("reset", "Reset", "out", "gate", w - 50, 272)]


def tpl_seq(w, p):
    return [dict(type="steps", pattern=p.get("pattern", [0] * 16), x=16, y=54, w=w - 32, h=62),
            k("length", "Length", "16", .5, "S", 50, 168),
            k("transpose", "Transpose", p.get("transpose", "+0 st"), p.get("tt", .5), "S", w / 2, 168),
            k("gate_len", "Gate", "50%", .5, "S", w - 50, 168)], [
        port("clock", "Clock", "in", "gate", 40, 272), port("reset", "Reset", "in", "gate", 100, 272),
        port("pitch", "Pitch", "out", "pitch", w - 100, 272), port("gate", "Gate", "out", "gate", w - 40, 272)]


def tpl_layers(w, p):
    la, lb = p.get("levels", (.8, .62))
    return [k("level_a", "Seq A level", f"{la:.2f}", la, "L", w / 4 + 10, 112),
            k("level_b", "Seq B level", f"{lb:.2f}", lb, "L", 3 * w / 4 - 10, 112),
            btn_ctl("mute_a", "Mute A", False, w / 4 - 25, 190, 70),
            btn_ctl("mute_b", "Mute B", False, 3 * w / 4 - 45, 190, 70)], [
        port("in_a", "In A", "in", "audio", 34, 272), port("gate_a", "Gate A", "in", "gate", 94, 272),
        port("in_b", "In B", "in", "audio", 154, 272), port("gate_b", "Gate B", "in", "gate", 214, 272),
        port("out", "Out", "out", "audio", w - 34, 272)]


def tpl_delay(w, p):
    return [k("time", "Time", "3/16", .4, "S", 40, 140), k("feedback", "Feedback", "0.55", .55, "S", w / 2, 140),
            k("mix", "Mix", "0.30", .3, "S", w - 40, 140)], [
        port("in", "In", "in", "audio", 40, 272), port("out", "Out", "out", "audio", w - 40, 272)]


def tpl_reverb(w, p):
    return [k("size", "Size", "0.80", .8, "S", 50, 140), k("mix", "Mix", "0.25", .25, "S", w - 50, 140)], [
        port("in", "In", "in", "audio", 34, 272), port("l", "L", "out", "audio", 100, 272),
        port("r", "R", "out", "audio", 150, 272)]


CONCEPT_KINDS = {"clock", "seq.step", "mix.layers", "fx.delay", "fx.reverb"}

KINDS = {  # kind: (default name, width_units, template, rate)
    "midi.in": ("MIDI In", 5, tpl_midi, "voice"), "osc.va": ("VA Oscillator", 7, tpl_osc, "voice"),
    "filter.svf": ("SVF Filter", 7, tpl_filter, "voice"), "lfo": ("LFO", 7, tpl_lfo, "voice"),
    "env.adsr": ("ADSR Envelope", 8, tpl_env, "voice"), "vca": ("VCA", 7, tpl_vca, "voice"),
    "out": ("Output", 5, tpl_out, "global"), "mixer": ("Mixer", 8, tpl_mixer, "voice"),
    "ringmod": ("Ring Modulator", 5, tpl_ringmod, "voice"),
    "clock": ("Clock", 6, tpl_clock, "global"), "seq.step": ("Sequencer", 10, tpl_seq, "global"),
    "mix.layers": ("Layers", 10, tpl_layers, "global"), "fx.delay": ("Delay", 7, tpl_delay, "global"),
    "fx.reverb": ("Reverb", 6, tpl_reverb, "global"),
}


def build_patch(rows, cables, names=None, params=None):
    """rows: list of lists of (id, kind). Returns (modules dict, cable list) in rack coords."""
    names, params = names or {}, params or {}
    mods = {}
    for r, row in enumerate(rows):
        x = RACK_X
        for mid, kind in row:
            name, wu, tpl, rate = KINDS[kind]
            w = wu * UNIT
            controls, ports = tpl(w, params.get(mid, {}))
            mods[mid] = dict(id=mid, kind=kind, name=names.get(mid, name), x=x, y=ROW_Y[r], w=w,
                             row=r, controls=controls, ports={p["id"]: p for p in ports},
                             rate=rate)
            x += w
    cabs = []
    for cid, a, b in cables:
        (ma, pa), (mb, pb) = a.split("."), b.split(".")
        typ = mods[ma]["ports"][pa]["type"]
        cabs.append(dict(id=cid, src=(ma, pa), dst=(mb, pb), type=typ))
    return mods, cabs


# Row order keeps the two inter-row cables (pitch up, filter-to-VCA down) clear of knobs.
REF_ROWS = [[("osc", "osc.va"), ("lfo", "lfo"), ("filter", "filter.svf")],
            [("midi", "midi.in"), ("env", "env.adsr"), ("vca", "vca"), ("out", "out")]]
REF_CABLES = [("C1", "midi.gate", "env.gate"), ("C2", "midi.pitch", "osc.pitch"),
              ("C3", "osc.out", "filter.in"), ("C4", "filter.lp", "vca.in"),
              ("C5", "env.out", "vca.cv"), ("C6", "lfo.out", "filter.cutoff_cv"),
              ("C7", "vca.out", "out.left"), ("C8", "vca.out", "out.right")]
PORT_NAMES = {"cutoff_cv": "Cutoff CV", "resonance_cv": "Res CV", "lp": "LP", "bp": "BP",
              "hp": "HP", "in": "in", "out": "out", "left": "L", "right": "R", "cv": "CV"}
# Badge short labels: proposed per-instance "short name" (auto: kind abbreviation + index,
# user-renamable). Badges never print a full module name.
SHORT = {"midi": "MIDI", "osc": "Osc", "filter": "Filter", "lfo": "LFO", "env": "ADSR",
         "vca": "VCA", "out": "Out", "osc1": "Osc 1", "osc2": "Osc 2", "ring": "Ring",
         "mix": "Mixer", "filt2": "Filter 2", "lfo1": "LFO 1", "lfo2": "LFO 2", "env1": "Amp Env",
         "env2": "Filt Env", "vca1": "VCA 1", "vca2": "VCA 2", "clock": "Clock", "seqA": "Seq A",
         "seqB": "Seq B", "oscA": "Osc A", "oscB": "Osc B", "layers": "Layers", "delay": "Delay",
         "reverb": "Reverb"}


# --------------------------------------------------------------------------- drawing: defs

def defs(th):
    return f"""<defs>
<filter id="brushed" x="0" y="0" width="1" height="1">
  <feTurbulence type="fractalNoise" baseFrequency="0.9 0.012" numOctaves="2" seed="7"/>
  <feColorMatrix type="matrix" values="1 0 0 0 0  1 0 0 0 0  1 0 0 0 0  0 0 0 0 0.10"/>
  <feComposite in2="SourceGraphic" operator="in"/></filter>
<filter id="matte" x="0" y="0" width="1" height="1">
  <feTurbulence type="fractalNoise" baseFrequency="0.7" numOctaves="2" seed="3"/>
  <feColorMatrix type="matrix" values="1 0 0 0 0  1 0 0 0 0  1 0 0 0 0  0 0 0 0 0.05"/>
  <feComposite in2="SourceGraphic" operator="in"/></filter>
<filter id="paper" x="0" y="0" width="1" height="1">
  <feTurbulence type="fractalNoise" baseFrequency="0.45" numOctaves="3" seed="11"/>
  <feColorMatrix type="matrix" values="1 0 0 0 0  1 0 0 0 0  1 0 0 0 0  0 0 0 0 0.08"/>
  <feComposite in2="SourceGraphic" operator="in"/></filter>
<filter id="blur3" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="3"/></filter>
<filter id="blur2" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="2"/></filter>
<filter id="glow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="2.5" result="b"/>
  <feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>
<radialGradient id="knobcap" cx="0.38" cy="0.32" r="0.8">
  <stop offset="0" stop-color="{th['knob_hi']}"/><stop offset="1" stop-color="{th['knob']}"/></radialGradient>
<radialGradient id="nut" cx="0.35" cy="0.3" r="0.9">
  <stop offset="0" stop-color="#ffffff" stop-opacity="0.9"/><stop offset="0.35" stop-color="{th['nut']}"/>
  <stop offset="1" stop-color="{th['nut_edge']}"/></radialGradient>
<linearGradient id="rail" x1="0" y1="0" x2="0" y2="1">
  <stop offset="0" stop-color="{th['rail_hi']}"/><stop offset="0.5" stop-color="{th['rail']}"/>
  <stop offset="1" stop-color="{mix(th['rail'], '#000000', .35)}"/></linearGradient>
</defs>"""


# --------------------------------------------------------------------------- drawing: modules

def draw_art(th, m, a):
    """Printed panel art zone. A/B: restrained print; C: expressive illustration placeholder."""
    x, y, w, h = a["x"], a["y"], a["w"], a["h"]
    ink = th["ink2"]
    out = []
    if m["kind"] == "midi.in":  # printed key strip
        kw = w / 7
        for i in range(7):
            out.append(rect(x + i * kw + 1, y + 34, kw - 2, 58, fill="none", stroke=ink, sw=1.2, rx=2,
                            opacity=.55))
        for i in (0, 1, 3, 4, 5):
            out.append(rect(x + (i + 1) * kw - kw * .3, y + 34, kw * .6, 34, fill=ink, rx=1.5,
                            opacity=.55))
    elif m["kind"] == "out":  # speaker grille print
        cx, cy = x + w / 2, y + h / 2 + 4
        for r in (12, 22, 32, 42):
            out.append(circle(cx, cy, r, stroke=ink, sw=1.2, opacity=.5))
    elif m["kind"] == "ringmod":
        cx, cy = x + w / 2, y + h / 2
        out.append(circle(cx - 12, cy, 26, stroke=ink, sw=1.4, opacity=.55))
        out.append(circle(cx + 12, cy, 26, stroke=ink, sw=1.4, opacity=.55))
    return "".join(out)


def c_background(m):
    """Direction C: full-panel illustration (procedural stand-in for bespoke raster art)."""
    w, h = m["w"], PANEL_H
    base, dark = ART_COLORS.get(m["kind"], ("#999999", "#777777"))
    light = mix(base, "#ffffff", .35)
    o = [rect(0, 0, w, h, fill=base)]
    kind = m["kind"]
    if kind == "osc.va":
        for i in range(9):
            d = " ".join(f"{'M' if j == 0 else 'L'}{j * w / 40:.1f},"
                         f"{40 + i * 36 + 14 * math.sin(j / 40 * 4 * math.pi + i * .6):.1f}"
                         for j in range(41))
            o.append(f'<path d="{d}" fill="none" stroke="{light}" stroke-width="5" opacity=".55"/>')
    elif kind == "filter.svf":
        for i, (yy, col) in enumerate([(120, light), (190, mix(base, dark, .3)), (260, dark)]):
            d = f"M0,{h} L0,{yy} " + " ".join(
                f"L{j * w / 8:.1f},{yy - (22 if j % 2 else 0) - 10 * math.sin(i + j):.1f}"
                for j in range(9)) + f" L{w},{h} Z"
            o.append(f'<path d="{d}" fill="{col}" opacity=".8"/>')
    elif kind == "lfo":
        for i in range(12):
            cx, cy = 20 + (i * 53) % w, 30 + (i * 97) % (h - 40)
            o.append(circle(cx, cy, 1.6, fill="#f8f1ff", opacity=.7))
        o.append(circle(w - 46, 58, 30, fill="#f4e7b8"))
        o.append(circle(w - 34, 50, 28, fill=base))
    elif kind == "env.adsr":
        for i, yy in enumerate((130, 200, 270)):
            d = f"M0,{h} L0,{yy} L{w * .15},{yy - 50} L{w * .35},{yy - 20} L{w * .7},{yy - 20} L{w},{yy + 10} L{w},{h} Z"
            o.append(f'<path d="{d}" fill="{mix(base, dark, i * .35)}" opacity=".85"/>')
    elif kind == "vca":
        cx, cy = w / 2, 150
        for i in range(16):
            a = i * math.pi / 8
            o.append(f'<line x1="{cx}" y1="{cy}" x2="{cx + 300 * math.cos(a):.1f}" y2="{cy + 300 * math.sin(a):.1f}" stroke="{light}" stroke-width="14" opacity=".35"/>')
    elif kind == "midi.in":
        for i in range(7):
            o.append(rect(-20 + i * 30, 40 + i * 38, w + 40, 16, fill=dark, opacity=.35,
                          extra=f'transform="rotate(-18 {w / 2} {h / 2})"'))
    elif kind == "out":
        for r in range(20, 260, 26):
            o.append(circle(w / 2, h / 2, r, stroke=light, sw=6, opacity=.35))
    return "".join(o)


def clear_zone(th, x, y, w, h):
    """Direction C keeps every label/control on a plain plate so art never competes with text."""
    return rect(x, y, w, h, fill=th["panel"], rx=8, opacity=.96)


def draw_knob(th, c, mod=None, hot=False, focus=False, dim=False):
    cx, cy, r = c["cx"], c["cy"], c["r"]
    o = []
    # label above, value below: the value is always drawn by the UI, never baked into art
    # label/value baselines use the large-knob radius so a row of mixed knobs lines up
    o.append(text(cx, cy - 43, c["label"], 13, th["ink"], "middle", 500))
    o.append(text(cx, cy + 43, c["value"], 13, th["ink"], "middle", 500, mono=True))
    # scale ticks
    for i in range(11):
        x0, y0 = polar(cx, cy, r + 4, i / 10)
        x1, y1 = polar(cx, cy, r + (8 if i in (0, 5, 10) else 6.5), i / 10)
        o.append(f'<line x1="{x0:.1f}" y1="{y0:.1f}" x2="{x1:.1f}" y2="{y1:.1f}" stroke="{th["tick"]}" stroke-width="1.3"/>')
    if th.get("glow"):  # B: luminous value track
        o.append(f'<path d="{arc(cx, cy, r + 3, 0, max(c["t"], .001))}" fill="none" stroke="{th["display_ink"]}" stroke-width="1.6" opacity=".7"/>')
    if mod:  # modulation range: base pointer stays on the cap, range is a ring outside it
        lo, hi, strong, col = mod
        o.append(f'<path d="{arc(cx, cy, r + 11, lo, hi)}" fill="none" stroke="{col}" '
                 f'stroke-width="{4 if strong else 3}" stroke-linecap="round" opacity="{1 if strong else .55}"'
                 f'{" filter=" + chr(34) + "url(#glow)" + chr(34) if strong and th.get("glow") else ""}/>')
        for t in (lo, hi):
            x0, y0 = polar(cx, cy, r + 7, t)
            x1, y1 = polar(cx, cy, r + 15, t)
            o.append(f'<line x1="{x0:.1f}" y1="{y0:.1f}" x2="{x1:.1f}" y2="{y1:.1f}" stroke="{col}" stroke-width="2" opacity="{1 if strong else .55}"/>')
    o.append(circle(cx + 1.5, cy + 3, r + 1, fill="#000", opacity=.28, extra='filter="url(#blur2)"'))
    o.append(circle(cx, cy, r + 1.5, fill=th["skirt"] if not th.get("glow") else "#0f1113"))
    o.append(circle(cx, cy, r, fill="url(#knobcap)"))
    if th.get("glow"):  # knurled metal edge
        for i in range(36):
            a = i * 10
            o.append(f'<line x1="{cx}" y1="{cy - r}" x2="{cx}" y2="{cy - r + 3}" stroke="#7d848c" stroke-width="1" transform="rotate({a} {cx} {cy})"/>')
    x0, y0 = polar(cx, cy, r * .3, c["t"])
    x1, y1 = polar(cx, cy, r * .88, c["t"])
    o.append(f'<line x1="{x0:.1f}" y1="{y0:.1f}" x2="{x1:.1f}" y2="{y1:.1f}" stroke="{th["pointer"]}" stroke-width="3" stroke-linecap="round"/>')
    if hot:  # hover / drag: skirt lights up, nothing overlaps label or value
        o.append(circle(cx, cy, r + 2, stroke=th["sel"], sw=3))
    if focus:  # keyboard focus: dashed frame around the whole control
        o.append(rect(cx - r - 26, cy - r - 32, 2 * r + 52, 2 * r + 62, stroke=th["sel"], sw=2, rx=8,
                      extra='stroke-dasharray="5 3"'))
    g = "".join(o)
    return f'<g opacity="{.4 if dim else 1}">{g}</g>'


def draw_select(th, c, dim=False):
    x, y, w, n = c["x"], c["y"], c["w"], len(c["options"])
    o = [text(x + w / 2, y - 8, c["label"], 13, th["ink"], "middle", 500),
         rect(x, y, w, 28, fill=th["seg_bg"], rx=6)]
    sw = w / n
    for i, opt in enumerate(c["options"]):
        on = i == c["idx"]
        if on:
            o.append(rect(x + i * sw + 2, y + 2, sw - 4, 24, fill=th["seg_on"], rx=4,
                          extra='filter="url(#glow)"' if th.get("glow") else ""))
        o.append(text(x + i * sw + sw / 2, y + 19, opt, 12.5, th["seg_on_text"] if on else th["ink"],
                      "middle", 600 if on else 500, mono=True))
    return f'<g opacity="{.4 if dim else 1}">{"".join(o)}</g>'


def draw_wave(th, c):
    x, y, w, h, wave = c["x"], c["y"], c["w"], c["h"], c["wave"]
    o = [rect(x, y, w, h, fill=th["display"], rx=5)]
    pts = []
    for i in range(61):
        t = i / 60 * 2
        v = [math.sin(t * 2 * math.pi), 1 - 4 * abs((t % 1) - .5), 2 * (t % 1) - 1,
             1 if (t % 1) < .5 else -1, [.6, -.3, .9, -.7][int(t * 2) % 4]][wave]
        pts.append((x + 8 + i / 60 * (w - 16), y + h / 2 - v * (h / 2 - 10)))
    d = "M" + " L".join(f"{a:.1f},{b:.1f}" for a, b in pts)
    o.append(f'<path d="{d}" fill="none" stroke="{th["display_ink"]}" stroke-width="2"/>')
    return "".join(o)


def draw_button(th, c):
    x, y, w = c["x"], c["y"], c["w"]
    led = th["sig"]["gate"] if c["on"] else th["seg_bg"]
    return (rect(x, y, w, 28, fill=th["seg_on"] if c["on"] else th["seg_bg"], rx=6)
            + circle(x + 13, y + 14, 4, fill=led, stroke=th["ink2"], sw=.8)
            + text(x + w / 2 + 7, y + 19, c["label"], 13, th["seg_on_text"] if c["on"] else th["ink"],
                   "middle", 600))


def draw_steps(th, c):
    x, y, w, h, pat = c["x"], c["y"], c["w"], c["h"], c["pattern"]
    o = [rect(x, y, w, h, fill=th["display"], rx=5)]
    sw = (w - 12) / len(pat)
    for i, v in enumerate(pat):
        bx = x + 6 + i * sw
        if v is None:  # rest
            o.append(rect(bx + 2, y + h - 12, sw - 4, 3, fill=th["display_ink"], opacity=.35))
        else:
            bh = 8 + v * (h - 22)
            o.append(rect(bx + 2, y + h - 6 - bh, sw - 4, bh, fill=th["display_ink"], rx=1.5,
                          opacity=.9 if i % 4 == 0 else .7))
    return "".join(o)


def draw_env(th, c):
    x, y, w, h = c["x"], c["y"], c["w"], c["h"]
    a, d, s, r = c["adsr"]
    ta, td, tr = (exp_t(v, .1, 1e4) for v in (a, d, r))
    tot = ta + td + .5 + tr
    sx = (w - 16) / tot
    x0, yb, yt = x + 8, y + h - 8, y + 8
    ys = yb - s * (yb - yt)
    pts = [(x0, yb), (x0 + ta * sx, yt), (x0 + (ta + td) * sx, ys), (x0 + (ta + td + .5) * sx, ys),
           (x0 + tot * sx, yb)]
    d = "M" + " L".join(f"{p:.1f},{q:.1f}" for p, q in pts)
    return (rect(x, y, w, h, fill=th["display"], rx=5)
            + f'<path d="{d}" fill="none" stroke="{th["display_ink"]}" stroke-width="2" stroke-linejoin="round"/>')


def draw_jack(th, p, ring=None, dim=False, target=False):
    cx, cy = p["cx"], p["cy"]
    o = []
    if target:  # compatible drop target during a drag
        o.append(circle(cx, cy, 17, stroke=th["sel"], sw=2.5, fill=th["sel"], opacity=.9,
                        extra='fill-opacity=".15"'))
    o.append(circle(cx + 1, cy + 2, 12.5, fill="#000", opacity=.25, extra='filter="url(#blur2)"'))
    # hex nut
    pts = " ".join(f"{cx + 12.5 * math.cos(math.radians(30 + 60 * i)):.1f},{cy + 12.5 * math.sin(math.radians(30 + 60 * i)):.1f}" for i in range(6))
    o.append(f'<polygon points="{pts}" fill="url(#nut)" stroke="{th["nut_edge"]}" stroke-width="0.8"/>')
    o.append(circle(cx, cy, 7.5, fill=th["hole_c"]))
    o.append(circle(cx, cy, 7.5, stroke="#ffffff", sw=.8, opacity=.15))
    if ring:  # two-tone: the colour reads on dark grounds, the dark edge on light ones
        o.append(circle(cx, cy, 15, stroke=mix(ring, "#000000", .45), sw=5.5))
        o.append(circle(cx, cy, 15, stroke=ring, sw=3))
    return f'<g opacity="{.35 if dim else 1}">{"".join(o)}</g>'


def port_label(th, p, on_plate):
    ink = th["plate_ink"] if on_plate else th["ink"]
    if p["lab"] == "right":
        return text(p["cx"] + 24, p["cy"] + 4.5, p["label"], 13, ink, "start", 500)
    # label above the jack: cables hang downward, so they never cover it
    return text(p["cx"], p["cy"] - 19, p["label"], 13, ink, "middle", 500)


def badge(th, p, s, col, on_plate):
    size = 11
    s = ellipsize(s, size, 64)  # stays inside the jack's column
    w = tw(s, size) + 12
    if p["lab"] == "right":
        x, y = p["cx"] + 24, p["cy"] + 9
    else:
        x, y = p["cx"] - w / 2, p["cy"] + 17
    bg = th["plate"] if not on_plate else th["plate_ink"]
    fg = th["plate_ink"] if not on_plate else th["plate"]
    return (rect(x, y, w, 16, fill=bg, rx=8, stroke=col, sw=1.5)
            + text(x + w / 2, y + 12, s, size, fg, "middle", 600))


def out_plate(th, m):
    """Contrasting plate behind every output jack group (VCV convention)."""
    outs = [p for p in m["ports"].values() if p["dir"] == "out"]
    if not outs:
        return ""
    if outs[0]["lab"] == "right":
        x0, x1 = 10, m["w"] - 10
        y0, y1 = min(p["cy"] for p in outs) - 24, max(p["cy"] for p in outs) + 24
    else:
        x0 = min(p["cx"] for p in outs) - 30
        x1 = max(p["cx"] for p in outs) + 30
        y0, y1 = outs[0]["cy"] - 36, outs[0]["cy"] + 38
        x0, x1 = max(x0, 8), min(x1, m["w"] - 8)
    return rect(x0, y0, x1 - x0, y1 - y0, fill=th["plate"], rx=7)


def draw_module(th, m, sc):
    """sc: scene state (selection, modulation, hidden-mode badges, drag targets)."""
    w, mid = m["w"], m["id"]
    if sc.get("overview"):
        return draw_module_overview(th, m, sc)
    o = []
    o.append(rect(3, 5, w - 2, PANEL_H, fill="#000", opacity=.35, extra='filter="url(#blur3)"'))
    o.append(rect(0, 0, w, PANEL_H, fill=th["panel"], rx=2))
    if th["art"]:
        img = HERE / "art" / th["key"] / f"{m['kind']}.png"
        art = (f'<image href="{img.as_uri()}" width="{w}" height="{PANEL_H}" preserveAspectRatio="xMidYMid slice"/>'
               if img.exists() else c_background(m))
        o.append(f'<g clip-path="url(#clip-{mid})">{art}</g>')
    o.append(rect(0, 0, w, PANEL_H, fill="#ffffff", rx=2, extra=f'filter="url(#{th["texture"]})"'))
    o.append(rect(.5, .5, w - 1, PANEL_H - 1, stroke=th["panel_edge"], rx=2))
    # screws
    for sx, sy in ((10, 7), (w - 10, PANEL_H - 7)) if w < 180 else ((10, 7), (w - 10, 7), (10, PANEL_H - 7), (w - 10, PANEL_H - 7)):
        o.append(circle(sx, sy, 3.6, fill="url(#nut)", stroke=th["nut_edge"], sw=.6))
        o.append(f'<line x1="{sx - 2.2}" y1="{sy}" x2="{sx + 2.2}" y2="{sy}" stroke="{th["nut_edge"]}" stroke-width="1"/>')
    # header
    lines = wrap2(m["name"], 15, w - 36)
    if th["art"]:
        o.append(clear_zone(th, 14, 13, w - 28, 22 + 18 * len(lines)))
    for i, ln in enumerate(lines):
        o.append(text(w / 2, 31 + i * 18, ln, 15, th["ink"], "middle", 600))
    if len(lines) == 1:
        tag = ("concept · " if m["kind"] in CONCEPT_KINDS else "") + m["kind"] + (
            " · per voice" if m["kind"] == "lfo" else "")
        o.append(text(w / 2, 47, tag, 11, th["ink2"], "middle", 400, mono=True))
    # controls
    # C: all clear zones first, so a neighbouring zone never covers an already-drawn label
    for c in m["controls"]:
        if th["art"] and c["type"] != "art":
            if c["type"] == "knob":
                x0, x1 = max(6, c["cx"] - 44), min(w - 6, c["cx"] + 44)
                o.append(clear_zone(th, x0, c["cy"] - 58, x1 - x0, 110))
            elif c["type"] == "select":
                o.append(clear_zone(th, c["x"] - 6, c["y"] - 26, c["w"] + 12, 60))
            else:
                o.append(clear_zone(th, c["x"] - 4, c["y"] - 4, c["w"] + 8, c["h"] + 8))
    for c in m["controls"]:
        if c["type"] == "art":
            if not th["art"]:
                o.append(draw_art(th, m, c))
            continue
        dim = bool(sc.get("drag"))
        if c["type"] == "knob":
            key = (mid, c["id"])
            o.append(draw_knob(th, c, mod=sc["mods"].get(key), hot=key == sc.get("hot_knob"),
                               focus=key == sc.get("focus_knob"), dim=dim))
        elif c["type"] == "select":
            o.append(draw_select(th, c, dim=dim))
        elif c["type"] == "wave":
            o.append(draw_wave(th, c))
        elif c["type"] == "env":
            o.append(draw_env(th, c))
        elif c["type"] == "button":
            o.append(draw_button(th, c))
        elif c["type"] == "steps":
            o.append(draw_steps(th, c))
    # ports
    if th["art"]:
        for p in m["ports"].values():
            if p["dir"] == "in":
                if p["lab"] == "right":
                    o.append(clear_zone(th, 10, p["cy"] - 20, w - 20, 40))
                else:
                    o.append(clear_zone(th, p["cx"] - 30, p["cy"] - 36, 60, 74))
    o.append(out_plate(th, m))
    for p in m["ports"].values():
        key = (mid, p["id"])
        on_plate = p["dir"] == "out"
        drag = sc.get("drag")
        dim = drag and (p["dir"] == "out" or key not in drag["targets"]) and key != drag["from"]
        o.append(draw_jack(th, p, ring=sc["rings"].get(key), dim=dim,
                           target=drag and key in drag["targets"]))
        o.append(port_label(th, p, on_plate))
        if key in sc["badges"]:
            s, col = sc["badges"][key]
            o.append(badge(th, p, s, col, on_plate))
    if mid in sc.get("selected", ()):
        o.append(rect(-2, -2, w + 4, PANEL_H + 4, stroke=th["sel"], sw=3, rx=4))
    return f'<g transform="translate({m["x"]},{m["y"]})">{"".join(o)}</g>'


def draw_module_overview(th, m, sc):
    """Overview scale: panel, big name, bare controls. Labels would be unreadable, so none."""
    w = m["w"]
    o = [rect(0, 0, w, PANEL_H, fill=th["panel"], rx=2),
         rect(.5, .5, w - 1, PANEL_H - 1, stroke=th["panel_edge"], rx=2), out_plate(th, m)]
    for i, ln in enumerate(wrap2(m["name"], 26, w - 20)):
        o.append(text(w / 2, 44 + i * 30, ln, 26, th["ink"], "middle", 600))
    for c in m["controls"]:
        if c["type"] == "knob":
            o.append(circle(c["cx"], c["cy"], c["r"], fill=th["knob"]))
    for p in m["ports"].values():
        o.append(draw_jack(th, p))
    if m["id"] in sc.get("selected", ()):
        o.append(rect(-3, -3, w + 6, PANEL_H + 6, stroke=th["sel"], sw=6, rx=4))
    return f'<g transform="translate({m["x"]},{m["y"]})">{"".join(o)}</g>'


# --------------------------------------------------------------------------- drawing: cables

def jack_pos(mods, ref):
    m = mods[ref[0]]
    p = m["ports"][ref[1]]
    return m["x"] + p["cx"], m["y"] + p["cy"]


def cable_path(a, b):
    (x0, y0), (x1, y1) = a, b
    dist = math.hypot(x1 - x0, y1 - y0)
    sag = min(110, 24 + .22 * dist)  # bounded sag: long cables droop, never to the floor
    dx = (x1 - x0) * .2
    c1, c2 = (x0 + dx, y0 + sag), (x1 - dx, y1 + sag)
    return f"M{x0:.1f},{y0:.1f} C{c1[0]:.1f},{c1[1]:.1f} {c2[0]:.1f},{c2[1]:.1f} {x1:.1f},{y1:.1f}", c1, c2


def draw_cable(th, a, b, col, opacity=1.0, ghost=False, selected=False):
    d, c1, c2 = cable_path(a, b)
    edge, hi = mix(col, "#000000", .35), mix(col, "#ffffff", .45)
    o = []
    if opacity > .5 and not ghost:
        o.append(f'<path d="{d}" transform="translate(4,8)" fill="none" stroke="#000" stroke-width="8" opacity=".3" filter="url(#blur3)"/>')
    if selected:
        o.append(f'<path d="{d}" fill="none" stroke="{th["sel"]}" stroke-width="13" opacity=".85"/>')
    o.append(f'<path d="{d}" fill="none" stroke="{edge}" stroke-width="7.5" stroke-linecap="round"/>')
    o.append(f'<path d="{d}" fill="none" stroke="{col}" stroke-width="5.5" stroke-linecap="round"/>')
    o.append(f'<path d="{d}" transform="translate(-0.9,-1.4)" fill="none" stroke="{hi}" stroke-width="1.4" opacity=".6"/>')
    for (px, py), (qx, qy) in ((a, c1), (b, c2)):
        ln = math.hypot(qx - px, qy - py) or 1
        ux, uy = (qx - px) / ln, (qy - py) / ln
        # strain relief boot, then the plug head over the jack
        o.append(f'<line x1="{px:.1f}" y1="{py:.1f}" x2="{px + ux * 20:.1f}" y2="{py + uy * 20:.1f}" stroke="{edge}" stroke-width="10" stroke-linecap="round"/>')
        o.append(f'<line x1="{px:.1f}" y1="{py:.1f}" x2="{px + ux * 20:.1f}" y2="{py + uy * 20:.1f}" stroke="{mix(col, "#000000", .15)}" stroke-width="8" stroke-linecap="round"/>')
        o.append(circle(px, py, 11.5, fill="#1a1a1a"))
        o.append(circle(px, py, 9, fill=col))
        o.append(circle(px, py, 4.5, fill=edge))
        o.append(circle(px - 3, py - 3.5, 2.6, fill="#ffffff", opacity=.45))
    return f'<g opacity="{opacity}"{" stroke-dasharray=" + chr(34) + "1 0" + chr(34) if ghost else ""}>{"".join(o)}</g>'


# --------------------------------------------------------------------------- drawing: chrome

def button(th, x, y, label, on=False, w=None, h=30, size=13, primary=False):
    w = w or tw(label, size) + 22
    fill = th["btn_on"] if on else th["btn"]
    fg = th["btn_on_text"] if on else th["ctext"]
    return (rect(x, y, w, h, fill=fill, rx=6, stroke=th["chrome_edge"] if not on else "none")
            + text(x + w / 2, y + h / 2 + size * .36, label, size, fg, "middle", 600 if on or primary else 500)), w


def segmented(th, x, y, label, options, idx, size=13):
    o = [text(x, y + 20, label, 13, th["ctext2"], "start", 500)]
    x += tw(label, 13) + 10
    ws = [tw(s, size) + 22 for s in options]
    o.append(rect(x, y, sum(ws) + 4, 30, fill=th["btn"], rx=7, stroke=th["chrome_edge"]))
    xx = x + 2
    for i, (s, ww) in enumerate(zip(options, ws)):
        if i == idx:
            o.append(rect(xx, y + 2, ww, 26, fill=th["btn_on"], rx=5))
        o.append(text(xx + ww / 2, y + 20, s, size, th["btn_on_text"] if i == idx else th["ctext"],
                      "middle", 600 if i == idx else 500))
        xx += ww
    return "".join(o), xx + 2


def topbar(th, sc):
    o = [rect(0, 0, W, TOP_H, fill=th["chrome"]),
         f'<line x1="0" y1="{TOP_H - .5}" x2="{W}" y2="{TOP_H - .5}" stroke="{th["chrome_edge"]}"/>',
         text(18, 30, "kabl", 18, th["ctext"], "start", 700, extra='letter-spacing="0.5"')]
    name = ellipsize(sc.get("patch_name", "Reference patch"), 14, 150)
    o.append(text(72, 23, name, 14, th["ctext"], "start", 600))
    o.append(text(72, 39, sc.get("save_state", "Saved"), 11, th["ctext2"], "start", 400))
    x = 238
    for lab in ("Undo", "Redo"):
        b, bw = button(th, x, 9, lab, on=lab == sc.get("pressed"))
        o.append(b)
        x += bw + 6
    seg, x = segmented(th, x + 16, 9, "Cables", ["All", "Focus", "Hidden"],
                       {"all": 0, "focus": 1, "hidden": 2}[sc["cables"]])
    o.append(seg)
    x += 16
    if sc.get("transport"):  # mirrors the Clock module so run/reset never depend on the rack view
        b, bw = button(th, x, 9, f"Zoom {int(sc.get('zoom', 1) * 100)}%")
        o.append(b)
        x += bw + 12
        hot = sc["transport"] == "hot"
        o.append(rect(x, 9, 192, 30, fill=th["btn"], rx=7, stroke=th["sel"] if hot else th["chrome_edge"],
                      sw=2 if hot else 1))
        o.append(rect(x + 3, 12, 74, 24, fill=th["btn_on"], rx=5))
        o.append(f'<polygon points="{x + 12},{17} {x + 12},{31} {x + 23},{24}" fill="{th["btn_on_text"]}"/>')
        o.append(text(x + 50, 29, "Running", 12.5, th["btn_on_text"], "middle", 600))
        o.append(text(x + 110, 29, "112 BPM", 12.5, th["ctext"], "middle", 500, mono=True))
        o.append(text(x + 167, 29, "Reset", 12.5, th["ctext"], "middle", 500))
    else:
        o.append(text(x, 29, "Zoom", 13, th["ctext2"], "start", 500))
        x += 44
        for lab in ("−", f"{int(sc.get('zoom', 1) * 100)}%", "+", "Fit"):
            b, bw = button(th, x, 9, lab, w=None if lab not in "−+" else 30)
            o.append(b)
            x += bw + 4
    # right side
    xr = W - 16
    lvl = "−6.0 dB"
    o.append(text(xr, 29, lvl, 13, th["ctext"], "end", 500, mono=True))
    xr -= tw(lvl, 13, True) + 10
    o.append(rect(xr - 80, 21, 80, 6, fill=th["btn"], rx=3))
    o.append(rect(xr - 80, 21, 56, 6, fill=th["ctext2"], rx=3))
    o.append(circle(xr - 24, 24, 7, fill=th["ctext"]))
    o.append(text(xr - 90, 29, "Out", 13, th["ctext2"], "end", 500))
    xr -= 90 + tw("Out", 13) + 18
    for lab, on in (("Routing", sc.get("drawer") is not None), ("Modules", sc.get("browser", False))):
        bw = tw(lab, 13) + 22
        b, _ = button(th, xr - bw, 9, lab, on=on)
        o.append(b)
        xr -= bw + 6
    return "".join(o)


def footer(th, sc):
    y = H - FOOT_H
    o = [rect(0, y, W, FOOT_H, fill=th["chrome"]),
         f'<line x1="0" y1="{y + .5}" x2="{W}" y2="{y + .5}" stroke="{th["chrome_edge"]}"/>',
         rect(10, y + 4, 70, 16, fill="#c2412d", rx=3),
         text(45, y + 16, "CONCEPT", 11, "#ffffff", "middle", 700, extra='letter-spacing="1"'),
         text(88, y + 16.5, f"{th['title']} · {sc.get('caption', 'design mockup, not an app screenshot')}",
              12, th["ctext2"], "start", 400)]
    hint = sc.get("hint", "Drag a jack to connect · Esc cancels · Double-click a value to type it")
    o.append(text(W - 12, y + 16.5, hint, 12, th["ctext2"], "end", 400))
    return "".join(o)


def rack_bg(th, mods, width):
    o = [rect(0, TOP_H, width, H - TOP_H - FOOT_H, fill=th["rack"])]
    for y in ROW_Y:
        for ry in (y - 10, y + PANEL_H):
            o.append(rect(0, ry, width, 10, fill="url(#rail)"))
            for hx in range(RACK_X + 15, int(width), UNIT):
                o.append(rect(hx - 3, ry + 3, 6, 4, fill=th["hole"], rx=2))
    # empty rail hint where the row still has room
    for r in range(2):
        row = [m for m in mods.values() if m["row"] == r]
        if not row:
            continue
        x_end = max(m["x"] + m["w"] for m in row)
        if width - x_end > 150:
            y = ROW_Y[r]
            o.append(rect(x_end + 14, y + 14, 120, PANEL_H - 28, stroke=th["ctext2"], sw=1.2, rx=8,
                          opacity=.35, extra='stroke-dasharray="6 5"'))
            o.append(text(x_end + 74, y + PANEL_H / 2 + 5, "+ Add module", 13, th["ctext2"],
                          "middle", 500, opacity=.8))
    return "".join(o)


def drawer(th, sc, mods, cabs):
    x0, y0, y1 = W - DRAWER_W, TOP_H, H - FOOT_H
    o = [rect(x0, y0, DRAWER_W, y1 - y0, fill=th["chrome"]),
         f'<line x1="{x0 + .5}" y1="{y0}" x2="{x0 + .5}" y2="{y1}" stroke="{th["chrome_edge"]}"/>']
    x = x0 + 16
    mode = sc["drawer"]
    title = "Routing" if mode == "routing" else "Filter · Cutoff"
    o.append(text(x, y0 + 30, title, 16, th["ctext"], "start", 600))
    b, _ = button(th, W - 16 - 62, y0 + 11, "Close", w=62, h=28)
    o.append(b)
    y = y0 + 50
    amount = sc.get("amount", .75)

    def route_card(y, c, heading):
        src, dst = c["src"], c["dst"]
        col = th["sig"][c["type"]]
        h = 164
        o.append(rect(x - 4, y, DRAWER_W - 24, h, fill=mix(th["chrome"], th["app"], .5), rx=8, stroke=th["sel"], sw=1.5))
        o.append(rect(x - 4, y, 5, h, fill=col, rx=2))
        o.append(text(x + 10, y + 22, heading, 14, th["ctext"], "start", 600))
        o.append(text(x + 10, y + 40, "CV · per voice · cable C6", 12, th["ctext2"], "start", 400))
        # bipolar amount slider
        o.append(text(x + 10, y + 68, "Amount", 13, th["ctext"], "start", 500))
        sx, sw_ = x + 70, 130
        o.append(rect(sx, y + 60, sw_, 6, fill=th["chrome_edge"], rx=3))
        o.append(f'<line x1="{sx + sw_ / 2}" y1="{y + 54}" x2="{sx + sw_ / 2}" y2="{y + 72}" stroke="{th["ctext2"]}" stroke-width="1.5"/>')
        fx = sx + sw_ / 2 + amount / 2 * sw_ / 2  # slider spans ±2 oct
        o.append(rect(min(sx + sw_ / 2, fx), y + 60, abs(fx - sx - sw_ / 2), 6, fill=col, rx=3))
        o.append(circle(fx, y + 63, 8, fill=th["ctext"], stroke=col, sw=2))
        vb = f"{amount:+.2f} oct"
        o.append(rect(x + 214, y + 51, 80, 24, fill=th["chrome"], rx=5, stroke=th["chrome_edge"]))
        o.append(text(x + 254, y + 68, vb, 13, th["ctext"], "middle", 500, mono=True))
        lo, hi = 1200 * 2 ** -amount, 1200 * 2 ** amount
        fmt = lambda v: f"{v / 1000:.2f} kHz" if v >= 1000 else f"{v:.0f} Hz"
        o.append(text(x + 10, y + 96, f"Sweeps {fmt(lo)} – {fmt(hi)} around base 1.20 kHz", 12,
                      th["ctext2"], "start", 400))
        bx = x + 10
        for lab in ("Bypass", "Remove", "Show on rack"):
            bb, bw = button(th, bx, y + 116, lab, h=30)
            o.append(bb)
            bx += bw + 8
        return y + h + 16

    c6 = next((c for c in cabs if c["id"] == "C6"), None)
    if mode == "routing":
        chips = [("All", len(cabs)), ("Audio", sum(c["type"] == "audio" for c in cabs)),
                 ("CV & pitch", sum(c["type"] in ("cv", "pitch") for c in cabs)),
                 ("Gate", sum(c["type"] == "gate" for c in cabs))]
        cx = x
        for i, (lab, n) in enumerate(chips):
            b, bw = button(th, cx, y, f"{lab} {n}", on=i == 0, h=28, size=12.5)
            o.append(b)
            cx += bw + 6
        y += 46
        if c6 and "lfo" in sc.get("selected", ()):
            o.append(text(x, y + 4, "SELECTED SOURCE · LFO out", 11, th["ctext2"], "start", 600,
                          extra='letter-spacing="0.8"'))
            y = route_card(y + 14, c6, "LFO out → Filter Cutoff CV")
        o.append(text(x, y + 4, f"ALL CONNECTIONS · {len(cabs)}", 11, th["ctext2"], "start", 600,
                      extra='letter-spacing="0.8"'))
        y += 14
        for c in cabs:
            col = th["sig"][c["type"]]
            on = c["id"] == "C6" and "lfo" in sc.get("selected", ())
            if on:
                o.append(rect(x - 6, y, DRAWER_W - 20, 32, fill=th["btn"], rx=6))
            o.append(rect(x, y + 7, 4, 18, fill=col, rx=2))
            s = (f"{mods[c['src'][0]]['name']} {PORT_NAMES.get(c['src'][1], c['src'][1])} → "
                 f"{mods[c['dst'][0]]['name']} {PORT_NAMES.get(c['dst'][1], c['dst'][1])}")
            o.append(text(x + 14, y + 21, ellipsize(s, 13, 232), 13, th["ctext"], "start", 500))
            o.append(text(W - 22, y + 21, TYPE_LABEL[c["type"]], 12, th["ctext2"], "end", 400))
            y += 34
    else:  # destination inspector
        o.append(text(x, y + 16, "Base value", 13, th["ctext2"], "start", 500))
        o.append(rect(x + 90, y, 86, 26, fill=th["btn"], rx=5, stroke=th["chrome_edge"]))
        o.append(text(x + 133, y + 18, "1.20 kHz", 13, th["ctext"], "middle", 500, mono=True))
        b, _ = button(th, x + 186, y, "Reset", h=26)
        o.append(b)
        y += 48
        o.append(text(x, y, "MODULATION SOURCES · 1", 11, th["ctext2"], "start", 600,
                      extra='letter-spacing="0.8"'))
        y = route_card(y + 10, c6, "LFO out → Cutoff CV")
        note = ["Cutoff CV already has a cable. Dropping", "another source offers Replace; summing",
                "sources needs engine work (see REVIEW)."]
        for i, ln in enumerate(note):
            o.append(text(x, y + 4 + i * 17, ln, 12, th["ctext2"], "start", 400))
        y += 64
        b, _ = button(th, x, y, "+ Add source…", w=DRAWER_W - 32, h=32)
        o.append(b)
    b, _ = button(th, x, y1 - 48, "+ New route…", w=DRAWER_W - 32, h=34, primary=True)
    o.append(b)
    return "".join(o)


def tooltip(th, x, y, lines):
    w = max(tw(s, 13) for s in lines) + 20
    h = 12 + 18 * len(lines)
    o = [rect(x + 2, y + 3, w, h, fill="#000", rx=7, opacity=.3, extra='filter="url(#blur2)"'),
         rect(x, y, w, h, fill="#15140f", rx=7, stroke="#4a463f")]
    for i, s in enumerate(lines):
        o.append(text(x + 10, y + 22 + i * 18, s, 13, "#f5f0e6" if i == 0 else "#c9c1b3", "start",
                      600 if i == 0 else 400))
    return "".join(o)


def cursor(x, y, grab=False):
    pts = [(0, 0), (0, 17), (4.5, 13), (8, 20.5), (11, 19), (7.6, 12), (13, 12)]
    d = " ".join(f"{x + a},{y + b}" for a, b in pts)
    return f'<polygon points="{d}" fill="#ffffff" stroke="#000000" stroke-width="1.2"/>'


def toast(th, s, action):
    w = tw(s, 13) + tw(action, 13) + 56
    x, y = (W - DRAWER_W) / 2 - w / 2, H - FOOT_H - 54
    return (rect(x, y, w, 38, fill="#15140f", rx=8, stroke="#4a463f")
            + text(x + 14, y + 24, s, 13, "#f5f0e6", "start", 500)
            + text(x + w - 14, y + 24, action, 13, "#8fc2ff", "end", 600))


# --------------------------------------------------------------------------- scenes

def scene_state(th, sc, mods, cabs):
    """Derive per-module drawing state (mod arcs, jack rings, badges) from the scene."""
    st = dict(mods={}, rings={}, badges={}, selected=sc.get("selected", ()), drag=sc.get("drag"),
              hot_knob=sc.get("hot_knob"), focus_knob=sc.get("focus_knob"))
    amount = sc.get("amount", .75)
    if any(c["id"] == "C6" for c in cabs) and "filter" in mods:
        base = exp_t(1200, 20, 20000)
        span = amount * math.log(2) / math.log(1000)
        strong = "lfo" in st["selected"] or sc.get("drawer") == "dest"
        st["mods"][("filter", "cutoff_hz")] = (base - span, base + span, strong, th["sig"]["cv"])
    if sc["cables"] == "hidden":
        peers = {}
        for c in cabs:
            peers.setdefault(c["src"], []).append(("→", c["dst"], c["type"]))
            peers.setdefault(c["dst"], []).append(("←", c["src"], c["type"]))
        for key, lst in peers.items():
            col = th["sig"][lst[0][2]]
            st["rings"][key] = col
            if len(lst) == 1:
                arrow, other, _ = lst[0]
                s = f"{arrow} {SHORT.get(other[0], mods[other[0]]['name'])}"
            else:
                s = f"{lst[0][0]} {len(lst)} routes"
            st["badges"][key] = (s, col)
    if "lfo" in st["selected"] and sc["cables"] == "hidden":
        st["rings"][("lfo", "out")] = th["sel"]
        st["rings"][("filter", "cutoff_cv")] = th["sel"]
    return st


def render_svg(th, sc, patch=None):
    mods, cabs = patch or build_patch(REF_ROWS, REF_CABLES, params=sc.get("params"))
    if sc.get("omit"):
        cabs = [c for c in cabs if c["id"] not in sc["omit"]]
    st = scene_state(th, sc, mods, cabs)
    rack_w = W - (DRAWER_W if sc.get("drawer") else 0)
    pan, z = sc.get("pan", 0), sc.get("zoom", 1)
    st["overview"] = z < .75  # overview scale draws simplified modules (INTERACTIONS.md)
    total = max(m["x"] + m["w"] for m in mods.values()) + RACK_X
    body = [rack_bg(th, mods, max(W, total) / z + pan)]
    clips = "".join(f'<clipPath id="clip-{m["id"]}"><rect width="{m["w"]}" height="{PANEL_H}" rx="2"/></clipPath>'
                    for m in mods.values())
    body.append(f"<defs>{clips}</defs>")
    body.extend(draw_module(th, m, st) for m in mods.values())
    if sc["cables"] != "hidden":
        rel = set(sc.get("selected", ()))
        for c in cabs:
            focus_dim = sc["cables"] == "focus" and not ({c["src"][0], c["dst"][0]} & rel)
            body.append(draw_cable(th, jack_pos(mods, c["src"]), jack_pos(mods, c["dst"]),
                                   th["sig"][c["type"]], .13 if focus_dim else .96,
                                   selected=c["id"] == sc.get("selected_cable")))
    if sc.get("drag"):
        d = sc["drag"]
        body.append(draw_cable(th, jack_pos(mods, d["from"]), d["to"], th["sig"]["cv"], .75, ghost=True))
    rack = f'<g transform="translate({-pan},{TOP_H * (1 - z):.1f}) scale({z})">{"".join(body)}</g>'
    over = []
    for tt in sc.get("tooltips", ()):
        over.append(tooltip(th, *tt))
    if sc.get("toast"):
        over.append(toast(th, *sc["toast"]))
    if sc.get("cursor"):
        over.append(cursor(*sc["cursor"]))
    if sc.get("overlay_hits"):
        over.append(hit_overlay(th, mods))
    if sc.get("scrollbar"):
        over.append(scrollbar(th, mods, pan, rack_w))
    if sc.get("popover"):
        # the popover shows connections the hidden-mode way: rings + badges naming the other end
        pst = scene_state(th, dict(sc, cables="hidden"), mods, cabs)
        over.append(popover(th, mods, sc["popover"][0], *sc["popover"][1:], pst))
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">',
             defs(th), rect(0, 0, W, H, fill=th["rack"]),
             f'<svg x="0" y="{0}" width="{rack_w}" height="{H}" overflow="hidden">{rack}{"".join(over)}</svg>',
             topbar(th, sc)]
    if sc.get("drawer"):
        parts.append(drawer(th, sc, mods, cabs))
    parts.append(footer(th, sc))
    parts.append("</svg>")
    return "".join(parts), mods, cabs


def hit_regions(mods):
    """Proposed hit regions in window coordinates: (label, x, y, w, h)."""
    out = []
    for m in mods.values():
        for c in m["controls"]:
            if c["type"] == "knob":
                s = 2 * c["r"] + 8
                out.append((f"{m['id']}.{c['id']}", m["x"] + c["cx"] - s / 2, m["y"] + c["cy"] - s / 2, s, s))
            elif c["type"] == "select":
                n = len(c["options"])
                for i in range(n):
                    out.append((f"{m['id']}.{c['id']}[{i}]", m["x"] + c["x"] + i * c["w"] / n, m["y"] + c["y"],
                                c["w"] / n, 28))
        for p in m["ports"].values():
            out.append((f"{m['id']}.{p['id']}", m["x"] + p["cx"] - 14, m["y"] + p["cy"] - 14, 28, 28))
        out.append((f"{m['id']}.header", m["x"], m["y"], m["w"], 52))
    return out


def hit_overlay(th, mods):
    o = []
    for lab, x, y, w, h in hit_regions(mods):
        col = "#ff3b8d" if lab.endswith("header") else "#00d1ff"
        o.append(rect(x, y, w, h, stroke=col, sw=1.2, extra='stroke-dasharray="3 2"'))
    notes = [(820, 80, "Rails 10 px · panel 340 px · width unit 30 px"),
             (820, 106, "Magenta: module drag handle (header)"),
             (820, 132, "Cyan: control / jack hit region (min 28×28)")]
    for x, y, s in notes:
        o.append(rect(x, y, tw(s, 12) + 14, 20, fill="#000", rx=4, opacity=.75))
        o.append(text(x + 7, y + 14, s, 12, "#fff"))
    return "".join(o)


def scrollbar(th, mods, pan, view_w):
    """Horizontal pan indicator on the bottom rail; Fit Patch in the top bar is the overview."""
    total = max(m["x"] + m["w"] for m in mods.values()) + RACK_X
    y = H - FOOT_H - 8
    return (rect(8, y, view_w - 16, 6, fill="#000000", rx=3, opacity=.45)
            + rect(8 + pan / total * (view_w - 16), y, view_w / total * (view_w - 16), 6,
                   fill=th["ctext2"], rx=3))


def popover(th, mods, mid, x, y, st):
    """Selected module at 100% over an overview-scale rack (dense-patch access)."""
    m = dict(mods[mid], x=x + 12, y=y + 44)
    w = m["w"] + 24
    o = [rect(x + 3, y + 5, w, PANEL_H + 56, fill="#000", rx=10, opacity=.4, extra='filter="url(#blur3)"'),
         rect(x, y, w, PANEL_H + 56, fill=th["chrome"], rx=10, stroke=th["sel"], sw=2),
         text(x + 14, y + 27, "Selected · 100%", 13, th["ctext"], "start", 600)]
    b, _ = button(th, x + w - 70, y + 8, "Close", w=58, h=26)
    o.append(b)
    o.append(draw_module(th, m, dict(st, overview=False, selected=())))
    return "".join(o)


# --------------------------------------------------------------------------- component sheet

def component_sheet(th):
    mods, cabs = build_patch(REF_ROWS, REF_CABLES)
    kn = dict(mods["filter"]["controls"][0])
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">',
         defs(th), rect(0, 0, W, H, fill=th["rack"]),
         text(32, 44, f"Component sheet — {th['title']}", 20, th["ctext"], "start", 600),
         text(32, 66, "Concept. Live controls, values and states are drawn by the UI, never baked into panel art.",
              13, th["ctext2"])]

    def cell(x, y, w, h, title):
        o.append(rect(x, y, w, h, fill=th["panel"], rx=6))
        o.append(rect(x, y, w, h, fill="#fff", rx=6, extra=f'filter="url(#{th["texture"]})"'))
        o.append(text(x + w / 2, y + h - 12, title, 12, th["ink2"], "middle", 500))

    st = lambda **kw: dict(mods={}, rings={}, badges={}, selected=(), **kw)
    # row 1: knobs
    specs = [("Knob · default", {}), ("Hover / drag", dict(hot=True)), ("Keyboard focus", dict(focus=True)),
             ("Modulated (source selected)", dict(mod=(.52, .67, True, th["sig"]["cv"]))),
             ("Modulated (idle)", dict(mod=(.52, .67, False, th["sig"]["cv"]))), ("Disabled", dict(dim=True))]
    for i, (title, kw) in enumerate(specs):
        x, y = 32 + i * 202, 90
        cell(x, y, 190, 170, title)
        c = dict(kn, cx=x + 95, cy=y + 76)
        o.append(draw_knob(th, c, **kw))
    # row 2: value entry, selectors, switch
    x, y = 32, 280
    cell(x, y, 290, 150, "Value · editing (typed, unit parsed)")
    o.append(text(x + 145, y + 42, "Cutoff", 13, th["ink"], "middle", 500))
    o.append(rect(x + 85, y + 54, 120, 32, fill=th["seg_bg"], rx=6, stroke=th["sel"], sw=2))
    o.append(text(x + 145, y + 76, "1.35 kHz", 14, th["ink"], "middle", 500, mono=True))
    o.append(f'<line x1="{x + 180}" y1="{y + 60}" x2="{x + 180}" y2="{y + 81}" stroke="{th["ink"]}" stroke-width="1.5"/>')
    o.append(text(x + 145, y + 108, "Enter commits · Esc cancels", 12, th["ink2"], "middle"))
    x = 334
    cell(x, y, 290, 150, "Discrete selector · selected + focus")
    s = sel("waveform", "Waveform", ["SIN", "TRI", "SAW", "SQR", "S&H"], 1, x + 24, y + 58, 242)
    o.append(draw_select(th, s))
    o.append(rect(x + 24 + 242 / 5 * 3, y + 56, 242 / 5, 32, stroke=th["sel"], sw=2, rx=5,
                  extra='stroke-dasharray="4 3"'))
    x = 636
    cell(x, y, 290, 150, "Two-way switch")
    o.append(draw_select(th, sel("exponential", "Response", ["LIN", "EXP"], 0, x + 85, y + 58, 120)))
    x = 948
    cell(x, y, 300, 150, "Module selected (header = drag handle)")
    o.append(rect(x + 70, y + 24, 160, 80, fill=th["panel"], stroke=th["sel"], sw=3, rx=4))
    o.append(text(x + 150, y + 50, "SVF Filter", 15, th["ink"], "middle", 600))
    o.append(text(x + 150, y + 66, "filter.svf", 11, th["ink2"], "middle", mono=True))
    # row 3: jacks
    y = 450
    jspecs = [("Input · empty", None, False, False, None), ("Output · on plate", None, False, False, "plate"),
              ("Drop target (compatible)", None, False, True, None), ("Incompatible (dimmed)", None, True, False, None),
              ("Hidden mode · connected", th["sig"]["audio"], False, False, "badge"),
              ("Source selected", th["sel"], False, False, None)]
    for i, (title, ring, dim, target, extra) in enumerate(jspecs):
        x = 32 + i * 202
        cell(x, y, 190, 130, title)
        p = port("in", "Out" if extra == "plate" else "In", "in", "audio", x + 95, y + 48)
        if extra == "plate":
            o.append(rect(x + 60, y + 10, 70, 74, fill=th["plate"], rx=7))
        o.append(draw_jack(th, p, ring=ring, dim=dim, target=target))
        o.append(port_label(th, p, extra == "plate"))
        if extra == "badge":
            o.append(badge(th, p, "→ Filter", ring, False))
    # row 4: cables
    y = 600
    o.append(rect(32, y, 1216, 170, fill=th["panel"], rx=6))
    o.append(rect(32, y, 1216, 170, fill="#fff", rx=6, extra=f'filter="url(#{th["texture"]})"'))
    items = [("Audio", th["sig"]["audio"], 1, False), ("CV / pitch", th["sig"]["cv"], 1, False),
             ("Gate", th["sig"]["gate"], 1, False), ("Selected", th["sig"]["audio"], 1, True),
             ("Focus: unrelated", th["sig"]["audio"], .13, False)]
    for i, (title, col, op, selc) in enumerate(items):
        x = 80 + i * 240
        a, b = (x, y + 40), (x + 150, y + 40)
        o.append(draw_jack(th, port("a", "", "in", "audio", *a)))
        o.append(draw_jack(th, port("b", "", "in", "audio", *b)))
        o.append(draw_cable(th, a, b, col, op, selected=selc))
        o.append(text(x + 75, y + 156, title, 12, th["ink2"], "middle", 500))
    o.append("</svg>")
    return "".join(o)


# --------------------------------------------------------------------------- output

def chrome_render(svg_path, png_path, w=W, h=H, scale=1):
    subprocess.run(["google-chrome", "--headless=new", "--disable-gpu", "--hide-scrollbars",
                    "--no-sandbox", f"--window-size={w},{h}", f"--force-device-scale-factor={scale}",
                    f"--screenshot={png_path}", svg_path.as_uri()],
                   check=True, capture_output=True)


def emit(th_key, name, svg, scale=1):
    BUILD.mkdir(exist_ok=True)
    svg_path = BUILD / f"{th_key}-{name}.svg"
    svg_path.write_text(svg)
    out = HERE / "directions" / th_key / f"{name}.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    chrome_render(svg_path, out, scale=scale)
    return out


def theme(key):
    th = dict(THEMES[key])
    th["key"] = key
    return th


def base_scene(**kw):
    sc = dict(cables="all", drawer=None, selected=(), amount=.75)
    sc.update(kw)
    return sc


HIDDEN = dict(cables="hidden", drawer="routing", selected=("lfo",),
              hint="Hidden cables: jack badges and the Routing drawer edit the same connections")


def crowded_patch():
    rows = [[("midi", "midi.in"), ("osc1", "osc.va"), ("osc2", "osc.va"), ("ring", "ringmod"),
             ("mix", "mixer"), ("filter", "filter.svf"), ("filt2", "filter.svf")],
            [("lfo1", "lfo"), ("lfo2", "lfo"), ("env1", "env.adsr"), ("env2", "env.adsr"),
             ("vca1", "vca"), ("vca2", "vca"), ("out", "out")]]
    cables = [("K1", "midi.pitch", "osc1.pitch"), ("K2", "midi.pitch", "osc2.pitch"),
              ("K3", "midi.gate", "env1.gate"), ("K4", "midi.gate", "env2.gate"),
              ("K5", "osc1.out", "mix.in1"), ("K6", "osc2.out", "mix.in2"),
              ("K7", "osc1.out", "ring.a"), ("K8", "osc2.out", "ring.b"),
              ("K9", "ring.out", "mix.in3"), ("K10", "mix.out", "filter.in"),
              ("K11", "mix.out", "filt2.in"), ("K12", "lfo1.out", "filter.cutoff_cv"),
              ("K13", "lfo2.out", "filt2.cutoff_cv"), ("K14", "env2.out", "filter.resonance_cv"),
              ("K15", "filter.lp", "vca1.in"), ("K16", "filt2.bp", "vca2.in"),
              ("K17", "env1.out", "vca1.cv"), ("K18", "env1.out", "vca2.cv"),
              ("K19", "vca1.out", "out.left"), ("K20", "vca2.out", "out.right"),
              ("K21", "lfo1.out", "filt2.resonance_cv")]
    names = {"osc1": "VA Oscillator 1", "osc2": "VA Oscillator 2", "filt2": "SVF Filter 2",
             "lfo1": "LFO 1", "lfo2": "LFO 2", "env1": "ADSR Amp", "env2": "ADSR Filter",
             "vca1": "VCA 1", "vca2": "VCA 2"}
    params = {"osc2": dict(waveform=3, base_hz=262.9, base_hz_s="262.9 Hz"),
              "lfo2": dict(waveform=0, rate_hz=.23, rate_s="0.23 Hz"),
              "env2": dict(adsr=(120, 900, .3, 1500))}
    return build_patch(rows, cables, names, params)


def perf_patch(seqb_transpose="+0 st", tt=.5):
    """Future-performance scenario (PLAN.md musical north star). Conceptual modules only."""
    rows = [[("clock", "clock"), ("seqA", "seq.step"), ("seqB", "seq.step"), ("oscA", "osc.va"),
             ("oscB", "osc.va")],
            [("layers", "mix.layers"), ("filter", "filter.svf"), ("delay", "fx.delay"),
             ("reverb", "fx.reverb"), ("out", "out")]]
    cables = [("P1", "clock.clock", "seqA.clock"), ("P2", "clock.clock", "seqB.clock"),
              ("P3", "clock.reset", "seqA.reset"), ("P4", "clock.reset", "seqB.reset"),
              ("P5", "seqA.pitch", "oscA.pitch"), ("P6", "seqB.pitch", "oscB.pitch"),
              ("P7", "oscA.out", "layers.in_a"), ("P8", "seqA.gate", "layers.gate_a"),
              ("P9", "oscB.out", "layers.in_b"), ("P10", "seqB.gate", "layers.gate_b"),
              ("P11", "layers.out", "filter.in"), ("P12", "filter.lp", "delay.in"),
              ("P13", "delay.out", "reverb.in"), ("P14", "reverb.l", "out.left"),
              ("P15", "reverb.r", "out.right")]
    names = {"seqA": "Seq A · bass", "seqB": "Seq B · arp", "oscA": "Osc A", "oscB": "Osc B"}
    params = {"seqA": dict(pattern=[0, None, 0, .3, 0, None, .5, 0, 0, None, 0, .3, .7, None, .5, .3]),
              "seqB": dict(pattern=[.2, .5, .8, .5, .2, .6, .9, .6, .3, .6, 1, .6, .3, .5, .8, .5],
                           transpose=seqb_transpose, tt=tt),
              "oscB": dict(waveform=3, base_hz=523.3, base_hz_s="523.3 Hz")}
    return build_patch(rows, cables, names, params)


def perf_frames():
    cap = "future-performance concept: Clock, Sequencer, Layers, Delay, Reverb do not exist"
    common = dict(transport="on", patch_name="Night pulse (sketch)", caption=cap)
    return [
        ("1  Transport lives in the top bar, mirroring the Clock module; it never hides.",
         base_scene(**dict(common, transport="hot"), selected=("clock",),
                    tooltips=[(700, 52, ["Run / stop: Space · Reset: R", "Reset lands on the next bar"])]),
         perf_patch()),
        ("2  Two named layers. Seq B transpose +7 st, applied at the next bar.",
         base_scene(**common, cables="focus", selected=("seqB",), hot_knob=("seqB", "transpose"),
                    tooltips=[(560, 170, ["Seq B transpose  +7 st", "Takes effect at the next bar"])]),
         perf_patch("+7 st", .65)),
        ("3  Layer B level on Layers. Focus still leaves its long cables over filter/delay.",
         base_scene(**common, cables="focus", selected=("layers",),
                    hot_knob=("layers", "level_b"),
                    tooltips=[(262, 470, ["Seq B level  0.62", "Mute B keeps its level"])]),
         perf_patch("+7 st", .65)),
        ("4  Cables hidden: same places for transport, transpose, levels, filter, delay.",
         base_scene(**common, cables="hidden", selected=("seqB",)),
         perf_patch("+7 st", .65)),
    ]


def storyboard_frames(th):
    drag = dict(from_=None)
    targets = {("filter", "cutoff_cv"), ("filter", "resonance_cv"), ("vca", "cv")}
    frames = [
        ("1  Drag from LFO out. CV inputs light up; outputs dim. Esc cancels.",
         base_scene(omit={"C6"}, selected=("lfo",),
                    drag={"from": ("lfo", "out"), "to": (551, 264), "targets": targets},
                    tooltips=[(566, 206, ["Filter · Cutoff CV", "Release to connect"])],
                    cursor=(549, 262))),
        ("2  Connected. With LFO selected, drag Cutoff's ring to set amount.",
         base_scene(selected=("lfo",), hot_knob=("filter", "cutoff_hz"),
                    tooltips=[(560, 150, ["LFO → Cutoff  +0.75 oct", "Sweeps 714 Hz – 2.02 kHz"])],
                    cursor=(541, 160))),
        ("3  Cables hidden. Same layout; badges name each jack's other end.",
         base_scene(cables="hidden", selected=("lfo",))),
        ("4  Click Cutoff's modulation ring: drawer lists its sources, amount, bypass, remove.",
         base_scene(cables="hidden", drawer="dest", selected=("filter",),
                    focus_knob=("filter", "cutoff_hz"))),
        ("5  Undo restores the previous amount (+1.00, the default); toast offers Redo.",
         base_scene(cables="hidden", drawer="dest", selected=("filter",), amount=1.0, pressed="Undo",
                    toast=("Undid: LFO → Cutoff amount +0.75 → +1.00 oct (the connect default)", "Redo"))),
    ]
    return frames


def font(size, bold=False):
    path = subprocess.run(["fc-match", "-f", "%{file}", f"IBM Plex Sans:{'semibold' if bold else 'regular'}"],
                          capture_output=True, text=True).stdout
    return ImageFont.truetype(path, size)


def board(images, cols, cell_w, captions, title, out, subtitle=""):
    cell_h = round(cell_w * H / W)
    rows = math.ceil(len(images) / cols)
    pad, cap = 24, 44
    bw = pad + cols * (cell_w + pad)
    bh = 90 + rows * (cell_h + cap + pad)
    img = Image.new("RGB", (bw, bh), "#141312")
    d = ImageDraw.Draw(img)
    d.text((pad, 24), title, fill="#f1ece2", font=font(26, True))
    if subtitle:
        d.text((pad, 58), subtitle, fill="#b9b1a4", font=font(15))
    for i, (p, c) in enumerate(zip(images, captions)):
        x = pad + (i % cols) * (cell_w + pad)
        y = 90 + (i // cols) * (cell_h + cap + pad)
        im = Image.open(p).convert("RGB").resize((cell_w, cell_h), Image.LANCZOS)
        img.paste(im, (x, y))
        d.text((x, y + cell_h + 10), c, fill="#e8e2d6", font=font(16))
    img.save(out)

# --------------------------------------------------------------------------- QA measurements

def luminance(c):
    def ch(v):
        v /= 255
        return v / 12.92 if v <= .03928 else ((v + .055) / 1.055) ** 2.4
    r, g, b = (int(c[i:i + 2], 16) for i in (1, 3, 5))
    return .2126 * ch(r) + .7152 * ch(g) + .0722 * ch(b)


def contrast(a, b):
    la, lb = sorted((luminance(a), luminance(b)), reverse=True)
    return (la + .05) / (lb + .05)


def text_boxes(m):
    """Estimated boxes (x0, y0, x1, y1, label) of every essential text item, panel-local."""
    out = []
    for c in m["controls"]:
        if c["type"] == "knob":
            for s, y, mono in ((c["label"], c["cy"] - 43, False), (c["value"], c["cy"] + 43, True)):
                w = tw(s, 13, mono)
                out.append((c["cx"] - w / 2, y - 10, c["cx"] + w / 2, y + 3, s))
        elif c["type"] == "select":
            w = tw(c["label"], 13)
            out.append((c["x"] + c["w"] / 2 - w / 2, c["y"] - 18, c["x"] + c["w"] / 2 + w / 2, c["y"] - 5, c["label"]))
            out.append((c["x"], c["y"], c["x"] + c["w"], c["y"] + 28, c["label"] + " options"))
    for p in m["ports"].values():
        w = tw(p["label"], 13)
        if p["lab"] == "right":
            out.append((p["cx"] + 24, p["cy"] - 5, p["cx"] + 24 + w, p["cy"] + 8, p["label"]))
        else:
            out.append((p["cx"] - w / 2, p["cy"] - 29, p["cx"] + w / 2, p["cy"] - 16, p["label"]))
    return out


def overlaps(a, b, gap=0):
    """Boxes intersect, or come closer than `gap` px."""
    return a[0] - gap < b[2] and b[0] - gap < a[2] and a[1] - gap < b[3] and b[1] - gap < a[3]


def qa():
    res = {"contrast": {}, "hits": {}, "text": []}
    for key, th in THEMES.items():
        pairs = {
            "panel label (ink/panel)": (th["ink"], th["panel"]),
            "module kind line (ink2/panel)": (th["ink2"], th["panel"]),
            "output plate label": (th["plate_ink"], th["plate"]),
            "selector on": (th["seg_on_text"], th["seg_on"]),
            "selector off (ink/seg_bg)": (th["ink"], th["seg_bg"]),
            "chrome text": (th["ctext"], th["chrome"]),
            "chrome secondary": (th["ctext2"], th["chrome"]),
            "button text": (th["ctext"], th["btn"]),
            "active button": (th["btn_on_text"], th["btn_on"]),
            "display trace (non-text)": (th["display_ink"], th["display"]),
        }
        for sig in ("audio", "cv", "gate"):
            pairs[f"{sig} cable vs panel (non-text)"] = (th["sig"][sig], th["panel"])
            pairs[f"{sig} ring vs output plate (non-text)"] = (th["sig"][sig], th["plate"])
        for sig in ("audio", "cv", "gate"):
            edge = mix(th["sig"][sig], "#000000", .45)
            pairs[f"{sig} two-tone best vs panel (non-text)"] = max(
                (th["sig"][sig], th["panel"]), (edge, th["panel"]), key=lambda p: contrast(*p))
            pairs[f"{sig} two-tone best vs plate (non-text)"] = max(
                (th["sig"][sig], th["plate"]), (edge, th["plate"]), key=lambda p: contrast(*p))
        pairs["selection vs panel (non-text)"] = (th["sel"], th["panel"])
        pairs["selection vs rack (non-text)"] = (th["sel"], th["rack"])
        res["contrast"][key] = {k: round(contrast(*v), 2) for k, v in pairs.items()}
    mods, _ = build_patch(REF_ROWS, REF_CABLES)
    regs = hit_regions(mods)
    res["hits"]["count"] = len(regs)
    res["hits"]["min_w_h"] = [min(r[3] for r in regs), min(r[4] for r in regs)]
    boxes = [(r[1], r[2], r[1] + r[3], r[2] + r[4], r[0]) for r in regs]
    res["hits"]["overlapping_pairs"] = [(a[4], b[4]) for i, a in enumerate(boxes) for b in boxes[i + 1:]
                                        if overlaps(a, b)]
    for m in mods.values():
        tb = text_boxes(m)
        for b in tb:
            if b[0] < 4 or b[2] > m["w"] - 4:
                res["text"].append(f"{m['id']}: '{b[4]}' exceeds panel edge")
        for i, a in enumerate(tb):
            for b in tb[i + 1:]:
                if overlaps(a, b, gap=4):
                    res["text"].append(f"{m['id']}: '{a[4]}' overlaps '{b[4]}'")
    return res



def main():
    if sys.argv[1:] == ["--qa"]:
        print(json.dumps(qa(), indent=1))
        return
    only = set(sys.argv[1:])
    rendered = {}
    for key in THEMES:
        if only and key not in only:
            continue
        th = theme(key)
        rendered[(key, "all")] = emit(key, "all", render_svg(th, base_scene())[0])
        rendered[(key, "hidden")] = emit(key, "hidden", render_svg(th, base_scene(**HIDDEN))[0])
        emit(key, "components", component_sheet(th))
    if only and "a-warm" not in only:
        return
    th = theme("a-warm")
    emit("a-warm", "focus", render_svg(th, base_scene(cables="focus", selected=("filter",),
                                                      hint="Focus: cables touching the selection stay solid"))[0])
    emit("a-warm", "all-200pct", render_svg(th, base_scene())[0], scale=2)
    emit("a-warm", "layout-hit-regions", render_svg(th, base_scene(overlay_hits=True))[0])
    emit("a-warm", "long-names", render_svg(th, base_scene(
        patch_name="Evening pad — slow LFO sweep with long release tails (v3)", save_state="Unsaved changes"),
        patch=build_patch(REF_ROWS, REF_CABLES, names={
            "filter": "Resonant State-Variable Filter", "osc": "Virtual-Analog Oscillator (main voice)",
            "lfo": "Slow Sweep LFO", "env": "Amplitude Envelope Generator"},
            params={"filter": dict(res_cv_label="Res CV")}))[0])
    cp = crowded_patch()
    emit("a-warm", "crowded-working", render_svg(th, base_scene(
        cables="focus", selected=("filter",), pan=240, scrollbar=True, patch_name="Dual-voice sketch",
        caption="crowded: 14 modules, 21 connections, 100% zoom, panned",
        hint="Focus + pan keep a dense patch legible at working scale"), patch=cp)[0])
    emit("a-warm", "crowded-hidden", render_svg(th, base_scene(
        cables="hidden", selected=("filter",), pan=240, scrollbar=True, patch_name="Dual-voice sketch",
        caption="crowded, cables hidden", hint="Badges: '→ 2 routes' opens a list of both connections"),
        patch=cp)[0])
    emit("a-warm", "crowded-overview", render_svg(th, base_scene(
        cables="focus", selected=("filter",), zoom=.5, patch_name="Dual-voice sketch",
        popover=("filter", 1010, 60), caption="crowded at 50% overview, selection shown at 100%",
        hint="Overview simplifies panels; the selected module stays editable at full size"), patch=cp)[0])
    frames = []
    for i, (cap, sc) in enumerate(storyboard_frames(th)):
        p = emit("a-warm", f"story-{i + 1}", render_svg(th, sc)[0])
        frames.append((p, cap))
    board([p for p, _ in frames], 2, 620, [c for _, c in frames],
          "Storyboard — assign LFO, set amount, hide cables, inspect, undo (Direction A)",
          HERE / "storyboard.png", "Concept frames. Engine support for per-cable amount is not built yet (BRIEF.md).")
    pframes = []
    for i, (cap, sc, patch) in enumerate(perf_frames()):
        p = emit("a-warm", f"perf-{i + 1}", render_svg(th, sc, patch=patch)[0])
        pframes.append((p, cap))
    board([p for p, _ in pframes], 2, 620, [c for _, c in pframes],
          "Future performance — can the layout carry sequences, transport, layers and effects?",
          HERE / "storyboard-performance.png",
          "Concept only. Clock, sequencers, layer mixer, delay and reverb are not built (PLAN.md musical north star).")
    if not only and len(rendered) == 6:
        imgs, caps = [], []
        for view in ("all", "hidden"):
            for key in THEMES:
                imgs.append(rendered[(key, view)])
                caps.append(f"{THEMES[key]['title']} · cables {'visible' if view == 'all' else 'hidden'}")
        board(imgs, 3, 600, caps, "kabl — three visual directions, same patch and layout",
              HERE / "comparison.png", "Concept mockups at 1280×800, shown here at 47%. Open each PNG for actual size.")
    (BUILD / "qa.json").write_text(json.dumps(qa(), indent=1))


if __name__ == "__main__":
    main()
