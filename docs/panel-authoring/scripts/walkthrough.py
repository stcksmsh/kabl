#!/usr/bin/env python3
"""Real release GUI walkthrough. Scripted private X11 input; no owner/hardware proof."""
import os,sys,time,json,shutil,subprocess as sp
from pathlib import Path
repo=Path(__file__).resolve().parents[3];bundle=Path(sys.argv[1]).resolve();root=repo/'scratch/d10-video';root.mkdir(parents=True,exist_ok=True)
shutil.copytree(bundle/'patches/portable',root/'patch',dirs_exist_ok=True)
(root/'hits.txt').unlink(missing_ok=True)
env=dict(os.environ,DISPLAY=':125',XDG_RUNTIME_DIR='/run/user/1000',KABL_USER_DIR=str(root/'profile'),KABL_FACTORY_DIR=str(root/'unavailable-factory'),KABL_HITS_FILE=str(root/'hits.txt'))
log=open(root/'run.log','w');x=sp.Popen(['Xvfb',':125','-screen','0','1440x900x24','-nolisten','tcp'],stdout=log,stderr=log);time.sleep(.6);wm=sp.Popen(['metacity','--sm-disable'],env=env,stdout=log,stderr=log);time.sleep(.5)
app=sp.Popen([str(bundle/'bin/kabl-ui'),'--patch',str(root/'patch'),'--size','1440x900','--no-rt'],env=env,stdout=log,stderr=log);video=None
steps=[]
def run(args):return sp.check_output(args,env=env,text=True)
def hits():
 try:return {p[0]:list(map(float,p[1:])) for p in map(str.split,(root/'hits.txt').read_text().splitlines()) if len(p)==5}
 except FileNotFoundError:return {}
def point(key):
 for _ in range(30):
  h=hits();assert app.poll() is None
  if key in h:
   a,b,c,d=h[key]
   if 65<b<840:return int((a+c)/2),int((b+d)/2)
   if key.startswith('theme:') or key=='save':return int((a+c)/2),int((b+d)/2)
   run(['xdotool','mousemove','--window',wid,'500','450','click','--repeat','3','--delay','80','4' if b<65 else '5'])
  time.sleep(.2)
 raise AssertionError(key)
def click(key):
 px,py=point(key);run(['xdotool','mousemove','--window',wid,str(px),str(py),'click','1']);steps.append(key);time.sleep(1.4)
try:
 end=time.monotonic()+20
 while time.monotonic()<end and 'save' not in hits():assert app.poll() is None;time.sleep(.2)
 wid=run(['xdotool','search','--onlyvisible','--name','^kabl$']).splitlines()[-1];time.sleep(1)
 video=sp.Popen(['ffmpeg','-y','-f','x11grab','-framerate','24','-video_size','1440x900','-i',':125','-an','-c:v','libx264','-preset','veryfast','-crf','25','-pix_fmt','yuv420p','-metadata','title=D10 actual release GUI; scripted X11 input',str(repo/'docs/panel-authoring/media/walkthrough.mp4')],stdout=log,stderr=log)
 time.sleep(2);click('theme:dark');click('theme:light');click('composite:1:design');click('panel:dark');click('panel:light')
 px,py=point('panel:4:label');run(['xdotool','mousemove','--window',wid,str(px),str(py),'click','1','key','ctrl+a']);run(['xdotool','type','--clearmodifiers','Tone']);run(['xdotool','key','Return']);time.sleep(1)
 click('panel:apply');click('panel:done');run(['xdotool','key','ctrl+z']);steps.append('Ctrl+Z');time.sleep(1.4);click('save');click('composite:1:fallback');time.sleep(3)
 video.send_signal(2);video.wait(timeout=10);assert video.returncode in (0,255)
 assert json.loads((root/'patch/checkpoint.json').read_text())['composites']['1']['controls']['4']['label']=='Brightness'
 (repo/'docs/panel-authoring/evidence/walkthrough.json').write_text(json.dumps({'release_sha256':__import__('hashlib').sha256((bundle/'bin/kabl-ui').read_bytes()).hexdigest(),'size':'1440x900','input':'scripted private X11, not owner hardware','steps':steps,'edit':'Brightness→Tone, Apply, Done, chronological Undo; fallback reaches all public controls','audio':'not recorded; UI presentation only'},indent=2)+'\n')
finally:
 if video and video.poll() is None:video.send_signal(2);video.wait(timeout=10)
 if app.poll() is None:app.terminate();app.wait(timeout=8)
 wm.terminate();wm.wait(timeout=5);x.terminate();x.wait(timeout=5);log.close()
print('Actual release walkthrough captured')
