#!/usr/bin/env python3
"""Captures the effect displays in the real release editor, theme A (Satin) and B (Lacquer),
light and dark: all nine at 50 % and three close-ups at 100 %.

    cargo build --release -p kabl-ui --bin kabl-ui
    python3 docs/effect-displays/scripts/make_patch.py scratch/ed-patch
    Xvfb :153 -screen 0 1600x1000x24 &
    DISPLAY=:153 python3 docs/effect-displays/scripts/capture.py OUTDIR [1440x900]
"""
import subprocess
import sys
import tempfile
from pathlib import Path

out = Path(sys.argv[1])
size = sys.argv[2] if len(sys.argv) > 2 else '1440x900'
repo = Path(__file__).resolve().parents[3]
CLOSE = ['sleep 2', 'click routing', 'sleep 1']
CLOSEUPS = [
    'wheel knob:9.rate_hz 3 ctrl', 'sleep 1',
    'goto knob:9.rate_hz 250 250', 'sleep 1.5', 'shot {t}-{m}-row1',
    'goto knob:12.ratio 250 250', 'sleep 1.5', 'shot {t}-{m}-row2',
    'goto knob:20.freq_hz 250 250', 'sleep 1.5', 'shot {t}-{m}-row3',
]
for theme, tid in (('A', 'kabl.satin'), ('B', 'kabl.lacquer')):
    for mode in ('light', 'dark'):
        lines = CLOSE + [
            'click view-menu', 'sleep 0.5', f'click theme-id:{tid}', 'sleep 0.5',
            f'click theme:{mode}', 'key Escape', 'sleep 1.5', f'shot {theme}-{mode}-all50',
        ] + [l.format(t=theme, m=mode) for l in CLOSEUPS]
        with tempfile.NamedTemporaryFile('w', suffix='.txt', delete=False) as f:
            f.write('\n'.join(lines) + '\n')
        subprocess.run([sys.executable, str(repo / 'docs/rack-migration/drive.py'), f.name, size,
                        str(out / f'{theme}-{mode}'), str(repo / 'scratch/ed-patch')], check=True)
