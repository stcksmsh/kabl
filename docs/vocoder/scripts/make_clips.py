#!/usr/bin/env python3
"""Renders the listening clips for the vocoder into docs/vocoder/audio/ (FLAC from the float render, AAC 256 kb/s, MP3 320 kb/s).

    cargo build --release -p kabl-engine --example render_midi
    python3 docs/vocoder/scripts/make_clips.py [VOICE.wav]

robot-phrase.*     8 s of the robot phrase on its own (the modulator), then the same 8 s spoken by
                   the supersaw chord.
band-sweep.*      the same 4 s of phrase and chord at 8, 12, 16, 24 and 32 bands.
live-voice.*       VOICE.wav (default: docs/vocoder/audio/voice-espeak-16k.wav) alone, then through
                   the live-voice patch with two chords. Needs the voice at 48 kHz (sox converts).

Needs sox and ffmpeg. Scratch files go to scratch/vocoder-clips/.
"""
import subprocess as sp
import sys
from pathlib import Path

import numpy as np

here = Path(__file__).resolve().parent
sys.path.insert(0, str(here))
import make_vocoder_demos as make_demos  # noqa: E402

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


def pcm(path):
    """Decodes any file to 48 kHz float32 stereo, (samples, 2)."""
    raw = SCR / 'decoded.f32'
    run('ffmpeg', '-y', '-loglevel', 'error', '-i', path, '-f', 'f32le', '-ac', 2, '-ar', 48000, raw)
    return np.fromfile(raw, '<f4').reshape(-1, 2)


def error_report(wav, coded):
    """Coding error against the float render, per band, in dB under the signal of the same band."""
    a, b = pcm(wav), pcm(coded)
    # Align by the cross-correlation peak (encoder delay), then compare the common length.
    lag = np.argmax(np.correlate(b[:20000, 0], a[:20000, 0][4096:12192], 'valid')) - 4096
    b = b[lag:] if lag > 0 else np.pad(b, ((-lag, 0), (0, 0)))
    n = min(len(a), len(b))
    fa, fb = np.fft.rfft(a[:n], axis=0), np.fft.rfft(b[:n], axis=0)
    f = np.fft.rfftfreq(n, 1 / 48000)
    out = []
    for lo, hi in ((0, 24000), (6000, 12000), (12000, 24000)):
        m = (f >= lo) & (f < hi)
        sig, err = (np.abs(fa[m]) ** 2).sum(), (np.abs(fa[m] - fb[m]) ** 2).sum()
        out.append(10 * np.log10(sig / (err + 1e-30)))
    return out


def deliver(wav, name):
    """The float render as FLAC (lossless), 256 kb/s AAC and 320 kb/s MP3, and how far under the
    signal each codec's error sits (overall, 6 to 12 kHz, above 12 kHz)."""
    OUT.mkdir(parents=True, exist_ok=True)
    run('ffmpeg', '-y', '-loglevel', 'error', '-i', wav, '-c:a', 'flac', OUT / f'{name}.flac')
    run('ffmpeg', '-y', '-loglevel', 'error', '-i', wav, '-c:a', 'aac', '-b:a', '256k', OUT / f'{name}.m4a')
    run('ffmpeg', '-y', '-loglevel', 'error', '-i', wav, '-b:a', '320k', OUT / f'{name}.mp3')
    run('ffmpeg', '-y', '-loglevel', 'error', '-i', wav, '-c:a', 'aac', '-b:a', '160k', SCR / f'{name}-160.m4a')
    for label, p in (('flac', OUT / f'{name}.flac'), ('aac 256k', OUT / f'{name}.m4a'),
                     ('mp3 320k', OUT / f'{name}.mp3'), ('aac 160k', SCR / f'{name}-160.m4a')):
        t, mid, hi = error_report(wav, p)
        print(f'{name:14s} {label:9s} signal over error: all {t:5.1f} dB, 6-12 kHz {mid:5.1f} dB, >12 kHz {hi:5.1f} dB')


def encode(parts, name):
    wav = SCR / f'{name}.wav'
    run('sox', *parts, wav)
    deliver(wav, name)


def main():
    SCR.mkdir(parents=True, exist_ok=True)
    make_demos.clips(SCR / 'patches')
    ev = SCR / 'chord.txt'
    events(ev, [(0.2, 9.5, CHORD_E)])
    dry = render('phrase-dry', ev, SCR / 'dry.wav', 9)
    wet = render('phrase-20', ev, SCR / 'wet.wav', 9)
    encode([trim(dry, SCR / 'a.wav', 0.5, 8), silence(0.6), trim(wet, SCR / 'b.wav', 0.5, 8)], 'robot-phrase')
    dry = render('phrase-dry-cons', ev, SCR / 'dryc.wav', 9)
    wet = render('phrase-cons', ev, SCR / 'wetc.wav', 9)
    encode([trim(dry, SCR / 'ac.wav', 0.5, 8), silence(0.6), trim(wet, SCR / 'bc.wav', 0.5, 8)], 'robot-phrase-consonants')

    # Five separate renders joined by 60 ms crossfades: no silence, no fade to zero (a fade
    # and a gap at every join sounded like a dropout every 4 s).
    counts = (8, 12, 16, 24, 32)
    parts = []
    for n in counts:
        w = render(f'phrase-{n}', ev, SCR / f'p{n}.wav', 5)
        t = SCR / f'p{n}t.wav'
        run('sox', w, t, 'trim', 0.5, 4.06)
        parts += ['-i', t]
    steps = []
    for i in range(1, len(counts)):
        src = '[0]' if i == 1 else f'[x{i - 1}]'
        dst = '' if i == len(counts) - 1 else f'[x{i}]'
        steps.append(f'{src}[{i}]acrossfade=d=0.06:c1=tri:c2=tri{dst}')
    wav = SCR / 'band-sweep.wav'
    run('ffmpeg', '-y', '-loglevel', 'error', *parts, '-filter_complex', ';'.join(steps), '-c:a', 'pcm_f32le', wav)
    deliver(wav, 'band-sweep')

    voice = Path(sys.argv[1]) if len(sys.argv) > 1 else OUT / 'voice-espeak-16k.wav'
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
