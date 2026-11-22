#!/usr/bin/env python3
"""Promo v2 audio animatic (docs/promo/STORYBOARD-V2.md).

Renders every segment of the storyboard offline through the real engine (the `promo_render`
example), places them on a 124 BPM bar grid with sample-accurate cuts, layers the bed, the riser
and the impacts, and masters the mix to about -14 LUFS with true peak at or under -1 dBFS.

    python3 docs/promo/build_animatic.py OUT_DIR

Run from the repository root. Needs the release build of the renderer
(`cargo build --release -p kabl-tools --example promo_render`; `PROMO_RENDER` overrides its path),
numpy, scipy and ffmpeg. Writes OUT_DIR/promo-v2-animatic.wav and .mp3 (320k) and
OUT_DIR/promo-v2-cuesheet.txt; the segment renders go to OUT_DIR/segments/.
"""

import json
import os
import re
import subprocess
import sys

import numpy as np
from scipy.io import wavfile

BPM = 124.0
BEAT = 60.0 / BPM
BAR = 4 * BEAT
SR = 48000
P = "patches/presets/"
LICHEN_PLUS = "docs/promo/lichen-plus.kabl"
# Crossfade at a cut (seconds): the outgoing segment fades out over this after the bar line, the
# incoming one fades in over it from the bar line.
XFADE = 0.008

# The progression of the build, one chord a bar: Dm, Bb, F, C (D minor, Lichen's D).
DM, BB, F, C = [50, 53, 57, 62], [46, 50, 53, 58], [48, 53, 57, 60], [48, 52, 55, 60]


def held(notes, on=0.0, off=3.9, vel=110):
    """Hits for a chord held from beat `on` to beat `off`."""
    return [[on * BEAT, n, vel] for n in notes] + [[off * BEAT, n, 0] for n in notes]


def line(steps, vel=110):
    """Hits for (beat, length in beats, notes) steps."""
    out = []
    for at, length, notes in steps:
        for n in notes if isinstance(notes, list) else [notes]:
            out += [[at * BEAT, n, vel], [(at + length) * BEAT, n, 0]]
    return out


def seg(name, bar, bars, patch, *, what, visual, gain=-18.0, preroll=0.0, render_bars=None,
        offset=0.0, tail=0.05, fade_out=XFADE, fade_in=XFADE, layer=False, shape=None,
        **spec):
    """One placed render. `bar`/`bars`: where it sits in the promo; `offset`: seconds into its own
    render where the promo picks it up; `gain`: the RMS (dBFS) it is set to over its placed bars;
    `layer`: mixed under or over the cuts instead of being one of them."""
    return dict(name=name, bar=bar, bars=bars, patch=patch, what=what, visual=visual, gain=gain,
                preroll=preroll, render_bars=render_bars, offset=offset, tail=tail,
                fade_out=fade_out, fade_in=fade_in, layer=layer, shape=shape, spec=spec)


def impact(name, bar, note, *, boom=0.6, gain=-14.0, tail=3.0, what="Depth Charge thump on the cut, D2"):
    return seg(name, bar, 1, P + "depth-charge", what=what, visual="(hit on the cut)", gain=gain,
               tail=tail, fade_out=1.5, layer=True, macros={"1": boom, "2": 0.7},
               hits=[[0.0, note, 127], [0.4, note, 0]])


S = [
    # Cold open.
    seg("01-temple-bowl", 0, 1, P + "temple-bowl", gain=-17,
        what="cold open: hard mallet strike, D3 + A3, rings",
        visual="black, then the Sounds page card Temple Bowl lit; the Strike knob snaps to 0.9",
        macros={"1": 0.9, "3": 0.8}, hits=held([50, 57], 0, 3.5, 127)),
    seg("02-sock-puppet", 1, 1, P + "sock-puppet", gain=-17,
        what="talkbox line D-F-A-C, Mouth swept a to u, Chatter on",
        visual="Mouth macro knob sweeping, vocoder module's bands flickering",
        bpm=BPM, preroll=BAR, macros={"2": 0.6, "3": 0.6},
        ramps=[[1, 0.0, BAR, 0.05, 0.95]],
        hits=line([(0, .4, 62), (.5, .4, 62), (1, .4, 65), (1.5, .4, 69), (2, .9, 72),
                   (3, .4, 69), (3.5, .45, 74)])),
    # Build: one preset a bar over the Loading Dock pulse, half bars at the end.
    seg("03-laundromat", 2, 1, P + "laundromat", what="Piece: cue Full", bpm=BPM,
        preroll=4 * BAR, cues=[[-4 * BAR, 4]],
        visual="Perform view, pad Full fires (teal flash)"),
    seg("04-glass-elevator", 3, 1, P + "glass-elevator", what="Keys: Bb chord, 3-3-2 rhythm",
        visual="Rack, cables glowing from the keyboard into the tine voice",
        hits=line([(0, 1.3, BB), (1.5, 1.3, BB), (3, .9, BB)])),
    seg("05-acid-line", 4, 1, P + "acid-line", what="Groove: Cutoff swept 0.3 to 0.9",
        visual="macro knob Cutoff sweep, filter cable brightening", bpm=BPM, preroll=2 * BAR,
        ramps=[[1, 0.0, BAR, 0.3, 0.9]]),
    seg("06-horn-section", 5, 1, P + "horn-section", what="Lead: C-D-E-G horn line",
        visual="Sounds page flips to Lead, Horn Section card",
        hits=line([(0, .9, 72), (1, .4, 74), (1.5, 1.4, 76), (3, .9, 79)])),
    seg("07-balcony", 6, 1, P + "balcony", what="Piece: cue Chorus", bpm=BPM,
        preroll=4 * BAR, cues=[[-4 * BAR, 3]],
        visual="Perform scenes: Chorus pad fires, macro rings jump"),
    seg("08-vox-chop", 7, 1, P + "vox-chop", what="Voice: Bb house stabs, Vowels up",
        visual="cable palette switches (cables re-colour across the rack)", bpm=BPM,
        preroll=BAR, macros={"2": 0.8},
        hits=line([(0, .3, BB), (.75, .3, BB), (1.5, .3, BB), (2.5, .3, BB), (3.25, .5, BB)])),
    seg("09-dune-buggy", 8, 1, P + "dune-buggy", what="Groove: as stored", bpm=BPM,
        preroll=2 * BAR, visual="Rack zoomed out, every cable pulsing on the beat"),
    seg("10-pulpit", 9, 1, P + "pulpit", what="Keys: C organ chord, Rotor spun up",
        visual="Rotor knob sweep, the code module's waveform scope speeding up",
        macros={"3": 0.2}, ramps=[[3, 0.0, BAR, 0.2, 1.0]], hits=held([48, 55, 60, 64], 0, 4.2)),
    seg("11-oracle", 10, .5, P + "oracle", what="Voice: Dm choir, Vowel a to o",
        visual="Sounds page: Voice row", preroll=1.0, ramps=[[1, 0.0, BAR / 2, 0.0, 0.8]],
        hits=held([50, 57, 62, 65], -1.0 / BEAT, 2.2)),
    seg("12-undertow", 10.5, .5, P + "undertow", what="Bass: D wobble",
        visual="Wobble macro flick", hits=held([38], 0, 2.2, 120)),
    seg("13-pawn-shop", 11, .5, P + "pawn-shop", what="Piece: cue Hook", bpm=BPM,
        preroll=4 * BAR, cues=[[-4 * BAR, 2]], visual="Perform: Hook pad"),
    seg("14-spark-plug", 11.5, .5, P + "spark-plug", what="Pluck: Bb arpeggio in 16ths",
        visual="sequencer steps lighting",
        hits=line([(i * .25, .2, [58, 62, 65, 70][i % 4]) for i in range(8)])),
    seg("15-fault-line", 12, .5, P + "fault-line", what="Groove: reese and break at 124",
        bpm=BPM, preroll=2 * BAR, visual="Rack, the reese's cables glowing hot"),
    seg("16-copper-mile", 12.5, .5, P + "copper-mile", what="Groove: Sweep up", bpm=BPM,
        preroll=2 * BAR, ramps=[[1, 0.0, BAR / 2, 0.5, 1.0]], visual="Sweep knob turning"),
    seg("17-hot-wire", 13, .5, P + "hot-wire", what="Lead: C squelch in 16ths",
        visual="Squelch macro sweep", ramps=[[1, 0.0, BAR / 2, 0.3, 1.0]],
        hits=line([(i * .25, .2, [48, 60][i % 2]) for i in range(8)])),
    seg("18-short-circuit", 13.5, .375, P + "short-circuit", what="Lead: Siren on, G held; then 1/8 bar of silence",
        visual="Siren knob slammed, then a black frame", macros={"4": 1.0}, fade_out=0.004,
        hits=held([67], 0, 1.5, 120)),
    # Layers of the build.
    seg("bed-loading-dock", 2, 11.875, P + "loading-dock", gain=-25, layer=True,
        what="bed: Loading Dock Intro, Stabs 0; Sweep opens over bars 12-14",
        visual="(the pulse under the build)", bpm=BPM, preroll=2 * BAR, fade_out=0.004,
        cues=[[-2 * BAR, 1]], macros={"2": 0.0}, ramps=[[1, 10 * BAR, 11.8 * BAR, 0.12, 1.0]]),
    seg("riser-updraft", 11, 2.875, P + "updraft", gain=-21, layer=True, shape=(-14.0, 2.0),
        fade_out=0.004, what="riser: Updraft on D3, Lift 1, Build 0.45",
        visual="Updraft's Build knob, the Rack's filter cable brightening to white",
        macros={"1": 0.45, "2": 1.0, "3": 0.4}, hits=[[0.0, 50, 115], [2.9 * BAR, 50, 0]]),
    impact("hit-bar2", 2, 38, boom=0.3, gain=-20),
    impact("hit-bar6", 6, 38, boom=0.2, gain=-22),
    impact("hit-bar10", 10, 38, boom=0.2, gain=-21),
    # The drop.
    seg("19-lichen-bloom", 14, 4, LICHEN_PLUS, gain=-14,
        what="DROP: Lichen + Balcony chords, cue Bloom+ (Focus and Chords at 1)",
        visual="full Rack, every cable glowing, the Bloom+ pad lit; camera pushes in",
        bpm=BPM, preroll=6 * BAR, render_bars=8, cues=[[-6 * BAR, 4]],
        ramps=[[4, 7 * BAR, 8 * BAR, 0.35, 1.0]]),
    impact("hit-drop", 14, 38, boom=0.9, gain=-12, tail=6.0,
           what="Depth Charge impact on D2 (Boom 0.9)"),
    seg("20-gantry-peak", 18, 2, P + "gantry", gain=-14, what="Gantry: cue Peak at 124",
        visual="Perform view: Peak pad fires, all eight macro rings full", bpm=BPM,
        preroll=4 * BAR, cues=[[-4 * BAR, 5]]),
    impact("hit-gantry", 18, 33, boom=0.7, gain=-14, tail=4.0,
           what="Depth Charge impact on A1"),
    seg("21-lichen-back", 20, 2, LICHEN_PLUS, gain=-14, offset=6 * BAR, tail=3.0,
        fade_out=0.35, what="back to Lichen Bloom+ (the take kept running); Echo up to 1 in the last bar",
        visual="Rack again, the Echo macro sweeping, cables fading to the echo's colour",
        bpm=BPM, preroll=6 * BAR, render_bars=8, cues=[[-6 * BAR, 4]],
        ramps=[[4, 7 * BAR, 8 * BAR, 0.35, 1.0]]),
    # The breath.
    seg("22-estuary", 22, 4, P + "estuary", gain=-21, fade_in=0.15, tail=2.0, fade_out=1.5,
        what="breath: Estuary, cue High Tide, at 62 BPM (half of 124); under Chandelier from bar 24",
        visual="slow push over the Estuary rack, bells' cables blinking in 5, 7 and 11",
        bpm=62, preroll=16.0, cues=[[-16.0, 3]], shape=(0.0, 0.0, -7.0)),
    seg("23-chandelier", 24, 2, P + "chandelier", gain=-21, fade_in=0.2, tail=0.05,
        fade_out=0.004, what="Chandelier bowed D5 + A5 over Estuary's tail",
        visual="Bow knob turning, the chandelier's scope ringing; light dims",
        preroll=1.0, macros={"3": 0.8}, hits=held([74, 81], -1.0 / BEAT, 8.5)),
    # The final hit and end card.
    seg("24-final-lichen", 26, .25, LICHEN_PLUS, gain=-13, fade_out=0.03,
        what="final hit: one beat of Lichen Bloom+",
        visual="white flash, the whole rack for one beat", bpm=BPM, preroll=6 * BAR,
        cues=[[-6 * BAR, 4]]),
    impact("hit-final", 26, 38, boom=1.0, gain=-11, tail=7.0,
           what="Depth Charge on D2 (Boom 1)"),
    seg("hit-final-bowl", 26, 3, P + "temple-bowl", gain=-20, layer=True, tail=1.0,
        fade_out=1.0, what="Temple Bowl D3 + A3 rings under the end card",
        visual="end card: \"kabl 1.0: a whole song in one patch\"",
        macros={"1": 1.0, "3": 1.0}, hits=held([50, 57], 0, 2.0, 127)),
]
END = 28 + 0.6  # bars; the last bowl ring fades out over the last second


def render(out):
    spec = {"segments": []}
    for s in S:
        secs = max(s["offset"] + s["bars"] * BAR, (s["render_bars"] or 0) * BAR)
        spec["segments"].append(dict(name=s["name"], patch=s["patch"], preroll=s["preroll"],
                                     secs=secs + max(s["tail"], s["fade_out"]) + 0.05,
                                     **s["spec"]))
    path = os.path.join(out, "spec.json")
    json.dump(spec, open(path, "w"), indent=1)
    exe = os.environ.get("PROMO_RENDER", os.environ.get("CARGO_TARGET_DIR", "target")
                         + "/release/examples/promo_render")
    subprocess.run([exe, "render", path, os.path.join(out, "segments")], check=True)


def db(x):
    return 20 * np.log10(max(x, 1e-9))


def mix(out):
    total = int(END * BAR * SR)
    bus = np.zeros((total + SR * 8, 2))
    for s in S:
        _, x = wavfile.read(os.path.join(out, "segments", s["name"] + ".wav"))
        x = x.astype(np.float64)
        a = int(round(s["offset"] * SR))
        n = int(round(s["bars"] * BAR * SR))
        tail = int(round((s["tail"] if s["layer"] or s["fade_out"] > XFADE else 0) * SR))
        fo = int(round(s["fade_out"] * SR))
        fi = int(round(s["fade_in"] * SR))
        y = x[a:a + n + max(tail, fo)].copy()
        body = y[:n]
        rms = np.sqrt((body ** 2).mean()) if len(body) else 0
        y *= 10 ** ((s["gain"] - db(rms)) / 20)
        g = np.ones(len(y))
        if s["shape"]:
            # A gain curve over the placed bars (dB points, evenly spaced), then held.
            pts = np.array(s["shape"])
            g[:n] = 10 ** (np.interp(np.linspace(0, 1, n), np.linspace(0, 1, len(pts)), pts) / 20)
            g[n:] = 10 ** (pts[-1] / 20)
        g[:fi] *= np.linspace(0, 1, fi) if fi else 1
        end = n + fo
        g[n:end] *= np.linspace(1, 0, end - n)
        g[end:] = 0
        y *= g[:, None]
        at = int(round(s["bar"] * BAR * SR))
        bus[at:at + len(y)] += y
    bus = bus[:total + SR]
    fade = SR
    bus[-fade:] *= np.linspace(1, 0, fade)[:, None]
    return bus


def master(bus, out):
    raw = os.path.join(out, "promo-v2-mix.wav")
    wavfile.write(raw, SR, bus.astype(np.float32))
    wav = os.path.join(out, "promo-v2-animatic.wav")
    pre = 0.0
    for _ in range(6):
        chain = f"volume={pre:.2f}dB,alimiter=limit=0.79:attack=3:release=60:level=0"
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", raw, "-af", chain, "-c:a", "pcm_f32le",
                        wav], check=True)
        lufs, tp = measure(wav)
        if abs(lufs + 14.0) < 0.15:
            break
        pre += -14.0 - lufs
    mp3 = os.path.join(out, "promo-v2-animatic.mp3")
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", wav, "-c:a", "libmp3lame", "-b:a", "320k",
                    mp3], check=True)
    return measure(mp3)


def measure(path):
    r = subprocess.run(["ffmpeg", "-nostats", "-i", path, "-af", "ebur128=peak=true", "-f", "null",
                        "-"], capture_output=True, text=True).stderr
    summary = r[r.rfind("Summary:"):]
    lufs = float(re.search(r"I:\s+(-?[\d.]+) LUFS", summary).group(1))
    tp = float(re.search(r"Peak:\s+(-?[\d.]+|-inf) dBFS", summary).group(1))
    return lufs, tp


def stamp(bar):
    t = bar * BAR
    return f"{int(t // 60)}:{t % 60:06.3f}"


def cuesheet(out, lufs, tp):
    lines = [f"kabl promo v2 animatic: {BPM:.0f} BPM, bar = {BAR:.3f} s, {END * BAR:.1f} s,"
             f" {lufs:.1f} LUFS, true peak {tp:.1f} dBFS", ""]
    presets = set()
    for s in sorted(S, key=lambda s: (s["bar"], s["layer"])):
        presets.add(os.path.basename(s["patch"]).removesuffix(".kabl"))
        lines.append(f"{stamp(s['bar'])}  bar {s['bar']:>6g}  {'+' if s['layer'] else ' '} "
                     f"{os.path.basename(s['patch']):18}  {s['what']}")
    lines += [f"{stamp(26)}  bar     26    (end card)          \"kabl 1.0: a whole song in one"
              " patch\" over the ring-out",
              f"{stamp(END)}  end (the last second fades out)", "",
              f"{len(presets)} patches ('+' = layered over the cuts; lichen-plus is Lichen"
              " plus Balcony's chords, docs/promo/lichen-plus.kabl)."]
    open(os.path.join(out, "promo-v2-cuesheet.txt"), "w").write("\n".join(lines) + "\n")
    return len(presets)


if __name__ == "__main__":
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    if os.environ.get("PROMO_SKIP_RENDER") is None:
        render(out)
    lufs, tp = master(mix(out), out)
    n = cuesheet(out, lufs, tp)
    print(f"{END * BAR:.1f} s, {n} patches, {lufs:.1f} LUFS, true peak {tp:.1f} dBFS")
