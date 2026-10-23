#!/usr/bin/env python3
"""Renders a project in a scripted REAPER on its own Xvfb display and profile.

    python3 docs/vocoder/scripts/host-run.py PROJECT.rpp PLUGIN.so NAME [DISPLAY_NUM]

Copies PLUGIN.so to scratch/vocoder-host/plugins/kabl.clap (CLAP_PATH points there), renders
PROJECT with host-render.lua and leaves scratch/vocoder-host/NAME.wav and NAME.done. The profile
is scratch/vocoder-host/profile (a copy of reaper.ini from ~/.config/REAPER on first use). Prints
what REAPER reported about the plugin's pins.
"""
import os, shutil, subprocess as sp, sys, time
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
host = repo / 'scratch/vocoder-host'
project, plugin, name = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve(), sys.argv[3]
disp = ':' + (sys.argv[4] if len(sys.argv) > 4 else '172')
(host / 'plugins').mkdir(parents=True, exist_ok=True)
(host / 'profile').mkdir(parents=True, exist_ok=True)
shutil.copy(plugin, host / 'plugins/kabl.clap')
ini = host / 'profile/reaper.ini'
if not ini.exists():
    shutil.copy(Path.home() / '.config/REAPER/reaper.ini', ini)
for f in (host / f'{name}.wav', host / f'{name}.done'):
    f.unlink(missing_ok=True)
env = dict(os.environ, DISPLAY=disp, CLAP_PATH=str(host / 'plugins'), KABL_HOST_DIR=str(host),
           KABL_RENDER_NAME=name, XDG_RUNTIME_DIR='/run/user/1000')
log = (host / 'run.log').open('w')
xvfb = sp.Popen(['Xvfb', disp, '-screen', '0', '1440x900x24', '-ac', '-nolisten', 'tcp'], stdout=log, stderr=log)
time.sleep(1)
reaper = sp.Popen(['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile', str(ini),
                   str(project), str(Path(__file__).with_name(os.environ.get('KABL_LUA', 'host-render.lua')))],
                  env=env, cwd=str(repo), stdout=log, stderr=log)
try:
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline and not (host / f'{name}.done').exists() and reaper.poll() is None:
        # The update notice and similar windows can sit in front of the script.
        sp.run(['wmctrl', '-c', 'REAPER New Version Notification'], env=env, stdout=log, stderr=log)
        time.sleep(1)
    sp.run(['import', '-window', 'root', str(host / f'{name}.png')], env=env, stdout=log, stderr=log, timeout=10)
    time.sleep(2)
finally:
    if reaper.poll() is None:
        reaper.terminate()
        try:
            reaper.wait(timeout=8)
        except sp.TimeoutExpired:
            reaper.kill()
    xvfb.terminate()
done = host / f'{name}.done'
print(done.read_text() if done.exists() else 'NO COMPLETION MARKER')
print('wav', (host / f'{name}.wav').exists())
