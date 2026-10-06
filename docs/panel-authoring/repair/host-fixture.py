#!/usr/bin/env python3
"""Add repaired native face metadata to private D10 host fixture without changing DSP/mappings."""
import base64,json,re,textwrap,shutil
from pathlib import Path
repo=Path(__file__).resolve().parents[3];host=repo/'scratch/d10-repair/host';(host/'profile').mkdir(parents=True,exist_ok=True);(host/'plugins').mkdir(exist_ok=True)
source=repo/'scratch/d10-host/d10-arrangement.rpp';expected=[]
# Metadata comes from the actual UI's saved project, not a manually fabricated setting.
ui=json.loads((repo/'scratch/d10-repair/after/patch/checkpoint.json').read_text())
face={k:v for k,v in ui['modules']['20']['params'].items() if k.startswith('face.')};assert face.get('face.waveform')==0 and any(k.startswith('face.order.') for k in face)
def replace(m):
 raw=base64.b64decode(''.join(m[1].split()));state=json.loads(raw[8:]);before=json.loads(json.dumps(state));ids=[]
 for mid,module in state['patch']['modules'].items():
  if module['kind']=='osc.va':module['params'].update(face);ids.append(mid)
 assert ids,'Host fixture lacks oscillator';assert before['lanes']==state['lanes'] and before['slot_values']==state['slot_values'];assert before['patch']['cables']==state['patch']['cables'];assert before['patch']['composites']==state['patch']['composites']
 for mid,module in state['patch']['modules'].items():
  prior=before['patch']['modules'][mid];assert module['kind']==prior['kind'] and module['pos']==prior['pos'];assert {k:v for k,v in module['params'].items() if not k.startswith('face.')}=={k:v for k,v in prior['params'].items() if not k.startswith('face.')}
 expected.append(state);raw=json.dumps(state,separators=(',',':')).encode();encoded=base64.b64encode(len(raw).to_bytes(8,'little')+raw).decode();return '<STATE\n'+''.join('          '+line+'\n' for line in textwrap.wrap(encoded,120))+'        >'
project=re.sub(r'<STATE\s+(.*?)\s*>',replace,source.read_text(),flags=re.S);assert len(expected)==2
(host/'d10-arrangement.rpp').write_text(project);(host/'expected.json').write_text(json.dumps(expected,indent=2)+'\n')
shutil.copy2(repo/'scratch/d10-host/profile/reaper.ini',host/'profile/reaper.ini');shutil.copy2(repo/'target/release/libkabl_clap.so',host/'plugins/kabl.clap')
(host/'source-library').mkdir(exist_ok=True)
for name in ['voice.json','stereo-effect.json']:shutil.copy2(repo/f'docs/panel-authoring/examples/{name}',host/'source-library'/name)
shutil.copytree(repo/'docs/panel-authoring/examples/art',host/'source-library/art',dirs_exist_ok=True)
(repo/'docs/panel-authoring/repair/evidence/host-fixture.json').write_text(json.dumps({'source':str(source),'actual_UI_saved_face':face,'instances':2,'all_nonpresentation_params_graph_composites_lanes_bases_unchanged':True},indent=2)+'\n')
print('Private REAPER fixture retains exact graph/native identities and actual saved UI visibility/order')
