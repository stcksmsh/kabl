#!/usr/bin/env python3
"""Captures the drum faces in the real release editor, theme A (Satin) and B (Lacquer), light
and dark: the whole groove patch at 50 % and a close-up of each voice.

    cargo build --release -p kabl-ui --bin kabl-ui
    Xvfb :173 -screen 0 1600x1000x24 &
    DISPLAY=:173 python3 docs/drums/scripts/capture.py OUTDIR [1440x900]

The patch is `patches/sound-engines/demos/four-floor`: module 7 kick, 8 snare, 9 clap, 10 hat,
11 tom, 14 the MIDI-free output.
"""
import subprocess
import sys
import tempfile
from pathlib import Path

out = Path(sys.argv[1])
size = sys.argv[2] if len(sys.argv) > 2 else '1440x900'
repo = Path(__file__).resolve().parents[3]
CLOSE = ['sleep 2', 'click routing', 'sleep 1']
VOICES = [('kick', 'knob:7.decay_ms'), ('snare', 'knob:8.snappy'), ('clap', 'knob:9.spread_ms'),
          ('hat', 'knob:10.open_ms'), ('tom', 'knob:11.decay_ms')]
CLOSEUPS = ['wheel knob:7.decay_ms 3 ctrl', 'sleep 1']
for name, knob in VOICES:
    CLOSEUPS += [f'goto {knob} 250 420', 'sleep 1.5', 'shot {t}-{m}-' + name]
for theme, tid in (('A', 'kabl.satin'), ('B', 'kabl.lacquer')):
    for mode in ('light', 'dark'):
        lines = CLOSE + [
            'click view-menu', 'sleep 0.5', f'click theme-id:{tid}', 'sleep 0.5',
            f'click theme:{mode}', 'key Escape', 'sleep 1.5', f'shot {theme}-{mode}-all50',
        ] + [l.format(t=theme, m=mode) for l in CLOSEUPS]
        with tempfile.NamedTemporaryFile('w', suffix='.txt', delete=False) as f:
            f.write('\n'.join(lines) + '\n')
        subprocess.run([sys.executable, str(repo / 'docs/rack-migration/drive.py'), f.name, size,
                        str(out / f'{theme}-{mode}'),
                        str(repo / 'patches/sound-engines/demos/four-floor')], check=True)
