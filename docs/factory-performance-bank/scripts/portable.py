#!/usr/bin/env python3
"""Verify normal populated discovery and separate source-free complete-state recall."""
from pathlib import Path
import os, sys, time, json, subprocess as sp, hashlib

repo=Path(__file__).resolve().parents[3]
bundle=Path(sys.argv[1]).resolve()
scratch=repo/'scratch/factory-bank/portable'
scratch.mkdir(parents=True,exist_ok=True)
out=repo/'docs/factory-performance-bank'
manifest=json.loads((bundle/'manifest.json').read_text())
for name,digest in manifest['files'].items():
    assert hashlib.sha256((bundle/name).read_bytes()).hexdigest()==digest,name
env=dict(os.environ,DISPLAY=':139',WINIT_X11_SCALE_FACTOR='1',KABL_HITS_FILE=str(scratch/'hits.txt'),KABL_USER_DIR=str(scratch/'library'))
log=open(scratch/'run.txt','w')
x=sp.Popen(['Xvfb',':139','-screen','0','1440x900x24','-nolisten','tcp'],stdout=log,stderr=log);time.sleep(.6)
wm=sp.Popen(['metacity','--sm-disable'],env=env,stdout=log,stderr=log);time.sleep(.6)
app=None
def run(args): return sp.check_output(args,env=env,text=True)
def hits():
    try: return {p[0]:list(map(float,p[1:])) for p in map(str.split,(scratch/'hits.txt').read_text().splitlines()) if len(p)==5}
    except FileNotFoundError: return {}
def wait(key):
    end=time.monotonic()+20
    while time.monotonic()<end:
        if key in hits(): return
        assert app.poll() is None
        time.sleep(.15)
    raise AssertionError((key,list(hits())))
def click(key):
    wait(key);a,b,c,d=hits()[key]
    run(['xdotool','mousemove','--window',wid,str(round((a+c)/2)),str(round((b+d)/2)),'click','1']);time.sleep(.5)
try:
    for phase,launcher in [('normal','launch.sh'),('unavailable','launch-portability-test.sh')]:
        (scratch/'hits.txt').unlink(missing_ok=True)
        app=sp.Popen([str(bundle/launcher)],cwd=bundle,env=env,stdout=log,stderr=log)
        wait('save');time.sleep(.7)
        wid=run(['xdotool','search','--onlyvisible','--name','^kabl$']).splitlines()[-1]
        if phase=='normal':
            click('filter:factory');click('search');run(['xdotool','type','--clearmodifiers','Glass Keys']);run(['xdotool','key','Tab']);time.sleep(.5)
            click('sound:factory:palette/keys');click('open')
            assert any(k.startswith('pcard:') for k in hits())
        else:
            click('perform')
            assert any(k.startswith('pcue:') for k in hits())
            assert not (bundle/'unavailable-factory').exists()
            click('save');time.sleep(.5)
            expected=json.loads((repo/'docs/factory-performance-bank/performance-project/checkpoint.json').read_text())
            assert expected==json.loads((bundle/'patches/performance/checkpoint.json').read_text())
        run(['import','-window',wid,str(out/'media'/f'portable-{phase}.png')])
        app.terminate();app.wait(timeout=8);app=None
    (out/'evidence/portable.json').write_text(json.dumps({'bundle':str(bundle),'packaged_head':manifest['packaged_head'],'runtime_build_head':manifest['runtime_build_head'],'manifest_hashes_verified':True,'normal_bundled_factory_browse_open':True,'unavailable_factory_complete_state_equal_after_save':True,'working_directory':str(bundle),'input':'Scripted private-display real standalone input; no physical or owner acceptance.'},indent=2)+'\n')
finally:
    if app and app.poll() is None: app.terminate();app.wait(timeout=8)
    wm.terminate();wm.wait(timeout=5);x.terminate();x.wait(timeout=5);log.close()
