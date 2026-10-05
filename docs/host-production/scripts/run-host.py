#!/usr/bin/env python3
"""Own one isolated display/host lifetime; retain completion and failure evidence."""
import os, subprocess, sys, time
from pathlib import Path
repo=Path(__file__).resolve().parents[3]
host=Path(os.environ.get('D08_HOST',str(repo/'scratch/host'))).resolve()
profile=repo/'scratch/host/profile/reaper.ini'
script=Path(sys.argv[1]).resolve()
project=Path(sys.argv[2]).resolve() if len(sys.argv)>2 else host/'d08-arrangement.rpp'
marker=host/sys.argv[3] if len(sys.argv)>3 else host/'run.done'
if marker.exists(): marker.unlink()
env=dict(os.environ,DISPLAY=':110',XDG_RUNTIME_DIR='/run/user/1000',PIPEWIRE_QUANTUM='256/48000',KABL_D08_HOST=str(host),KABL_D08_SCRIPTS=str(repo/'docs/host-production/scripts'),KABL_FACTORY_DIR=str(host/'unavailable-factory'),CLAP_PATH=str(repo/'scratch/host/plugins'))
bundle=Path(os.environ['D08_BUNDLE']).resolve() if os.environ.get('D08_BUNDLE') else None
if bundle:
 profile=bundle/'profile/reaper.ini'
 env.update(CLAP_PATH=str(bundle/'plugins'),KABL_FACTORY_DIR=str(bundle/'unavailable-factory'),KABL_D08_SCRIPTS=str(bundle/'scripts'))
if os.environ.get('D08_PROXY'):
 env.update(CLAP_PATH=str(repo/'scratch/proxy'),KABL_TIMING_PLUGIN=str(repo/'target/release/libkabl_clap.so'),KABL_TIMING_OUTPUT=str(host/os.environ.get('D08_TRACE','diagnostic.csv')),KABL_RESET_TRACE='1')
with (host/'run-host.log').open('w') as log:
 xvfb=subprocess.Popen(['Xvfb',':110','-screen','0',os.environ.get('D08_SCREEN','1440x900')+'x24','-ac','-nolisten','tcp'],stdout=log,stderr=log)
 time.sleep(1)
 wm=subprocess.Popen(['metacity','--replace'],env=env,stdout=log,stderr=log)
 backend_log=(host/'backend-pw-top.txt').open('w')
 backend=subprocess.Popen(['pw-top','-b','-n','40'],env=env,stdout=backend_log,stderr=backend_log)
 video=None
 if os.environ.get('D08_VIDEO'):
  video=subprocess.Popen(['ffmpeg','-y','-loglevel','error','-f','x11grab','-framerate','15','-video_size',os.environ.get('D08_SCREEN','1440x900'),'-i',':110','-an','-c:v','libx264','-preset','ultrafast','-crf','23',str(host/'walkthrough.mp4')],env=env,stdout=log,stderr=log)
 proc=subprocess.Popen(['pw-jack','/usr/local/bin/reaper','-newinst','-nosplash','-cfgfile',str(profile),str(project),str(script)],env=env,cwd=str(bundle) if bundle else str(repo),stdout=log,stderr=log)
 (host/'reaper-pid.txt').write_text(str(proc.pid)+'\n')
 try:
  deadline=time.monotonic()+float(os.environ.get("D08_TIMEOUT","50"))
  while time.monotonic()<deadline and not marker.exists() and proc.poll() is None: time.sleep(.25)
  subprocess.run(['import','-window','root',str(host/'run-host.png')],env=env,stdout=log,stderr=log,timeout=5)
  assert marker.exists(),f'No completion marker: {marker}'
  mapped=[line for line in Path(f'/proc/{proc.pid}/maps').read_text().splitlines() if 'kabl' in line]
  (host/'mapped-plugin.txt').write_text('\n'.join(mapped)+'\n')
  (host/'host-cwd.txt').write_text(str(Path(f'/proc/{proc.pid}/cwd').resolve())+'\n')
  if bundle: assert any(str(bundle/'plugins/kabl.clap') in line for line in mapped),'Packaged plugin was not mapped'
  subprocess.run(['pw-jack','/usr/local/bin/reaper','-nonewinst','-cfgfile',str(profile),'-closeall:nosave:exit'],env=env,stdout=log,stderr=log,timeout=5)
  try: proc.wait(timeout=5)
  except subprocess.TimeoutExpired: proc.terminate();proc.wait(timeout=5)
 finally:
  if proc.poll() is None: proc.terminate();proc.wait(timeout=5)
  if video:
   video.send_signal(2);video.wait(timeout=5)
  backend.terminate();backend.wait(timeout=5);backend_log.close()
  wm.terminate();wm.wait(timeout=5);xvfb.terminate();xvfb.wait(timeout=5)
print(marker.read_text())
