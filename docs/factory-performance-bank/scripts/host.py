#!/usr/bin/env python3
"""Two curated complete states, fresh REAPER recall and closed-editor native replay."""
from pathlib import Path
import os, re, base64, json, struct, textwrap, subprocess as sp, time, shutil, hashlib

repo = Path(__file__).resolve().parents[3]
scratch = repo / 'scratch/factory-bank/host'
out = repo / 'docs/factory-performance-bank/evidence/host'
scratch.mkdir(parents=True, exist_ok=True); out.mkdir(parents=True, exist_ok=True)
(scratch/'profile').mkdir(exist_ok=True)
plugin_dir = Path(os.environ.get('KABL_BANK_PLUGIN_DIR', scratch/'plugins'))
plugin_dir.mkdir(exist_ok=True)
if 'KABL_BANK_PLUGIN_DIR' not in os.environ:
    shutil.copy2(repo/'target/release/libkabl_clap.so', plugin_dir/'kabl.clap')
profile = f'[reaper]\nvstpath={scratch}/empty-vst\nvstpath64={scratch}/empty-vst\nlinux_audio_bsize=256\nlinux_audio_mode=0\nlinux_audio_srate=48000\nwnd_w=1280\nwnd_h=800\nwnd_x=0\nwnd_y=0\n'
(scratch/'empty-vst').mkdir(exist_ok=True)
(scratch/'profile/reaper.ini').write_text(profile)

expected=[]
paths=['palette/keys','palette/progression']
source=(repo/'docs/composites/examples/projects/arrangement.rpp').read_text()
def replace(match):
    raw=base64.b64decode(''.join(match[1].split()))
    state=json.loads(raw[8:])
    patch=json.loads((repo/'patches'/paths[len(expected)]/'checkpoint.json').read_text())
    patch.setdefault('composites',{})
    macro=int(next(key for key,value in patch['modules'].items() if value['kind']=='macro'))
    state['patch']=patch; state['version']=2; state['host_clock']=True; state['output_gain']=.4
    state['lanes']=[{'retired':False,'target':{'kind':'macro','module':macro,'param':f'm{i+1}'}} if i<4 else {'retired':False,'target':None} for i in range(16)]
    state['slot_values']=[patch['modules'][str(macro)]['params'][f'm{i+1}'] if i<4 else 0.0 for i in range(16)]
    expected.append(state)
    raw=json.dumps(state,separators=(',',':')).encode()
    encoded=base64.b64encode(len(raw).to_bytes(8,'little')+raw).decode()
    return '<STATE\n'+''.join('          '+line+'\n' for line in textwrap.wrap(encoded,120))+'        >'
project=re.sub(r'<STATE\s+(.*?)\s*>',replace,source,flags=re.S)
assert len(expected)==2
project=project.replace('D08 played lead · virtual/scripted MIDI','Glass Keys - virtual/scripted MIDI').replace('D08 Host sequence · seeded probability','Slow Horizons - editable real sequencer')
project_path=scratch/'arrangement.rpp'
project_path.write_text(project)
(scratch/'expected.json').write_text(json.dumps(expected,indent=2)+'\n')

env=dict(os.environ,DISPLAY=':138',XDG_RUNTIME_DIR='/run/user/1000',KABL_D10_HOST=str(scratch),KABL_FACTORY_DIR=str(scratch/'unavailable-factory'),KABL_USER_DIR=str(scratch/'unavailable-library'),CLAP_PATH=str(plugin_dir))
log=open(scratch/'run.txt','w')
x=sp.Popen(['Xvfb',':138','-screen','0','1440x900x24','-nolisten','tcp'],stdout=log,stderr=log);time.sleep(.6)
wm=sp.Popen(['metacity','--sm-disable'],env=env,stdout=log,stderr=log);time.sleep(.6)
app=None

def f32(value):
    if isinstance(value,float): return struct.unpack('<f',struct.pack('<f',value))[0]
    if isinstance(value,list): return [f32(v) for v in value]
    if isinstance(value,dict): return {k:f32(v) for k,v in value.items()}
    return value

try:
    for phase in ['initial','full-restart']:
        (scratch/'done').unlink(missing_ok=True)
        app=sp.Popen(['pw-jack','/usr/local/bin/reaper','-newinst','-nosplash','-cfgfile',str(scratch/'profile/reaper.ini'),str(project_path),str(repo/'docs/panel-authoring/scripts/host.lua')],env=env,cwd=scratch,stdout=log,stderr=log)
        end=time.monotonic()+55
        while time.monotonic()<end and not (scratch/'done').exists() and app.poll() is None: time.sleep(.25)
        assert (scratch/'done').exists(), 'Host completion missing; inspect retained run.txt'
        maps=[line for line in Path(f'/proc/{app.pid}/maps').read_text().splitlines() if 'kabl.clap' in line]
        assert maps
        (out/f'{phase}-mapped-plugin.txt').write_text('\n'.join(maps)+'\n')
        actual=[]
        for chunk in re.findall(r'<STATE\s+(.*?)\s*>',(scratch/'recalled.rpp').read_text(),re.S):
            raw=base64.b64decode(''.join(chunk.split()));assert struct.unpack_from('<Q',raw)[0]==len(raw)-8
            actual.append(json.loads(raw[8:]))
        assert f32(expected)==f32(actual), 'Complete recalled state differs'
        shutil.copy2(scratch/'native-replay.txt',out/f'{phase}-native-replay.txt')
        sp.run(['import','-window','root',str(repo/f'docs/factory-performance-bank/media/reaper-{phase}.png')],env=env,check=True)
        app.terminate();app.wait(timeout=8);app=None
        project_path=scratch/'recalled.rpp'
    shutil.copy2(project_path,out/'arrangement.rpp')
    (out/'recall.json').write_text(json.dumps({'factory_ids':['factory:'+p for p in paths],'phases':['initial','full-restart'],'complete_f32_state_equal':True,'guides_pins_banks_embedded':True,'factory_unavailable':True,'native_closed_editor_times':[2,6,13],'native_tolerance':.025,'plugin_sha256':hashlib.sha256((plugin_dir/'kabl.clap').read_bytes()).hexdigest(),'input':'Scripted MIDI/native automation in private REAPER; not owner listening or physical-controller acceptance. This is recall/replay proof, not deterministic-render evidence.'},indent=2)+'\n')
finally:
    if app and app.poll() is None: app.terminate();app.wait(timeout=8)
    wm.terminate();wm.wait(timeout=5);x.terminate();x.wait(timeout=5);log.close()
    shutil.copy2(scratch/'run.txt',out/'run.txt')
