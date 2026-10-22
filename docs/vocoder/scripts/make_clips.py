#!/usr/bin/env python3
"""Renders the listening clips for the vocoder into docs/vocoder/audio/ (AAC, 160 kb/s).

    cargo build --release -p kabl-engine --example render_midi
    python3 docs/vocoder/scripts/make_clips.py [VOICE.wav]

robot-phrase.m4a   8 s of the robot phrase on its own (the modulator), then the same 8 s spoken by
                   the supersaw chord.
band-sweep.m4a     the same 4 s of phrase and chord at 8, 12, 16, 24 and 32 bands.
live-voice.m4a     VOICE.wav (default: docs/vocoder/audio/voice-espeak.flac) alone, then through
                   the live-voice patch with two chords. Needs the voice at 48 kHz (sox converts).

Needs sox and ffmpeg. Scratch files go to scratch/vocoder-clips/.
"""
import subprocess as sp
import sys
from pathlib import Path

here = Path(__file__).resolve().parent
sys.path.insert(0, str(here))
import make_demos  # noqa: E402

repo = here.parents[2]
SCR = repo / 'scratch/vocoder-clips'
OUT = repo / 'docs/vocoder/audio'
RENDER = repo / 'target/release/examples/render_midi'
CHORD_E = [0x34, 0x3B, 0x40, 0x43]  # E3 B3 E4 G4
CHORD_C = [0x30, 0x37, 0x3C, 0x40]  # C3 G3 C4 E4


def run(*a):
    sp.run([str(x) for x in a], check=True, stdout=sp.DEVNULL)


def events(path, spans):
    """spans: (start s, end s, notes)."""
    lines = []
    for a, b, notes in spans:
        lines += [f'{a} 90 {n:02x} 5a' for n in notes] + [f'{b} 80 {n:02x} 00' for n in notes]
    path.write_text('\n'.join(lines) + '\n')


def render(patch, ev, wav, secs, audio_in=None):
    extra = ['--audio-in', audio_in] if audio_in else []
    run(RENDER, SCR / 'patches' / patch, ev, wav, '--tail', 0.5, *extra)
    run('sox', wav, wav.with_suffix('.cut.wav'), 'trim', 0, secs)
    return wav.with_suffix('.cut.wav')


def trim(src, dst, start, secs):
    run('sox', src, dst, 'trim', start, secs, 'fade', 'h', 0.02, secs, 0.05)
    return dst


def silence(secs):
    f = SCR / f'sil{secs}.wav'
    run('sox', '-n', '-r', 48000, '-c', 2, '-b', 32, '-e', 'floating-point', f, 'trim', 0, secs)
    return f


def encode(parts, name):
    OUT.mkdir(parents=True, exist_ok=True)
    wav = SCR / f'{name}.wav'
    run('sox', *parts, wav)
    run('ffmpeg', '-y', '-loglevel', 'error', '-i', wav, '-c:a', 'aac', '-b:a', '160k', OUT / f'{name}.m4a')
    print('wrote', OUT / f'{name}.m4a')


def main():
    SCR.mkdir(parents=True, exist_ok=True)
    make_demos.clips(SCR / 'patches')
    ev = SCR / 'chord.txt'
    events(ev, [(0.2, 9.5, CHORD_E)])
    dry = render('phrase-dry', ev, SCR / 'dry.wav', 9)
    wet = render('phrase-20', ev, SCR / 'wet.wav', 9) if (SCR / 'patches/phrase-20').exists() else None
    if wet is None:
        make_demos.robot_voice(bands=20).save(SCR / 'patches/phrase-20')
        wet = render('phrase-20', ev, SCR / 'wet.wav', 9)
    encode([trim(dry, SCR / 'a.wav', 0.5, 8), silence(0.6), trim(wet, SCR / 'b.wav', 0.5, 8)], 'robot-phrase')

    parts = []
    for n in (8, 12, 16, 24, 32):
        w = render(f'phrase-{n}', ev, SCR / f'p{n}.wav', 5)
        parts += [trim(w, SCR / f'p{n}t.wav', 0.5, 4), silence(0.4)]
    encode(parts, 'band-sweep')

    voice = Path(sys.argv[1]) if len(sys.argv) > 1 else OUT / 'voice-espeak.flac'
    v48 = SCR / 'voice48.wav'
    run('sox', voice, '-r', 48000, '-c', 1, '-b', 16, v48)
    secs = float(sp.check_output(['soxi', '-D', v48]).decode()) + 0.5
    evl = SCR / 'chords.txt'
    events(evl, [(0.2, secs / 2, CHORD_E), (secs / 2, secs, CHORD_C)])
    live = render('live', evl, SCR / 'live.wav', secs, audio_in=v48)
    v2 = SCR / 'voice-stereo.wav'
    run('sox', v48, '-c', 2, '-b', 32, '-e', 'floating-point', v2)
    encode([v2, silence(0.6), live], 'live-voice')


main()
