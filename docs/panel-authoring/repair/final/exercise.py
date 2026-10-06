#!/usr/bin/env python3
"""Private scripted X11 proof. Only task-owned copies/profile/display are opened."""
from pathlib import Path
import os,time,json,hashlib,shutil,subprocess as sp
repo=Path(__file__).resolve().parents[4];root=repo/'scratch/d10-final';root.mkdir(exist_ok=True)
out=repo/'docs/panel-authoring/repair/final';binary=Path(os.environ.get('KABL_D10_FINAL_BINARY',repo/'target/debug/kabl-ui')).resolve()
patch=root/'patch';shutil.copytree(repo/'scratch/d10-repair/after/patch',patch,dirs_exist_ok=True)
# Seed one existing native sequencer into the task-only fixture for true viewport overflow.
state=json.loads((patch/'checkpoint.json').read_text());state['modules']['25']={'kind':'seq','pos':{'x':700,'y':750},'params':{}}
(patch/'checkpoint.json').write_text(json.dumps(state));sp.run([str(repo/'target/debug/examples/panel_seed'),'--state',str(patch/'checkpoint.json'),str(patch)],check=True)
# No owner project is opened.
env=dict(os.environ,DISPLAY=':128',XDG_RUNTIME_DIR='/run/user/1000',KABL_FACTORY_DIR=str(repo/'patches'),KABL_USER_DIR=str(root/'library'),KABL_HITS_FILE=str(root/'hits.txt'))
log=open(root/'run.txt','w');x=sp.Popen(['Xvfb',':128','-screen','0','2560x1600x24','-nolisten','tcp'],stdout=log,stderr=log);time.sleep(.6);wm=sp.Popen(['metacity','--sm-disable'],env=env,stdout=log,stderr=log);time.sleep(.6)
app=None;video=None;scale=1.;results=[]
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
def pointer(p):run(['xdotool','mousemove','--window',wid,str(round(p[0]*scale)),str(round(p[1]*scale))]);time.sleep(.15)
def point(key):wait(key);a,b,c,d=hits()[key];return ((a+c)/2,(b+d)/2)
def click(key):pointer(point(key));run(['xdotool','click','1']);time.sleep(.65)
def wheel(p,n,button=5,modifier=None):
 pointer(p)
 if modifier:run(['xdotool','keydown',modifier])
 run(['xdotool','click','--repeat',str(n),'--delay','55',str(button)])
 if modifier:run(['xdotool','keyup',modifier])
 time.sleep(.65)
def capture(name):run(['import','-window',wid,str(out/'media'/name)])
def rack():return {k:r for k,r in hits().items() if (k.startswith('module:') and k.count(':')==1) or k.startswith('composite:') and k.endswith(':body')}
def values(cid):return {k:r for k,r in hits().items() if k.startswith(f'composite:{cid}:control:') or k.startswith(f'composite:{cid}:port:')}
def stop():
 global app
 app.terminate();app.wait(timeout=8);app=None
try:
 baseline=json.loads((patch/'checkpoint.json').read_text())
 for size,scale in [('1440x900',1.),('1280x800',1.),('1280x800',1.5)]:
  label=size+('-150' if scale>1 else '')
  env['WINIT_X11_SCALE_FACTOR']=str(scale);(root/'hits.txt').unlink(missing_ok=True)
  app=sp.Popen([str(binary),'--patch',str(patch),'--size',size,'--no-rt'],env=env,stdout=log,stderr=log);wait('save');time.sleep(.8)
  wid=run(['xdotool','search','--onlyvisible','--name','^kabl$']).splitlines()[-1];run(['xdotool','windowmove',wid,'0','0']);time.sleep(.3)
  if label=='1440x900':
   video=sp.Popen(['ffmpeg','-y','-f','x11grab','-framerate','24','-video_size','1600x1000','-i',':128','-an','-c:v','libx264','-threads','2','-preset','ultrafast','-crf','24','-pix_fmt','yuv420p',str(out/'media/final-interactions.mp4')],stdout=log,stderr=log)
  click('theme:light');click('zoom:fit');base=rack();click('toggle:20');closed=hits()['more:dialog'];click('more:exact');expanded=hits()['more:dialog'];assert expanded[3]-expanded[1]>closed[3]-closed[1]+40,(closed,expanded)
  assert expanded[:2]==closed[:2],('position moved',closed,expanded);assert expanded[3]<=int(size.split('x')[1])-5
  capture(f'exact-expanded-{label}.png');click('more:exact');assert hits()['more:dialog']==closed;click('more:exact');assert hits()['more:dialog']==expanded
  # Keyboard and parsed exact entry use the original native parameter.
  a,b,c,d=hits()['more:value:20:base_hz'];pointer((c-24,(b+d)/2));run(['xdotool','click','--repeat','2','--delay','100','1']);run(['xdotool','key','ctrl+a']);run(['xdotool','type','--clearmodifiers','250 Hz']);run(['xdotool','key','Return']);time.sleep(.5)
  # Scroll only if necessary; Close stays fixed and reachable.
  close=hits()['more:close'];wheel((expanded[0]+30,expanded[3]-40),12);assert hits()['more:close']==close
  capture(f'exact-scrolled-{label}.png');click('more:close');assert rack()==base,'More moved rack';click('save');saved=json.loads((patch/'checkpoint.json').read_text());assert abs(saved['modules']['20']['params']['base_hz']-250)<.5,saved['modules']['20']['params']['base_hz'];assert saved['composites']==baseline['composites'];assert all(v['pos']==baseline['modules'][k]['pos'] for k,v in saved['modules'].items())
  # Genuinely overflowing native exact-entry content uses one capped body, with fixed Close.
  base=rack();click('toggle:25')
  if not any(k.startswith('more:value:25:') for k in hits()):click('more:exact')
  tall=hits()['more:dialog'];entries={k:r for k,r in hits().items() if k.startswith('more:value:25:')};last=max(entries,key=lambda k:entries[k][3]);assert entries[last][3]>tall[3],('not genuine overflow',tall,entries[last]);fixed_close=hits()['more:close']
  wheel((tall[0]+30,tall[3]-40),50);assert hits()['more:close']==fixed_close;assert hits()[last][3]<=hits()['more:dialog'][3];capture(f'exact-overflow-{label}.png');click('more:close');assert rack()==base
  # Default face scroll: internal rows move, parent rack and header stay fixed through both boundaries.
  base=rack();vals=values(3);a,b,c,d=hits()['composite:3:body'];p=(a+30,b+110);wheel(p,3);moved=values(3);assert moved!=vals,('inner did not move',vals,moved);assert rack()==base,'child + rack moved'
  wheel(p,40);boundary=values(3);assert rack()==base;wheel(p,12);assert values(3)==boundary and rack()==base,'bottom boundary moved parent'
  wheel(p,60,4);top=values(3);wheel(p,12,4);assert values(3)==top and rack()==base,'top boundary moved parent'
  wheel(p,3,5,'ctrl');assert rack()==base,'child modifier wheel zoomed rack';capture(f'default-wheel-{label}.png')
  # Authored Public controls owns its wheel, including a short content boundary.
  click('composite:1:fallback');dialog=hits()['more:dialog'] if 'more:dialog' in hits() else None
  header=hits()['more:close'];wheel((header[0]-60,header[3]+110),10);assert rack()==base and hits()['more:close']==header;capture(f'authored-more-{label}.png');click('more:close')
  # Real nested menu over original native knob; parent stays put on wheel and submenu navigation.
  a,b,c,d=hits()['module:20'];pointer((a+12,b+16));run(['xdotool','click','3']);time.sleep(.5);wait('menu:explain');wheel(point('menu:explain'),6);assert rack()==base;parent_menu=hits()['menu:explain'];click('menu:pin');wait('menu:pin:base_hz');wheel(point('menu:pin:base_hz'),6);assert rack()==base and hits()['menu:explain']==parent_menu;capture(f'menu-wheel-{label}.png');run(['xdotool','key','Escape','Escape']);time.sleep(.5);assert 'menu:explain' not in hits()
  # Uncovered rack keeps pan and modifier zoom. Body regions are not wheel children.
  wheel((int(size.split('x')[0])-400,430),3);assert rack()!=base,'outside wheel did not pan';panned=rack();wheel((int(size.split('x')[0])-400,430),3,4,'ctrl');assert rack()!=panned,'modifier zoom missing';click('zoom:fit')
  # Occupied-row preview stays visible; compare exact recorded preview with committed native face.
  a,b,c,d=hits()['module:20'];px,py=a+15,b+18;pointer((px,py));run(['xdotool','mousedown','1']);time.sleep(.15)
  target=hits()['module:21'];tx,ty=target[0]+30,target[1]+45
  for n in range(1,13):pointer((px+(tx-px)*n/12,py+(ty-py)*n/12));time.sleep(.04)
  preview=hits()['rack:drop-preview'];capture(f'occupied-insertion-{label}.png');time.sleep(.6);run(['xdotool','mouseup','1']);time.sleep(.6);committed=hits()['module:20'];assert all(abs(a-b)<.1 for a,b in zip(preview,committed)),(preview,committed);run(['xdotool','key','ctrl+z']);time.sleep(.6)
  click('theme:dark');capture(f'dark-{label}.png');click('save')
  if video:video.send_signal(2);video.wait(timeout=10);assert video.returncode in [0,255];video=None
  stop();results.append({'logical_size':size,'native_scale':scale,'closed_dialog':closed,'expanded_dialog':expanded,'stable_origin':True,'exact_entry_base_hz':250,'genuine_overflow_body_scrolled_with_Close_fixed':True,'rack_unchanged_by_sections':True,'default_child_motion_parent_stationary':True,'top_bottom_boundary_parent_stationary':True,'child_blocks_modifier_zoom':True,'authored_more_parent_stationary':True,'menu_parent_stationary':True,'outside_rack_pan_and_modifier_zoom':True,'preview':preview,'committed':committed,'preview_matches_committed':True})
 (out/'evidence/interaction.json').write_text(json.dumps({'binary':str(binary),'sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'input':'Scripted private X11 input; not owner or physical hardware acceptance.','video':'1600x1000 private surface contains complete 1440x900 client','cases':results},indent=2)+'\n')
finally:
 if video and video.poll() is None:video.send_signal(2);video.wait(timeout=10)
 if app and app.poll() is None:stop()
 wm.terminate();wm.wait(timeout=5);x.terminate();x.wait(timeout=5);log.close();shutil.copy2(root/'run.txt',out/'evidence/run.txt')
print('Final More, exact entry, wheel boundaries, rack/zoom and occupied insertion passed all sizes/scales')
