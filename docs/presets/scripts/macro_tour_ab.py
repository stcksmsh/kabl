#!/usr/bin/env python3
"""A/B listening tours of macros: old macro 1, new macro 1, old macro 2, new macro 2, ...

    MACRO_TOUR_BPM=<bpm> target/release/examples/macro_audit <patches> <phrases.json> <renders> SLUG
    (once for the old patches, once for the new, each with its own renders dir)
    python3 docs/presets/scripts/macro_tour_ab.py --old-patches P --old-renders R --new-patches P \
        --new-renders R --out DIR SLUG ...

Each macro clip is the 20 s tour render (held at 0 for 6 s, up over 4 s, held at 1 for 6 s, down over
4 s). Writes DIR/<slug>-ab.mp3 (clips back to back, 1 s of silence between) and DIR/tours.txt with the
start second and name of every clip. Pairs old and new by macro slot. Needs numpy, soundfile, ffmpeg.
"""
import argparse, json, subprocess as sp, tempfile
from pathlib import Path
import numpy as np
import soundfile as sf

SR = 48000
GAP = 1.0
FADE = 0.02


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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--old-patches', required=True)
    ap.add_argument('--old-renders', required=True)
    ap.add_argument('--new-patches', required=True)
    ap.add_argument('--new-renders', required=True)
    ap.add_argument('--out', required=True)
    ap.add_argument('slugs', nargs='+')
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    gap = np.zeros((int(GAP * SR), 2))
    lines = []
    for slug in a.slugs:
        old_n, new_n = names(a.old_patches, slug), names(a.new_patches, slug)
        parts, at = [], 0.0
        lines.append(f'{slug}-ab.mp3 (each clip 20 s: 0 for 6 s, up 4 s, 1 for 6 s, down 4 s; 1 s silence between)')
        for k in range(1, 5):
            for tag, renders, nm in (('old', a.old_renders, old_n), ('new', a.new_renders, new_n)):
                x, sr = sf.read(Path(renders) / 'sweep' / f'{slug}-m{k}.wav')
                assert sr == SR
                lines.append(f'  {at:6.1f} s  {tag} macro {k}: {nm.get(f"name.m{k}", f"m{k}")}')
                parts += [faded(x), gap]
                at += len(x) / SR + GAP
        encode(np.concatenate(parts), out / f'{slug}-ab.mp3')
        lines.append(f'  total {at:.1f} s')
        lines.append('')
    (out / 'tours.txt').write_text('\n'.join(lines))
    print('\n'.join(lines))


if __name__ == '__main__':
    main()
