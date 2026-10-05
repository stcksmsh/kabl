#!/usr/bin/env python3
"""Add one continuous CC and one unique button mapping to a complete saved fixture."""
import base64, json, re, sys, textwrap
from pathlib import Path
index=0
def edit(match):
    global index
    raw=base64.b64decode(''.join(match[1].split()))
    state=json.loads(raw[raw.index(b'{'):]);index+=1
    if index==2:
        state['host_clock']=False
        state['patch']['modules']['3']['params']['cc.m1']=20
        state['patch']['modules']['1']['params']['btn.run']=25
    raw=json.dumps(state,separators=(',',':')).encode()
    encoded=base64.b64encode(len(raw).to_bytes(8,'little')+raw).decode()
    return '<STATE\n'+''.join('          '+line+'\n' for line in textwrap.wrap(encoded,120))+'        >'
text=re.sub(r'<STATE\s+(.*?)\s*>',edit,Path(sys.argv[1]).read_text(),flags=re.S)
assert index==2
Path(sys.argv[2]).write_text(text)
