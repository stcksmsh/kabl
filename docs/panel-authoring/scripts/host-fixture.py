#!/usr/bin/env python3
"""Add D10 presentation to saved D09 host states; retain every leaf/cable/native mapping."""
import base64,json,re,textwrap,struct,shutil
from pathlib import Path
repo=Path(__file__).resolve().parents[3]
host=repo/'scratch/d10-host';(host/'profile').mkdir(parents=True,exist_ok=True);(host/'plugins').mkdir(exist_ok=True)
source=repo/'docs/composites/examples/projects/arrangement.rpp'
project=source.read_text();expected=[]
artroot=repo/'docs/panel-authoring/examples/art'
def replace(m):
 raw=base64.b64decode(''.join(m[1].split()));state=json.loads(raw[8:]);before=json.loads(json.dumps(state))
 for key,c in state['patch']['composites'].items():
  controls=sorted(c['controls'],key=int)[:4];ports=list(c['ports']);width=max(300,((len(controls)*100+24+29)//30)*30)
  if len(ports)>4:raise ValueError("Fixture ports exceed one-row example")
  placements={}
  for n,k in enumerate(controls):placements[k]={'kind':'knob','x':12+n*100,'y':72,'width':84,'height':110}
  for n,k in enumerate(ports):placements[k]={'kind':'jack','x':12+n*100,'y':244,'width':84,'height':58}
  width=max(width,((len(ports)*100+24+29)//30)*30)
  c['panel']={'width':width,'placements':placements,'light':{'name':'voice-light.png','png':list((artroot/'voice-light.png').read_bytes())},'dark':{'name':'voice-dark.png','png':list((artroot/'voice-dark.png').read_bytes())}}
  for n,k in enumerate(ports):c['ports'][k]['label']=['Pitch','Gate','Audio','Aux'][n]
 assert before['patch']['modules']==state['patch']['modules'] and before['patch']['cables']==state['patch']['cables']
 assert before['lanes']==state['lanes'] and before['slot_values']==state['slot_values']
 expected.append(state);raw=json.dumps(state,separators=(',',':')).encode();encoded=base64.b64encode(len(raw).to_bytes(8,'little')+raw).decode()
 return '<STATE\n'+''.join('          '+line+'\n' for line in textwrap.wrap(encoded,120))+'        >'
project=re.sub(r'<STATE\s+(.*?)\s*>',replace,project,flags=re.S);assert len(expected)==2
(host/'d10-arrangement.rpp').write_text(project)
(host/'expected.json').write_text(json.dumps(expected,indent=2)+'\n')
profile=Path('/home/stcksmsh/.codex/visualizations/2026/10/05/01a10979-ac89-7a01-bd38-576bb907cce1/d09-owner-test/profile/reaper.ini')
shutil.copy2(profile,host/'profile/reaper.ini');shutil.copy2(repo/'target/release/libkabl_clap.so',host/'plugins/kabl.clap')
# Only these private files are source dependencies. Recall runner will remove them before restart.
(host/'source-library').mkdir(exist_ok=True)
for name in ['voice.json','stereo-effect.json']:shutil.copy2(repo/f'docs/panel-authoring/examples/{name}',host/'source-library'/name)
(repo/'docs/panel-authoring/evidence/host-fixture.json').write_text(json.dumps({'source':str(source),'source_runtime_head':'a337f2c1eae867671f9deabadfa10f681de2bb26','instances':2,'leaves_cables_lanes_bases_unchanged':True,'panel_groups':[len(s['patch']['composites']) for s in expected]},indent=2)+'\n')
