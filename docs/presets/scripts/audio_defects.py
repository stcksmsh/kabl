#!/usr/bin/env python3
"""Offline defect scan for rendered preset audio (FLAC or WAV): no audio device, no engine.

    python3 docs/presets/scripts/audio_defects.py PATH [PATH ...]   # files or directories
    python3 docs/presets/scripts/audio_defects.py --json PATH ...
    python3 docs/presets/scripts/audio_defects.py --allow docs/presets/scripts/audio_defects.allow PATH ...

Exit status 1 if an ERROR flag (NONFINITE, DENORMAL, CLICK, PINNED, PEAK) is raised that is not listed in
the allow file. DC and HF_BURST are advisory: printed, never fatal. Needs numpy, scipy and
soundfile. Every flag prints the time stamps (seconds) it was seen at.

Flags
  NONFINITE  NaN or infinity in the samples.
  DENORMAL   subnormal float samples (only a float WAV can hold them; FLAC cannot).
  CLICK      an isolated single-sample spike: in a 50 ms window the largest second difference
             |x[n] - 2x[n-1] + x[n-2]| is at least CLICK_ISOLATION times the largest second
             difference more than 3 samples away, and at least CLICK_MIN_DB. Smooth attacks,
             saw edges (many per window) and onsets from digital silence do not qualify.
             Windows whose peak is only a few 16-bit LSB (below CLICK_QUIET_DB) are skipped:
             the staircase of a rounded tail is not a click.
  PINNED     the output sits on its ceiling: at least PIN_PER_SEC samples per active second are
             within 0.04 dB of the file's peak (only checked when the peak is above PIN_FROM_DB).
  PEAK       a sample above PEAK_LIMIT_DB (-1 dBFS): the presets have no limiter, so headroom
             is patch gain; this also catches clipping (0 dBFS and over).
  DC         mean of the active signal above DC_LIMIT.
  HF_BURST   5 ms of energy above 6 kHz that is HF_RISE_DB over the local median while the
             full-band energy rises less than HF_BAND_DB (an attack raises both; a crackle only HF).
"""
import argparse, json, sys
from pathlib import Path
import numpy as np
import soundfile as sf
from scipy.signal import butter, sosfilt
from scipy.ndimage import median_filter

# Thresholds tuned on the committed clips (see docs/presets/AUDIO-DEFECTS.md).
CLICK_ISOLATION = 5.0
CLICK_MIN_DB = -45.0
CLICK_NEIGHBOUR_MIN = 3e-5
CLICK_QUIET_DB = -66.0  # 16 LSB of 16 bits
PEAK_LIMIT_DB = -1.0
PIN_FROM_DB = -3.0
PIN_PER_SEC = 30.0
DC_LIMIT = 0.01
HF_RISE_DB = 30.0
HF_BAND_DB = 3.0
HF_MIN_DB = -50.0
ACTIVE_DB = -50.0
ERRORS = {'NONFINITE', 'DENORMAL', 'CLICK', 'PINNED', 'PEAK'}


def db(v):
    return 20 * np.log10(max(float(v), 1e-12))


def merge(times, gap=0.002):
    out = []
    for t in sorted(times):
        if not out or t - out[-1] > gap:
            out.append(t)
    return out


def clicks(x, sr):
    w, found = int(.05 * sr), []
    for c in range(x.shape[1]):
        d2 = np.abs(np.diff(x[:, c], 2))
        for i in range(len(d2) // w):
            b = d2[i * w:(i + 1) * w]
            k = int(b.argmax())
            top = b[k]
            if db(top) < CLICK_MIN_DB or db(np.abs(x[i * w:(i + 1) * w + 2, c]).max()) < CLICK_QUIET_DB:
                continue
            # A spike in the first 2 ms or last 1 ms is read with the neighbouring half window too: a hit
            # that starts right at a window boundary has no earlier (or later) audio in its own
            # window, so its attack looks isolated there and only there.
            lo = max(0, i * w - w // 2) if k < w // 24 else i * w
            hi = (i + 1) * w + w // 2 if k >= w - w // 50 else (i + 1) * w
            m = d2[lo:hi].copy()
            j = i * w + k - lo
            m[max(0, j - 3):j + 4] = 0
            rest = m.max()
            if rest > CLICK_NEIGHBOUR_MIN and top / rest >= CLICK_ISOLATION:
                found.append(i * .05 + k / sr)
    return merge(found)


def peak(x, sr):
    over = np.abs(x).max(1) > 10 ** (PEAK_LIMIT_DB / 20)
    return merge(np.where(over)[0] / sr, gap=0.05)


def pinned(x, sr):
    pk = np.abs(x).max()
    if db(pk) < PIN_FROM_DB:
        return []
    near = np.abs(x).max(1) >= pk * 0.9954  # 0.04 dB
    active = (np.abs(x).max(1) > 10 ** (ACTIVE_DB / 20)).sum() / sr
    if near.sum() / max(active, 1e-9) < PIN_PER_SEC:
        return []
    return merge(np.where(near)[0] / sr, gap=0.05)


def dc(x, sr):
    act = np.abs(x).max(1) > 10 ** (ACTIVE_DB / 20)
    if not act.any():
        return []
    return [0.0] if np.abs(x[act].mean(0)).max() > DC_LIMIT else []


def hf_bursts(x, sr):
    w, found = int(.005 * sr), []
    hp = sosfilt(butter(4, 6000, 'hp', fs=sr, output='sos'), x, axis=0)
    for c in range(x.shape[1]):
        n = len(x) // w
        h = (hp[:n * w, c].reshape(n, w) ** 2).mean(1) + 1e-20
        b = (x[:n * w, c].reshape(n, w) ** 2).mean(1) + 1e-20
        hm = median_filter(h, size=41, mode='nearest')  # +-100 ms
        bm = median_filter(b, size=41, mode='nearest')
        rise_h = 10 * np.log10(h / hm)
        rise_b = 10 * np.log10(b / bm)
        ok = (rise_h >= HF_RISE_DB) & (rise_b < HF_BAND_DB) & (10 * np.log10(h) > 2 * HF_MIN_DB)
        found += list(np.where(ok)[0] * .005)
    return merge(found, gap=0.01)


def scan(path):
    x, sr = sf.read(str(path), always_2d=True, dtype='float64')
    flags = {}
    bad = ~np.isfinite(x)
    if bad.any():
        flags['NONFINITE'] = merge(np.where(bad.any(1))[0] / sr, gap=0.01)
        x = np.nan_to_num(x, nan=0.0, posinf=0.0, neginf=0.0)
    tiny = (x != 0) & (np.abs(x) < 1.18e-38)
    if tiny.any():
        flags['DENORMAL'] = merge(np.where(tiny.any(1))[0] / sr, gap=0.01)
    for name, f in (('CLICK', clicks), ('PINNED', pinned), ('PEAK', peak), ('DC', dc), ('HF_BURST', hf_bursts)):
        t = f(x, sr)
        if t:
            flags[name] = [round(float(v), 3) for v in t]
    return dict(file=str(path), seconds=round(len(x) / sr, 2), peak_db=round(db(np.abs(x).max()), 2), flags=flags)


def files(paths):
    for p in map(Path, paths):
        if p.is_dir():
            yield from sorted(q for q in p.rglob('*') if q.suffix in ('.flac', '.wav') and q.stem != 'tour')
        else:
            yield p


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('paths', nargs='+')
    ap.add_argument('--json', action='store_true')
    ap.add_argument('--allow', help='file of "<path suffix> <FLAG>" lines that are accepted')
    a = ap.parse_args()
    allow = set()
    if a.allow:
        for line in Path(a.allow).read_text().splitlines():
            if line.strip() and not line.startswith('#'):
                allow.add(tuple(line.split()[:2]))
    results, failed = [], False
    for f in files(a.paths):
        r = scan(f)
        r['accepted'] = [k for k in r['flags'] if any(str(f).endswith(p) and k == fl for p, fl in allow)]
        failed |= any(k in ERRORS and k not in r['accepted'] for k in r['flags'])
        results.append(r)
    if a.json:
        print(json.dumps(results, indent=1))
    else:
        for r in results:
            parts = [f"{k}{'*' if k in r['accepted'] else ''}{'' if k in ERRORS else '(advisory)'} x{len(v)} @ {', '.join(map(str, v[:6]))}{' ...' if len(v) > 6 else ''}" for k, v in r['flags'].items()]
            print(f"{r['file']}: {'; '.join(parts) if parts else 'clean'}")
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
