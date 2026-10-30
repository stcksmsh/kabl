#!/usr/bin/env python3
"""Harshness check of rendered clips (float WAVs).

    python3 docs/presets/scripts/harshness.py clip.wav [...]

Prints, per file, the energy in 2-5 kHz against the total (dB; the target is -18 dB or lower) and the
strongest tonal peak above 2 kHz: its frequency, level (dBFS of a sine with that amplitude) and
prominence over the median of the spectrum 150-600 Hz either side (the target is under 15 dB).
Spectrum: mean of Hann-windowed 16384-point frames of the mid channel, silent frames skipped.
"""
import sys
import numpy as np
import soundfile as sf

N = 16384


def analyse(path):
    x, sr = sf.read(path, dtype='float64')
    m = x.mean(axis=1) if x.ndim > 1 else x
    w = np.hanning(N)
    frames = [m[i:i + N] * w for i in range(0, len(m) - N, N // 2) if np.abs(m[i:i + N]).max() > 1e-3]
    spec = np.mean([np.abs(np.fft.rfft(f)) ** 2 for f in frames], axis=0)
    f = np.fft.rfftfreq(N, 1 / sr)
    band = (f >= 2000) & (f < 5000)
    ratio = 10 * np.log10(spec[band].sum() / spec.sum())
    amp = 20 * np.log10(2 * np.sqrt(spec) / w.sum() + 1e-12)
    best = None
    for k in np.where(f >= 2000)[0][2:-2]:
        if not (spec[k] >= spec[k - 1] and spec[k] > spec[k + 1]):
            continue
        near = np.abs(f - f[k])
        ring = (near > 150) & (near < 600)
        prom = amp[k] - np.median(amp[ring])
        if best is None or prom > best[2]:
            best = (f[k], amp[k], prom)
    return ratio, best


for p in sys.argv[1:]:
    ratio, (hz, lvl, prom) = analyse(p)
    flag = '' if ratio <= -18 and prom <= 15 else '  <-- HARSH'
    print(f'{p.split("/")[-1]:22} 2-5k/total {ratio:6.1f} dB   peak {hz:6.0f} Hz  {lvl:6.1f} dBFS  prominence {prom:5.1f} dB{flag}')
