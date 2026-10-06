#!/usr/bin/env python3
"""Run relocated release standalone and REAPER without source paths, on private displays."""
import os,sys,json,hashlib,subprocess as sp,time,shutil
from pathlib import Path
repo=Path(__file__).resolve().parents[3]; bundle=Path(sys.argv[1]).resolve();out=repo/'docs/panel-authoring/evidence'; scratch=repo/'scratch/d10-bundle-check';scratch.mkdir(parents=True,exist_ok=True)
manifest=json.loads((bundle/'manifest.json').read_text())
for name,digest in manifest['files'].items():assert hashlib.sha256((bundle/name).read_bytes()).hexdigest()==digest,name
log=open(scratch/'standalone.log','w');env=dict(os.environ,DISPLAY=':124',XDG_RUNTIME_DIR='/run/user/1000',KABL_HITS_FILE=str(scratch/'hits.txt'))
x=sp.Popen(['Xvfb',':124','-screen','0','1600x1000x24','-nolisten','tcp'],stdout=log,stderr=log);time.sleep(.6);wm=sp.Popen(['metacity','--sm-disable'],env=env,stdout=log,stderr=log);time.sleep(.5)
p=sp.Popen([str(bundle/'launch.sh')],env=env,cwd=str(bundle),stdout=log,stderr=log)
try:
 end=time.monotonic()+20;window=None
 while time.monotonic()<end and p.poll() is None:
  result=sp.run(['xdotool','search','--onlyvisible','--name','^kabl$'],env=env,capture_output=True,text=True)
  if result.stdout.strip():window=result.stdout.strip().splitlines()[0];break
  time.sleep(.25)
 assert window,'Relocated release window missing';time.sleep(2)
 assert 'kabl-ui' in Path(f'/proc/{p.pid}/maps').read_text() and str(bundle/'bin/kabl-ui') in Path(f'/proc/{p.pid}/maps').read_text()
 hits=(scratch/'hits.txt').read_text();assert 'composite:1:control:4' in hits and 'composite:2:control:5' in hits
 sp.run(['import','-window',window,str(repo/'docs/panel-authoring/media/bundle-release.png')],env=env,check=True)
finally:
 if p.poll() is None:p.terminate();p.wait(timeout=8)
 wm.terminate();wm.wait(timeout=5);x.terminate();x.wait(timeout=5);log.close()
shutil.copy2(scratch/'standalone.log',out/'bundle-standalone.txt')
host=scratch/'host';(host/'profile').mkdir(parents=True,exist_ok=True)
shutil.copy2(bundle/'profile/reaper.ini',host/'profile/reaper.ini');shutil.copy2(repo/'scratch/d10-host/expected.json',host/'expected.json')
env=dict(os.environ,KABL_D10_HOST_ROOT=str(host),KABL_D10_EVIDENCE=str(out/'bundle-host'),KABL_D10_PLUGIN_DIR=str(bundle/'plugins'),KABL_D10_PROJECT=str(bundle/'projects/arrangement.rpp'),KABL_D10_HOST_SCREENSHOT=str(repo/'docs/panel-authoring/media/bundle-reaper.png'))
sp.run(['python3',str(repo/'docs/panel-authoring/scripts/run-host.py')],env=env,cwd=str(bundle),check=True)
assert str(bundle/'plugins/kabl.clap') in (out/'bundle-host/mapped-plugin.txt').read_text()
(out/'bundle-check.json').write_text(json.dumps({'bundle':str(bundle),'manifest_hashes_passed':True,'standalone_release_window':True,'both_embedded_faces_hit_targets':True,'plugin_mapped_from_bundle':True,'complete_REAPER_state_and_native_replay_passed':True,'unavailable_factory_library':True,'input':'scripted private X11, not owner hardware'},indent=2)+'\n')
print('Source-free release bundle passed')
