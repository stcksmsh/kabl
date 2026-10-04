#!/usr/bin/env python3
"""Decode D07 complete CLAP states from saved REAPER projects."""
import base64
import json
import re
import sys
from pathlib import Path

def states(path):
    result = []
    for block in re.findall(r'<STATE\s+(.*?)\s*>', Path(path).read_text(), re.S):
        raw = base64.b64decode(''.join(block.split()))
        result.append(json.loads(raw[raw.index(b'{'):]))
    return result

if __name__ == '__main__':
    values = [states(p) for p in sys.argv[1:]]
    print(json.dumps({'files': sys.argv[1:], 'states_equal': all(v == values[0] for v in values), 'instances': [[{'modules': len(s['patch']['modules']), 'cables': len(s['patch']['cables']), 'gain': s['output_gain']} for s in v] for v in values]}, indent=2))
