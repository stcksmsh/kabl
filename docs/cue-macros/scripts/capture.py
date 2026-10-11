#!/usr/bin/env python3
"""Real-input captures of the cue-macros patch: standalone at 1280x800 (the standalone's minimum) on a private X display.

    cargo build -p kabl-ui && docs/cue-macros/scripts/capture.py [list]

`list` only prints the drawn target keys of the Perform view. Needs Xvfb, metacity, xdotool,
ImageMagick (import).
"""
from pathlib import Path
import os, sys, time, subprocess as sp

repo = Path(__file__).resolve().parents[3]
out = repo / 'docs/cue-macros/img'
scratch = repo / 'scratch/cue-macros'
out.mkdir(parents=True, exist_ok=True)
scratch.mkdir(parents=True, exist_ok=True)
binary = Path(os.environ.get('KABL_BINARY', repo / 'target/debug/kabl-ui')).resolve()
disp = ':144'
env = dict(os.environ, DISPLAY=disp, WINIT_X11_SCALE_FACTOR='1',
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


def wait(key):
    end = time.monotonic() + 20
    while time.monotonic() < end:
        if key in hits():
            return
        assert app.poll() is None, 'app exited'
        time.sleep(.15)
    raise AssertionError(('missing', key, sorted(hits())))


def click(key, pause=.5):
    wait(key)
    a, b, c, d = hits()[key]
    run(['xdotool', 'mousemove', '--window', wid, str(round((a + c) / 2)), str(round((b + d) / 2)), 'click', '1'])
    time.sleep(pause)


def theme(name):
    click('view-menu', .4)
    click('theme:' + name, .3)
    click('view-menu', .5)


def shot(name):
    run(['import', '-window', wid, str(out / name)])


try:
    (scratch / 'hits.txt').unlink(missing_ok=True)
    app = sp.Popen([str(binary), '--patch', str(repo / 'patches/cue-macros'), '--perform', '--size', '1280x800', '--no-rt'],
                   env=env, stdout=log, stderr=log)
    time.sleep(2.5)
    wid = run(['xdotool', 'search', '--onlyvisible', '--name', '^kabl$']).splitlines()[-1]
    run(['xdotool', 'windowmove', wid, '0', '0'])
    time.sleep(1)
    if 'list' in sys.argv:
        print('\n'.join(sorted(hits())))
        sys.exit(0)
    cues = sorted(k for k in hits() if k.startswith('pcue:'))
    verse, chorus, bridge = cues[0], cues[1], cues[2]
    click('perform-size', 1)  # Perform fills the window
    # Per theme: targets set and nothing launched; then Chorus and Bridge each caught part-way
    # through their morph (a cue lands on the next bar, 0..2.4 s after the click; the morph
    # then takes 4.8 s and 9.6 s).
    for n, mode in enumerate(['light', 'dark']):
        theme(mode)
        shot(f'{n * 3 + 1:02d}-{mode}-targets-set.png')
        click(verse)
        time.sleep(3.0)
        click(chorus)
        time.sleep(3.4)
        shot(f'{n * 3 + 2:02d}-{mode}-morph-chorus.png')
        time.sleep(4.0)
        click(bridge)
        time.sleep(3.4)
        shot(f'{n * 3 + 3:02d}-{mode}-morph-bridge.png')
        click('perform-size', 1)  # Show rack: stops the pad clicks landing on a stale layout
        click('perform-size', 1)
        click(verse)
        time.sleep(4.0)
    # The rack face of the cues module, a macro-only cue playing.
    theme('light')
    click(verse)
    time.sleep(3.0)
    click('perform-size', 1)  # dock Perform under the rack: the rack shows again
    shot('07-light-rack-cues-face.png')
finally:
    if app:
        app.terminate()
    wm.terminate()
    x.terminate()
