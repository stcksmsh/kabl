#!/usr/bin/env python3
"""Real-input captures of the osc.va face in the standalone, themes A and B, on a private X display.

    cargo build -p kabl-ui && docs/unison/scripts/capture.py [list]

Audio goes to the silent `kabl_agents` sink (see CLAUDE.md, Audio). `list` prints the drawn hit keys.
Needs Xvfb, metacity, xdotool, ImageMagick (import).
"""
from pathlib import Path
import os, sys, time, subprocess as sp

repo = Path(__file__).resolve().parents[3]
out = repo / 'docs/unison/img'
scratch = repo / 'scratch/unison'
out.mkdir(parents=True, exist_ok=True)
scratch.mkdir(parents=True, exist_ok=True)
binary = Path(os.environ.get('KABL_BINARY', repo / 'target/debug/kabl-ui')).resolve()
patch = repo / os.environ.get('KABL_PATCH', 'patches/palette/pad')
disp = ':146'
env = dict(os.environ, DISPLAY=disp, WINIT_X11_SCALE_FACTOR='1', PIPEWIRE_NODE='kabl_agents', PULSE_SINK='kabl_agents',
           KABL_USER_DIR=str(scratch / 'library'), KABL_HITS_FILE=str(scratch / 'hits.txt'))
log = open(scratch / 'run.txt', 'w')
x = sp.Popen(['Xvfb', disp, '-screen', '0', '1400x900x24', '-nolisten', 'tcp'], stdout=log, stderr=log)
time.sleep(.6)
wm = sp.Popen(['metacity', '--sm-disable'], env=env, stdout=log, stderr=log)
time.sleep(.6)
app = None


def run(args):
    return sp.check_output(args, env=env, text=True)


def hits():
    try:
        return {p[0]: list(map(float, p[1:])) for p in map(str.split, (scratch / 'hits.txt').read_text().splitlines()) if len(p) == 5}
    except FileNotFoundError:
        return {}


def click(key, pause=.5):
    end = time.monotonic() + 20
    while key not in hits():
        assert time.monotonic() < end and app.poll() is None, ('missing', key, sorted(hits()))
        time.sleep(.15)
    a, b, c, d = hits()[key]
    run(['xdotool', 'mousemove', '--window', wid, str(round((a + c) / 2)), str(round((b + d) / 2)), 'click', '1'])
    time.sleep(pause)


def theme(name):
    click('view-menu', .4)
    click('theme:' + name, .3)
    click('view-menu', .5)


try:
    (scratch / 'hits.txt').unlink(missing_ok=True)
    app = sp.Popen([str(binary), '--patch', str(patch), '--size', '1280x800', '--no-rt'], env=env, stdout=log, stderr=log)
    time.sleep(2.5)
    print(run(['pactl', 'list', 'sink-inputs', 'short']))  # streams must sit on kabl_agents
    wid = run(['xdotool', 'search', '--onlyvisible', '--name', '^kabl$']).splitlines()[-1]
    run(['xdotool', 'windowmove', wid, '0', '0'])
    time.sleep(1)
    if 'list' in sys.argv:
        print('\n'.join(sorted(hits())))
        sys.exit(0)
    for n, mode in enumerate(['light', 'dark']):
        theme(mode)
        run(['import', '-window', wid, str(out / f'{n + 1:02d}-{mode}-rack.png')])
finally:
    if app:
        app.terminate()
    wm.terminate()
    x.terminate()
