#!/usr/bin/env python3
"""Real private X11 interaction evidence. Before binaries retained; owner projects never opened."""
import os,sys,subprocess as sp,time,json,shutil,hashlib
from pathlib import Path
repo=Path(__file__).resolve().parents[3];mode=sys.argv[1];binary=Path(sys.argv[2]).resolve();root=repo/'scratch/d10-repair'/mode;root.mkdir(parents=True,exist_ok=True)
patch=root/'patch';patch.mkdir(exist_ok=True)
s=json.loads((repo/'docs/panel-authoring/examples/portable/checkpoint.json').read_text())
for cid,x,y in [('1',24,10),('2',24,380)]:s['composites'][cid]['pos']={'x':x,'y':y}
for mid,kind,x,y in [(20,'osc.va',444,10),(21,'filter.svf',624,10),(22,'env.adsr',744,10),(23,'lfo',444,380),(24,'osc.va',624,380)]:s['modules'][str(mid)]={'kind':kind,'pos':{'x':x,'y':y},'params':{}}
c=json.loads(json.dumps(s['composites']['1']));c['definition']['id']='d10-repair-default';c['name']='Default voice';c['panel']=None;c['pos']={'x':24,'y':750};c['members']=[i+30 for i in c['members']]
for entry in [*c['controls'].values(),*c['ports'].values()]:next(iter(entry['target'].values()))['id']+=30
for mid in [1,2,3,4]:s['modules'][str(mid+30)]=json.loads(json.dumps(s['modules'][str(mid)]))
s['composites']['3']=c
# Nested oscillator: ownership only, original graph/targets/internal positions remain intact.
c=json.loads(json.dumps(s['composites']['1']));c['definition']['id']='d10-repair-nested';c['name']='Nested oscillator';c['panel']=None;c['parent']=1;c['members']=[1];c['controls']={};c['ports']={};c['pos']={'x':24,'y':10};s['composites']['4']=c;s['composites']['1']['members'].remove(1)
(patch/'checkpoint.json').write_text(json.dumps(s));(patch/'log.jsonl').write_text('');(patch/'meta.toml').write_text('schema_version = 4\n')
sp.run([str(repo/'target/debug/examples/panel_seed'),'--state',str(patch/'checkpoint.json'),str(patch)],check=True)
(root/'hits.txt').unlink(missing_ok=True)
env=dict(os.environ,DISPLAY=':126',XDG_RUNTIME_DIR='/run/user/1000',KABL_FACTORY_DIR=str(repo/'patches'),KABL_USER_DIR=str(root/'library'),KABL_HITS_FILE=str(root/'hits.txt'))
log=open(root/'run.log','w');x=sp.Popen(['Xvfb',':126','-screen','0','1600x1000x24','-nolisten','tcp'],stdout=log,stderr=log);time.sleep(.5);wm=sp.Popen(['metacity','--sm-disable'],env=env,stdout=log,stderr=log);time.sleep(.5)
app=sp.Popen([str(binary),'--patch',str(patch),'--size','1440x900','--no-rt'],env=env,stdout=log,stderr=log);video=None;steps=[]
def run(args):return sp.check_output(args,env=env,text=True)
def hits():
 try:return {v[0]:list(map(float,v[1:])) for v in map(str.split,(root/'hits.txt').read_text().splitlines()) if len(v)==5}
 except FileNotFoundError:return {}
def point(key):
 end=time.monotonic()+10
 while time.monotonic()<end:
  assert app.poll() is None
  if key in hits():a,b,c,d=hits()[key];return int((a+c)/2),int((b+d)/2)
  time.sleep(.15)
 raise AssertionError(('missing',key,list(hits())))
def click(key):
 px,py=point(key);run(['xdotool','mousemove','--window',wid,str(px),str(py),'click','1']);steps.append(key);time.sleep(.8)
def drag_at(px,py,dx,dy,cancel=False):
 run(['xdotool','mousemove','--window',wid,str(px),str(py),'mousedown','1']);time.sleep(.2)
 for n in range(1,13):run(['xdotool','mousemove','--window',wid,str(round(px+dx*n/12)),str(round(py+dy*n/12))]);time.sleep(.085)
 if cancel:run(['xdotool','key','Escape']);time.sleep(.3)
 run(['xdotool','mouseup','1']);time.sleep(.8)
try:
 end=time.monotonic()+20
 while time.monotonic()<end and 'save' not in hits():assert app.poll() is None;time.sleep(.2)
 time.sleep(.8);wid=run(['xdotool','search','--onlyvisible','--name','^kabl$']).splitlines()[-1];run(['xdotool','windowmove','--sync',wid,'0','0']);time.sleep(.5)
 g=dict(line.split('=',1) for line in run(['xdotool','getwindowgeometry','--shell',wid]).splitlines() if '=' in line)
 video=sp.Popen(['ffmpeg','-y','-f','x11grab','-framerate','24','-video_size','1600x1000','-i',':126','-an','-c:v','libx264','-threads','2','-preset','ultrafast','-crf','25','-pix_fmt','yuv420p',str(repo/f'docs/panel-authoring/repair/media/{mode}-interaction.mp4')],stdout=log,stderr=log)
 time.sleep(1);click('theme:light')
 a,b,c,d=hits()['composite:1:open'];drag_at(round((a+c)/2),round(b-18),320,70);steps.append('authored title drag across occupied neighbours')
 a,b,c,d=hits()['module:20'];drag_at(round(a+15),round(b+18),-230,80);steps.append('native title drag across occupied neighbours')
 click('toggle:20');time.sleep(1)
 if mode=='after':click('more:close')
 else:click('toggle:20')
 if mode=='after':
  px,py=point('knob:20.base_hz');drag_at(px,py,0,-24);steps.append('native knob gesture retains item position')
  click('sel:1.waveform.1');px,py=point('knob:2.cutoff_hz');drag_at(px,py,0,-18);steps.append('authored selector and knob address original internal targets')
  px,py=point('out:23.out');tx,ty=point('knob:20.base_hz');drag_at(px,py,tx-px,ty-py);steps.append('native jack drag creates modulation route without item drag')
 click('composite:1:open');time.sleep(1)
 if mode=='after':
  click('composite:4:open');click('subpatch:back');click('subpatch:back')
  click('edit-face:20');click('face:20:waveform');click('face:down:20:base_hz');click('face:apply');click('save')
  click('toggle:20');click('more:close')
  a,b,c,d=hits()['module:20'];drag_at(round(a+15),round(b+18),100,150,True);steps.append('Escape cancels native item drag')
  # Default-card title uses the same selection/drop behavior.
  a,b,c,d=hits()['composite:3:body'];drag_at(round(a+13),round(b+14),280,-260);steps.append('default composite title crosses occupied row');run(['xdotool','key','ctrl+z']);time.sleep(.6)
  # Edge scroll reveals a new destination row without decorative minimum rows.
  a,b,c,d=hits()['module:24'];px,py=round(a+13),round(b+14);run(['xdotool','mousemove','--window',wid,str(px),str(py),'mousedown','1']);time.sleep(.2)
  for n in range(1,13):run(['xdotool','mousemove','--window',wid,str(px),str(round(py+(865-py)*n/12))]);time.sleep(.085)
  time.sleep(2.5);run(['xdotool','mouseup','1']);time.sleep(.8);steps.append('edge scroll/drop reaches extra row');run(['xdotool','key','ctrl+z']);time.sleep(.6);click('zoom:fit')
  click('theme:dark');click('browser');time.sleep(1);click('browser')
 click('save');time.sleep(1)
 video.send_signal(2);video.wait(timeout=10);assert video.returncode in (0,255)
 (repo/f'docs/panel-authoring/repair/evidence/{mode}-interaction.json').write_text(json.dumps({'binary':str(binary),'sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'input':'scripted X11, not owner/hardware proof','steps':steps,'size':'1440x900'},indent=2)+'\n')
finally:
 if video and video.poll() is None:video.send_signal(2);video.wait(timeout=10)
 if app.poll() is None:app.terminate();app.wait(timeout=8)
 wm.terminate();wm.wait(timeout=5);x.terminate();x.wait(timeout=5);log.close()
print(mode,'real interaction recording complete')
