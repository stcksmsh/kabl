#!/usr/bin/env python3
"""Opens the CLAP plugin's editor in a real REAPER, on a private display, and leaves both running
so the file browser can be driven with real X input (xdotool) and captured (import).

    cargo build --release -p kabl-clap
    python3 docs/file-browser/scripts/host.py WORKDIR [CONFIG_DIR]

WORKDIR gets the Xvfb log, a REAPER profile, a copy of the plugin on CLAP_PATH and the project
(docs/wavetable-editor/host-make-project.py with this checkout's imported-morph patch: it has an
osc.wt, so the wavetable browser is reachable). CONFIG_DIR is KABL_CONFIG_DIR for the plugin (its
settings.toml holds the remembered folders). The display is :148, the plugin editor opens
floating, 1180x680. Ends when REAPER ends or on SIGTERM.
"""
import os, re, shutil, signal, subprocess as sp, sys, time
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
work = Path(sys.argv[1]).resolve()
cfg = Path(sys.argv[2]).resolve() if len(sys.argv) > 2 else work / 'config'
DISP = ':148'
shutil.rmtree(work / 'profile', ignore_errors=True)
for d in ('profile', 'plugin', 'empty-vst', 'config'):
    (work / d).mkdir(parents=True, exist_ok=True)

maker = (repo / 'docs/wavetable-editor/host-make-project.py').read_text()
maker = re.sub(r"repo = Path\('[^']*'\)", f"repo = Path('{repo}')", maker)
(work / 'make-project.py').write_text(maker)
sp.run([sys.executable, str(work / 'make-project.py'), str(work / 'project.rpp')], check=True)

# Show the plugin's window shortly after the project loads.
(work / 'show.lua').write_text(
    'local t0 = reaper.time_precise()\n'
    'local function step()\n'
    '  if reaper.time_precise() - t0 < 3 then reaper.defer(step) return end\n'
    '  reaper.OnStopButton()\n'
    '  reaper.TrackFX_Show(reaper.GetTrack(0, 0), 0, 3)\n'
    'end\n'
    'reaper.defer(step)\n')
(work / 'profile/reaper.ini').write_text(
    f'[reaper]\nvstpath={work}/empty-vst\nvstpath64={work}/empty-vst\nlinux_audio_bsize=256\n'
    'linux_audio_mode=0\nlinux_audio_srate=48000\nwnd_w=1280\nwnd_h=800\nwnd_x=0\nwnd_y=0\n')
shutil.copy2(repo / 'target/release/libkabl_clap.so', work / 'plugin/kabl.clap')

env = dict(os.environ, DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000',
           CLAP_PATH=str(work / 'plugin'), KABL_CONFIG_DIR=str(cfg))
log = open(work / 'run.txt', 'w')
x = sp.Popen(['Xvfb', DISP, '-screen', '0', '1440x900x24', '-nolisten', 'tcp'], stdout=log, stderr=log)
time.sleep(0.8)
wm = sp.Popen(['metacity', '--sm-disable'], env=env, stdout=log, stderr=log)
time.sleep(0.8)
app = sp.Popen(['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile',
                str(work / 'profile/reaper.ini'), str(work / 'project.rpp'), str(work / 'show.lua')],
               env=env, cwd=work, stdout=log, stderr=log)
(work / 'pid').write_text(f'{app.pid} {wm.pid} {x.pid}\n')
signal.signal(signal.SIGTERM, lambda *_: (app.terminate(), wm.terminate(), x.terminate()))
app.wait()
wm.terminate()
x.terminate()
