#!/usr/bin/env python3
"""Captures the drum grid face in the real release editor, theme A (Satin) and B (Lacquer),
light and dark.

    cargo build --release -p kabl-ui --bin kabl-ui
    Xvfb :231 -screen 0 1600x1000x24 &
    DISPLAY=:231 python3 docs/drum-grid/scripts/capture.py OUTDIR [1440x900]

The patch is `patches/sound-engines/demos/four-floor`: module 2 is the grid.
"""
import subprocess
import sys
import tempfile
from pathlib import Path

out = Path(sys.argv[1])
size = sys.argv[2] if len(sys.argv) > 2 else '1440x900'
repo = Path(__file__).resolve().parents[3]
for theme, tid in (('A', 'kabl.satin'), ('B', 'kabl.lacquer')):
    for mode in ('light', 'dark'):
        lines = [
            'sleep 2', 'click routing', 'sleep 1',
            'click view-menu', 'sleep 0.5', f'click theme-id:{tid}', 'sleep 0.5',
            f'click theme:{mode}', 'key Escape', 'sleep 1.5',
            'wheel grid:2 2 ctrl', 'sleep 1', 'goto grid:2 600 330', 'sleep 1', 'at 1300 850', 'sleep 1', f'shot {theme}-{mode}-grid',
        ]
        with tempfile.NamedTemporaryFile('w', suffix='.txt', delete=False) as f:
            f.write('\n'.join(lines) + '\n')
        subprocess.run([sys.executable, str(repo / 'docs/rack-migration/drive.py'), f.name, size,
                        str(out / f'{theme}-{mode}'),
                        str(repo / 'patches/sound-engines/demos/four-floor')], check=True)
