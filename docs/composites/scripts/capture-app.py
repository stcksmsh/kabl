#!/usr/bin/env python3
"""Real standalone GUI captures through scripted X11 input; no hardware-input claim."""
import os,subprocess,time,json
from pathlib import Path
repo=Path(__file__).resolve().parents[3]
scratch=repo/'scratch/d09-ui';scratch.mkdir(parents=True,exist_ok=True)
media=repo/'docs/composites/media';media.mkdir(exist_ok=True)
def run(args,env,**kw):return subprocess.run(args,env=env,check=True,**kw)
for size in ['1440x900','1280x800']:
 env=dict(os.environ,DISPLAY=':111',KABL_USER_DIR=str(scratch/'profile'),KABL_FACTORY_DIR=str(repo/'patches'),KABL_HITS_FILE=str(scratch/'hits.txt'))
 (scratch/'hits.txt').unlink(missing_ok=True)
 log=open(scratch/f'{size}.log','w')
 xvfb=subprocess.Popen(['Xvfb',':111','-screen','0','1920x1080x24','-nolisten','tcp'],stdout=log,stderr=log);time.sleep(.4)
 wm=subprocess.Popen(['metacity','--sm-disable'],env=env,stdout=log,stderr=log);time.sleep(.3)
 app=subprocess.Popen([str(repo/'target/release/kabl-ui'),'--patch',str(repo/'docs/composites/examples/two-instances'),'--size',size,'--no-rt'],env=env,stdout=log,stderr=log)
 try:
  deadline=time.monotonic()+12
  while time.monotonic()<deadline:
   if (scratch/'hits.txt').exists() and 'composite:1:open' in (scratch/'hits.txt').read_text():break
   assert app.poll() is None,'Application exited';time.sleep(.2)
  wid=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^kabl$'],env=env,text=True).splitlines()[-1]
  geom=subprocess.check_output(['xdotool','getwindowgeometry','--shell',wid],env=env,text=True)
  offset={k:int(v) for k,v in (line.split('=',1) for line in geom.splitlines()) if k in ['X','Y']}
  def hits():
   data={}
   for line in (scratch/'hits.txt').read_text().splitlines():
    parts=line.split();
    try:data[parts[0]]=tuple(map(float,parts[1:]))
    except ValueError:pass
   return data
  def click(key):
   values=hits()[key];x=(values[0]+values[2])/2;y=(values[1]+values[3])/2
   run(['xdotool','mousemove','--window',wid,str(int(x)),str(int(y)),'click','1'],env);time.sleep(.5)
  time.sleep(1)
  # Main-window movement is detected through current hit coordinates; no edited screenshots.
  for theme in ['light','dark']:
   click('theme:'+theme)
   run(['import','-window',wid,str(media/f'app-{size}-{theme}.png')],env)
  click('composite:1:open');time.sleep(.6)
  run(['import','-window',wid,str(media/f'app-{size}-internals.png')],env)
  (scratch/f'{size}-hits.json').write_text(json.dumps(hits(),indent=2)+'\n')
 finally:
  app.terminate();app.wait(timeout=5);wm.terminate();wm.wait(timeout=5);xvfb.terminate();xvfb.wait(timeout=5);log.close()
print('Actual application screenshots captured; scripted X11 input, no owner approval.')
