#!/usr/bin/env python3
"""Repaired real widgets at both logical sizes, native scale and full process restart."""
import os,time,json,subprocess as sp,shutil
from pathlib import Path
repo=Path(__file__).resolve().parents[3];root=repo/'scratch/d10-repair/capture';root.mkdir(parents=True,exist_ok=True)
media=repo/'docs/panel-authoring/repair/media';out=repo/'docs/panel-authoring/repair/evidence'
patch=root/'project';shutil.copytree(repo/'scratch/d10-repair/after/patch',patch,dirs_exist_ok=True)
env=dict(os.environ,DISPLAY=':127',XDG_RUNTIME_DIR='/run/user/1000',KABL_USER_DIR=str(root/'library'),KABL_FACTORY_DIR=str(repo/'patches'),KABL_HITS_FILE=str(root/'hits.txt'))
log=open(root/'run.log','w');x=sp.Popen(['Xvfb',':127','-screen','0','2560x1600x24','-nolisten','tcp'],stdout=log,stderr=log);time.sleep(.5);wm=sp.Popen(['metacity','--sm-disable'],env=env,stdout=log,stderr=log);time.sleep(.5)
app=None;scale=1.;results=[]
def run(a):return sp.check_output(a,env=env,text=True)
def hits():
 try:return {p[0]:tuple(map(float,p[1:])) for p in map(str.split,(root/'hits.txt').read_text().splitlines()) if len(p)==5}
 except FileNotFoundError:return {}
def wait(key):
 end=time.monotonic()+15
 while time.monotonic()<end:
  if key in hits():return
  assert app.poll() is None;time.sleep(.15)
 raise AssertionError(('missing',key,list(hits())))
def point(key):wait(key);a,b,c,d=hits()[key];return round((a+c)*scale/2),round((b+d)*scale/2)
def click(key):px,py=point(key);run(['xdotool','mousemove','--window',wid,str(px),str(py),'click','1']);time.sleep(.6)
def capture(name):run(['import','-window',wid,str(media/name)])
def start(size):
 global app,wid
 (root/'hits.txt').unlink(missing_ok=True);app=sp.Popen([str(repo/'target/debug/kabl-ui'),'--patch',str(patch),'--size',size,'--no-rt'],env=env,stdout=log,stderr=log);wait('save');time.sleep(1)
 wid=run(['xdotool','search','--onlyvisible','--name','^kabl$']).splitlines()[-1]
def stop():
 global app
 app.terminate();app.wait(timeout=8);app=None
try:
 state=json.loads((patch/'checkpoint.json').read_text());assert state['modules']['20']['params']['face.waveform']==0
 for size in ['1440x900','1280x800']:
  start(size);click('zoom:fit')
  for theme in ['light','dark']:
   click('theme:'+theme);capture(f'rack-{size}-{theme}.png')
   before={k:r for k,r in hits().items() if k.startswith(('module:','composite:')) and k.endswith((':body',':open'))}
   click('toggle:20');capture(f'more-{size}-{theme}.png')
   assert 'sel:20.waveform.1' in hits(),'Hidden waveform remains reachable in More'
   click('more:close');after={k:r for k,r in hits().items() if k in before};assert before==after,'More changes neighbours/camera'
   click('composite:1:open');capture(f'internals-{size}-{theme}.png');assert 'module:20' not in hits()
   click('composite:4:open');capture(f'nested-{size}-{theme}.png');assert 'module:1' in hits() and 'module:2' not in hits()
   click('subpatch:back');click('subpatch:back');assert {k:hits()[k] for k in before}==before,'Back changes outer camera'
   click('edit-face:20');capture(f'edit-face-{size}-{theme}.png');run(['xdotool','key','Escape']);time.sleep(.5)
   click('browser');time.sleep(.5);assert any(k.startswith('sound:factory:') for k in hits()),'Factory Sounds empty';capture(f'sounds-{size}-{theme}.png');click('browser')
  click('save');stop();results.append({'logical_size':size,'themes':['light','dark'],'more_neighbours_camera_equal':True,'nested_back_equal':True,'hidden_waveform_accessible':True,'factory_browser_populated':True,'full_restart_visibility':True})
 scale=1.5;env['WINIT_X11_SCALE_FACTOR']='1.5';start('1280x800');assert '# pixels_per_point 1.5' in (root/'hits.txt').read_text();click('theme:light');capture('native-scale-150.png')
 # Real selector + knob gestures at native scale; original IDs and positions stay fixed.
 before=json.loads((patch/'checkpoint.json').read_text());click('toggle:20');capture('more-native-scale-150.png');click('sel:20.waveform.1')
 px,py=point('knob:20.base_hz');run(['xdotool','mousemove','--window',wid,str(px),str(py),'mousedown','1','sleep','0.2','mousemove','--window',wid,str(px),str(py-40),'sleep','0.3','mouseup','1']);time.sleep(.5);click('more:close');click('save')
 edited=json.loads((patch/'checkpoint.json').read_text());assert edited['modules']['20']['params']['waveform']==1;assert edited['modules']['20']['params'].get('base_hz')!=before['modules']['20']['params'].get('base_hz');assert edited['modules']['20']['pos']==before['modules']['20']['pos'];assert edited['composites']==before['composites'];stop()
 (out/'viewport-checks.json').write_text(json.dumps({'input':'scripted X11, not owner/hardware proof','screens':results,'native_scale':1.5,'native_selector_knob_changed_original_ids':True,'internal_and_outer_positions_preserved':True},indent=2)+'\n')
finally:
 if app and app.poll() is None:stop()
 wm.terminate();wm.wait(timeout=5);x.terminate();x.wait(timeout=5);log.close();shutil.copy2(root/'run.log',out/'viewport-run.txt')
print('Both sizes/themes, nested Back, More, factory browser and native scaling passed')
