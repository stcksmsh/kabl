#!/usr/bin/env python3
"""Builds the self-playing demo patches for the 1.0 effects and filters and renders one clip per
module: a dry stretch, then the same music through the module.

    cargo build --release -p kabl-engine --example render_midi
    python3 docs/effects-filters/scripts/make_demos.py [name ...]

Writes patch directories to scratch/effects-demos/<name>-{dry,wet}/ (meta.toml + log.jsonl, the
layout `kabl_core::load` reads), renders each with the engine's own offline renderer
(`render_midi`, no events: the clock plays the music), joins dry + wet and encodes
docs/effects-filters/audio/<name>.m4a (AAC, 160 kb/s). Needs ffmpeg.

The music is the one in patches/sound-engines/demos/scale-walk (random loop held to D dorian,
saw voice, decaying envelope) without its delay and reverb, so the module is heard alone.
"""
import json, os, subprocess as sp, sys
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
SCR = repo / 'scratch/effects-demos'
AUDIO = repo / 'docs/effects-filters/audio'
RENDER = repo / 'target/release/examples/render_midi'
SECS = 7  # per half


class Patch:
    def __init__(self):
        self.ops, self.n, self.cables = [], 0, 0

    def op(self, op, inv):
        self.ops.append({'seq': len(self.ops), 't_ms': 0, 'op': op, 'inverse': inv, 'source': 'User'})

    def add(self, kind, **params):
        self.n += 1
        i = self.n
        self.op({'AddModule': {'id': i, 'kind': kind, 'pos': {'x': 24.0 + 190 * ((i - 1) % 5), 'y': 20.0 + 300 * ((i - 1) // 5)}}},
                {'RemoveModule': {'id': i}})
        for k, v in params.items():
            t = {'Module': {'id': i, 'param': k}}
            self.op({'SetParam': {'target': t, 'value': float(v)}}, {'UnsetParam': {'target': t}})
        return i

    def wire(self, a, pa, b, pb, amount=None, param=False):
        self.cables += 1
        c = self.cables
        to = {'Param': {'id': b, 'param': pb}} if param else {'Module': {'id': b, 'port': pb}}
        self.op({'Connect': {'id': c, 'from': {'Module': {'id': a, 'port': pa}}, 'to': to}}, {'Disconnect': {'id': c}})
        if amount is not None:
            t = {'Cable': {'id': c, 'param': 'amount'}}
            self.op({'SetParam': {'target': t, 'value': float(amount)}}, {'UnsetParam': {'target': t}})

    def save(self, d):
        d.mkdir(parents=True, exist_ok=True)
        (d / 'meta.toml').write_text('schema_version = 5\n')
        (d / 'log.jsonl').write_text(''.join(json.dumps(o, separators=(',', ':')) + '\n' for o in self.ops))


def tune(p):
    """clock -> random -> quantizer; returns (quantizer, clock)."""
    clock = p.add('clock', bpm=96)
    rnd = p.add('random', change=8, length=16, range=12)
    q = p.add('quantizer', root=2, scale=4, transpose=-12)
    p.wire(clock, 'gate', rnd, 'clock')
    p.wire(rnd, 'out', q, 'in')
    return q


def saw_voice(p, q, cutoff=3500):
    osc = p.add('osc.va', detune=14, unison=3, waveform=2)
    env = p.add('env.adsr', attack_ms=2, decay_ms=320, release_ms=380, sustain=0)
    filt = p.add('filter.ladder', cutoff_hz=cutoff, drive_db=0, resonance=0.3)
    vca = p.add('vca', gain=0)
    g = p.add('gain', gain_db=-6)
    p.wire(q, 'out', osc, 'pitch')
    p.wire(q, 'trig', env, 'gate')
    p.wire(osc, 'out', filt, 'in')
    p.wire(filt, 'out', vca, 'in')
    p.wire(env, 'out', vca, 'cv')
    p.wire(vca, 'out', g, 'in')
    p.wire(env, 'out', filt, 'cutoff_hz', amount=0.35, param=True)
    return g


def stereo(p, src, kind, **params):
    """src (mono) -> kind (stereo in) -> out. kind None: straight to out."""
    out = p.add('out')
    if kind is None:
        p.wire(src, 'out', out, 'left')
        p.wire(src, 'out', out, 'right')
        return out
    fx = p.add(kind, **params)
    p.wire(src, 'out', fx, 'in_l')
    p.wire(src, 'out', fx, 'in_r')
    p.wire(fx, 'left', out, 'left')
    p.wire(fx, 'right', out, 'right')
    return fx


def lfo(p, rate, depth_to, param, amount, waveform=0):
    l = p.add('lfo', rate_hz=rate, waveform=waveform)
    p.wire(l, 'out', depth_to, param, amount=amount, param=True)


def with_stereo_fx(kind, **params):
    def build(wet):
        p = Patch()
        g = saw_voice(p, tune(p))
        stereo(p, g, kind if wet else None, **params)
        return p
    return build


def eq(wet):
    p = Patch()
    g = saw_voice(p, tune(p))
    fx = stereo(p, g, 'eq' if wet else None, low_db=3, low_hz=120, mid1_db=6, mid1_hz=700, mid1_q=3,
                high_db=-6, high_hz=6000)
    if wet:
        lfo(p, 0.25, fx, 'mid1_hz', 0.45)
    return p


def comb(wet):
    p = Patch()
    q = tune(p)
    noise = p.add('noise', level_db=-12)
    env = p.add('env.adsr', attack_ms=0, decay_ms=25, release_ms=20, sustain=0)
    vca = p.add('vca', gain=0)
    g = p.add('gain', gain_db=15)
    p.wire(q, 'trig', env, 'gate')
    p.wire(noise, 'out', vca, 'in')
    p.wire(env, 'out', vca, 'cv')
    if wet:
        f = p.add('filter.comb', feedback=94, damp=35)
        p.wire(vca, 'out', f, 'in')
        p.wire(q, 'out', f, 'pitch')
        p.wire(f, 'out', g, 'in')
    else:
        p.wire(vca, 'out', g, 'in')
    stereo(p, g, None)
    return p


def mono_filter(kind, sweep, **params):
    def build(wet):
        p = Patch()
        q = tune(p)
        g = saw_voice(p, q, cutoff=6000)
        if wet:
            f = p.add(kind, **params)
            p.wire(g, 'out', f, 'in')
            if sweep:
                lfo(p, sweep[0], f, sweep[1], sweep[2])
            post = p.add('gain', gain_db=12 if kind == 'filter.formant' else 0)
            p.wire(f, 'out' if kind != 'filter.svf' else 'notch', post, 'in')
            g = post
        stereo(p, g, None)
        return p
    return build


DEMOS = {
    'phaser': with_stereo_fx('phaser', stages=6, rate_hz=0.35, depth=85, feedback=60, mix=55),
    'flanger': with_stereo_fx('flanger', rate_hz=0.18, depth=90, feedback=70, mix=50),
    'compressor': with_stereo_fx('compressor', threshold_db=-30, ratio=8, attack_ms=2, release_ms=250,
                                 knee_db=6, makeup_db=5),
    'eq': eq,
    'limiter': with_stereo_fx('limiter', gain_db=18, ceiling_db=-6, release_ms=120),
    'bitcrusher': with_stereo_fx('bitcrusher', bits=4, downsample=8, mix=100),
    'comb': comb,
    'formant': mono_filter('filter.formant', (0.4, 'vowel', 0.5), vowel=2.0),
    'notch': mono_filter('filter.svf', (0.3, 'cutoff_hz', 0.3), cutoff_hz=1400, resonance=0.5),
}


def render(d, wav):
    ev = SCR / 'no-events.txt'
    ev.write_text('# the clock plays the music\n')
    sp.run([RENDER, d, ev, wav, '--tail', str(SECS)], check=True, capture_output=True)


def main():
    names = sys.argv[1:] or list(DEMOS)
    SCR.mkdir(parents=True, exist_ok=True)
    AUDIO.mkdir(parents=True, exist_ok=True)
    for name in names:
        wavs = []
        for half in ('dry', 'wet'):
            d = SCR / f'{name}-{half}'
            DEMOS[name](half == 'wet').save(d)
            w = SCR / f'{name}-{half}.wav'
            render(d, w)
            wavs.append(w)
        out = AUDIO / f'{name}.m4a'
        cmd = ['ffmpeg', '-y', '-loglevel', 'error', '-i', str(wavs[0]), '-i', str(wavs[1]),
               '-filter_complex', '[0][1]concat=n=2:v=0:a=1[a]', '-map', '[a]', '-c:a', 'aac', '-b:a', '160k', str(out)]
        sp.run(cmd, check=True)
        print(name, out)


if __name__ == '__main__':
    main()
