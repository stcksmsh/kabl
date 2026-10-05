#!/usr/bin/env python3
"""Drive production standalone widgets. X11 gestures, no hardware-performance claim."""
import os, subprocess, time, shutil, json
from pathlib import Path
repo=Path(__file__).resolve().parents[3]
scratch=repo/'scratch/d09-workflow';scratch.mkdir(parents=True,exist_ok=True)
patch=scratch/'workflow';shutil.copytree(repo/'docs/composites/examples/flat-voice',patch,dirs_exist_ok=True)
hitfile=scratch/'hits.txt';hitfile.unlink(missing_ok=True)
env=dict(os.environ,DISPLAY=':112',KABL_USER_DIR=str(scratch/'profile'),KABL_FACTORY_DIR=str(repo/'patches'),KABL_HITS_FILE=str(hitfile))
log=open(scratch/'app.log','w')
xvfb=subprocess.Popen(['Xvfb',':112','-screen','0','1920x1080x24','-nolisten','tcp'],stdout=log,stderr=log);time.sleep(.4)
wm=subprocess.Popen(['metacity','--sm-disable'],env=env,stdout=log,stderr=log);time.sleep(.3)
app=subprocess.Popen([str(repo/'target/release/kabl-ui'),'--patch',str(patch),'--size','1440x900','--no-rt'],env=env,stdout=log,stderr=log)
video=None
try:
 deadline=time.monotonic()+12
 while time.monotonic()<deadline and not hitfile.exists():
  assert app.poll() is None;time.sleep(.2)
 wid=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^kabl$'],env=env,text=True).splitlines()[-1]
 def run(args):return subprocess.run(args,env=env,check=True)
 def hits():
  return {p[0]:tuple(map(float,p[1:])) for p in (line.split() for line in hitfile.read_text().splitlines()) if len(p)==5}
 def click(key):
  deadline=time.monotonic()+3
  while key not in hits() and time.monotonic()<deadline:time.sleep(.1)
  x,y,r,b=hits()[key]
  assert 0<=y<900 and b>0,(key,hits()[key])
  run(['xdotool','mousemove','--window',wid,str(int((x+r)/2)),str(int((y+b)/2)),'click','1']);time.sleep(.5)
 video=subprocess.Popen(['ffmpeg','-y','-loglevel','error','-f','x11grab','-framerate','12','-video_size','1920x1080','-i',':112','-an','-c:v','libx264','-preset','ultrafast','-crf','25',str(scratch/'workflow.mp4')],env=env,stdout=log,stderr=log)
 time.sleep(1)
 click('composites')
 for leaf in [2,3,4,5]:click(f'composite:select:{leaf}')
 click('composite:name');run(['xdotool','type','--clearmodifiers','Workflow voice'])
 click('composite:encapsulate');click('composite:1:tools');click('composite:1:duplicate')
 click('composites');click('zoom:fit')
 run(['import','-window',wid,str(repo/'docs/composites/media/workflow-copies.png')])
 # Change the public brightness alias on the original; the copy stays unchanged.
 key='composite:1:control:4';x,y,r,b=hits()[key]
 run(['xdotool','mousemove','--window',wid,str(int(x+8)),str(int((y+b)/2)),'mousedown','1','sleep','0.2','mousemove','--window',wid,str(int(x+25)),str(int((y+b)/2)),'sleep','0.3','mouseup','1']);time.sleep(.5)
 run(['xdotool','key','ctrl+z']);time.sleep(.4);run(['xdotool','key','ctrl+y']);time.sleep(.4);click('composite:1:open');time.sleep(.6)
 run(['import','-window',wid,str(repo/'docs/composites/media/workflow-original-open.png')])
 click('composite:close-all');click('composite:2:open');time.sleep(.6)
 run(['import','-window',wid,str(repo/'docs/composites/media/workflow-copy-open.png')])
 click('composite:close-all');click('save');time.sleep(1)
 saved=json.loads((patch/'checkpoint.json').read_text())
 assert len(saved['composites'])==2
 groups=saved['composites'];a=groups['1'];b=groups['2'];assert set(a['members']).isdisjoint(b['members'])
 cutoffs=[]
 for c in [a,b]:
  leaf=next(str(i) for i in c['members'] if saved['modules'][str(i)]['kind']=='filter.svf')
  cutoffs.append(saved['modules'][leaf]['params'].get('cutoff_hz',3000))
 assert cutoffs[0]!=cutoffs[1],cutoffs
 shutil.copytree(patch,repo/'docs/composites/examples/workflow',dirs_exist_ok=True)
 (repo/'docs/composites/evidence/workflow.json').write_text(json.dumps({'actions':['select 4 leaves','encapsulate','duplicate','edit original public alias','project Undo/Redo','open original','open copy','save'],'disjoint_leaves':True,'cutoffs':cutoffs,'history':'chronological project Undo demonstration; owner decision still pending'},indent=2)+'\n')
 print('Real widgets encapsulated, duplicated, edited, Undo/Redo, opened both instances and saved; independent cutoff values:',cutoffs)
finally:
 if video:video.send_signal(2);video.wait(timeout=5)
 app.terminate();app.wait(timeout=5);wm.terminate();wm.wait(timeout=5);xvfb.terminate();xvfb.wait(timeout=5);log.close()
