#!/usr/bin/env python3
"""Renders the keys and plucks clips of one delivery group: FLAC plus 320 kbps mp3 per preset
(mp3 checked against the float WAV as in clips.py), and one tour FLAC with the presets back to
back, a second of silence between (each clip faded 20 ms at both ends), plus tour.txt with the start times.

    cargo build --release -p kabl-ui --example presets_keys_pluck
    python3 docs/presets/scripts/clips_keys_pluck.py 1        # group 1 -> docs/presets/clips/keys-pluck/group1/

Run from anywhere; it changes to the repo root.
"""
import os, struct, subprocess as sp, sys, tempfile
from pathlib import Path
import numpy as np

repo = Path(__file__).resolve().parents[3]
os.chdir(repo)
group = sys.argv[1]
tool = 'target/release/examples/presets_keys_pluck'
out = Path(f'docs/presets/clips/keys-pluck/group{group}')
out.mkdir(parents=True, exist_ok=True)
SR = 48000


def pcm(path):
    b = path.read_bytes(); at = 12; fmt = None
    while at < len(b):
        cid, size = b[at:at + 4], struct.unpack_from('<I', b, at + 4)[0]
        if cid == b'fmt ': fmt = struct.unpack_from('<HHIIHH', b, at + 8)
        if cid == b'data':
            assert fmt[0] in (3, 65534) and fmt[5] == 32, fmt
            return np.frombuffer(b[at + 8:at + 8 + size], '<f4').reshape(-1, fmt[1])
        at += 8 + size + (size & 1)


def flac(src, dst, extra=()):
    sp.run(['ffmpeg', '-y', '-loglevel', 'error', *extra, '-i', src, '-c:a', 'flac', '-sample_fmt', 's32',
            '-bits_per_raw_sample', '24', dst], check=True)


def fade(x, ms=20):
    """Raised-cosine fade in and out at the ends of a tour clip, so the splice to silence does not click."""
    n = min(int(48000 * ms / 1000), len(x) // 2)
    w = (0.5 - 0.5 * np.cos(np.pi * (np.arange(n) + 0.5) / n)).astype('<f4')[:, None]
    x = x.copy(); x[:n] *= w; x[len(x) - n:] *= w[::-1]
    return x


def encode_and_check(wav, stem):
    mp3 = str(stem) + '.mp3'
    sp.run(['ffmpeg', '-y', '-loglevel', 'error', '-i', wav, '-b:a', '320k', mp3], check=True)
    flac(str(wav), str(stem) + '.flac')
    dec = Path(str(wav) + '.dec.wav')
    sp.run(['ffmpeg', '-y', '-loglevel', 'error', '-i', mp3, '-f', 'wav', '-acodec', 'pcm_f32le', dec], check=True)
    a, b = pcm(Path(wav)), pcm(dec)
    n = min(len(a), len(b))
    a, b = a[:n], b[:n]
    keep = np.fft.rfftfreq(n, 1 / SR) < 16000
    lp = lambda x: np.fft.irfft(np.fft.rfft(x, axis=0) * keep[:, None], n, axis=0)
    a_full = a
    a, b = lp(a), lp(b)
    err = 20 * np.log10(np.sqrt(((a - b) ** 2).mean()) + 1e-12)
    peak = 20 * np.log10(np.abs(a_full).max() + 1e-12), 20 * np.log10(np.abs(b).max() + 1e-12)
    snr = 20 * np.log10(np.sqrt((a ** 2).mean()) + 1e-12) - err
    ok = snr > 35 and abs(peak[0] - peak[1]) < 0.5
    print(f'{Path(mp3).name:26} error {err:6.1f} dBFS  snr {snr:5.1f} dB  peak {peak[0]:5.1f} -> {peak[1]:5.1f}  {"ok" if ok else "FAIL"}')
    return ok


good = True
with tempfile.TemporaryDirectory() as tmp:
    env = dict(os.environ, PP_F32='1')
    r = sp.run([tool, 'clips', 'patches', tmp, group], env=env, check=True, capture_output=True, text=True)
    order = [line.split()[0] for line in r.stdout.splitlines() if ' peak ' in line]
    names = dict(l.split('\t') for l in sp.run([tool, 'names'], check=True, capture_output=True, text=True).stdout.splitlines())
    tour, t, lines = [], 0.0, []
    for slug in order:
        wav = Path(tmp) / (slug + '.wav')
        good &= encode_and_check(wav, out / slug)
        x = pcm(wav)
        lines.append(f'{int(t // 60)}:{t % 60:05.2f}  {names.get(slug, slug)}')
        tour += [fade(x), np.zeros((SR, 2), '<f4')]
        t += len(x) / SR + 1.0
    raw = Path(tmp) / 'tour.f32'
    raw.write_bytes(np.concatenate(tour[:-1]).astype('<f4').tobytes())
    flac(str(raw), str(out / 'tour.flac'), ['-f', 'f32le', '-ar', str(SR), '-ac', '2'])
    (out / 'tour.txt').write_text('\n'.join(lines) + '\n')
    print('\n'.join(lines))
# Defect scan of the rendered clips (impulsive clicks, limiter pinning, DC, NaN, HF bursts).
good &= sp.run([sys.executable, 'docs/presets/scripts/audio_defects.py', '--allow', 'docs/presets/scripts/audio_defects.allow', str(out)]).returncode == 0
sys.exit(0 if good else 1)
