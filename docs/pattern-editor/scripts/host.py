#!/usr/bin/env python3
"""Starts REAPER (private Xvfb, metacity, PipeWire) with one kabl instance on a patch, opens the
plugin editor at 1180x680 and plays the project, then waits until SCRATCH/quit exists.

    cargo build --release -p kabl-clap
    python3 docs/pattern-editor/scripts/host.py PATCH_DIR SCRATCH [DISPLAY_NUMBER]

PATCH_DIR holds a checkpoint.json (e.g. patches/functional-cables/morph-arc). Drive and capture
with DISPLAY=:N xdotool / import while it runs. Method as docs/reaper-early-notes/scripts/host.py.
"""
from pathlib import Path
import base64, json, os, shutil, subprocess as sp, sys, textwrap, time

repo = Path(__file__).resolve().parents[3]
patch_dir, S = Path(sys.argv[1]), Path(sys.argv[2])
DISP = ':%s' % (sys.argv[3] if len(sys.argv) > 3 else '248')
SECS = 600
shutil.rmtree(S, ignore_errors=True)
S.mkdir(parents=True)
patch = json.loads((patch_dir / 'checkpoint.json').read_text())
lanes = [{'retired': False, 'target': None} for _ in range(16)]
state = {'version': 4, 'patch': patch, 'output_gain': .4, 'host_clock': False, 'lanes': lanes,
         'slot_values': [0.0] * 16}
raw = json.dumps(state, separators=(',', ':')).encode()
enc = base64.b64encode(len(raw).to_bytes(8, 'little') + raw).decode()
chunk = ''.join('          ' + l + '\n' for l in textwrap.wrap(enc, 120))
proj = f'''<REAPER_PROJECT 0.1 "7.75/linux-x86_64" 0
  TEMPO 120 4 4
  SAMPLERATE 48000 1 0
  <TRACK
    NAME "pattern"
    VOLPAN 0.65 0 1 -1
    <FXCHAIN
      SHOW -1
      <CLAP "CLAPi: kabl (stcksmsh)" dev.stcksmsh.kabl ""
        CFG 4 1180 680 ""
        <STATE
{chunk}        >
      >
      WAK 0 0
    >
    <ITEM
      POSITION 0
      LENGTH {SECS}
      <SOURCE MIDI
        HASDATA 1 960 QN
        E 960 90 3c 64
        E 1920000 80 3c 00
      >
    >
  >
>
'''
(S / 'p.rpp').write_text(proj)
env = dict(os.environ, DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000', KABL_FACTORY_DIR=str(S / 'nf'),
           KABL_USER_DIR=str(S / 'nl'), KABL_CONFIG_DIR=str(S / 'cfg'),
           CLAP_PATH=str(S / 'plugin'))
log = open(S / 'run.txt', 'w')
x = sp.Popen(['Xvfb', DISP, '-screen', '0', '1600x1000x24', '-nolisten', 'tcp'], stdout=log, stderr=log)
time.sleep(.8)
wm = sp.Popen(['metacity', '--sm-disable'], env=env, stdout=log, stderr=log)
time.sleep(.8)
try:
    (S / 'plugin').mkdir()
    (S / 'empty').mkdir()
    shutil.copy2(os.environ.get('PLUGIN', repo / 'target/release/libkabl_clap.so'), S / 'plugin/kabl.clap')
    (S / 'cfg').mkdir()
    theme, _, mode = os.environ.get('HOST_THEME', 'kabl.satin:light').partition(':')
    (S / 'cfg/settings.toml').write_text(f'theme = "{theme}"\ndark = {"true" if mode == "dark" else "false"}\n')
    (S / 'profile').mkdir()
    (S / 'profile/reaper.ini').write_text(
        f'[reaper]\nvstpath={S}/empty\nvstpath64={S}/empty\nlinux_audio_bsize=256\n'
        'linux_audio_mode=0\nlinux_audio_srate=48000\nwnd_w=1280\nwnd_h=800\nwnd_x=0\nwnd_y=0\n')
    app = sp.Popen(['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile',
                    str(S / 'profile/reaper.ini'), str(S / 'p.rpp'), str(Path(__file__).with_name('host.lua'))],
                   env=env, cwd=S, stdout=log, stderr=log)
    print('up', DISP, flush=True)
    shot = os.environ.get('SHOT')  # PNG path: open the pattern editor, capture, and end
    if shot:
        def tool(*a):
            sp.run(a, env=env, stdout=log, stderr=log)
        time.sleep(14)
        for _ in range(8):  # the update notice can arrive late: keep closing it
            tool('wmctrl', '-c', 'About REAPER')
            tool('wmctrl', '-c', 'REAPER New Version')
            time.sleep(1.5)
        # Select the ladder filter's cutoff, then open the pattern of the route into it.
        for x_, y_ in ((748, 541), (877, 551)):
            tool('xdotool', 'mousemove', str(x_), str(y_))
            time.sleep(.5)
            tool('xdotool', 'click', '1')
            time.sleep(1.5)
        time.sleep(2)
        tool('import', '-window', 'root', '-crop', '1180x745+64+92', shot)
        (S / 'quit').write_text('')
    while not (S / 'quit').exists() and app.poll() is None:
        time.sleep(.5)
    app.terminate()
    app.wait(timeout=15)
finally:
    for pr in (wm, x):
        pr.terminate()
        pr.wait(timeout=8)
    log.close()
