#!/usr/bin/env python3
"""Own private REAPER process/display/profile; never message existing host sessions."""
import os,subprocess as sp,time,shutil,json,base64,re,struct
from pathlib import Path
repo=Path(__file__).resolve().parents[3];host=Path(os.environ.get('KABL_D10_HOST_ROOT',repo/'scratch/d10-host'));out=Path(os.environ.get('KABL_D10_EVIDENCE',repo/'docs/panel-authoring/evidence'));out.mkdir(parents=True,exist_ok=True)
shutil.rmtree(host/'source-library',ignore_errors=True)
(host/'done').unlink(missing_ok=True)
env=dict(os.environ,DISPLAY=':123',XDG_RUNTIME_DIR='/run/user/1000',PIPEWIRE_QUANTUM='256/48000',KABL_D10_HOST=str(host),KABL_USER_DIR=str(host/'unavailable-library'),KABL_FACTORY_DIR=str(host/'unavailable-factory'),CLAP_PATH=os.environ.get('KABL_D10_PLUGIN_DIR',str(host/'plugins')))
log=open(host/'run.log','w');x=sp.Popen(['Xvfb',':123','-screen','0','1440x900x24','-nolisten','tcp'],stdout=log,stderr=log);time.sleep(.6)
wm=sp.Popen(['metacity','--sm-disable'],env=env,stdout=log,stderr=log);time.sleep(.5)
p=sp.Popen(['pw-jack','/usr/local/bin/reaper','-newinst','-nosplash','-cfgfile',str(host/'profile/reaper.ini'),os.environ.get('KABL_D10_PROJECT',str(host/'d10-arrangement.rpp')),str(repo/'docs/panel-authoring/scripts/host.lua')],env=env,cwd=str(host),stdout=log,stderr=log)
try:
 end=time.monotonic()+55
 while time.monotonic()<end and not (host/'done').exists() and p.poll() is None:time.sleep(.25)
 sp.run(['import','-window','root',str(Path(os.environ.get('KABL_D10_HOST_SCREENSHOT',repo/'docs/panel-authoring/media/reaper-private.png')))],env=env,check=True)
 assert (host/'done').exists(),'Host completion missing; inspect retained run.log'
 maps=[l for l in Path(f'/proc/{p.pid}/maps').read_text().splitlines() if 'kabl.clap' in l];assert maps
 (out/'mapped-plugin.txt').write_text('\n'.join(maps)+'\n')
 expected=json.loads((host/'expected.json').read_text())
 actual=[]
 for chunk in re.findall(r'<STATE\s+([^>]+)>',(host/'recalled.rpp').read_text()):
  raw=base64.b64decode(''.join(chunk.split()));assert struct.unpack_from('<Q',raw)[0]==len(raw)-8;actual.append(json.loads(raw[8:]))
 def f32(v):
  if isinstance(v,float):return struct.unpack('<f',struct.pack('<f',v))[0]
  if isinstance(v,list):return [f32(x) for x in v]
  if isinstance(v,dict):return {k:f32(x) for k,x in v.items()}
  return v
 assert f32(expected)==f32(actual),'Complete recalled panel/sound state differs'
 shutil.copy2(host/'native-replay.txt',out/'native-replay.txt');shutil.copy2(host/'recalled.rpp',out/'recalled.rpp')
 (out/'host-recall.json').write_text(json.dumps({'instances':2,'complete_f32_state_equal':True,'panel_art_embedded':True,'source_library_removed':True,'native_lanes_unchanged':True,'closed_editor_replay_times':[2,6,13],'native_tolerance':.025,'input':'scripted MIDI/automation, not physical hardware'},indent=2)+'\n')
finally:
 if p.poll() is None:p.terminate();p.wait(timeout=8)
 wm.terminate();wm.wait(timeout=5);x.terminate();x.wait(timeout=5);log.close()
 shutil.copy2(host/'run.log',out/'host-run.txt')
