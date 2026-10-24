#!/usr/bin/env python3
"""Per-window level and spectral centroid of renders: host-compare.py A.wav B.wav [...]

Renders are not sample-identical between runs (free-running oscillators), so renders are
compared by window RMS (dBFS) and spectral centroid (Hz), 0.5 s windows."""
import sys
import numpy as np
import soundfile as sf

SR = 48000
rows = {}
for p in sys.argv[1:]:
    x, sr = sf.read(p)
    x = x.mean(axis=1) if x.ndim > 1 else x
    out = []
    for i in range(0, len(x) - SR // 2 + 1, SR // 2):
        s = x[i:i + SR // 2]
        rms = float(np.sqrt(np.mean(s ** 2)))
        spec = np.abs(np.fft.rfft(s * np.hanning(len(s))))
        f = np.fft.rfftfreq(len(s), 1 / sr)
        out.append((20 * np.log10(rms + 1e-9), float((spec * f).sum() / max(spec.sum(), 1e-12))))
    rows[p] = out
names = list(rows)
print('window   ' + '  '.join('%-26s' % Path(n).name if False else '%-26s' % n.split('/')[-1] for n in names))
for k in range(len(rows[names[0]])):
    print('%4.1f s   ' % (k * 0.5) + '  '.join('%6.1f dB  centroid %6.0f Hz' % rows[n][k] for n in names))
if len(names) >= 2:
    d = [abs(rows[names[0]][k][0] - rows[names[1]][k][0]) for k in range(len(rows[names[0]]))
         if rows[names[0]][k][0] > -60]
    print('max level difference (windows above -60 dB): %.2f dB' % max(d))
