#!/usr/bin/env python3
"""The plugin editor in REAPER (floating window), captured while the project's CC cue buttons play.

    cargo build --release -p kabl-clap && python3 docs/cue-macros/scripts/host.py && python3 docs/cue-macros/scripts/editor.py

Uses the project docs/cue-macros/scripts/host.py wrote (scratch/cue-macros-host/cues.rpp). Screenshots
of the whole X screen go to docs/cue-macros/img/host-*.png; the editor window's size is printed.
"""
from pathlib import Path
import os, shutil, subprocess as sp, time

repo = Path(__file__).resolve().parents[3]
S = repo / 'scratch/cue-macros-host'
OUT = repo / 'docs/cue-macros/img'
DISP = ':149'
env0 = dict(os.environ, DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000', KABL_FACTORY_DIR=str(S / 'unavailable-factory'),
            KABL_USER_DIR=str(S / 'unavailable-library'), KABL_SE_HOST=str(S))
log = open(S / 'editor-run.txt', 'w')
(S / 'editor-open').unlink(missing_ok=True)
x = sp.Popen(['Xvfb', DISP, '-screen', '0', '1600x1000x24', '-nolisten', 'tcp'], stdout=log, stderr=log); time.sleep(.8)
wm = sp.Popen(['metacity', '--sm-disable'], env=env0, stdout=log, stderr=log); time.sleep(.8)
pdir = S / 'plugin'; pdir.mkdir(exist_ok=True)
shutil.copy2(repo / 'target/release/libkabl_clap.so', pdir / 'kabl.clap')
env = dict(env0, CLAP_PATH=str(pdir))
app = sp.Popen(['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile', str(S / 'profile/reaper.ini'),
                str(S / 'cues.rpp'), str(Path(__file__).with_name('editor.lua'))], env=env, cwd=S, stdout=log, stderr=log)
try:
    end = time.monotonic() + 60
    while time.monotonic() < end and not (S / 'editor-open').exists(): time.sleep(.3)
    assert (S / 'editor-open').exists(), 'editor did not open'
    time.sleep(12)  # playing: Verse at 1.5 s, Chorus at 6 s lands on a bar
    print(sp.check_output(['xdotool', 'search', '--onlyvisible', '--name', 'kabl'], env=env0, text=True))
    sp.check_call(['xdotool', 'mousemove', '230', '196', 'click', '1'], env=env0)  # the Perform tab
    for i, wait in enumerate((2, 3)):
        time.sleep(wait)
        sp.check_call(['import', '-window', 'root', str(OUT / f'host-{i + 1}.png')], env=env0)
finally:
    app.terminate(); app.wait(timeout=15)
    for p in (wm, x): p.terminate()
