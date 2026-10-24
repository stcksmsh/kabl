#!/usr/bin/env python3
"""Screen recording with sound of the Grid Cues piece in the real release editor.

    cargo build --release -p kabl-ui --bin kabl-ui
    python3 docs/grid-cues/scripts/record.py [list]

A private Xvfb display with metacity, the standalone in Perform view at 1280x800, real mouse
input (xdotool) on the cue pads and the transport. The video is grabbed from the display, the
sound from the monitor of the silent sink `kabl_agents`: the standalone plays into that sink
(PIPEWIRE_NODE and PULSE_SINK), never into the default output. The script checks with
`pactl list sink-inputs` that the standalone's stream is on the sink and moves it if it is not.
`list` only prints the drawn target keys.
"""
from pathlib import Path
import os, sys, time, subprocess as sp

repo = Path(__file__).resolve().parents[3]
out = repo / 'docs/grid-cues/media'
scratch = repo / 'scratch/grid-cues-rec'
out.mkdir(parents=True, exist_ok=True)
scratch.mkdir(parents=True, exist_ok=True)
binary = Path(os.environ.get('KABL_BINARY', repo / 'target/release/kabl-ui')).resolve()
SINK = 'kabl_agents'
disp = ':146'
env = dict(os.environ, DISPLAY=disp, WINIT_X11_SCALE_FACTOR='1', PIPEWIRE_NODE=SINK, PULSE_SINK=SINK,
           KABL_USER_DIR=str(scratch / 'library'), KABL_HITS_FILE=str(scratch / 'hits.txt'))
log = open(scratch / 'run.txt', 'w')
x = sp.Popen(['Xvfb', disp, '-screen', '0', '1400x900x24', '-nolisten', 'tcp'], stdout=log, stderr=log)
time.sleep(.6)
wm = sp.Popen(['metacity', '--sm-disable'], env=env, stdout=log, stderr=log)
time.sleep(.6)
app = rec = None


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


def keep_on_silent_sink():
    """Every sink input must be on the silent sink; move any that is not."""
    sinks = {l.split()[0]: l.split()[1] for l in run(['pactl', 'list', 'short', 'sinks']).splitlines()}
    want = next(i for i, n in sinks.items() if n == SINK)
    for l in run(['pactl', 'list', 'short', 'sink-inputs']).splitlines():
        f = l.split()
        if f[1] != want:
            print('moving sink input', f[0], 'from sink', f[1])
            sp.run(['pactl', 'move-sink-input', f[0], SINK], check=True)
    print(run(['pactl', 'list', 'short', 'sink-inputs']))


try:
    (scratch / 'hits.txt').unlink(missing_ok=True)
    app = sp.Popen([str(binary), '--patch', str(repo / 'patches/grid-cues'), '--perform', '--size', '1280x800', '--no-rt'],
                   env=env, stdout=log, stderr=log)
    wid = None
    for _ in range(60):
        time.sleep(.5)
        found = sp.run(['xdotool', 'search', '--onlyvisible', '--name', '^kabl$'], env=env, capture_output=True, text=True).stdout.split()
        if found:
            wid = found[-1]
            break
    assert wid, 'no kabl window'
    run(['xdotool', 'windowmove', wid, '0', '0'])
    time.sleep(1)
    if 'list' in sys.argv:
        print('\n'.join(sorted(hits())))
        sys.exit(0)
    keep_on_silent_sink()
    cues = sorted(k for k in hits() if k.startswith('pcue:'))
    assert len(cues) == 4, cues
    click('perform-size', 1)
    click(cues[0], 1)     # Intro lands at once; the video starts on it
    rec = sp.Popen(['ffmpeg', '-y', '-loglevel', 'error',
                    '-f', 'x11grab', '-framerate', '30', '-video_size', '1280x800', '-i', f'{disp}.0+0,0',
                    '-f', 'pulse', '-i', f'{SINK}.monitor',
                    '-c:v', 'libx264', '-preset', 'fast', '-crf', '23', '-pix_fmt', 'yuv420p',
                    '-c:a', 'aac', '-b:a', '192k', '-shortest', str(out / 'grid-cues-recording.mp4')],
                   env=env, stdout=log, stderr=log, stdin=sp.PIPE)
    time.sleep(1)
    # The patch's clock runs from the start (Run is already on): do not click the transport.
    print(run(['pactl', 'list', 'sink-inputs']).split('Properties')[0])
    # Intro lands at once, the others on the next bar (1.9 s a bar).
    time.sleep(4.0)       # Intro, hats only
    click(cues[1])        # Groove
    time.sleep(9.0)
    click(cues[2])        # Fill: lands on the next bar, then plays one bar at least
    time.sleep(4.0)
    click(cues[3])        # Break
    time.sleep(8.0)
finally:
    if rec:
        rec.communicate(b'q')
        rec.wait(timeout=30)
    if app:
        app.terminate()
    wm.terminate()
    x.terminate()
