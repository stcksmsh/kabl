#!/usr/bin/env python3
"""Own one isolated display/host lifetime; retain completion and failure evidence."""
import os, subprocess, sys, time
from pathlib import Path
repo=Path(__file__).resolve().parents[3]
host=repo/'scratch/host'
script=Path(sys.argv[1]).resolve()
project=Path(sys.argv[2]).resolve() if len(sys.argv)>2 else host/'d08-arrangement.rpp'
marker=host/sys.argv[3] if len(sys.argv)>3 else host/'run.done'
if marker.exists(): marker.unlink()
env=dict(os.environ,DISPLAY=':110',XDG_RUNTIME_DIR='/run/user/1000',PIPEWIRE_QUANTUM='256/48000',KABL_D08_HOST=str(host),KABL_FACTORY_DIR=str(host/'unavailable-factory'),CLAP_PATH=str(repo/'scratch/host/plugins'))
if os.environ.get('D08_PROXY'):
 env.update(CLAP_PATH=str(repo/'scratch/proxy'),KABL_TIMING_PLUGIN=str(repo/'target/release/libkabl_clap.so'),KABL_TIMING_OUTPUT=str(host/'diagnostic.csv'),KABL_RESET_TRACE='1')
with (host/'run-host.log').open('w') as log:
 xvfb=subprocess.Popen(['Xvfb',':110','-screen','0','1440x900x24','-ac','-nolisten','tcp'],stdout=log,stderr=log)
 time.sleep(1)
 wm=subprocess.Popen(['metacity','--replace'],env=env,stdout=log,stderr=log)
 proc=subprocess.Popen(['pw-jack','/usr/local/bin/reaper','-newinst','-nosplash','-cfgfile',str(host/'profile/reaper.ini'),str(project),str(script)],env=env,stdout=log,stderr=log)
 try:
  deadline=time.monotonic()+50
  while time.monotonic()<deadline and not marker.exists() and proc.poll() is None: time.sleep(.25)
  subprocess.run(['import','-window','root',str(host/'run-host.png')],env=env,stdout=log,stderr=log,timeout=5)
  assert marker.exists(),f'No completion marker: {marker}'
  subprocess.run(['pw-jack','/usr/local/bin/reaper','-nonewinst','-cfgfile',str(host/'profile/reaper.ini'),'-closeall:nosave:exit'],env=env,stdout=log,stderr=log,timeout=5)
  try: proc.wait(timeout=5)
  except subprocess.TimeoutExpired: proc.terminate();proc.wait(timeout=5)
 finally:
  if proc.poll() is None: proc.terminate();proc.wait(timeout=5)
  wm.terminate();wm.wait(timeout=5);xvfb.terminate();xvfb.wait(timeout=5)
print(marker.read_text())
