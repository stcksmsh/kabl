#!/usr/bin/env python3
"""Embed grouping in D08's actual saved project; native targets and musical leaves unchanged."""
from pathlib import Path
import re,json,base64,subprocess,textwrap,struct
repo=Path(__file__).resolve().parents[3]
host=repo/'scratch/host';host.mkdir(parents=True,exist_ok=True)
project=(repo/'docs/host-production/projects/arrangement.rpp').read_text()
def canonical(v):
 if isinstance(v,float):return struct.unpack('<f',struct.pack('<f',v))[0]
 if isinstance(v,list):return [canonical(x) for x in v]
 if isinstance(v,dict):return {k:canonical(x) for k,x in v.items()}
 return v
index=0
originals=[];grouped=[]
def replace(match):
 global index
 payload=base64.b64decode(''.join(match[1].split()))
 state=json.loads(payload[payload.index(b'{'):]);originals.append(state)
 src=host/f'flat-{index}.json';dst=host/f'grouped-{index}.json';src.write_text(json.dumps(state))
 subprocess.run([str(repo/'target/debug/examples/composite_host_fixture'),str(src),str(dst)],check=True)
 new=json.loads(dst.read_text());grouped.append(new)
 assert state['lanes']==new['lanes'] and state['slot_values']==new['slot_values']
 assert canonical(state['patch']['modules'])==canonical(new['patch']['modules']) and canonical(state['patch']['cables'])==canonical(new['patch']['cables'])
 raw=json.dumps(new,separators=(',',':')).encode();encoded=base64.b64encode(len(raw).to_bytes(8,'little')+raw).decode();index+=1
 return '<STATE\n'+''.join('          '+line+'\n' for line in textwrap.wrap(encoded,120))+'        >'
project=re.sub(r'<STATE\s+(.*?)\s*>',replace,project,flags=re.S)
assert index==2
(host/'d09-arrangement.rpp').write_text(project)
(repo/'docs/composites/evidence/host-fixture.json').write_text(json.dumps({'source_project':'docs/host-production/projects/arrangement.rpp','source_product':'f7157619836075d0d7704e59be5d66e6224890b1','instances':index,'unchanged':'all leaves, cables, native lane assignments and native bases','flat':originals,'composite':grouped},indent=2)+'\n')
