#!/usr/bin/env python3
"""Listening tours of macros: the new macros 1, 2, 3, ... of a preset back to back (as many as the preset has).

    MACRO_TOUR_BPM=<bpm> target/release/examples/macro_audit <patches> <phrases.json> <renders> SLUG
    python3 docs/presets/scripts/macro_tour_ab.py --patches P --renders R --out DIR SLUG ...

Each macro clip is the 20 s tour render (held at 0 for 6 s, up over 4 s, held at 1 for 6 s, down over
4 s; the phrase loops at its own length in whole bars, see `macro_audit`). Writes DIR/<slug>-tour.mp3 (the
four clips back to back, 1 s of silence between) and DIR/tours.txt with the start second and name of
every clip. With --old-patches and --old-renders it writes the old/new A/B (DIR/<slug>-ab.mp3, old and new
interleaved per macro slot) instead. Needs numpy, soundfile, ffmpeg.

Guard: a clip that has a silent gap (below GAP_DB for more than GAP_MAX s) after its first second fails the
run: the phrase must keep sounding for the whole hold.
"""
import argparse, json, subprocess as sp, sys, tempfile
from pathlib import Path
import numpy as np
import soundfile as sf

SR = 48000
GAP = 1.0
FADE = 0.02
GAP_DB = -45.0
GAP_MAX = 1.0
WINDOW = 0.05


def names(patches, slug):
    ck = json.loads((Path(patches) / 'presets' / slug / 'checkpoint.json').read_text())
    kid = next(i for i, m in ck['modules'].items() if m['kind'] == 'macro')
    return ck['labels'].get(kid, {})


def faded(x):
    n = int(FADE * SR)
    x = x.copy()
    x[:n] *= np.linspace(0, 1, n)[:, None]
    x[-n:] *= np.linspace(1, 0, n)[:, None]
    return x


def encode(x, path):
    with tempfile.TemporaryDirectory() as d:
        wav = Path(d) / 'a.wav'
        sf.write(wav, x.astype(np.float32), SR, subtype='FLOAT')
        sp.run(['ffmpeg', '-y', '-loglevel', 'error', '-i', str(wav), '-b:a', '192k', str(path)], check=True)


def longest_gap(x):
    """Longest run (s) of 50 ms windows under GAP_DB, after the first second."""
    w = int(WINDOW * SR)
    n = len(x) // w
    peak = np.abs(x[: n * w]).max(axis=1).reshape(n, w).max(axis=1)
    quiet = 20 * np.log10(np.maximum(peak, 1e-9)) < GAP_DB
    quiet[: int(1.0 / WINDOW)] = False
    best = run = 0
    for q in quiet:
        run = run + 1 if q else 0
        best = max(best, run)
    return best * WINDOW


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--patches')
    ap.add_argument('--renders')
    ap.add_argument('--old-patches')
    ap.add_argument('--old-renders')
    ap.add_argument('--new-patches')
    ap.add_argument('--new-renders')
    ap.add_argument('--out', required=True)
    ap.add_argument('--max-gap', type=float, default=GAP_MAX, help='longest silent gap allowed in a clip, s')
    ap.add_argument('slugs', nargs='+')
    a = ap.parse_args()
    new_patches, new_renders = a.new_patches or a.patches, a.new_renders or a.renders
    ab = bool(a.old_patches and a.old_renders)
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    gap = np.zeros((int(GAP * SR), 2))
    lines, bad = [], []
    for slug in a.slugs:
        new_n = names(new_patches, slug)
        old_n = names(a.old_patches, slug) if ab else {}
        parts, at = [], 0.0
        suffix = 'ab' if ab else 'tour'
        count = len(list((Path(new_renders) / 'sweep').glob(f'{slug}-m*.wav')))
        what = 'old and new interleaved per slot' if ab else f'new macros 1 to {count}'
        lines.append(f'{slug}-{suffix}.mp3 ({what}; each clip 20 s: 0 for 6 s, up 4 s, 1 for 6 s, down 4 s; 1 s silence between)')
        for k in range(1, count + 1):
            for tag, renders, nm in ((('old', a.old_renders, old_n),) if ab else ()) + (('new', new_renders, new_n),):
                x, sr = sf.read(Path(renders) / 'sweep' / f'{slug}-m{k}.wav')
                assert sr == SR
                g = longest_gap(x)
                if g > a.max_gap:
                    bad.append(f'{slug} {tag} macro {k}: {g:.2f} s of silence')
                label = f'{tag} macro {k}' if ab else f'macro {k}'
                lines.append(f'  {at:6.1f} s  {label}: {nm.get(f"name.m{k}", f"m{k}")}')
                parts += [faded(x), gap]
                at += len(x) / SR + GAP
        encode(np.concatenate(parts), out / f'{slug}-{suffix}.mp3')
        lines.append(f'  total {at:.1f} s')
        lines.append('')
    (out / 'tours.txt').write_text('\n'.join(lines))
    print('\n'.join(lines))
    if bad:
        print('FAIL: the phrase does not sound for the whole clip:\n  ' + '\n  '.join(bad), file=sys.stderr)
        sys.exit(1)


if __name__ == '__main__':
    main()
