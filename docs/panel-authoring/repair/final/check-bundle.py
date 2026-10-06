#!/usr/bin/env python3
"""Relocated release bundle: normal factory startup and separate unavailable recall path."""
import os,sys,json,hashlib,subprocess as sp,time,shutil
from pathlib import Path
repo=Path(__file__).resolve().parents[4];bundle=Path(sys.argv[1]).resolve();out=repo/'docs/panel-authoring/repair/final/evidence';scratch=repo/'scratch/d10-final/bundle-check';scratch.mkdir(parents=True,exist_ok=True)
manifest=json.loads((bundle/'manifest.json').read_text())
for name,digest in manifest['files'].items():assert hashlib.sha256((bundle/name).read_bytes()).hexdigest()==digest,name
assert manifest['factory_sounds']>0
log=open(scratch/'standalone.log','w');env=dict(os.environ,DISPLAY=':124',XDG_RUNTIME_DIR='/run/user/1000',KABL_HITS_FILE=str(scratch/'hits.txt'),KABL_USER_DIR=str(scratch/'normal-library'))
x=sp.Popen(['Xvfb',':124','-screen','0','1600x1000x24','-nolisten','tcp'],stdout=log,stderr=log);time.sleep(.5);wm=sp.Popen(['metacity','--sm-disable'],env=env,stdout=log,stderr=log);time.sleep(.5);p=None
result=[]
def hits():
 try:return {v[0]:list(map(float,v[1:])) for v in map(str.split,(scratch/'hits.txt').read_text().splitlines()) if len(v)==5}
 except FileNotFoundError:return {}
def click(key):
 a,b,c,d=hits()[key];sp.run(['xdotool','mousemove','--window',wid,str(round((a+c)/2)),str(round((b+d)/2)),'click','1'],env=env,check=True);time.sleep(.8)
try:
 for mode,launcher in [('normal','launch.sh'),('unavailable','launch-portability-test.sh')]:
  (scratch/'hits.txt').unlink(missing_ok=True);p=sp.Popen([str(bundle/launcher)],env=env,cwd=str(bundle),stdout=log,stderr=log)
  end=time.monotonic()+20
  while time.monotonic()<end and 'save' not in hits():assert p.poll() is None;time.sleep(.2)
  assert 'save' in hits();time.sleep(1);wid=sp.check_output(['xdotool','search','--onlyvisible','--name','^kabl$'],env=env,text=True).splitlines()[-1]
  assert str(bundle/'bin/kabl-ui') in Path(f'/proc/{p.pid}/maps').read_text();assert 'composite:1:body' in hits() and 'composite:2:body' in hits()
  click('browser');cards=[k for k in hits() if k.startswith('sound:factory:')]
  if mode=='normal':assert cards,'Normal factory browser empty'
  else:assert not cards,'Unavailable test unexpectedly has factory'
  sp.run(['import','-window',wid,str(repo/f'docs/panel-authoring/repair/final/media/bundle-{mode}-sounds.png')],env=env,check=True)
  result.append({'mode':mode,'factory_card_hits':cards,'embedded_faces_recalled':True,'mapped_binary':str(bundle/'bin/kabl-ui')})
  p.terminate();p.wait(timeout=8);p=None;time.sleep(.5)
finally:
 if p and p.poll() is None:p.terminate();p.wait(timeout=8)
 wm.terminate();wm.wait(timeout=5);x.terminate();x.wait(timeout=5);log.close()
shutil.copy2(scratch/'standalone.log',out/'bundle-standalone.txt')
host=scratch/'host';(host/'profile').mkdir(parents=True,exist_ok=True);shutil.copy2(bundle/'profile/reaper.ini',host/'profile/reaper.ini');shutil.copy2(repo/'scratch/d10-repair/host/expected.json',host/'expected.json')
env=dict(os.environ,KABL_D10_HOST_ROOT=str(host),KABL_D10_EVIDENCE=str(out/'bundle-host'),KABL_D10_PLUGIN_DIR=str(bundle/'plugins'),KABL_D10_PROJECT=str(bundle/'projects/arrangement.rpp'),KABL_D10_HOST_SCREENSHOT=str(repo/'docs/panel-authoring/repair/final/media/bundle-reaper.png'))
sp.run(['python3',str(repo/'docs/panel-authoring/scripts/run-host.py')],env=env,cwd=str(bundle),check=True)
assert str(bundle/'plugins/kabl.clap') in (out/'bundle-host/mapped-plugin.txt').read_text()
(out/'bundle-check.json').write_text(json.dumps({'bundle':str(bundle),'product_head':manifest['product_head'],'manifest_hashes_passed':True,'factory_sounds':manifest['factory_sounds'],'standalone':result,'complete_REAPER_state_native_replay_passed':True,'unavailable_factory_library_host':True,'input':'scripted private X11; not physical owner acceptance'},indent=2)+'\n')
print('Relocated release normal factory and explicit source-free unavailable recall passed')
