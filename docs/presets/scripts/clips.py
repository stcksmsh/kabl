#!/usr/bin/env python3
"""Renders the pilot clips (docs/presets/clips/*.mp3) and checks each against its WAV.

    cargo build --release -p kabl-ui --example presets_pilot
    python3 docs/presets/scripts/clips.py

The engine renders 32-bit float WAVs (PP_F32), so there is no 16-bit step; ffmpeg encodes them
at 320 kbps CBR; the script decodes every mp3 again and fails if the encoding error is
less than 35 dB under the signal (RMS, below 16 kHz) or the decoded peak differs from the WAV's by more than 0.5 dB. At 192
kbps the same clips carried an error near -46 dBFS (about 30 dB under the signal, 7 to 14 dB under
it above 6 kHz), which reads as static on a dense pad. Run from the repo root.
"""
import os, struct, subprocess as sp, sys, tempfile
from pathlib import Path
import numpy as np

repo = Path(__file__).resolve().parents[3]
os.chdir(repo)
# Another generator: PP_TOOL=presets_bass_lead PP_OUT=docs/presets/clips/bass-lead/group1 PP_GROUP=1.
# Its clips also get a tour: all of them back to back with a second of silence between.
tool = 'target/release/examples/' + os.environ.get('PP_TOOL', 'presets_pilot')
out = Path(os.environ.get('PP_OUT', 'docs/presets/clips'))
group = os.environ.get('PP_GROUP')
out.mkdir(parents=True, exist_ok=True)
# (slug, clip name, env overrides: PP_SET, PP_HITS): the extra clips show a macro moved
extras = [] if 'PP_TOOL' in os.environ else [('salt-flat', 'salt-flat-textured', {'PP_SET': 'macro.m4=0.7'}), ('parquet-loop', 'parquet-loop-no-glide', {'PP_SET': 'macro.m4=0'})]
# A legato line (each note overlaps the next by 0.1 s), Parquet Loop in POLY and in MONO.
line = [48, 60, 52, 64, 55, 67, 60, 72]
hits = ';'.join(f'{i * 0.8:.2f}:{n}:100;{i * 0.8 + 0.9:.2f}:{n}:0' for i, n in enumerate(line))
extras += [] if 'PP_TOOL' in os.environ else [('parquet-loop', 'parquet-loop-legato-poly', {'PP_HITS': hits}),
           ('parquet-loop', 'parquet-loop-legato-mono', {'PP_HITS': hits, 'PP_SET': 'midi.in.mode=1'})]


def pcm(path):
    b = path.read_bytes(); at = 12; fmt = None
    while at < len(b):
        cid, size = b[at:at + 4], struct.unpack_from('<I', b, at + 4)[0]
        if cid == b'fmt ': fmt = struct.unpack_from('<HHIIHH', b, at + 8)
        if cid == b'data':
            assert fmt[0] in (3, 65534) and fmt[5] == 32, fmt
            return np.frombuffer(b[at + 8:at + 8 + size], '<f4').reshape(-1, fmt[1])
        at += 8 + size + (size & 1)


def encode_and_check(wav, mp3):
    sp.run(['ffmpeg', '-y', '-loglevel', 'error', '-i', wav, '-b:a', '320k', mp3], check=True)
    # For ears: the same float render as 24-bit FLAC, next to the mp3.
    sp.run(['ffmpeg', '-y', '-loglevel', 'error', '-i', wav, '-c:a', 'flac', '-sample_fmt', 's32', '-bits_per_raw_sample', '24', str(mp3)[:-4] + '.flac'], check=True)
    dec = Path(str(wav) + '.dec.wav')
    sp.run(['ffmpeg', '-y', '-loglevel', 'error', '-i', mp3, '-f', 'wav', '-acodec', 'pcm_f32le', dec], check=True)
    a, b = pcm(Path(wav)), pcm(dec)
    n = min(len(a), len(b))
    a, b = a[:n], b[:n]
    # Judge the encoder where it works: below 16 kHz (it drops what is above).
    keep = np.fft.rfftfreq(n, 1 / 48000) < 16000
    lp = lambda x: np.fft.irfft(np.fft.rfft(x, axis=0) * keep[:, None], n, axis=0)
    a_full = a
    a, b = lp(a), lp(b)
    err = 20 * np.log10(np.sqrt(((a - b) ** 2).mean()) + 1e-12)
    # Peaks too are compared below 16 kHz: a saw's edges above it are not the encoder's doing.
    peak = 20 * np.log10(np.abs(a).max() + 1e-12), 20 * np.log10(np.abs(b).max() + 1e-12)
    snr = 20 * np.log10(np.sqrt((a ** 2).mean()) + 1e-12) - err
    ok = snr > 35 and abs(peak[0] - peak[1]) < 0.5
    print(f'{Path(mp3).name:26} error {err:6.1f} dBFS  snr {snr:5.1f} dB  peak {peak[0]:5.1f} -> {peak[1]:5.1f}  {"ok" if ok else "FAIL"}')
    return ok


good = True
with tempfile.TemporaryDirectory() as tmp:
    for slug, name, override in [(None, None, None)] + extras:
        d = Path(tmp) / (name or 'main')
        env = dict(os.environ, PP_F32='1')
        env.update(override or {})
        sp.run([tool, 'clips', 'patches', str(d)] + ([group] if group else []), env=env, check=True, capture_output=True)
        if name is None:
            wavs = sorted(d.glob('*.wav'))
            for wav in wavs:
                good &= encode_and_check(wav, out / (wav.stem + '.mp3'))
            if group:
                gap = np.zeros((48000, 2), '<f4')
                parts, at = [], 0.0
                for wav in wavs:
                    x = pcm(wav)
                    print(f'tour {at:6.1f} s  {wav.stem}')
                    parts += [x, gap]
                    at += (len(x) + len(gap)) / 48000
                sp.run(['ffmpeg', '-y', '-loglevel', 'error', '-f', 'f32le', '-ar', '48000', '-ac', '2', '-i', '-', '-c:a', 'flac', '-sample_fmt', 's32', '-bits_per_raw_sample', '24', str(out / 'tour.flac')], input=np.concatenate(parts[:-1]).astype('<f4').tobytes(), check=True)
        else:
            good &= encode_and_check(d / (slug + '.wav'), out / (name + '.mp3'))
sys.exit(0 if good else 1)
