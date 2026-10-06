#!/usr/bin/env python3
"""Real production widgets through isolated X11. Scripted input, no hardware/owner proof."""
import os, subprocess as sp, time, json, shutil
from pathlib import Path
repo=Path(__file__).resolve().parents[3]
root=repo/'scratch/d10-gui';root.mkdir(parents=True,exist_ok=True)
media=repo/'docs/panel-authoring/media';media.mkdir(exist_ok=True)
evidence=repo/'docs/panel-authoring/evidence'
env=dict(os.environ,DISPLAY=':121',KABL_USER_DIR=str(root/'profile'),KABL_FACTORY_DIR=str(repo/'patches'),KABL_HITS_FILE=str(root/'hits.txt'))
log=open(root/'gui.log','w')
x=sp.Popen(['Xvfb',':121','-screen','0','1920x1080x24','-nolisten','tcp'],stdout=log,stderr=log);time.sleep(.4)
wm=sp.Popen(['metacity','--sm-disable'],env=env,stdout=log,stderr=log);time.sleep(.4)
app=None

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
  if (not scroll and 0<=b<900) or (scroll and 55<b<780):return int((a+c)/2),int((b+d)/2)
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
 for name,controls,ports in [('voice',[4,5,6,7],[1,2,3]),('stereo-effect',[5,6,7,8],[1,2,3,4])]:
  patch=root/name
  (root/f'{name}-portable.json').unlink(missing_ok=True)
  sp.run([str(repo/'target/debug/examples/panel_seed'),str(repo/f'docs/composites/examples/{name}.json'),str(patch)],check=True)
  rootid=json.loads((repo/f'docs/composites/examples/{name}.json').read_text())['root']
  start(patch);click(f'composite:{rootid}:design');time.sleep(.5)
  field('panel:width',420,True)
  artkind='voice' if name=='voice' else 'stereo'
  for theme in ['light','dark']:
   field('panel:path',repo/f'docs/panel-authoring/examples/art/{artkind}-{theme}.png')
   click('panel:import-'+theme,True)
  for n,key in enumerate(controls+ports):
   click(f'panel:place:{key}',True)
   field(f'panel:{key}:x',12+(n if n<4 else n-4)*100,True)
   if key in ports:
    field(f'panel:{key}:y',244,True);field(f'panel:{key}:w',84,True)
    label=(['Pitch','Gate','Audio'] if name=='voice' else ['Left','Right','Left','Right'])[ports.index(key)]
    field(f'panel:{key}:label',label)
  if name=='voice':
   click('panel:place:8',True);field('panel:8:y',184,True);field('panel:8:w',390,True);field('panel:8:label','Waveform')
  if name=='stereo-effect':field('panel:8:label','Space mix')
  click('panel:apply',True);time.sleep(.5)
  click('panel:light',True);capture(f'{name}-authoring.png')
  field('panel:path',root/f'{name}-portable.json')
  click('panel:export',True)
  click('panel:done',True);click('save');time.sleep(.7)
  saved=json.loads((patch/'checkpoint.json').read_text());panel=saved['composites'][str(rootid)]['panel']
  assert panel['width']==420 and len(panel['placements'])==len(controls+ports)+(name=='voice'),(panel,hits())
  assert panel['light'] and panel['dark']
  shutil.copy2(root/f'{name}-portable.json',repo/f'docs/panel-authoring/examples/{name}.json')
  shutil.copytree(patch,repo/f'docs/panel-authoring/examples/{name}',dirs_exist_ok=True)
  run(['xdotool','key','ctrl+z']);time.sleep(.4);run(['xdotool','key','ctrl+y']);time.sleep(.4);click('save')
  results.append({'example':name,'GUI_actions':['import light/dark PNG','set width420','place/bind exposed IDs','size/position','apply','preview','export','Undo/Redo','save'],'placements':panel['placements']})
  stop()
  # Full restart uses embedded art, no source image path in project.
  start(patch);click('zoom:fit');time.sleep(.5)
  for theme in ['light','dark']:
   click('theme:'+theme);capture(f'{name}-1440x900-{theme}.png')
  stop()
  for size in ['1280x800']:
   start(patch,size);click('zoom:fit')
   for theme in ['light','dark']:click('theme:'+theme);capture(f'{name}-{size}-{theme}.png')
   stop()
 (evidence/'gui-workflow.json').write_text(json.dumps(results,indent=2)+'\n')
finally:
 if app:stop()
 wm.terminate();wm.wait(timeout=5);x.terminate();x.wait(timeout=5);log.close()
