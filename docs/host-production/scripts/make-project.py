#!/usr/bin/env python3
"""Create a complete scripted played/sequenced REAPER fixture; no physical MIDI claim."""
import base64, json, textwrap, sys
from pathlib import Path
repo = Path(__file__).resolve().parents[3]
out = Path(sys.argv[1]).resolve()
out.parent.mkdir(parents=True, exist_ok=True)
lead = json.loads((repo/'patches/expressive/lead/checkpoint.json').read_text())
original = json.loads((repo/'patches/composition/checkpoint.json').read_text())
keep = {1,3,6,10,11,12,14,33,36,39}
seq = {'modules': {k:v for k,v in original['modules'].items() if int(k) in keep}, 'cables': {}, 'labels': {}}
seq['modules']['40'] = {'kind':'vca','pos':{'x':620,'y':220},'params':{'gain':1.0}}
for i, m in enumerate(seq['modules'].values()): m['pos']={'x':24+(i%6)*170,'y':10+(i//6)*230}
seq['modules']['6']['params']['gate_mode']=1.0
seq['modules']['6']['params']['r3']=70.0
seq['modules']['33']['params']['mode']=0.0
seq['modules']['33']['params']['time_ms']=220.0
seq['modules']['36']['params']['decay_s']=1.8
seq['modules']['3']['params']['m1']=0.4

def port(i,key): return {'Module': {'id':i,'port':key}}
def param(i,key): return {'Param': {'id':i,'param':key}}
def cable(a,b,params=None):
    seq['cables'][str(len(seq['cables'])+1)]={'from':a,'to':b,'params':params or {},'steps':[]}
for a,b in [(port(1,'gate'),port(6,'clock')),(port(1,'reset'),port(6,'reset')),(port(6,'pitch'),port(10,'pitch')),
 (port(6,'gate'),port(12,'gate')),(port(10,'out'),port(11,'in')),(port(11,'lp'),port(14,'in')),
 (port(12,'out'),port(14,'cv')),(port(14,'out'),port(40,'in')),(port(40,'out'),port(33,'in')),
 (port(33,'left'),port(36,'in_l')),(port(36,'left'),port(39,'left')),(port(36,'right'),port(39,'right'))]: cable(a,b)
cable(port(3,'m1'),param(11,'cutoff_hz'),{'amount':0.35})

def state(patch, targets, values, gain):
    lanes=[{'target':None,'retired':False} for _ in range(16)]
    for i,(module,key) in enumerate(targets): lanes[i]['target']={'module':module,'kind':patch['modules'][str(module)]['kind'],'param':key}
    return {'version':2,'patch':patch,'output_gain':gain,'lanes':lanes,'slot_values':values+[0.0]*(16-len(values)),'host_clock':True}
states=[state(lead,[(1,'m1'),(1,'m2'),(1,'m3'),(1,'m4')],[0.5,0.3,0.3,0.4],0.45),state(seq,[(3,'m1'),(40,'gain')],[0.4,1.0],0.6)]
header='''<REAPER_PROJECT 0.1 "7.75/linux-x86_64" 0
  TEMPO 120 4 4
  SAMPLERATE 48000 1 0
  RENDER_FILE "render"
  RENDER_FMT 0 2 48000
  RENDER_RANGE 0 0 36 0 1000
  RENDER_STEMS 0
  RENDER_DITHER 0
  <RENDER_CFG
    ZXZhdyAAAA==
  >
'''
tracks=[]
for i,s in enumerate(states):
    raw=json.dumps(s,separators=(',',':')).encode()
    encoded=base64.b64encode(len(raw).to_bytes(8,'little')+raw).decode()
    name=['D08 played lead · virtual/scripted MIDI','D08 Host sequence · seeded probability'][i]
    track=f'''  <TRACK
    NAME "{name}"
    VOLPAN 0.65 0 1 -1
    <FXCHAIN
      SHOW -1
      <CLAP "CLAPi: kabl (stcksmsh)" dev.stcksmsh.kabl ""
        CFG 4 1180 740 ""
        <STATE
'''+''.join('          '+line+'\n' for line in textwrap.wrap(encoded,120))+'''        >
      >
      WAK 0 0
    >
'''
    if i==0:
        events=[]
        for j,note in enumerate([60,64,67,69,67,64,60,55,60,64,67,72]):
            start=960+j*1920; events += [(start,f'90 {note:02x} 64'),(start+1440,f'80 {note:02x} 00')]
        events.sort(); previous=0
        midi=''
        for t,e in events: midi+=f'        E {t-previous} {e}\n';previous=t
        track+='''    <ITEM
      POSITION 0
      LENGTH 25
      <SOURCE MIDI
        HASDATA 1 960 QN
'''+midi+'''      >
    >
'''
    tracks.append(track+'  >\n')
out.write_text(header+''.join(tracks)+'>\n')
(out.parent/'expected-states.json').write_text(json.dumps(states,indent=2)+'\n')
print(out)
