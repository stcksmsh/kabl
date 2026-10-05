#!/usr/bin/env python3
"""Fresh isolated host cases; exact GUI handles, callback timing, raw backend telemetry."""
import base64, hashlib, itertools, json, os, re, subprocess, textwrap, time
from pathlib import Path
repo=Path(__file__).resolve().parents[3]
source=(repo/'scratch/host/d08-arrangement.rpp').read_text()
index=0
def replace(match):
    global index
    name=('init-keyboard','sound-palette')[index];index+=1
    state={'version':2,'patch':json.loads((repo/'patches'/name/'checkpoint.json').read_text()),'output_gain':.5,'lanes':[{'target':None,'retired':False} for _ in range(16)],'slot_values':[0.0]*16,'host_clock':True}
    raw=json.dumps(state,separators=(',',':')).encode();encoded=base64.b64encode(len(raw).to_bytes(8,'little')+raw).decode()
    return '<STATE\n'+''.join('          '+line+'\n' for line in textwrap.wrap(encoded,120))+'        >'
project=repo/'scratch/host/measurement-fixture.rpp';project.write_text(re.sub(r'<STATE\s+(.*?)\s*>',replace,source,flags=re.S))
evidence=Path(os.environ.get('D08_MEASURE_EVIDENCE', str(repo/'docs/host-production/evidence/host-measurements.json')))
evidence.parent.mkdir(parents=True,exist_ok=True)
results=json.loads(evidence.read_text()) if evidence.exists() else []
for kind,count,opened in itertools.product(('light','dense'),(1,2),(0,1)):
    case=f'{kind}-{count}-'+('open' if opened else 'closed')
    if any(r['case']==case for r in results): continue
    out=Path(os.environ.get('D08_MEASURE_ROOT',str(repo/'scratch/measure')))/case
    if out.exists(): out.rename(out.with_name(case+'-failed-'+str(int(time.time()))))
    out.mkdir(parents=True,exist_ok=False)
    env=dict(os.environ,D08_HOST=str(out),D08_PROXY='1',D08_TRACE='callbacks.csv',D08_MEASURE_KIND=kind,D08_MEASURE_COUNT=str(count),D08_MEASURE_OPEN=str(opened))
    run=subprocess.run(['python3',str(repo/'docs/host-production/scripts/run-host.py'),str(repo/'docs/host-production/scripts/measure-host.lua'),str(project),'measure.done'],env=env,cwd=repo,check=True,capture_output=True,text=True)
    states=[]
    for block in re.findall(r'<STATE\s+(.*?)\s*>',(out/'measured-project.rpp').read_text(),re.S):
        raw=base64.b64decode(''.join(block.split()));states.append(json.loads(raw[raw.index(b'{'):]))
    expected=6 if kind=='light' else 94
    assert len(states)==2 and all(len(s['patch']['modules'])==expected for s in states),'actual saved fixture differs from measured case'
    entry={'verified_module_counts':[len(s['patch']['modules']) for s in states],'case':case,'product_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),'binary_sha256':hashlib.sha256((repo/'target/release/libkabl_clap.so').read_bytes()).hexdigest(),'patch': 'init-keyboard (6 modules/7 cables)' if kind=='light' else 'sound-palette (94 modules/156 cables)','device':(out/'measure-device.txt').read_text(),'gui_handles':(out/'gui-handles.txt').read_text(),'callback_csv':(out/'callbacks.csv').read_text(),'reaper_pid':int((out/'reaper-pid.txt').read_text()),'raw_dir':str(out)}
    assert all(v in entry['device'] for v in ('GUI_checks=3','SRATE=true 48000','BSIZE=true 256','MODE=true JACK'))
    results.append(entry)
    (repo/'docs/host-production/evidence/host-measurements.json').write_text(json.dumps(results,indent=2)+'\n')
    print(case+' complete',flush=True)
