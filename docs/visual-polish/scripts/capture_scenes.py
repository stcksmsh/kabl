#!/usr/bin/env python3
"""Chrome scenes (rack, Perform, drawers, dialogs, drum grid, wavetable editor) under every theme and mode.

    python3 docs/visual-polish/scripts/capture_scenes.py OUTDIR [WORKERS]

Writes OUTDIR/<size>/<theme>-<mode>/<scene>/<shot>.png. SIZES (default "1440x900 1180x680"), THEMES and
MODES as in capture_faces.py. Displays start at :261. Output goes to the silent sink. Run from the repo root.
"""
import os, subprocess as sp, sys, tempfile, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
os.chdir(repo)
out = Path(sys.argv[1]).resolve()
workers = int(sys.argv[2]) if len(sys.argv) > 2 else 4
themes = os.environ.get('THEMES', 'kabl.satin kabl.lacquer').split()
modes = os.environ.get('MODES', 'light dark').split()
sizes = os.environ.get('SIZES', '1440x900 1180x680').split()
sd = repo / 'docs/visual-polish/scripts/scenes'
scenes = {
    'rack-simple': 'patches/reference', 'rack-dense': 'patches/composition', 'perform': 'patches/composition',
    'pattern': 'patches/functional-cables/five-against-eight', 'sounds': 'patches/composition', 'dialogs': 'patches/composition',
    'drum': 'patches/sound-engines/demos/four-floor', 'wt': 'patches/sound-engines/demos/imported-morph',
}
only = os.environ.get('SCENES', ' '.join(scenes)).split()
jobs = [(s, t, m, sc) for s in sizes for t in themes for m in modes for sc in only]
cfg = Path(tempfile.mkdtemp())
disp = [f':{261 + i}' for i in range(workers)]
xs = [sp.Popen(['Xvfb', d, '-screen', '0', '1600x1000x24'], stdout=sp.DEVNULL, stderr=sp.DEVNULL) for d in disp]
time.sleep(2)


def run(i):
    s, t, m, sc = jobs[i]
    env = dict(os.environ, DISPLAY=disp[i % workers], KABL_CONFIG_DIR=str(cfg), KABL_THEME=f'{t}:{m}',
               PIPEWIRE_NODE='kabl_agents', PULSE_SINK='kabl_agents')
    return sp.run([sys.executable, 'docs/rack-migration/drive.py', str(sd / f'{sc}.txt'), s,
                   str(out / s / f'{t}-{m}' / sc), scenes[sc]], env=env,
                  stdout=sp.DEVNULL, stderr=sp.DEVNULL).returncode


try:
    with ThreadPoolExecutor(workers) as ex:
        res = [r for lane in ex.map(lambda w: [(i, run(i)) for i in range(w, len(jobs), workers)], range(workers))
               for r in lane]
finally:
    for x in xs:
        x.terminate()
bad = [jobs[i] for i, rc in res if rc]
print(f'{len(jobs) - len(bad)} of {len(jobs)} ok', *bad[:20])
