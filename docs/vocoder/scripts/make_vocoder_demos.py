#!/usr/bin/env python3
"""Writes the two vocoder demo patches into patches/sound-engines/demos/.

    python3 docs/vocoder/scripts/make_vocoder_demos.py [OUTROOT]

robot-voice  A saw chord (play keys) spoken by a phrase that plays itself: a buzzing saw through
             the formant filter, its vowel stepping through a random loop, gated by a clock, with
             a noise burst at the start of each syllable. No microphone needed.
live-voice   The same chord vocoded by whatever arrives on Audio in: a microphone in the
             standalone (kabl-ui --input), the host's audio input in the plugin.

Both write meta.toml + log.jsonl (the layout `kabl_core::load` reads) plus sound.toml.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'effects-filters/scripts'))
from make_demos import Patch  # noqa: E402

repo = Path(__file__).resolve().parents[3]


def place(p, i, x, y):
    for o in p.ops:
        if 'AddModule' in o['op'] and o['op']['AddModule']['id'] == i:
            o['op']['AddModule']['pos'] = {'x': float(x), 'y': float(y)}


def chord(p, row, self_playing=False):
    """midi.in -> supersaw -> envelope VCA -> gain. Returns (gain, ids). `self_playing`: a clock
    and a random loop in place of the keyboard, for captures that need no player."""
    if self_playing:
        ck = p.add('clock', bpm=52)
        rnd = p.add('random', change=100, length=4, range=9)
        kb = p.add('quantizer', root=4, scale=2, transpose=-12)
        p.wire(ck, 'gate', rnd, 'clock')
        p.wire(rnd, 'out', kb, 'in')
        pitch, gate = 'out', 'trig'
    else:
        kb = p.add('midi.in')
        pitch, gate = 'pitch', 'gate'
    osc = p.add('osc.va', waveform=2, unison=4, detune=18)
    env = p.add('env.adsr', attack_ms=12, decay_ms=300, sustain=0.85, release_ms=260)
    vca = p.add('vca', gain=0)
    g = p.add('gain', gain_db=-6)
    p.wire(kb, pitch, osc, 'pitch')
    p.wire(kb, gate, env, 'gate')
    p.wire(osc, 'out', vca, 'in')
    p.wire(env, 'out', vca, 'cv')
    p.wire(vca, 'out', g, 'in')
    for k, i in enumerate((kb, osc, env, vca, g)):
        place(p, i, 24 + 400 * (k % 3), 10 + 370 * (row + k // 3))
    return g


def finish(p, voc, tail_gain_db, row):
    gl = p.add('gain', gain_db=tail_gain_db)
    gr = p.add('gain', gain_db=tail_gain_db)
    out = p.add('out')
    p.wire(voc, 'left', gl, 'in')
    p.wire(voc, 'right', gr, 'in')
    p.wire(gl, 'out', out, 'left')
    p.wire(gr, 'out', out, 'right')
    place(p, gl, 24 + 400 * 2, 10 + 370 * row)
    place(p, gr, 24 + 400 * 2, 10 + 370 * (row + 1))
    place(p, out, 24 + 400 * 3, 10 + 370 * row)


def robot_voice(bands=20, dry=False, self_playing=False):
    """dry: the modulator alone (the phrase, before the vocoder), for the clips."""
    p = Patch()
    car = chord(p, 0, self_playing)
    # The phrase: a buzz whose pitch and vowel wander on two random loops, one syllable per tick.
    clock = p.add('clock', bpm=104)
    pitch = p.add('random', change=100, length=16, range=7)
    q = p.add('quantizer', root=2, scale=4, transpose=-24)
    vowel = p.add('random', change=100, length=16, range=24)
    buzz = p.add('osc.va', waveform=2)
    form = p.add('filter.formant', vowel=2.0, q=1.2)
    syl = p.add('env.adsr', attack_ms=6, decay_ms=170, sustain=0.5, release_ms=70)
    vca = p.add('vca', gain=0)
    noise = p.add('noise', level_db=-12)
    burst = p.add('env.adsr', attack_ms=10, decay_ms=70, sustain=0, release_ms=20)
    nvca = p.add('vca', gain=0)
    mix = p.add('mixer', level1=1.0, level2=0.5)
    p.wire(clock, 'gate', pitch, 'clock')
    p.wire(pitch, 'out', q, 'in')
    p.wire(clock, 'gate', vowel, 'clock')
    p.wire(q, 'out', buzz, 'pitch')
    p.wire(buzz, 'out', form, 'in')
    p.wire(vowel, 'out', form, 'vowel', amount=0.5, param=True)
    p.wire(clock, 'gate', syl, 'gate')
    p.wire(form, 'out', vca, 'in')
    p.wire(syl, 'out', vca, 'cv')
    p.wire(clock, 'gate', burst, 'gate')
    p.wire(noise, 'out', nvca, 'in')
    p.wire(burst, 'out', nvca, 'cv')
    p.wire(vca, 'out', mix, 'in1')
    p.wire(nvca, 'out', mix, 'in2')
    if dry:
        g = p.add('gain', gain_db=-9)
        out = p.add('out')
        p.wire(mix, 'out', g, 'in')
        p.wire(g, 'out', out, 'left')
        p.wire(g, 'out', out, 'right')
        return p
    voc = p.add('vocoder', bands=bands, hf=0.4, drive_db=30, release_ms=45)
    p.wire(car, 'out', voc, 'in_l')
    p.wire(car, 'out', voc, 'in_r')
    p.wire(mix, 'out', voc, 'mod')
    finish(p, voc, 5.0, 2)
    for k, i in enumerate((clock, pitch, q, vowel, buzz, form, syl, vca, noise, burst, nvca, mix)):
        place(p, i, 24 + 400 * (k % 4), 10 + 370 * (3 + k // 4))
    place(p, voc, 24 + 400 * 1, 10 + 370 * 2)
    return p


def live_voice():
    p = Patch()
    car = chord(p, 0)
    ain = p.add('audio.in', gain_db=0)
    voc = p.add('vocoder', bands=24, hf=0.5, release_ms=45)
    p.wire(car, 'out', voc, 'in_l')
    p.wire(car, 'out', voc, 'in_r')
    p.wire(ain, 'left', voc, 'mod')
    finish(p, voc, 14.5, 2)
    place(p, ain, 24, 10 + 370 * 2)
    place(p, voc, 24 + 400 * 1, 10 + 370 * 2)
    return p


DEMOS = {
    'robot-voice': (robot_voice, '''name = "Robot Voice"
category = "Lead"
tags = [
    "vocoder",
    "robot",
    "poly",
    "keyboard",
    "vocal",
]
description = "A supersaw chord that speaks. The phrase plays itself: a buzz through the formant filter, a new vowel and a noise click on every tick. Hold chords; Bands is the detail, Shift moves the voice up or down, HF keeps the consonants."
keys = true
sequence = false
'''),
    'live-voice': (live_voice, '''name = "Live Voice"
category = "Lead"
tags = [
    "vocoder",
    "robot",
    "poly",
    "keyboard",
    "vocal",
    "audio input",
]
description = "Your voice, sung through a supersaw chord. Sound comes in on Audio in: start the standalone with --input, or route a track into the plugin's second input. Hold chords while you speak. Silent until something arrives on the input."
keys = true
sequence = false
'''),
}

def clips(root):
    """Patch variants for the clips: the modulator alone, and the phrase at each band count."""
    robot_voice(dry=True).save(root / 'phrase-dry')
    for n in (8, 12, 16, 24, 32):
        robot_voice(bands=n).save(root / f'phrase-{n}')
    live_voice().save(root / 'live')
    robot_voice(self_playing=True).save(root / 'capture')


if __name__ == '__main__':
    if len(sys.argv) > 2 and sys.argv[1] == '--clips':
        clips(Path(sys.argv[2]))
        sys.exit()
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else repo / 'patches/sound-engines/demos'
    for name, (build, toml) in DEMOS.items():
        d = root / name
        build().save(d)
        (d / 'sound.toml').write_text(toml)
        print('wrote', d)
