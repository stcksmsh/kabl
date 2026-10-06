#!/usr/bin/env python3
"""Real production widgets through isolated X11. Scripted input, no hardware/owner proof."""
import os, subprocess as sp, time, json, shutil
from pathlib import Path
repo=Path(__file__).resolve().parents[3]
root=repo/'scratch/d10-capture';root.mkdir(parents=True,exist_ok=True)
media=repo/'docs/panel-authoring/media';media.mkdir(exist_ok=True)
evidence=repo/'docs/panel-authoring/evidence'
env=dict(os.environ,DISPLAY=':122',KABL_USER_DIR=str(root/'profile'),KABL_FACTORY_DIR=str(repo/'patches'),KABL_HITS_FILE=str(root/'hits.txt'))
log=open(root/'gui.log','w')
x=sp.Popen(['Xvfb',':122','-screen','0','2560x1600x24','-nolisten','tcp'],stdout=log,stderr=log);time.sleep(.4)
wm=sp.Popen(['metacity','--sm-disable'],env=env,stdout=log,stderr=log);time.sleep(.4)
app=None
scale=1.0

def run(args):return sp.run(args,env=env,check=True,stdout=sp.PIPE,text=True).stdout

def hits():
 try:return {p[0]:tuple(map(float,p[1:])) for p in (l.split() for l in (root/'hits.txt').read_text().splitlines()) if len(p)==5}
 except FileNotFoundError:return {}
def waitkey(key):
 end=time.monotonic()+15
 while time.monotonic()<end:
  if key in hits():return
  assert app.poll() is None,'App exited';time.sleep(.1)
 raise AssertionError(('missing',key,list(hits())))
def point(key,scroll=False):
 waitkey(key)
 for _ in range(20):
  a,b,c,d=hits()[key]
  if (not scroll and 0<=b<900) or (scroll and 55<b<780):return int((a+c)/2*scale),int((b+d)/2*scale)
  assert scroll,(key,(a,b,c,d))
  run(['xdotool','mousemove','--window',wid,'500','450','click','--repeat','3','--delay','80','4' if b<55 else '5']);time.sleep(.2)
 raise AssertionError(('offscreen',key,hits()[key]))
def click(key,scroll=False):
 px,py=point(key,scroll);run(['xdotool','mousemove','--window',wid,str(px),str(py),'click','1']);time.sleep(.3)
def field(key,text,drag=False,scroll=True):
 px,py=point(key,scroll);run(['xdotool','mousemove','--window',wid,str(px),str(py),'click','--repeat','2' if drag else '1','--delay','90','1']);time.sleep(.15)
 run(['xdotool','key','ctrl+a']);run(['xdotool','type','--clearmodifiers',str(text)]);run(['xdotool','key','Return']);time.sleep(.25)
def capture(name):run(['import','-window',wid,str(media/name)])
def start(patch,size='1440x900'):
 global app,wid
 (root/'hits.txt').unlink(missing_ok=True)
 app=sp.Popen([str(repo/'target/debug/kabl-ui'),'--patch',str(patch),'--size',size,'--no-rt'],env=env,stdout=log,stderr=log)
 end=time.monotonic()+15
 while time.monotonic()<end and 'save' not in hits():assert app.poll() is None;time.sleep(.2)
 time.sleep(1)
 wid=run(['xdotool','search','--onlyvisible','--name','^kabl$']).splitlines()[-1]
 time.sleep(.5)
def stop():
 global app
 app.terminate();app.wait(timeout=8);app=None
try:
 results=[]
 # Fresh profile import uses only portable files; factory and source library are unavailable.
 env['KABL_FACTORY_DIR']=str(root/'unavailable-factory')
 patch=root/'fresh-project'
 sp.run([str(repo/'target/debug/examples/panel_seed'),'--empty',str(patch)],check=True)
 start(patch);click('composites')
 for name in ['voice','stereo-effect']:
  local=root/(name+'.json');shutil.copy2(repo/f'docs/panel-authoring/examples/{name}.json',local)
  field('package:path',local);click('package:import');time.sleep(.5)
 click('composites');click('save');time.sleep(.5)
 saved=json.loads((patch/'checkpoint.json').read_text());assert len(saved['composites'])==2
 assert all(c['panel']['light'] and c['panel']['dark'] for c in saved['composites'].values())
 # Remove imported source packages before full restart. Project retains exact snapshots.
 for name in ['voice','stereo-effect']:(root/(name+'.json')).unlink()
 stop()
 for size in ['1440x900','1280x800']:
  start(patch,size);click('zoom:100')
  for theme in ['light','dark']:
   click('theme:'+theme);capture(f'portable-{size}-{theme}.png')
  # Turn actual knob: original leaf identity and closed authored face remain active.
  target=saved['composites']['1']['controls']['4']['target']['Param']
  key=f"knob:{target['id']}.{target['param']}";px,py=point(key)
  run(['xdotool','mousemove','--window',wid,str(px),str(py),'mousedown','1','sleep','0.2','mousemove','--window',wid,str(px),str(py-18),'sleep','0.3','mouseup','1']);time.sleep(.5)
  assert 'composite:1:design' in hits(),'Control inspection unexpectedly opened internals'
  capture(f'portable-{size}-focus.png')
  click('save');time.sleep(.4)
  edited=json.loads((patch/'checkpoint.json').read_text())
  assert edited['modules'][str(target['id'])]['params']!=saved['modules'][str(target['id'])]['params'],'Knob gesture did not edit parameter'
  run(['xdotool','key','ctrl+z']);time.sleep(.4)
  click('save');time.sleep(.4)
  assert json.loads((patch/'checkpoint.json').read_text())['composites']==saved['composites'],'Undo changed panel/import instead of knob'
  click('zoom:in');click('zoom:out');click('save');stop()
 # Native X11 desktop scaling. Physical screenshot is 1920x1200 for 1280x800 logical points.
 scale=1.5;env['WINIT_X11_SCALE_FACTOR']='1.5';start(patch,'1280x800');click('zoom:100')
 assert '# pixels_per_point 1.5' in (root/'hits.txt').read_text()
 capture('portable-native-scale-150.png');click('composite:1:fallback');capture('fallback-native-scale-150.png')
 stop();scale=1.0;env.pop('WINIT_X11_SCALE_FACTOR')
 # Existing dense patch and browser: no new synthesis or feature expansion.
 for size in ['1440x900','1280x800']:
  start(repo/'patches/crowded',size);click('zoom:fit')
  for theme in ['light','dark']:
   click('theme:'+theme);capture(f'dense-{size}-{theme}.png')
  click('browser');capture(f'browser-{size}.png');stop()
 shutil.copytree(patch,repo/'docs/panel-authoring/examples/portable',dirs_exist_ok=True)
 (evidence/'portability-scaling.json').write_text(json.dumps({'fresh_profile':True,'source_packages_removed_before_restart':True,'embedded_artwork':True,'themes':['light','dark'],'logical_sizes':['1440x900','1280x800'],'native_pixels_per_point':1.5,'physical_scaled_size':'1920x1200','zoom_gesture_no_internal_reveal':True,'input':'scripted X11, not physical hardware'},indent=2)+'\n')
finally:
 if app:stop()
 wm.terminate();wm.wait(timeout=5);x.terminate();x.wait(timeout=5);log.close()
