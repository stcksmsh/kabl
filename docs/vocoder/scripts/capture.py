#!/usr/bin/env python3
"""Captures the vocoder face in the real release editor, theme A (Satin) and B (Lacquer), light and
dark: the whole self-playing patch at 50 % and the vocoder close up at 100 %.

    cargo build --release -p kabl-ui --bin kabl-ui
    python3 docs/vocoder/scripts/make_vocoder_demos.py --clips scratch/vocoder-clips/patches
    Xvfb :153 -screen 0 1600x1000x24 &
    DISPLAY=:153 python3 docs/vocoder/scripts/capture.py OUTDIR [1440x900]

The capture patch is the robot voice with a clock and random loop in place of the keyboard, so it
plays by itself and the band bars move. Module 20 is the vocoder.
"""
import subprocess
import sys
import tempfile
from pathlib import Path

out = Path(sys.argv[1])
size = sys.argv[2] if len(sys.argv) > 2 else '1440x900'
repo = Path(__file__).resolve().parents[3]
patch = repo / 'scratch/vocoder-clips/patches/capture'
CLOSE = ['sleep 2', 'click routing', 'sleep 1']
CLOSEUP = [
    'wheel knob:20.shift_st 3 ctrl', 'sleep 1',
    'goto knob:20.shift_st 250 250', 'sleep 2.5', 'shot {t}-{m}-vocoder',
]
for theme, tid in (('A', 'kabl.satin'), ('B', 'kabl.lacquer')):
    for mode in ('light', 'dark'):
        lines = CLOSE + [
            'click view-menu', 'sleep 0.5', f'click theme-id:{tid}', 'sleep 0.5',
            f'click theme:{mode}', 'key Escape', 'sleep 2.5', f'shot {theme}-{mode}-all50',
        ] + [l.format(t=theme, m=mode) for l in CLOSEUP]
        with tempfile.NamedTemporaryFile('w', suffix='.txt', delete=False) as f:
            f.write('\n'.join(lines) + '\n')
        subprocess.run([sys.executable, str(repo / 'docs/rack-migration/drive.py'), f.name, size,
                        str(out / f'{theme}-{mode}'), str(patch)], check=True)
