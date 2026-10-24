#!/usr/bin/env python3
"""Every built-in module kind alone in a patch, under every theme and mode, at 100% and 50%.

    cargo build --release -p kabl-ui --bin kabl-ui
    python3 docs/visual-polish/scripts/capture_faces.py OUTDIR [WORKERS] [KINDS...]

Writes OUTDIR/<theme>-<mode>/<kind>/{z100,z50}.png (1280x800 window, release editor, real input
through docs/rack-migration/drive.py). Runs WORKERS Xvfb displays in parallel (default 4, from :241).
Nothing plays on a real device: PIPEWIRE_NODE and PULSE_SINK point at the silent sink. Run from the
repo root.
"""
import os, subprocess as sp, sys, tempfile, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
os.chdir(repo)
out = Path(sys.argv[1]).resolve()
workers = int(sys.argv[2]) if len(sys.argv) > 2 else 4
kinds = sys.argv[3:] or sorted(
    {l.split('"')[1] for p in Path('crates/modules/src/builtins').glob('*.rs') for l in p.read_text().splitlines()
     if l.strip().startswith('kind: "')})
themes = os.environ.get('THEMES', 'kabl.satin kabl.lacquer').split()
modes = os.environ.get('MODES', 'light dark').split()
size = os.environ.get('SIZE', '1280x800')
script = repo / 'docs/visual-polish/scripts/face.txt'
patches = out / 'patches'
sp.run([sys.executable, 'docs/effects-filters/scripts/face_patches.py', str(patches)], check=True,
       env=dict(os.environ, KINDS=' '.join(kinds)))
jobs = [(t, m, k) for t in themes for m in modes for k in kinds]
cfg = Path(tempfile.mkdtemp())
disp = [f':{241 + i}' for i in range(workers)]
xs = [sp.Popen(['Xvfb', d, '-screen', '0', '1600x1000x24'], stdout=sp.DEVNULL, stderr=sp.DEVNULL) for d in disp]
time.sleep(2)


def run(i):
    d = disp[i % workers]
    t, m, k = jobs[i]
    o = out / f'{t}-{m}' / k
    env = dict(os.environ, DISPLAY=d, KABL_CONFIG_DIR=str(cfg), KABL_THEME=f'{t}:{m}',
               PIPEWIRE_NODE='kabl_agents', PULSE_SINK='kabl_agents')
    # one display per worker: jobs i, i+workers, ... run in order on that display
    return sp.run([sys.executable, 'docs/rack-migration/drive.py', str(script), size, str(o), str(patches / k)],
                  env=env, stdout=sp.DEVNULL, stderr=sp.DEVNULL).returncode


def lane(w):
    return [(i, run(i)) for i in range(w, len(jobs), workers)]


try:
    with ThreadPoolExecutor(workers) as ex:
        res = [r for lane_res in ex.map(lane, range(workers)) for r in lane_res]
finally:
    for x in xs:
        x.terminate()
bad = [jobs[i] for i, rc in res if rc]
print(f'{len(jobs) - len(bad)} of {len(jobs)} ok', *bad[:20])
