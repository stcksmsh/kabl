#!/usr/bin/env python3
"""Writes a self-playing patch with all nine effect and filter modules on screen, for capturing
and timing their displays in the real editor.

    python3 docs/effect-displays/scripts/make_patch.py OUTDIR

A saw voice (random loop, D dorian) runs through phaser, flanger, EQ, compressor, bitcrusher and
limiter in series; a noise-burst comb, a formant and an SVF with its notch output patched run
beside it. LFOs move the EQ bell, the formant vowel and the SVF cutoff, so every display animates.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'effects-filters/scripts'))
from make_demos import Patch, lfo, saw_voice, tune  # noqa: E402


def place(p, i, x, y):
    for o in p.ops:
        if 'AddModule' in o['op'] and o['op']['AddModule']['id'] == i:
            o['op']['AddModule']['pos'] = {'x': float(x), 'y': float(y)}


def stereo_chain(p, src, kinds):
    prev, ports = src, ('out', 'out')
    ids = []
    for kind, params in kinds:
        fx = p.add(kind, **params)
        p.wire(prev, ports[0], fx, 'in_l')
        p.wire(prev, ports[1], fx, 'in_r')
        prev, ports = fx, ('left', 'right')
        ids.append(fx)
    return ids, prev


def main():
    p = Patch()
    q = tune(p)
    g = saw_voice(p, q)
    ids, last = stereo_chain(p, g, [
        ('phaser', dict(stages=6, rate_hz=0.35, depth=85, feedback=60, mix=55)),
        ('flanger', dict(rate_hz=0.18, depth=90, feedback=70, mix=50)),
        ('eq', dict(low_db=3, low_hz=120, mid1_db=6, mid1_hz=700, mid1_q=3, high_db=-6, high_hz=6000)),
        ('compressor', dict(threshold_db=-30, ratio=8, attack_ms=2, release_ms=250, knee_db=6, makeup_db=5)),
        ('bitcrusher', dict(bits=4, downsample=8, mix=100)),
        ('limiter', dict(gain_db=18, ceiling_db=-6, release_ms=120)),
    ])
    out = p.add('out')
    p.wire(last, 'left', out, 'left')
    p.wire(last, 'right', out, 'right')
    lfo(p, 0.25, ids[2], 'mid1_hz', 0.45)
    # Beside the chain: the comb on noise bursts, the formant and the SVF notch on the voice.
    noise = p.add('noise', level_db=-12)
    env = p.add('env.adsr', attack_ms=0, decay_ms=25, release_ms=20, sustain=0)
    vca = p.add('vca', gain=0)
    comb = p.add('filter.comb', feedback=94, damp=35)
    p.wire(q, 'trig', env, 'gate')
    p.wire(noise, 'out', vca, 'in')
    p.wire(env, 'out', vca, 'cv')
    p.wire(vca, 'out', comb, 'in')
    p.wire(q, 'out', comb, 'pitch')
    formant = p.add('filter.formant', vowel=2.0)
    p.wire(g, 'out', formant, 'in')
    lfo(p, 0.4, formant, 'vowel', 0.5)
    svf = p.add('filter.svf', cutoff_hz=1400, resonance=0.5)
    p.wire(g, 'out', svf, 'in')
    lfo(p, 0.3, svf, 'cutoff_hz', 0.3)
    notch = p.add('gain', gain_db=0)
    p.wire(svf, 'notch', notch, 'in')
    sink = p.add('out')
    p.wire(notch, 'out', sink, 'left')
    p.wire(comb, 'out', sink, 'right')
    sink2 = p.add('out')
    p.wire(formant, 'out', sink2, 'left')
    # Rows (370 apart): sources and the voice, then three effects to a row, the filters last.
    shown = ids + [comb, formant, svf]
    for n in range(1, p.n + 1):
        if n not in shown:
            place(p, n, 24 + 190 * n, 10)
    for n, i in enumerate(shown):
        place(p, i, 24 + 400 * (n % 3), 10 + 370 * (1 + n // 3))
    p.save(Path(sys.argv[1]))


main()
