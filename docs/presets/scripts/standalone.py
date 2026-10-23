#!/usr/bin/env python3
"""Opens each pilot preset in the standalone (release kabl-ui, private X display, real audio
device and real-time priority), plays a chord through a virtual MIDI controller, saves the
Rack and Perform views and the app's log, and fails if the app is not still running.

    cargo build --release -p kabl-ui --bins --examples
    python3 docs/presets/scripts/standalone.py

Evidence goes to docs/presets/evidence/standalone/<slug>.{log,png}. Needs Xvfb, metacity,
xdotool and ImageMagick; the player is target/release/examples/midi_player (ALSA sequencer).
Run from the repo root.
"""
import os, subprocess as sp, sys, time
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
os.chdir(repo)
out = repo / 'docs/presets/evidence/standalone'
out.mkdir(parents=True, exist_ok=True)
scratch = repo / 'scratch/presets-standalone'
scratch.mkdir(parents=True, exist_ok=True)
disp = ':152'
# slug -> notes of the chord (a single note for the mono presets)
presets = {
    'cinder-block': [40], 'parquet-loop': [52, 59, 62, 66], 'loading-dock': [64],
    'glass-elevator': [52, 59, 64, 67], 'undertow': [28], 'salt-flat': [50, 57, 62, 65],
    'boxwood': [60, 64, 67], 'neon-highway': [64],
}
env = dict(os.environ, DISPLAY=disp, WINIT_X11_SCALE_FACTOR='1', KABL_PLAYER='1', KABL_ARGS='--midi kabl-player',
           KABL_CAPTURE_WINDOW='1', KABL_USER_DIR=str(scratch / 'library'))
x = sp.Popen(['Xvfb', disp, '-screen', '0', '1600x1000x24', '-nolisten', 'tcp'], stdout=sp.DEVNULL, stderr=sp.DEVNULL)
time.sleep(1)
wm = sp.Popen(['metacity', '--sm-disable'], env=env, stdout=sp.DEVNULL, stderr=sp.DEVNULL)
time.sleep(1)
failed = []
try:
    for slug, notes in presets.items():
        lines = ['sleep 3', *(f'midi on {n} 100' for n in notes), 'sleep 3', 'shot rack', 'click perform', 'sleep 1',
                 'shot perform', 'sleep 2', *(f'midi off {n}' for n in notes), 'sleep 1', 'running']
        script = scratch / f'{slug}.txt'
        script.write_text('\n'.join(lines) + '\n')
        e = dict(env, KABL_APP_LOG=str(out / f'{slug}.log'))
        r = sp.run([sys.executable, 'docs/rack-migration/drive.py', str(script), '1280x800', str(scratch / slug),
                    str(repo / 'patches/presets' / slug)], env=e, timeout=120)
        for shot in ('rack', 'perform'):
            src = scratch / slug / f'{shot}.png'
            if src.exists():
                os.replace(src, out / f'{slug}-{shot}.png')
        log = (out / f'{slug}.log').read_text()
        ok = r.returncode == 0 and 'real-time priority granted' in log and 'device=' in log
        print(slug, 'ok' if ok else 'FAILED')
        if not ok:
            failed.append(slug)
finally:
    wm.terminate()
    x.terminate()
sys.exit(1 if failed else 0)
